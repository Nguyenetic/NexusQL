//! HDC Engine — Hyperdimensional Computing
//!
//! Vector Symbolic Architecture primitives (BSC today; MAP / HRR / FHRR /
//! SCH land in later PRs). Public surface for callers inside NexusQL:
//!
//! - `Hypervector` — current type alias to `BscVec`. Becomes an enum once
//!   PR 3 adds multi-variant support.
//! - `variants::bsc::BscVec` — BSC variant, Binary Spatter Codes.
//! - `variants::HvOps` — trait every variant implements.
//! - `error::HdcError` — shared error type.

pub mod error;
pub mod hypervector; // backcompat shim, re-exports `Hypervector`
pub mod variants;

pub use error::HdcError;
pub use variants::bsc::BscVec;
pub use variants::HvOps;

/// Public hypervector type.
///
/// Today this is a type alias to `BscVec` so existing callers compile
/// unchanged. PR 3 converts it to a true enum (`Hypervector::Bsc(BscVec)`
/// plus `::Map(MapVec)`, etc.) — at that point inherent methods go away
/// and callers must match or use the dispatching methods on the enum.
pub type Hypervector = BscVec;

use crate::engine::{EngineCapabilities, Query, QueryCost, StorageEngine};
use crate::types::{ResultSet, Row, Value};
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// HDC Classifier
pub struct HDCEngine {
    name: String,
    dimension: usize,
    /// Class prototypes (class_name -> hypervector)
    prototypes: Arc<RwLock<HashMap<String, Hypervector>>>,
}

impl HDCEngine {
    pub fn new() -> Self {
        Self {
            name: "hdc".to_string(),
            dimension: 10000,
            prototypes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Train/add a class prototype
    pub async fn add_class(&self, class_name: String, prototype: Hypervector) {
        self.prototypes.write().await.insert(class_name, prototype);
    }

    /// Classify input
    pub async fn classify(&self, input: &Hypervector) -> Option<(String, f32)> {
        let prototypes = self.prototypes.read().await;

        let mut best_class = None;
        let mut best_similarity = 0.0f32;

        for (class_name, prototype) in prototypes.iter() {
            // similarity() now returns Result; mismatched dims would be a
            // programming error at this layer (all prototypes share dim),
            // so unwrap here is acceptable.
            let similarity = input
                .similarity(prototype)
                .expect("classifier prototypes must share dimension");
            if similarity > best_similarity {
                best_similarity = similarity;
                best_class = Some(class_name.clone());
            }
        }

        best_class.map(|name| (name, best_similarity))
    }
}

#[async_trait]
impl StorageEngine for HDCEngine {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            semantic_search: true,
            low_power: true, // ✓ Energy champion
            sql: false,      // HDC does not parse/execute standard SQL
            ..Default::default()
        }
    }

    async fn execute(&self, _query: &Query) -> Result<ResultSet> {
        // Demo classification
        let test_input = Hypervector::random_seeded(self.dimension, 999);

        if let Some((class, confidence)) = self.classify(&test_input).await {
            let rows = vec![Row::new(
                vec!["class".to_string(), "confidence".to_string()],
                vec![
                    Value::Text(class),
                    Value::Float(ordered_float::OrderedFloat(confidence as f64)),
                ],
            )];
            Ok(ResultSet::new(rows))
        } else {
            Ok(ResultSet::empty())
        }
    }

    fn estimate_cost(&self, _query: &Query) -> QueryCost {
        QueryCost {
            latency_ms: 1.0, // Ultra-fast!
            memory_mb: 50.0,
            energy_mw: 10.0, // 50× less than neural!
            accuracy: 0.85,
        }
    }

    async fn initialize(&mut self) -> Result<()> {
        tracing::info!("HDC engine initialized (dimension: {})", self.dimension);

        // Create demo class prototypes
        let class_a = Hypervector::random_seeded(self.dimension, 100);
        let class_b = Hypervector::random_seeded(self.dimension, 200);

        self.add_class("category_a".to_string(), class_a).await;
        self.add_class("category_b".to_string(), class_b).await;

        tracing::info!("✓ Initialized 2 class prototypes");

        Ok(())
    }
}
