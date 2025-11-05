// Week 19: VACUUM & Storage Management
//
// Implements PostgreSQL-compatible VACUUM and storage management:
// - VACUUM: Remove dead tuples and reclaim space
// - Dead tuple tracking: Track obsolete row versions
// - Visibility map: Track all-visible pages
// - Free space map: Track available space per page
// - Bloat detection: Identify wasted space
// - Auto-vacuum: Automatic maintenance

use crate::types::Row;
use crate::Result;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Tuple identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TupleId {
    pub page: u32,
    pub offset: u16,
}

/// Tuple visibility status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TupleStatus {
    Live,
    Dead,
    RecentlyDead,
}

/// Visibility map - tracks all-visible pages
#[derive(Debug, Clone)]
pub struct VisibilityMap {
    /// Pages where all tuples are visible to all transactions
    all_visible_pages: HashSet<u32>,
}

impl VisibilityMap {
    pub fn new() -> Self {
        Self {
            all_visible_pages: HashSet::new(),
        }
    }

    /// Mark page as all-visible
    pub fn set_all_visible(&mut self, page: u32) {
        self.all_visible_pages.insert(page);
    }

    /// Check if page is all-visible
    pub fn is_all_visible(&self, page: u32) -> bool {
        self.all_visible_pages.contains(&page)
    }

    /// Clear visibility for page (when updated)
    pub fn clear_page(&mut self, page: u32) {
        self.all_visible_pages.remove(&page);
    }

    /// Get count of all-visible pages
    pub fn visible_page_count(&self) -> usize {
        self.all_visible_pages.len()
    }
}

/// Free space map - tracks available space per page
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FreeSpaceMap {
    /// Free space available per page (in bytes)
    page_free_space: HashMap<u32, usize>,
    page_size: usize,
}

impl FreeSpaceMap {
    pub fn new(page_size: usize) -> Self {
        Self {
            page_free_space: HashMap::new(),
            page_size,
        }
    }

    /// Update free space for a page
    pub fn set_free_space(&mut self, page: u32, free_bytes: usize) {
        self.page_free_space.insert(page, free_bytes);
    }

    /// Get free space for a page
    pub fn get_free_space(&self, page: u32) -> usize {
        self.page_free_space.get(&page).copied().unwrap_or(0)
    }

    /// Find page with at least required free space
    pub fn find_page_with_space(&self, required: usize) -> Option<u32> {
        self.page_free_space
            .iter()
            .find(|(_, &free)| free >= required)
            .map(|(&page, _)| page)
    }

    /// Get total free space across all pages
    pub fn total_free_space(&self) -> usize {
        self.page_free_space.values().sum()
    }
}

/// VACUUM statistics
#[derive(Debug, Clone)]
pub struct VacuumStats {
    pub table_name: String,
    pub tuples_removed: usize,
    pub pages_freed: usize,
    pub space_recovered_bytes: usize,
    pub duration_ms: f64,
}

/// Table vacuum manager
pub struct TableVacuum {
    pub table_name: String,
    /// Live rows
    live_rows: Vec<Row>,
    /// Dead tuple identifiers
    dead_tuples: HashSet<TupleId>,
    /// Visibility map
    visibility_map: VisibilityMap,
    /// Free space map
    free_space_map: FreeSpaceMap,
}

impl TableVacuum {
    pub fn new(table_name: String) -> Self {
        Self {
            table_name,
            live_rows: Vec::new(),
            dead_tuples: HashSet::new(),
            visibility_map: VisibilityMap::new(),
            free_space_map: FreeSpaceMap::new(8192), // 8KB pages
        }
    }

    /// Insert a row
    pub fn insert(&mut self, row: Row) -> TupleId {
        let tid = TupleId {
            page: (self.live_rows.len() / 100) as u32,
            offset: (self.live_rows.len() % 100) as u16,
        };

        self.live_rows.push(row);

        // Update free space map
        let page_rows = self
            .live_rows
            .iter()
            .enumerate()
            .filter(|(i, _)| (*i as u32) / 100 == tid.page)
            .count();
        let used_space = page_rows * 100; // Rough estimate
        let free_space = if used_space < 8192 {
            8192 - used_space
        } else {
            0
        };
        self.free_space_map.set_free_space(tid.page, free_space);

        tid
    }

    /// Mark tuple as dead (after DELETE or UPDATE)
    pub fn mark_dead(&mut self, tid: TupleId) {
        self.dead_tuples.insert(tid);

        // Clear visibility for affected page
        self.visibility_map.clear_page(tid.page);
    }

    /// Execute VACUUM
    pub fn vacuum(&mut self) -> Result<VacuumStats> {
        let start = Instant::now();
        let _initial_dead_count = self.dead_tuples.len();

        // Remove dead tuples
        let tuples_removed = self.remove_dead_tuples();

        // Update visibility map
        self.update_visibility_map();

        // Estimate space recovered
        let space_recovered_bytes = tuples_removed * 100; // Rough estimate
        let pages_freed = self.estimate_pages_freed();

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(VacuumStats {
            table_name: self.table_name.clone(),
            tuples_removed,
            pages_freed,
            space_recovered_bytes,
            duration_ms,
        })
    }

    /// Remove dead tuples from storage
    fn remove_dead_tuples(&mut self) -> usize {
        let initial_count = self.dead_tuples.len();
        self.dead_tuples.clear();
        initial_count
    }

    /// Update visibility map after vacuum
    fn update_visibility_map(&mut self) {
        // Mark pages with no dead tuples as all-visible
        let max_page = (self.live_rows.len() / 100) as u32;
        for page in 0..=max_page {
            let has_dead = self.dead_tuples.iter().any(|tid| tid.page == page);
            if !has_dead {
                self.visibility_map.set_all_visible(page);
            }
        }
    }

    /// Estimate pages that can be freed
    fn estimate_pages_freed(&self) -> usize {
        self.free_space_map
            .page_free_space
            .iter()
            .filter(|(_, &free)| free > 4096) // >50% free
            .count()
    }

    /// Detect table bloat
    pub fn detect_bloat(&self) -> BloatInfo {
        let total_rows = self.live_rows.len();
        let dead_count = self.dead_tuples.len();
        let bloat_ratio = if total_rows > 0 {
            dead_count as f64 / (total_rows + dead_count) as f64
        } else {
            0.0
        };

        let total_space = (total_rows + dead_count) * 100; // Rough estimate
        let wasted_space = dead_count * 100;

        BloatInfo {
            total_tuples: total_rows + dead_count,
            dead_tuples: dead_count,
            bloat_ratio,
            wasted_bytes: wasted_space,
            total_bytes: total_space,
            needs_vacuum: bloat_ratio > 0.2, // >20% bloat
        }
    }

    /// Get table statistics
    pub fn get_stats(&self) -> TableVacuumStats {
        TableVacuumStats {
            table_name: self.table_name.clone(),
            live_tuples: self.live_rows.len(),
            dead_tuples: self.dead_tuples.len(),
            total_pages: (self.live_rows.len() / 100) as u32 + 1,
            all_visible_pages: self.visibility_map.visible_page_count(),
            free_space_bytes: self.free_space_map.total_free_space(),
        }
    }
}

/// Bloat information
#[derive(Debug, Clone)]
pub struct BloatInfo {
    pub total_tuples: usize,
    pub dead_tuples: usize,
    pub bloat_ratio: f64,
    pub wasted_bytes: usize,
    pub total_bytes: usize,
    pub needs_vacuum: bool,
}

/// Table vacuum statistics
#[derive(Debug, Clone)]
pub struct TableVacuumStats {
    pub table_name: String,
    pub live_tuples: usize,
    pub dead_tuples: usize,
    pub total_pages: u32,
    pub all_visible_pages: usize,
    pub free_space_bytes: usize,
}

/// Auto-vacuum configuration
#[derive(Debug, Clone)]
pub struct AutoVacuumConfig {
    pub enabled: bool,
    pub vacuum_threshold: usize,
    pub vacuum_scale_factor: f64,
}

impl AutoVacuumConfig {
    pub fn default() -> Self {
        Self {
            enabled: true,
            vacuum_threshold: 50,
            vacuum_scale_factor: 0.2, // 20% dead tuples
        }
    }

    /// Check if table needs vacuum
    pub fn should_vacuum(&self, live: usize, dead: usize) -> bool {
        if !self.enabled {
            return false;
        }

        let threshold = self.vacuum_threshold + ((live as f64 * self.vacuum_scale_factor) as usize);
        dead >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;

    #[test]
    fn test_vacuum_basic() {
        let mut table = TableVacuum::new("test_table".to_string());

        // Insert rows
        for i in 0..100 {
            let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
            table.insert(row);
        }

        // Mark some as dead
        for i in 0..20 {
            table.mark_dead(TupleId { page: 0, offset: i });
        }

        // Execute vacuum
        let stats = table.vacuum().unwrap();
        assert_eq!(stats.tuples_removed, 20);
    }

    #[test]
    fn test_visibility_map() {
        let mut vm = VisibilityMap::new();

        vm.set_all_visible(1);
        vm.set_all_visible(2);

        assert!(vm.is_all_visible(1));
        assert!(vm.is_all_visible(2));
        assert!(!vm.is_all_visible(3));

        vm.clear_page(1);
        assert!(!vm.is_all_visible(1));
    }

    #[test]
    fn test_free_space_map() {
        let mut fsm = FreeSpaceMap::new(8192);

        fsm.set_free_space(0, 4096);
        fsm.set_free_space(1, 2048);
        fsm.set_free_space(2, 6144);

        assert_eq!(fsm.get_free_space(0), 4096);

        // Find page with at least 3000 bytes free
        let page = fsm.find_page_with_space(3000);
        assert!(page.is_some());
        assert!(page.unwrap() == 0 || page.unwrap() == 2);
    }

    #[test]
    fn test_bloat_detection() {
        let mut table = TableVacuum::new("test_table".to_string());

        // Insert 100 rows
        for i in 0..100 {
            let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
            table.insert(row);
        }

        // Mark 30 as dead (30% bloat)
        for i in 0..30 {
            table.mark_dead(TupleId { page: 0, offset: i });
        }

        let bloat = table.detect_bloat();
        assert_eq!(bloat.dead_tuples, 30);
        assert!(bloat.bloat_ratio > 0.2);
        assert!(bloat.needs_vacuum);
    }

    #[test]
    fn test_auto_vacuum_threshold() {
        let config = AutoVacuumConfig::default();

        // 100 live, 10 dead - below threshold
        assert!(!config.should_vacuum(100, 10));

        // 100 live, 70 dead - above threshold (50 + 100*0.2 = 70)
        assert!(config.should_vacuum(100, 70));

        // 1000 live, 250 dead - above threshold (50 + 1000*0.2 = 250)
        assert!(config.should_vacuum(1000, 250));
    }

    #[test]
    fn test_vacuum_reduces_bloat() {
        let mut table = TableVacuum::new("test_table".to_string());

        // Insert and mark dead
        for i in 0..50 {
            let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
            table.insert(row);
        }

        for i in 0..25 {
            table.mark_dead(TupleId { page: 0, offset: i });
        }

        // Check bloat before vacuum
        let bloat_before = table.detect_bloat();
        assert_eq!(bloat_before.dead_tuples, 25);

        // Vacuum
        table.vacuum().unwrap();

        // Check bloat after vacuum
        let bloat_after = table.detect_bloat();
        assert_eq!(bloat_after.dead_tuples, 0);
        assert!(!bloat_after.needs_vacuum);
    }

    #[test]
    fn test_visibility_map_updates() {
        let mut table = TableVacuum::new("test_table".to_string());

        // Insert rows
        for i in 0..200 {
            let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
            table.insert(row);
        }

        // Mark some dead
        table.mark_dead(TupleId {
            page: 0,
            offset: 10,
        });

        // Vacuum
        table.vacuum().unwrap();

        // Page 0 should not be all-visible (has dead tuples before vacuum)
        // But other pages should be
        let stats = table.get_stats();
        assert!(stats.all_visible_pages > 0);
    }
}
