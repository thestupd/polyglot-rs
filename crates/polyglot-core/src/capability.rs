use crate::{Limits, Result};

/// Necessary layout constraints, never sufficient proof of parser compatibility.
/// Permitting a prefix/suffix still requires validating the complete candidate.
pub trait AppendCapability: Send + Sync {
    fn permits_prefix(&self) -> bool;
    fn permits_suffix(&self) -> bool;
    fn alignment(&self) -> usize {
        1
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingLocation {
    pub name: String,
    pub max_payload_bytes: usize,
    pub limitations: Vec<String>,
}

/// Optional native extension; absence means that embedding is not available.
/// Locations are descriptive identifiers, not unchecked offsets into input data.
pub trait EmbeddingCapability: Send + Sync {
    fn locations(&self, host: &[u8], limits: Limits) -> Result<Vec<EmbeddingLocation>>;
    fn embed(
        &self,
        host: &[u8],
        payload: &[u8],
        location: &str,
        max_output_bytes: usize,
    ) -> Result<Vec<u8>>;
}
