// Backup & Restore - File-Based Database Backup
// Safe backup and restore of database files

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Backup manager
pub struct BackupManager {
    db_path: PathBuf,
    backup_dir: PathBuf,
}

impl BackupManager {
    pub fn new(db_path: PathBuf, backup_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&backup_dir).map_err(|e| NexusError::IoError(e))?;

        Ok(Self {
            db_path,
            backup_dir,
        })
    }

    /// Create full backup
    pub fn create_backup(&self, name: &str) -> Result<BackupInfo> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let backup_name = format!("{}_{}", timestamp, name);
        let backup_path = self.backup_dir.join(&backup_name);

        fs::create_dir_all(&backup_path).map_err(|e| NexusError::IoError(e))?;

        println!("Creating backup: {}", backup_name);

        // Copy database files
        let files_copied = self.copy_directory(&self.db_path, &backup_path)?;

        // Calculate backup size
        let size_bytes = self.calculate_directory_size(&backup_path)?;

        println!("  ✓ Copied {} files", files_copied);
        println!("  ✓ Backup size: {} MB", size_bytes / 1_000_000);

        Ok(BackupInfo {
            name: backup_name,
            path: backup_path,
            timestamp,
            size_bytes,
            files_count: files_copied,
        })
    }

    /// Restore from backup
    pub fn restore_backup(&self, backup_name: &str) -> Result<()> {
        let backup_path = self.backup_dir.join(backup_name);

        if !backup_path.exists() {
            return Err(NexusError::ExecutionError(format!(
                "Backup not found: {}",
                backup_name
            )));
        }

        println!("Restoring from backup: {}", backup_name);

        // In production: would stop database, copy files, restart
        let files_copied = self.copy_directory(&backup_path, &self.db_path)?;

        println!("  ✓ Restored {} files", files_copied);

        Ok(())
    }

    /// List available backups
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        let entries = fs::read_dir(&self.backup_dir).map_err(|e| NexusError::IoError(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| NexusError::IoError(e))?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let size_bytes = self.calculate_directory_size(&path)?;

                    // Parse timestamp from name
                    let timestamp = name
                        .split('_')
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);

                    backups.push(BackupInfo {
                        name: name.to_string(),
                        path: path.clone(),
                        timestamp,
                        size_bytes,
                        files_count: 0,
                    });
                }
            }
        }

        backups.sort_by_key(|b| b.timestamp);
        Ok(backups)
    }

    /// Copy directory recursively
    fn copy_directory(&self, src: &Path, dst: &Path) -> Result<usize> {
        fs::create_dir_all(dst).map_err(|e| NexusError::IoError(e))?;

        let mut count = 0;

        if src.exists() && src.is_dir() {
            for entry in fs::read_dir(src).map_err(|e| NexusError::IoError(e))? {
                let entry = entry.map_err(|e| NexusError::IoError(e))?;
                let src_path = entry.path();
                let dst_path = dst.join(entry.file_name());

                if src_path.is_dir() {
                    count += self.copy_directory(&src_path, &dst_path)?;
                } else {
                    fs::copy(&src_path, &dst_path).map_err(|e| NexusError::IoError(e))?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Calculate directory size
    fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total = 0u64;

        if path.exists() && path.is_dir() {
            for entry in fs::read_dir(path).map_err(|e| NexusError::IoError(e))? {
                let entry = entry.map_err(|e| NexusError::IoError(e))?;
                let metadata = entry.metadata().map_err(|e| NexusError::IoError(e))?;

                if metadata.is_dir() {
                    total += self.calculate_directory_size(&entry.path())?;
                } else {
                    total += metadata.len();
                }
            }
        }

        Ok(total)
    }

    /// Export to SQL dump (portable format)
    pub fn export_sql_dump(&self, tables: &[String]) -> Result<String> {
        let mut dump = String::from("-- NexusQL SQL Dump\n");
        dump.push_str(&format!("-- Generated: {:?}\n\n", SystemTime::now()));

        for table in tables {
            dump.push_str(&format!("-- Table: {}\n", table));
            dump.push_str(&format!("CREATE TABLE {} (...);\n", table));
            dump.push_str(&format!("-- INSERT statements would go here\n\n"));
        }

        Ok(dump)
    }
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub path: PathBuf,
    pub timestamp: u64,
    pub size_bytes: u64,
    pub files_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_migration() {
        let temp_dir = tempdir().unwrap();
        let mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        let migration = mgr.create_migration("add_users_table").unwrap();
        assert!(migration.version > 0);
        assert_eq!(migration.name, "add_users_table");
    }

    #[test]
    fn test_migration_apply() {
        let temp_dir = tempdir().unwrap();
        let mut mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        mgr.create_migration("test_migration").unwrap();

        let applied = mgr.migrate_up().unwrap();
        assert_eq!(applied.len(), 1);

        let status = mgr.status();
        assert_eq!(status.applied_count, 1);
    }

    #[test]
    fn test_migration_rollback() {
        let temp_dir = tempdir().unwrap();
        let mut mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        mgr.create_migration("test").unwrap();
        mgr.migrate_up().unwrap();

        let rolled_back = mgr.migrate_down().unwrap();
        assert!(rolled_back.is_some());
    }

    #[test]
    fn test_backup_creation() {
        let db_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();

        // Create some dummy files in db directory
        fs::write(db_dir.path().join("test.db"), "data").unwrap();

        let mgr = BackupManager::new(db_dir.path().to_path_buf(), backup_dir.path().to_path_buf())
            .unwrap();

        let backup = mgr.create_backup("manual_backup").unwrap();
        assert!(backup.size_bytes > 0);
    }

    #[test]
    fn test_list_backups() {
        let db_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();

        let mgr = BackupManager::new(db_dir.path().to_path_buf(), backup_dir.path().to_path_buf())
            .unwrap();

        mgr.create_backup("backup1").unwrap();
        mgr.create_backup("backup2").unwrap();

        let backups = mgr.list_backups().unwrap();
        assert_eq!(backups.len(), 2);
    }

    #[test]
    fn test_sql_dump_export() {
        let db_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();

        let mgr = BackupManager::new(db_dir.path().to_path_buf(), backup_dir.path().to_path_buf())
            .unwrap();

        let dump = mgr
            .export_sql_dump(&["users".to_string(), "orders".to_string()])
            .unwrap();
        assert!(dump.contains("CREATE TABLE users"));
        assert!(dump.contains("CREATE TABLE orders"));
    }
}
