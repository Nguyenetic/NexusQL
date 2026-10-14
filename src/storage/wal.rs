//! Framed redo log for the SQL disk backend. See docs/development/durability.md.
//!
//! Inspired by SQLite's public-domain src/wal.c (see NOTICE). This is a
//! NexusQL format, not a SQLite file. Integers and checksum words are LE.

use crate::{NexusError, Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const WAL_HEADER_LEN: usize = 32;
pub const FRAME_HEADER_LEN: usize = 40;
pub const FRAME_PAYLOAD_SIZE: usize = 64 * 1024;
const MAGIC: &[u8; 8] = b"NQLWAL01";
const VERSION: u32 = 1;

/// FULL syncs each commit; NORMAL syncs at checkpoints; OFF omits WAL barriers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SyncPolicy {
    #[default]
    Full,
    Normal,
    Off,
}

impl SyncPolicy {
    pub fn parse(value: &str) -> Result<Self> {
        match value
            .trim_matches(['\'', '"'])
            .to_ascii_lowercase()
            .as_str()
        {
            "full" | "2" => Ok(Self::Full),
            "normal" | "1" => Ok(Self::Normal),
            "off" | "0" => Ok(Self::Off),
            _ => Err(NexusError::ConfigError(
                "synchronous must be FULL (2), NORMAL (1), or OFF (0)".into(),
            )),
        }
    }

    pub fn as_integer(self) -> i64 {
        match self {
            Self::Full => 2,
            Self::Normal => 1,
            Self::Off => 0,
        }
    }
}

/// One writer, protected by DiskStorage's mutex and sled's database lock.
pub struct WriteAheadLog {
    file: File,
    salt: u64,
    checksum: [u32; 2],
    next_frame: u64,
    next_transaction: u64,
    len: u64,
    policy: SyncPolicy,
    failed: bool,
}

impl WriteAheadLog {
    /// Open, validate the committed prefix, and remove an incomplete tail.
    /// Returned payloads are complete transactions, in commit order.
    pub fn open(path: &Path, policy: SyncPolicy) -> Result<(Self, Vec<Vec<u8>>)> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let mut wal = Self {
            len: file.metadata()?.len(),
            file,
            salt: 0,
            checksum: [0; 2],
            next_frame: 1,
            next_transaction: 1,
            policy,
            failed: false,
        };
        // A short header can only be initial creation or an interrupted reset:
        // reset truncates only AFTER the main database has been synced.
        if wal.len < WAL_HEADER_LEN as u64 {
            wal.reset()?;
            sync_directory(path.parent().unwrap_or_else(|| Path::new(".")))?;
            return Ok((wal, Vec::new()));
        }

        let mut header = [0; WAL_HEADER_LEN];
        wal.file.read_exact(&mut header)?;
        if &header[..8] != MAGIC
            || u32_at(&header, 8) != VERSION
            || u32_at(&header, 12) != FRAME_PAYLOAD_SIZE as u32
            || checksum([0; 2], &header[..24]) != [u32_at(&header, 24), u32_at(&header, 28)]
        {
            return Err(invalid(
                "Invalid NexusQL WAL header (not a torn frame tail)",
            ));
        }
        wal.salt = u64_at(&header, 16);
        wal.checksum = [u32_at(&header, 24), u32_at(&header, 28)];
        let mut running_checksum = wal.checksum;
        let mut frame_number = 1;
        let mut position = WAL_HEADER_LEN as u64;
        let mut committed_end = position;
        let mut pending = Vec::new();
        let mut transactions = Vec::new();

        while wal.len - position >= FRAME_HEADER_LEN as u64 {
            let mut frame = [0; FRAME_HEADER_LEN];
            wal.file.read_exact(&mut frame)?;
            position += FRAME_HEADER_LEN as u64;
            let payload_len = u32_at(&frame, 16) as usize;
            let kind = u32_at(&frame, 20);
            if u64_at(&frame, 0) != frame_number
                || u64_at(&frame, 8) != wal.salt
                || u64_at(&frame, 24) != wal.next_transaction
                || payload_len > FRAME_PAYLOAD_SIZE
                || (kind == 0 && payload_len == 0)
                || (kind == 1 && (payload_len != 0 || pending.is_empty()))
                || kind > 1
            {
                break;
            }
            let padded_len = (payload_len + 7) & !7;
            if wal.len - position < padded_len as u64 {
                break;
            }
            let mut payload = vec![0; padded_len];
            wal.file.read_exact(&mut payload)?;
            position += padded_len as u64;
            let next_checksum = checksum(checksum(running_checksum, &frame[..32]), &payload);
            if next_checksum != [u32_at(&frame, 32), u32_at(&frame, 36)] {
                break;
            }
            running_checksum = next_checksum;
            frame_number += 1;
            if kind == 0 {
                pending.extend_from_slice(&payload[..payload_len]);
            } else {
                transactions.push(std::mem::take(&mut pending));
                committed_end = position;
                wal.checksum = running_checksum;
                wal.next_frame = frame_number;
                wal.next_transaction += 1;
            }
        }
        wal.file.set_len(committed_end)?;
        wal.file.seek(SeekFrom::Start(committed_end))?;
        wal.len = committed_end;
        Ok((wal, transactions))
    }

    /// Append the complete redo payload and a separate checksummed commit frame.
    /// An I/O error poisons this writer: reopen to resolve an uncertain commit.
    pub fn append(&mut self, payload: &[u8]) -> Result<()> {
        self.ensure_healthy()?;
        if payload.is_empty() {
            return Err(invalid("A WAL transaction must have a payload"));
        }
        let result = (|| {
            for chunk in payload.chunks(FRAME_PAYLOAD_SIZE) {
                self.write_frame(0, chunk)?;
            }
            self.write_frame(1, &[])?;
            if self.policy == SyncPolicy::Full {
                self.file.sync_all()?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        } else {
            self.next_transaction += 1;
        }
        result
    }

    fn write_frame(&mut self, kind: u32, payload: &[u8]) -> Result<()> {
        let mut frame = [0; FRAME_HEADER_LEN];
        frame[..8].copy_from_slice(&self.next_frame.to_le_bytes());
        frame[8..16].copy_from_slice(&self.salt.to_le_bytes());
        frame[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        frame[20..24].copy_from_slice(&kind.to_le_bytes());
        frame[24..32].copy_from_slice(&self.next_transaction.to_le_bytes());
        let mut padded = payload.to_vec();
        padded.resize((padded.len() + 7) & !7, 0);
        let sum = checksum(checksum(self.checksum, &frame[..32]), &padded);
        frame[32..36].copy_from_slice(&sum[0].to_le_bytes());
        frame[36..40].copy_from_slice(&sum[1].to_le_bytes());
        self.file.write_all(&frame)?;
        self.file.write_all(&padded)?;
        self.checksum = sum;
        self.next_frame += 1;
        self.len += (FRAME_HEADER_LEN + padded.len()) as u64;
        Ok(())
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == WAL_HEADER_LEN as u64
    }

    pub fn policy(&self) -> SyncPolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: SyncPolicy) -> Result<()> {
        self.ensure_healthy()?;
        if policy == SyncPolicy::Full {
            self.file.sync_all()?;
        }
        self.policy = policy;
        Ok(())
    }

    /// Checkpoint barrier, also required in NORMAL mode.
    pub fn sync(&self) -> Result<()> {
        self.ensure_healthy()?;
        if self.policy != SyncPolicy::Off {
            self.file.sync_all()?;
        }
        Ok(())
    }

    pub(crate) fn ensure_healthy(&self) -> Result<()> {
        if self.failed {
            return Err(invalid(
                "WAL I/O failed; reopen the database before writing again",
            ));
        }
        Ok(())
    }

    /// Caller MUST sync the main database before resetting a nonempty log.
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.reset_with(&mut |_| {})
    }

    pub(crate) fn reset_with(&mut self, observe: &mut impl FnMut(&'static str)) -> Result<()> {
        self.ensure_healthy()?;
        let result = (|| {
            self.file.set_len(0)?;
            // Persist truncation before replacing the salt/header, preventing
            // an old frame tail from reappearing beneath a new generation.
            self.sync()?;
            observe("wal_truncated");
            self.file.seek(SeekFrom::Start(0))?;
            self.salt = rand::random();
            let mut header = [0; WAL_HEADER_LEN];
            header[..8].copy_from_slice(MAGIC);
            header[8..12].copy_from_slice(&VERSION.to_le_bytes());
            header[12..16].copy_from_slice(&(FRAME_PAYLOAD_SIZE as u32).to_le_bytes());
            header[16..24].copy_from_slice(&self.salt.to_le_bytes());
            self.checksum = checksum([0; 2], &header[..24]);
            header[24..28].copy_from_slice(&self.checksum[0].to_le_bytes());
            header[28..32].copy_from_slice(&self.checksum[1].to_le_bytes());
            self.file.write_all(&header)?;
            observe("header_written");
            self.sync()?;
            self.len = WAL_HEADER_LEN as u64;
            self.next_frame = 1;
            self.next_transaction = 1;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

fn invalid(message: &str) -> NexusError {
    io::Error::new(io::ErrorKind::InvalidData, message).into()
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// SQLite's pair of rolling Fibonacci-weighted sums, with wrapping arithmetic.
/// Covers the previous checksum, frame identity, lengths, flags, and padded data.
fn checksum(mut sum: [u32; 2], bytes: &[u8]) -> [u32; 2] {
    debug_assert_eq!(bytes.len() % 8, 0);
    for pair in bytes.chunks_exact(8) {
        sum[0] = sum[0].wrapping_add(u32_at(pair, 0)).wrapping_add(sum[1]);
        sum[1] = sum[1].wrapping_add(u32_at(pair, 4)).wrapping_add(sum[0]);
    }
    sum
}

fn sync_directory(path: &Path) -> Result<()> {
    // POSIX requires syncing the directory entry of a newly created WAL.
    // Windows sync_all uses FlushFileBuffers on the WAL file itself.
    #[cfg(unix)]
    {
        // sled may have just created several nested database directories.
        // Sync their ancestors too, so their names survive the first commit.
        for directory in path.canonicalize()?.ancestors() {
            File::open(directory)?.sync_all()?;
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_failure_poisoning_prevents_later_commits() {
        let root = tempfile::TempDir::new().unwrap();
        let path = root.path().join("failure.wal");
        let (mut wal, _) = WriteAheadLog::open(&path, SyncPolicy::Full).unwrap();
        wal.append(b"committed").unwrap();
        wal.file = File::open(&path).unwrap(); // read-only handle injects write failure
        assert!(wal.append(b"uncommitted").is_err());
        assert!(wal
            .append(b"later")
            .unwrap_err()
            .to_string()
            .contains("reopen"));
        assert!(wal.reset().is_err());
        drop(wal);
        assert_eq!(
            WriteAheadLog::open(&path, SyncPolicy::Full).unwrap().1,
            vec![b"committed".to_vec()]
        );
    }
}
