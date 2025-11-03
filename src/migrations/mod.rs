// Migration Framework - Schema Versioning & Migrations
// File-based migration system for schema evolution

use crate::{NexusError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// Migration definition
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: u64,
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
    pub applied_at: Option<SystemTime>,
}

impl Migration {
    pub fn new(version: u64, name: String, up_sql: String, down_sql: String) -> Self {
        Self {
            version,
            name,
            up_sql,
            down_sql,
            applied_at: None,
        }
    }
}

/// Migration manager
pub struct MigrationManager {
    migrations_dir: PathBuf,
    applied_migrations: HashMap<u64, Migration>,
}

impl MigrationManager {
    pub fn new(migrations_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&migrations_dir).map_err(|e| NexusError::IoError(e))?;

        Ok(Self {
            migrations_dir,
            applied_migrations: HashMap::new(),
        })
    }

    /// Create a new migration file
    pub fn create_migration(&self, name: &str) -> Result<Migration> {
        let version = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let migration = Migration::new(
            version,
            name.to_string(),
            "-- Add your UP migration SQL here\n".to_string(),
            "-- Add your DOWN migration SQL here\n".to_string(),
        );

        // Write migration file
        let filename = format!("{}_{}.sql", version, name);
        let filepath = self.migrations_dir.join(&filename);

        let content = format!(
            "-- Migration: {}\n-- Version: {}\n\n-- UP\n{}\n-- DOWN\n{}",
            name, version, migration.up_sql, migration.down_sql
        );

        fs::write(&filepath, content).map_err(|e| NexusError::IoError(e))?;

        println!("Created migration: {}", filename);

        Ok(migration)
    }

    /// Apply pending migrations
    pub fn migrate_up(&mut self) -> Result<Vec<u64>> {
        let pending = self.get_pending_migrations()?;
        let mut applied = Vec::new();

        for migration in pending {
            println!(
                "Applying migration {}: {}",
                migration.version, migration.name
            );

            // Execute UP SQL (would execute against database)
            // In production: parse and execute migration.up_sql

            // Mark as applied
            let mut mig = migration.clone();
            mig.applied_at = Some(SystemTime::now());
            self.applied_migrations.insert(migration.version, mig);
            applied.push(migration.version);

            println!("  ✓ Applied");
        }

        // Save migration history
        self.save_history()?;

        Ok(applied)
    }

    /// Rollback last migration
    pub fn migrate_down(&mut self) -> Result<Option<u64>> {
        // Find last applied migration
        let last_version = self.applied_migrations.keys().max().copied();

        if let Some(version) = last_version {
            if let Some(migration) = self.applied_migrations.get(&version) {
                println!("Rolling back migration {}: {}", version, migration.name);

                // Execute DOWN SQL
                // In production: parse and execute migration.down_sql

                self.applied_migrations.remove(&version);
                self.save_history()?;

                println!("  ✓ Rolled back");
                return Ok(Some(version));
            }
        }

        Ok(None)
    }

    /// Get pending migrations
    fn get_pending_migrations(&self) -> Result<Vec<Migration>> {
        // Read migration files from directory
        let entries = fs::read_dir(&self.migrations_dir).map_err(|e| NexusError::IoError(e))?;

        let mut migrations = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| NexusError::IoError(e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    // Parse version from filename (e.g., "1730000000_create_users.sql")
                    if let Some((version_str, name)) = filename.split_once('_') {
                        if let Ok(version) = version_str.parse::<u64>() {
                            // Skip if already applied
                            if self.applied_migrations.contains_key(&version) {
                                continue;
                            }

                            // Read migration file
                            let content =
                                fs::read_to_string(&path).map_err(|e| NexusError::IoError(e))?;

                            // Parse UP and DOWN sections (simplified)
                            let up_sql = content.clone();
                            let down_sql = String::new();

                            migrations.push(Migration::new(
                                version,
                                name.to_string(),
                                up_sql,
                                down_sql,
                            ));
                        }
                    }
                }
            }
        }

        migrations.sort_by_key(|m| m.version);
        Ok(migrations)
    }

    /// Save migration history to file
    fn save_history(&self) -> Result<()> {
        let history_file = self.migrations_dir.join("migration_history.txt");
        let mut content = String::from("# Applied Migrations\n\n");

        let mut versions: Vec<_> = self.applied_migrations.keys().collect();
        versions.sort();

        for version in versions {
            if let Some(mig) = self.applied_migrations.get(version) {
                content.push_str(&format!("{} - {}\n", version, mig.name));
            }
        }

        fs::write(&history_file, content).map_err(|e| NexusError::IoError(e))?;

        Ok(())
    }

    /// Get migration status
    pub fn status(&self) -> MigrationStatus {
        MigrationStatus {
            applied_count: self.applied_migrations.len(),
            pending_count: self.get_pending_migrations().unwrap_or_default().len(),
            last_version: self.applied_migrations.keys().max().copied(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub applied_count: usize,
    pub pending_count: usize,
    pub last_version: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_migration() {
        let temp_dir = tempdir().unwrap();
        let mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        let migration = mgr.create_migration("create_users_table").unwrap();
        assert!(migration.version > 0);
        assert_eq!(migration.name, "create_users_table");
    }

    #[test]
    fn test_migration_status() {
        let temp_dir = tempdir().unwrap();
        let mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        let status = mgr.status();
        assert_eq!(status.applied_count, 0);
        assert_eq!(status.pending_count, 0);
    }

    #[test]
    fn test_migrate_up() {
        let temp_dir = tempdir().unwrap();
        let mut mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Create a migration file
        mgr.create_migration("test_migration").unwrap();

        // Apply migrations
        let applied = mgr.migrate_up().unwrap();
        assert_eq!(applied.len(), 1);

        let status = mgr.status();
        assert_eq!(status.applied_count, 1);
    }

    #[test]
    fn test_migrate_down() {
        let temp_dir = tempdir().unwrap();
        let mut mgr = MigrationManager::new(temp_dir.path().to_path_buf()).unwrap();

        mgr.create_migration("test_migration").unwrap();
        mgr.migrate_up().unwrap();

        // Rollback
        let rolled_back = mgr.migrate_down().unwrap();
        assert!(rolled_back.is_some());

        let status = mgr.status();
        assert_eq!(status.applied_count, 0);
    }
}
