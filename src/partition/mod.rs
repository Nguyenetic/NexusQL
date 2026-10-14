// Week 12: Table Partitioning
//
// Implements PostgreSQL-compatible table partitioning for scaling to billion-row tables:
// - Range Partitioning: Partition by continuous ranges (dates, numbers)
// - List Partitioning: Partition by discrete values (regions, categories)
// - Hash Partitioning: Distribute data evenly across partitions
// - Partition Pruning: Query optimization to skip irrelevant partitions
// - Partition Management: ATTACH/DETACH partitions dynamically

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// Partition strategy types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionStrategy {
    /// PARTITION BY RANGE (column) - for dates, numbers
    Range,
    /// PARTITION BY LIST (column) - for categorical values
    List,
    /// PARTITION BY HASH (column) - for even distribution
    Hash,
}

/// Represents a partitioned table
#[derive(Debug, Clone)]
pub struct PartitionedTable {
    /// Table name
    pub name: String,
    /// Partition strategy
    pub strategy: PartitionStrategy,
    /// Partition key column name
    pub partition_key: String,
    /// All partitions
    partitions: Arc<RwLock<Vec<Partition>>>,
    /// Default partition (catches unmatched rows)
    default_partition: Arc<RwLock<Option<Partition>>>,
}

/// Individual partition
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition name
    pub name: String,
    /// Parent table name
    pub parent_table: String,
    /// Partition bounds
    pub bounds: PartitionBounds,
    /// Rows stored in this partition
    pub rows: Vec<Row>,
    /// Statistics
    pub stats: PartitionStats,
}

/// Partition bounds based on strategy
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBounds {
    /// Range: FROM (min) TO (max) - max is exclusive
    Range { from: Value, to: Value },
    /// List: IN (value1, value2, ...)
    List { values: Vec<Value> },
    /// Hash: WITH (MODULUS m, REMAINDER r)
    Hash { modulus: usize, remainder: usize },
    /// Default partition (no bounds)
    Default,
}

/// Partition statistics
#[derive(Debug, Clone)]
pub struct PartitionStats {
    pub row_count: usize,
    pub size_bytes: usize,
    pub last_modified: std::time::Instant,
}

impl PartitionedTable {
    /// Create a new partitioned table
    pub fn new(name: String, strategy: PartitionStrategy, partition_key: String) -> Self {
        Self {
            name,
            strategy,
            partition_key,
            partitions: Arc::new(RwLock::new(Vec::new())),
            default_partition: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a new partition
    pub fn add_partition(&self, name: String, bounds: PartitionBounds) -> Result<()> {
        // Validate bounds match strategy
        match (&self.strategy, &bounds) {
            (PartitionStrategy::Range, PartitionBounds::Range { .. }) => {}
            (PartitionStrategy::List, PartitionBounds::List { .. }) => {}
            (PartitionStrategy::Hash, PartitionBounds::Hash { .. }) => {}
            (_, PartitionBounds::Default) => {
                // Default partition allowed for Range and List
                if self.strategy == PartitionStrategy::Hash {
                    return Err(NexusError::ExecutionError(
                        "Hash partitioned tables cannot have DEFAULT partition".to_string(),
                    ));
                }
                let mut default = self.default_partition.write().unwrap();
                if default.is_some() {
                    return Err(NexusError::ExecutionError(
                        "DEFAULT partition already exists".to_string(),
                    ));
                }
                *default = Some(Partition::new(name, self.name.clone(), bounds));
                return Ok(());
            }
            _ => {
                return Err(NexusError::ExecutionError(
                    "Partition bounds do not match table partition strategy".to_string(),
                ))
            }
        }

        // Check for overlapping partitions
        let partitions = self.partitions.read().unwrap();
        for existing in partitions.iter() {
            if Self::bounds_overlap(&existing.bounds, &bounds)? {
                return Err(NexusError::ExecutionError(format!(
                    "Partition bounds overlap with existing partition: {}",
                    existing.name
                )));
            }
        }
        drop(partitions);

        // Add partition
        let partition = Partition::new(name, self.name.clone(), bounds);
        self.partitions.write().unwrap().push(partition);
        Ok(())
    }

    /// Check if two partition bounds overlap
    fn bounds_overlap(a: &PartitionBounds, b: &PartitionBounds) -> Result<bool> {
        match (a, b) {
            (
                PartitionBounds::Range {
                    from: a_from,
                    to: a_to,
                },
                PartitionBounds::Range {
                    from: b_from,
                    to: b_to,
                },
            ) => {
                // Ranges overlap if: a_from < b_to AND b_from < a_to
                Ok(a_from < b_to && b_from < a_to)
            }
            (
                PartitionBounds::List { values: a_values },
                PartitionBounds::List { values: b_values },
            ) => {
                // Lists overlap if they share any values
                let a_set: HashSet<_> = a_values.iter().collect();
                Ok(b_values.iter().any(|v| a_set.contains(v)))
            }
            (
                PartitionBounds::Hash {
                    modulus: a_mod,
                    remainder: a_rem,
                },
                PartitionBounds::Hash {
                    modulus: b_mod,
                    remainder: b_rem,
                },
            ) => {
                // Hash partitions overlap if they have same modulus and remainder
                Ok(a_mod == b_mod && a_rem == b_rem)
            }
            _ => Ok(false), // Different bound types don't overlap
        }
    }

    /// Insert a row into the appropriate partition
    pub fn insert(&self, row: Row) -> Result<String> {
        // Extract partition key value
        let key_value = self.extract_partition_key(&row)?;

        // Find matching partition name first (without holding lock)
        let matching_partition_name = {
            let partitions = self.partitions.read().unwrap();
            let mut found = None;
            for partition in partitions.iter() {
                if Self::value_matches_bounds(&key_value, &partition.bounds, &self.strategy)? {
                    found = Some(partition.name.clone());
                    break;
                }
            }
            found
        };

        // If we found a matching partition, insert into it
        if let Some(partition_name) = matching_partition_name {
            let mut partitions_write = self.partitions.write().unwrap();
            if let Some(p) = partitions_write
                .iter_mut()
                .find(|p| p.name == partition_name)
            {
                p.insert(row)?;
                return Ok(p.name.clone());
            }
        }

        // Try default partition
        let mut default = self.default_partition.write().unwrap();
        if let Some(ref mut partition) = *default {
            partition.insert(row)?;
            return Ok(partition.name.clone());
        }

        Err(NexusError::ExecutionError(format!(
            "No partition found for value: {:?}",
            key_value
        )))
    }

    /// Extract partition key value from row
    fn extract_partition_key(&self, row: &Row) -> Result<Value> {
        // Find the column index
        let col_idx = row
            .columns
            .iter()
            .position(|c| c == &self.partition_key)
            .ok_or_else(|| {
                NexusError::ExecutionError(format!(
                    "Partition key column not found: {}",
                    self.partition_key
                ))
            })?;

        Ok(row.values[col_idx].clone())
    }

    /// Check if a value matches partition bounds
    fn value_matches_bounds(
        value: &Value,
        bounds: &PartitionBounds,
        strategy: &PartitionStrategy,
    ) -> Result<bool> {
        match (strategy, bounds) {
            (PartitionStrategy::Range, PartitionBounds::Range { from, to }) => {
                // value >= from AND value < to (to is exclusive)
                Ok(value >= from && value < to)
            }
            (PartitionStrategy::List, PartitionBounds::List { values }) => {
                // value IN (values)
                Ok(values.contains(value))
            }
            (PartitionStrategy::Hash, PartitionBounds::Hash { modulus, remainder }) => {
                // hash(value) % modulus == remainder
                let hash = Self::hash_value(value);
                Ok(hash % modulus == *remainder)
            }
            (_, PartitionBounds::Default) => Ok(true), // Default matches everything
            _ => Err(NexusError::ExecutionError(
                "Value and bounds mismatch".to_string(),
            )),
        }
    }

    /// Hash a value for hash partitioning
    fn hash_value(value: &Value) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        match value {
            Value::Integer(i) => i.hash(&mut hasher),
            Value::Float(f) => f.hash(&mut hasher),
            Value::Text(s) => s.hash(&mut hasher),
            Value::Geometry(g) => g.hash(&mut hasher),
            Value::Boolean(b) => b.hash(&mut hasher),
            Value::Null => 0usize.hash(&mut hasher),
            Value::Blob(b) => b.hash(&mut hasher),
            Value::Vector(v) => {
                // Hash vectors by hashing their string representation
                // For proper vector partitioning, use partition keys on metadata fields instead
                for f in v {
                    f.to_bits().hash(&mut hasher);
                }
            }
        }
        hasher.finish() as usize
    }

    /// Query with partition pruning
    pub fn query_with_pruning(&self, filter: Option<PartitionFilter>) -> Result<Vec<Row>> {
        let mut results = Vec::new();

        if let Some(filter) = filter {
            // Prune partitions based on filter
            let pruned = self.prune_partitions(&filter)?;

            // Query only relevant partitions
            let partitions = self.partitions.read().unwrap();
            for partition in partitions.iter() {
                if pruned.contains(&partition.name) {
                    results.extend(partition.rows.clone());
                }
            }

            // Check default partition if exists
            let default = self.default_partition.read().unwrap();
            if let Some(ref partition) = *default {
                if pruned.contains(&partition.name) {
                    results.extend(partition.rows.clone());
                }
            }
        } else {
            // No filter - scan all partitions
            let partitions = self.partitions.read().unwrap();
            for partition in partitions.iter() {
                results.extend(partition.rows.clone());
            }

            let default = self.default_partition.read().unwrap();
            if let Some(ref partition) = *default {
                results.extend(partition.rows.clone());
            }
        }

        Ok(results)
    }

    /// Prune partitions based on filter (partition pruning optimization)
    fn prune_partitions(&self, filter: &PartitionFilter) -> Result<Vec<String>> {
        let mut relevant = Vec::new();
        let partitions = self.partitions.read().unwrap();

        for partition in partitions.iter() {
            if Self::filter_matches_partition(filter, &partition.bounds, &self.strategy)? {
                relevant.push(partition.name.clone());
            }
        }

        // Always include default partition if it exists
        let default = self.default_partition.read().unwrap();
        if let Some(ref partition) = *default {
            relevant.push(partition.name.clone());
        }

        Ok(relevant)
    }

    /// Check if a filter can match rows in a partition
    fn filter_matches_partition(
        filter: &PartitionFilter,
        bounds: &PartitionBounds,
        strategy: &PartitionStrategy,
    ) -> Result<bool> {
        match (strategy, bounds, filter) {
            // Range partitioning with equality filter
            (
                PartitionStrategy::Range,
                PartitionBounds::Range { from, to },
                PartitionFilter::Equals(value),
            ) => {
                // Check if value falls in range [from, to)
                Ok(value >= from && value < to)
            }
            // Range partitioning with range filter
            (
                PartitionStrategy::Range,
                PartitionBounds::Range {
                    from: part_from,
                    to: part_to,
                },
                PartitionFilter::Range { min, max },
            ) => {
                // Check if ranges overlap
                Ok(part_from < max && min < part_to)
            }
            // List partitioning with equality filter
            (
                PartitionStrategy::List,
                PartitionBounds::List { values },
                PartitionFilter::Equals(value),
            ) => Ok(values.contains(value)),
            // List partitioning with IN filter
            (
                PartitionStrategy::List,
                PartitionBounds::List {
                    values: part_values,
                },
                PartitionFilter::In(filter_values),
            ) => {
                // Check if any filter value is in partition values
                Ok(filter_values.iter().any(|v| part_values.contains(v)))
            }
            // Hash partitioning with equality filter
            (
                PartitionStrategy::Hash,
                PartitionBounds::Hash { modulus, remainder },
                PartitionFilter::Equals(value),
            ) => {
                let hash = Self::hash_value(value);
                Ok(hash % modulus == *remainder)
            }
            // Default partition matches everything
            (_, PartitionBounds::Default, _) => Ok(true),
            // Unsupported combination - be conservative and include partition
            _ => Ok(true),
        }
    }

    /// Get partition statistics
    pub fn get_stats(&self) -> PartitionedTableStats {
        let partitions = self.partitions.read().unwrap();
        let default = self.default_partition.read().unwrap();

        let mut stats = PartitionedTableStats {
            table_name: self.name.clone(),
            strategy: self.strategy.clone(),
            partition_count: partitions.len(),
            total_rows: 0,
            total_size_bytes: 0,
            partitions: Vec::new(),
        };

        for partition in partitions.iter() {
            stats.total_rows += partition.stats.row_count;
            stats.total_size_bytes += partition.stats.size_bytes;
            stats
                .partitions
                .push((partition.name.clone(), partition.stats.clone()));
        }

        if let Some(ref partition) = *default {
            stats.partition_count += 1;
            stats.total_rows += partition.stats.row_count;
            stats.total_size_bytes += partition.stats.size_bytes;
            stats
                .partitions
                .push((partition.name.clone(), partition.stats.clone()));
        }

        stats
    }

    /// Detach a partition (remove from table but keep data)
    pub fn detach_partition(&self, partition_name: &str) -> Result<Partition> {
        let mut partitions = self.partitions.write().unwrap();

        if let Some(idx) = partitions.iter().position(|p| p.name == partition_name) {
            return Ok(partitions.remove(idx));
        }

        // Check default partition
        let mut default = self.default_partition.write().unwrap();
        if let Some(ref partition) = *default {
            if partition.name == partition_name {
                return Ok(default.take().unwrap());
            }
        }

        Err(NexusError::ExecutionError(format!(
            "Partition not found: {}",
            partition_name
        )))
    }

    /// Attach an existing partition
    pub fn attach_partition(&self, partition: Partition) -> Result<()> {
        // Validate partition
        if partition.parent_table != self.name {
            return Err(NexusError::ExecutionError(format!(
                "Partition parent mismatch: expected {}, got {}",
                self.name, partition.parent_table
            )));
        }

        // Check bounds match strategy
        match (&self.strategy, &partition.bounds) {
            (PartitionStrategy::Range, PartitionBounds::Range { .. })
            | (PartitionStrategy::List, PartitionBounds::List { .. })
            | (PartitionStrategy::Hash, PartitionBounds::Hash { .. }) => {}
            (_, PartitionBounds::Default) => {
                if self.strategy == PartitionStrategy::Hash {
                    return Err(NexusError::ExecutionError(
                        "Hash partitioned tables cannot have DEFAULT partition".to_string(),
                    ));
                }
                let mut default = self.default_partition.write().unwrap();
                if default.is_some() {
                    return Err(NexusError::ExecutionError(
                        "DEFAULT partition already exists".to_string(),
                    ));
                }
                *default = Some(partition);
                return Ok(());
            }
            _ => {
                return Err(NexusError::ExecutionError(
                    "Partition bounds do not match table partition strategy".to_string(),
                ))
            }
        }

        // Check for overlaps
        let partitions = self.partitions.read().unwrap();
        for existing in partitions.iter() {
            if Self::bounds_overlap(&existing.bounds, &partition.bounds)? {
                return Err(NexusError::ExecutionError(format!(
                    "Partition bounds overlap with existing partition: {}",
                    existing.name
                )));
            }
        }
        drop(partitions);

        self.partitions.write().unwrap().push(partition);
        Ok(())
    }
}

impl Partition {
    /// Create a new partition
    pub fn new(name: String, parent_table: String, bounds: PartitionBounds) -> Self {
        Self {
            name,
            parent_table,
            bounds,
            rows: Vec::new(),
            stats: PartitionStats {
                row_count: 0,
                size_bytes: 0,
                last_modified: std::time::Instant::now(),
            },
        }
    }

    /// Insert a row
    pub fn insert(&mut self, row: Row) -> Result<()> {
        let row_size = std::mem::size_of_val(&row);
        self.rows.push(row);
        self.stats.row_count += 1;
        self.stats.size_bytes += row_size;
        self.stats.last_modified = std::time::Instant::now();
        Ok(())
    }

    /// Get all rows
    pub fn scan(&self) -> Vec<Row> {
        self.rows.clone()
    }
}

/// Filter for partition pruning
#[derive(Debug, Clone)]
pub enum PartitionFilter {
    /// column = value
    Equals(Value),
    /// column IN (values)
    In(Vec<Value>),
    /// column BETWEEN min AND max
    Range { min: Value, max: Value },
}

/// Statistics for partitioned table
#[derive(Debug, Clone)]
pub struct PartitionedTableStats {
    pub table_name: String,
    pub strategy: PartitionStrategy,
    pub partition_count: usize,
    pub total_rows: usize,
    pub total_size_bytes: usize,
    pub partitions: Vec<(String, PartitionStats)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_partitioning() {
        let table = PartitionedTable::new(
            "measurement".to_string(),
            PartitionStrategy::Range,
            "logdate".to_string(),
        );

        // Add range partitions for 2024 months
        table
            .add_partition(
                "measurement_2024_01".to_string(),
                PartitionBounds::Range {
                    from: Value::Text("2024-01-01".to_string()),
                    to: Value::Text("2024-02-01".to_string()),
                },
            )
            .unwrap();

        table
            .add_partition(
                "measurement_2024_02".to_string(),
                PartitionBounds::Range {
                    from: Value::Text("2024-02-01".to_string()),
                    to: Value::Text("2024-03-01".to_string()),
                },
            )
            .unwrap();

        // Insert rows
        let row1 = Row::new(
            vec!["logdate".to_string(), "temp".to_string()],
            vec![Value::Text("2024-01-15".to_string()), Value::Integer(25)],
        );
        let partition = table.insert(row1).unwrap();
        assert_eq!(partition, "measurement_2024_01");

        let row2 = Row::new(
            vec!["logdate".to_string(), "temp".to_string()],
            vec![Value::Text("2024-02-15".to_string()), Value::Integer(30)],
        );
        let partition = table.insert(row2).unwrap();
        assert_eq!(partition, "measurement_2024_02");

        // Test partition pruning
        let filter = PartitionFilter::Equals(Value::Text("2024-01-15".to_string()));
        let results = table.query_with_pruning(Some(filter)).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_list_partitioning() {
        let table = PartitionedTable::new(
            "sales".to_string(),
            PartitionStrategy::List,
            "region".to_string(),
        );

        // Add list partitions for regions
        table
            .add_partition(
                "sales_americas".to_string(),
                PartitionBounds::List {
                    values: vec![
                        Value::Text("North America".to_string()),
                        Value::Text("South America".to_string()),
                    ],
                },
            )
            .unwrap();

        table
            .add_partition(
                "sales_europe".to_string(),
                PartitionBounds::List {
                    values: vec![
                        Value::Text("Europe".to_string()),
                        Value::Text("UK".to_string()),
                    ],
                },
            )
            .unwrap();

        // Insert rows
        let row1 = Row::new(
            vec!["region".to_string(), "amount".to_string()],
            vec![
                Value::Text("North America".to_string()),
                Value::Integer(1000),
            ],
        );
        let partition = table.insert(row1).unwrap();
        assert_eq!(partition, "sales_americas");

        let row2 = Row::new(
            vec!["region".to_string(), "amount".to_string()],
            vec![Value::Text("Europe".to_string()), Value::Integer(2000)],
        );
        let partition = table.insert(row2).unwrap();
        assert_eq!(partition, "sales_europe");
    }

    #[test]
    fn test_hash_partitioning() {
        let table = PartitionedTable::new(
            "events".to_string(),
            PartitionStrategy::Hash,
            "user_id".to_string(),
        );

        // Add 4 hash partitions
        for i in 0..4 {
            table
                .add_partition(
                    format!("events_p{}", i),
                    PartitionBounds::Hash {
                        modulus: 4,
                        remainder: i,
                    },
                )
                .unwrap();
        }

        // Insert rows with different user_ids
        for user_id in 0..100 {
            let row = Row::new(
                vec!["user_id".to_string(), "event_type".to_string()],
                vec![Value::Integer(user_id), Value::Text("click".to_string())],
            );
            table.insert(row).unwrap();
        }

        // Verify distribution
        let stats = table.get_stats();
        assert_eq!(stats.partition_count, 4);
        assert_eq!(stats.total_rows, 100);

        // Each partition should have roughly 25 rows (100 / 4)
        for (_, part_stats) in stats.partitions {
            assert!(part_stats.row_count > 10 && part_stats.row_count < 40);
        }
    }

    #[test]
    fn test_default_partition() {
        let table = PartitionedTable::new(
            "sales".to_string(),
            PartitionStrategy::List,
            "region".to_string(),
        );

        // Add specific region partition
        table
            .add_partition(
                "sales_americas".to_string(),
                PartitionBounds::List {
                    values: vec![Value::Text("North America".to_string())],
                },
            )
            .unwrap();

        // Add default partition for other regions
        table
            .add_partition("sales_other".to_string(), PartitionBounds::Default)
            .unwrap();

        // Insert matching row
        let row1 = Row::new(
            vec!["region".to_string(), "amount".to_string()],
            vec![
                Value::Text("North America".to_string()),
                Value::Integer(1000),
            ],
        );
        let partition1 = table.insert(row1).unwrap();
        assert_eq!(partition1, "sales_americas");

        // Insert non-matching row (goes to default)
        let row2 = Row::new(
            vec!["region".to_string(), "amount".to_string()],
            vec![Value::Text("Asia".to_string()), Value::Integer(2000)],
        );
        let partition2 = table.insert(row2).unwrap();
        assert_eq!(partition2, "sales_other");
    }

    #[test]
    fn test_attach_detach_partition() {
        let table = PartitionedTable::new(
            "measurement".to_string(),
            PartitionStrategy::Range,
            "logdate".to_string(),
        );

        // Create and attach partition
        let partition = Partition::new(
            "measurement_2024_01".to_string(),
            "measurement".to_string(),
            PartitionBounds::Range {
                from: Value::Text("2024-01-01".to_string()),
                to: Value::Text("2024-02-01".to_string()),
            },
        );
        table.attach_partition(partition).unwrap();

        // Verify partition exists
        let stats = table.get_stats();
        assert_eq!(stats.partition_count, 1);

        // Detach partition
        let detached = table.detach_partition("measurement_2024_01").unwrap();
        assert_eq!(detached.name, "measurement_2024_01");

        // Verify partition removed
        let stats = table.get_stats();
        assert_eq!(stats.partition_count, 0);
    }

    #[test]
    fn test_partition_pruning_range() {
        let table = PartitionedTable::new(
            "measurement".to_string(),
            PartitionStrategy::Range,
            "logdate".to_string(),
        );

        // Add 3 monthly partitions
        for month in 1..=3 {
            table
                .add_partition(
                    format!("measurement_2024_{:02}", month),
                    PartitionBounds::Range {
                        from: Value::Text(format!("2024-{:02}-01", month)),
                        to: Value::Text(format!("2024-{:02}-01", month + 1)),
                    },
                )
                .unwrap();
        }

        // Insert data across partitions
        for month in 1..=3 {
            for day in 1..=10 {
                let row = Row::new(
                    vec!["logdate".to_string(), "value".to_string()],
                    vec![
                        Value::Text(format!("2024-{:02}-{:02}", month, day)),
                        Value::Integer(month * 10 + day),
                    ],
                );
                table.insert(row).unwrap();
            }
        }

        // Query with range filter - should prune 2 partitions
        let filter = PartitionFilter::Range {
            min: Value::Text("2024-02-01".to_string()),
            max: Value::Text("2024-03-01".to_string()),
        };
        let results = table.query_with_pruning(Some(filter)).unwrap();
        // Should only get data from measurement_2024_02
        assert_eq!(results.len(), 10);
    }
}
