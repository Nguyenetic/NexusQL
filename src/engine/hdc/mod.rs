//! HDC Engine — Hyperdimensional Computing
//!
//! Vector Symbolic Architecture primitives. Public surface:
//!
//! - [`Hypervector`] — multi-variant enum. Today it carries a single
//!   `Bsc(BscVec)` variant; MAP / HRR / FHRR / SCH land in later PRs.
//!   SQL boundaries go through this type so variant is determined at
//!   runtime.
//! - [`BscVec`] — Binary Spatter Codes concrete type. Internal compute
//!   uses the concrete type directly for static dispatch.
//! - [`HvOps`] — trait every variant implements.
//! - [`HdcError`] — shared error type.

pub mod error;
pub mod hypervector; // backcompat shim — re-exports `Hypervector` from here
pub mod variants;

pub use error::HdcError;
pub use variants::bsc::BscVec;
pub use variants::HvOps;

use crate::engine::{EngineCapabilities, Query, QueryCost, StorageEngine};
use crate::types::{ResultSet, Row, Value};
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tagged union over every VSA variant. The SQL layer stores this
/// directly; inside a given variant's math we pattern-match once and
/// then call the concrete-type methods so the hot path is monomorphic.
///
/// Byte-level serialization reserves the first byte for a variant tag:
/// `0=BSC, 1=MAP, 2=HRR, 3=FHRR, 4=SCH`. Tags never change — add new
/// variants with new tag values.
#[derive(Debug, Clone)]
pub enum Hypervector {
    Bsc(BscVec),
}

// Variant tags written as the first byte of the on-disk format. Kept
// here (not in the individual variant modules) so the numeric space is
// canonical and visible at a glance.
const TAG_BSC: u8 = 0;

impl Hypervector {
    /// Human-readable variant name for error messages and SQL
    /// `hv_variant()` responses.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Bsc(_) => "BSC",
        }
    }

    /// Conceptual dimensionality — independent of the variant's storage
    /// layout (BSC packs 64 bits per u64).
    pub fn dim(&self) -> usize {
        match self {
            Self::Bsc(v) => v.dim(),
        }
    }

    /// Convenience: BSC variant from a `u64` seed. Callers that want
    /// non-BSC variants use the matching `BscVec::random_seeded` /
    /// `MapVec::random_seeded` / ... directly and wrap in this enum.
    pub fn bsc_random_seeded(dim: usize, seed: u64) -> Self {
        Self::Bsc(BscVec::random_seeded(dim, seed))
    }

    /// Binding: `A ⊗ B`. Returns `VariantMismatch` if the two vectors
    /// come from different variants — every VSA has its own binding
    /// operator so cross-variant binding is ill-defined.
    pub fn bind(&self, other: &Self) -> std::result::Result<Self, HdcError> {
        match (self, other) {
            (Self::Bsc(a), Self::Bsc(b)) => a.bind(b).map(Self::Bsc),
        }
    }

    /// Bundling across a slice of same-variant vectors. Returns
    /// `EmptyBundle` if the slice is empty, `VariantMismatch` if mixed.
    pub fn bundle(vectors: &[&Self]) -> std::result::Result<Self, HdcError> {
        if vectors.is_empty() {
            return Err(HdcError::EmptyBundle);
        }
        // Enforce: all elements are the same variant. Cheap check
        // because it's just tag comparison.
        let first_name = vectors[0].variant_name();
        if let Some(bad) = vectors.iter().find(|v| v.variant_name() != first_name) {
            return Err(HdcError::VariantMismatch {
                left: first_name,
                right: bad.variant_name(),
            });
        }
        match vectors[0] {
            Self::Bsc(_) => {
                let inners: Vec<&BscVec> = vectors
                    .iter()
                    .map(|v| match v {
                        Self::Bsc(b) => b,
                    })
                    .collect();
                BscVec::bundle(&inners).map(Self::Bsc)
            }
        }
    }

    /// Permutation: `ρ(A)`. Pure within-variant op, no mismatch case.
    pub fn permute(&self, shift: isize) -> Self {
        match self {
            Self::Bsc(v) => Self::Bsc(v.permute(shift)),
        }
    }

    /// Similarity. Cross-variant similarity returns `VariantMismatch`.
    pub fn similarity(&self, other: &Self) -> std::result::Result<f32, HdcError> {
        match (self, other) {
            (Self::Bsc(a), Self::Bsc(b)) => a.similarity(b),
        }
    }

    /// Serialize to bytes with a leading variant tag.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Bsc(v) => {
                let body = v.to_bytes();
                let mut out = Vec::with_capacity(1 + body.len());
                out.push(TAG_BSC);
                out.extend_from_slice(&body);
                out
            }
        }
    }

    /// Deserialize from bytes. Reads the variant tag and dispatches.
    pub fn from_bytes(bytes: &[u8]) -> std::result::Result<Self, HdcError> {
        if bytes.is_empty() {
            return Err(HdcError::Serialization(
                "Hypervector: missing variant tag byte".into(),
            ));
        }
        match bytes[0] {
            TAG_BSC => Ok(Self::Bsc(BscVec::from_bytes(&bytes[1..])?)),
            other => Err(HdcError::Serialization(format!(
                "unknown variant tag byte: {other}"
            ))),
        }
    }
}

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
            // Mixed variants in the prototype set are a programming
            // error at this layer; the .expect reflects that.
            let similarity = input
                .similarity(prototype)
                .expect("classifier prototypes must share variant + dimension");
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
        let test_input = Hypervector::bsc_random_seeded(self.dimension, 999);

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
        let class_a = Hypervector::bsc_random_seeded(self.dimension, 100);
        let class_b = Hypervector::bsc_random_seeded(self.dimension, 200);

        self.add_class("category_a".to_string(), class_a).await;
        self.add_class("category_b".to_string(), class_b).await;

        tracing::info!("✓ Initialized 2 class prototypes");

        Ok(())
    }
}

#[cfg(test)]
mod enum_tests {
    use super::*;

    #[test]
    fn variant_name_bsc() {
        let hv = Hypervector::bsc_random_seeded(1000, 1);
        assert_eq!(hv.variant_name(), "BSC");
        assert_eq!(hv.dim(), 1000);
    }

    #[test]
    fn bind_same_variant_ok() {
        let a = Hypervector::bsc_random_seeded(10_000, 11);
        let b = Hypervector::bsc_random_seeded(10_000, 22);
        let c = a.bind(&b).unwrap();
        assert_eq!(c.variant_name(), "BSC");
        // XOR is self-inverse, so (A⊗B)⊗B == A
        let recovered = c.bind(&b).unwrap();
        assert_eq!(
            recovered.similarity(&a).unwrap(),
            1.0,
            "self-inverse binding failed"
        );
    }

    #[test]
    fn empty_bundle_errors() {
        assert!(matches!(
            Hypervector::bundle(&[]),
            Err(HdcError::EmptyBundle)
        ));
    }

    #[test]
    fn roundtrip_bytes_preserves_variant_tag() {
        let a = Hypervector::bsc_random_seeded(10_000, 42);
        let bytes = a.to_bytes();
        assert_eq!(bytes[0], TAG_BSC);
        let b = Hypervector::from_bytes(&bytes).unwrap();
        assert_eq!(a.variant_name(), b.variant_name());
        assert_eq!(a.similarity(&b).unwrap(), 1.0);
    }

    #[test]
    fn from_bytes_rejects_empty_input() {
        assert!(matches!(
            Hypervector::from_bytes(&[]),
            Err(HdcError::Serialization(_))
        ));
    }

    #[test]
    fn from_bytes_rejects_unknown_tag() {
        assert!(matches!(
            Hypervector::from_bytes(&[99, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(HdcError::Serialization(_))
        ));
    }
}
