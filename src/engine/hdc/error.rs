//! HDC error types shared across all VSA variants.

#[derive(Debug, thiserror::Error)]
pub enum HdcError {
    #[error("Variant mismatch: {left} vs {right}")]
    VariantMismatch {
        left: &'static str,
        right: &'static str,
    },

    #[error("Dimension mismatch: {left} vs {right}")]
    DimensionMismatch { left: usize, right: usize },

    #[error("Invalid dimension: {0}. Must be >= 64 for quasi-orthogonality to hold.")]
    InvalidDimension(usize),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Conversion {from} -> {to} not supported")]
    UnsupportedConversion {
        from: &'static str,
        to: &'static str,
    },

    #[error("Empty bundle (at least one vector required)")]
    EmptyBundle,
}
