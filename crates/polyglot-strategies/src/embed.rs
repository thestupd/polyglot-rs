use polyglot_core::{
    CompatibilityResult, CompositionResult, CompositionStrategy, Error, PolyglotInput, Result,
    StrategyRequirements,
};

/// Reserved interface for native embedding algorithms; no built-in algorithm yet.
pub struct Embed;

impl CompositionStrategy for Embed {
    fn name(&self) -> &'static str {
        "embed"
    }
    fn requirements(&self) -> StrategyRequirements {
        StrategyRequirements { description: "Embed a payload using a native format capability".into(), min_inputs: 2, requires_native_handlers: true,
            limitations: vec!["Requires a format-specific embedding implementation and independently verified parser profile".into()] }
    }
    fn check_compatibility(&self, _: &[PolyglotInput]) -> Result<CompatibilityResult> {
        Ok(CompatibilityResult::unsupported(
            "No built-in embedding algorithm is enabled",
        ))
    }
    fn compose(&self, _: &[PolyglotInput]) -> Result<CompositionResult> {
        Err(Error::UnsupportedCombination(
            "Embedding is not implemented".into(),
        ))
    }
    fn check_formats(&self, _: &[String]) -> Result<CompatibilityResult> {
        self.check_compatibility(&[])
    }
}
