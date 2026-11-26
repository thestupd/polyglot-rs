use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityStatus {
    Unsupported,
    Conditional,
    Supported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityResult {
    pub status: CompatibilityStatus,
    pub strategies: Vec<String>,
    pub limitations: Vec<String>,
    pub explanation: Option<String>,
}

impl CompatibilityResult {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            status: CompatibilityStatus::Unsupported,
            strategies: vec![],
            limitations: vec![],
            explanation: Some(reason.into()),
        }
    }

    /// Only for strategies whose documented parser/decoding tests pass.
    pub fn supported(
        strategy: &str,
        explanation: impl Into<String>,
        limitations: Vec<String>,
    ) -> Self {
        Self {
            status: CompatibilityStatus::Supported,
            strategies: vec![strategy.into()],
            limitations,
            explanation: Some(explanation.into()),
        }
    }

    pub fn conditional(
        strategy: &str,
        explanation: impl Into<String>,
        limitations: Vec<String>,
    ) -> Self {
        Self {
            status: CompatibilityStatus::Conditional,
            strategies: vec![strategy.into()],
            limitations,
            explanation: Some(explanation.into()),
        }
    }
}
