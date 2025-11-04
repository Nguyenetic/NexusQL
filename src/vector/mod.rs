//! Vector Search Module
//!
//! Implements vector similarity search with multiple index types:
//! - HNSW (Hierarchical Navigable Small World) for approximate search
//! - Flat (Brute-force) for exact search
//!
//! Based on research from:
//! - pgvector (PostgreSQL)
//! - USearch (high-performance HNSW)
//! - Qdrant (production vector database)

pub mod flat;
pub mod hnsw;
pub mod index;
mod test;

pub use flat::FlatIndex;
pub use hnsw::{HNSWConfig, HNSWIndex};
pub use index::{VectorIndex, VectorSearchResult};
