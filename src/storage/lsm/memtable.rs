//! MemTable - In-Memory Write Buffer
//! PROPRIETARY - RocksDB-inspired

use std::collections::BTreeMap;

/// Entry in memtable (includes sequence number for MVCC)
#[derive(Clone)]
pub struct MemTableEntry {
    pub value: Option<Vec<u8>>,  // None = deleted
    pub sequence: u64,
}

/// In-memory sorted table
pub struct MemTable {
    data: BTreeMap<Vec<u8>, MemTableEntry>,
    size_bytes: usize,
}

impl MemTable {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            size_bytes: 0,
        }
    }
    
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>, sequence: u64) {
        let value_size = value.len();
        let key_size = key.len();
        
        self.data.insert(key, MemTableEntry {
            value: Some(value),
            sequence,
        });
        
        self.size_bytes += key_size + value_size + 8;  // +8 for sequence
    }
    
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key)
            .and_then(|entry| entry.value.clone())
    }
    
    pub fn delete(&mut self, key: Vec<u8>, sequence: u64) {
        self.data.insert(key, MemTableEntry {
            value: None,  // Tombstone
            sequence,
        });
    }
    
    pub fn size(&self) -> usize {
        self.size_bytes
    }
    
    pub fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &MemTableEntry)> {
        self.data.iter()
    }
    
    pub fn clear(&mut self) {
        self.data.clear();
        self.size_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memtable() {
        let mut mt = MemTable::new();
        
        mt.put(b"key1".to_vec(), b"value1".to_vec(), 1);
        mt.put(b"key2".to_vec(), b"value2".to_vec(), 2);
        
        assert_eq!(mt.get(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(mt.get(b"key2"), Some(b"value2".to_vec()));
        assert!(mt.size() > 0);
    }
    
    #[test]
    fn test_delete() {
        let mut mt = MemTable::new();
        
        mt.put(b"key1".to_vec(), b"value1".to_vec(), 1);
        mt.delete(b"key1".to_vec(), 2);
        
        assert_eq!(mt.get(b"key1"), None);
    }
}
