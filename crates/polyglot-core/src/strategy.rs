use crate::{CompatibilityResult, CompositionResult, PolyglotInput, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyRequirements {
    pub description: String,
    pub min_inputs: usize,
    pub requires_native_handlers: bool,
    pub limitations: Vec<String>,
}

/// Custom algorithms belong in Rust. A supported status is an assertion by the
/// strategy author that its documented parser/content tests have passed.
pub trait CompositionStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            description: self.name().into(),
            min_inputs: 2,
            requires_native_handlers: false,
            limitations: vec![],
        }
    }
    fn check_compatibility(&self, inputs: &[PolyglotInput]) -> Result<CompatibilityResult>;
    /// Formats alone cannot be evaluated as if they were empty binary inputs.
    /// Override this for documented format/profile-specific guarantees.
    fn check_formats(&self, _: &[String]) -> Result<CompatibilityResult> {
        Ok(CompatibilityResult::conditional(
            self.name(),
            "Input bytes are required to evaluate this strategy",
            self.requirements().limitations,
        ))
    }
    fn compose(&self, inputs: &[PolyglotInput]) -> Result<CompositionResult>;
}
