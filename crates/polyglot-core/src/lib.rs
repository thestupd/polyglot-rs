//! Runtime-independent inspection contracts. No Node.js or format dependencies.
mod buffer;
mod capability;
mod compatibility;
mod composer;
mod error;
mod input;
mod registry;
mod strategy;

pub use buffer::{Cursor, Limits};
pub use capability::{AppendCapability, EmbeddingCapability, EmbeddingLocation};
pub use compatibility::{CompatibilityResult, CompatibilityStatus};
pub use composer::{
    Composer, CompositionOptions, CompositionReport, CompositionResult, InputReport, Region,
};
pub use error::{Error, Result};
pub use input::PolyglotInput;
pub use registry::Registry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use strategy::{CompositionStrategy, StrategyRequirements};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub format: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn valid(format: impl Into<String>) -> Self {
        Self {
            valid: true,
            format: format.into(),
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn invalid(format: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            valid: false,
            format: format.into(),
            errors: vec![error.into()],
            warnings: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub format: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub validation: ValidationResult,
}

impl FormatInfo {
    pub fn new(format: &str) -> Self {
        Self {
            format: format.into(),
            metadata: BTreeMap::new(),
            validation: ValidationResult::valid(format),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectionResult {
    pub formats: Vec<String>,
    pub metadata: BTreeMap<String, FormatInfo>,
    pub size_bytes: usize,
}

/// Signature detection is a candidate match, never proof of validity.
/// Override the limit-aware methods when a handler allocates or decodes data.
pub trait FormatHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self, data: &[u8]) -> bool;
    fn validate(&self, data: &[u8]) -> Result<ValidationResult>;
    fn inspect(&self, data: &[u8]) -> Result<FormatInfo>;

    fn append_capability(&self) -> Option<&dyn AppendCapability> {
        None
    }

    fn embedding_capability(&self) -> Option<&dyn EmbeddingCapability> {
        None
    }

    fn validate_with_limits(&self, data: &[u8], limits: Limits) -> Result<ValidationResult> {
        limits.check_input(data.len())?;
        self.validate(data)
    }

    fn inspect_with_limits(&self, data: &[u8], limits: Limits) -> Result<FormatInfo> {
        limits.check_input(data.len())?;
        self.inspect(data)
    }
}
