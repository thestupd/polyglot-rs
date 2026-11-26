use polyglot_core::{
    CompatibilityResult, CompositionResult, CompositionStrategy, Error, PolyglotInput, Result,
    StrategyRequirements,
};

/// Overlap algorithms implement CompositionStrategy with their own constraints.
pub struct Overlap;

impl CompositionStrategy for Overlap {
    fn name(&self) -> &'static str {
        "overlap"
    }
    fn requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            description: "Construct compatible overlapping binary structures".into(),
            min_inputs: 2,
            requires_native_handlers: true,
            limitations: vec![
                "Requires an independently verified, format-specific constraint solver".into(),
            ],
        }
    }
    fn check_compatibility(&self, _: &[PolyglotInput]) -> Result<CompatibilityResult> {
        Ok(CompatibilityResult::unsupported(
            "No verified overlap algorithm is enabled",
        ))
    }
    fn compose(&self, _: &[PolyglotInput]) -> Result<CompositionResult> {
        Err(Error::UnsupportedCombination(
            "Overlap is not implemented".into(),
        ))
    }
    fn check_formats(&self, _: &[String]) -> Result<CompatibilityResult> {
        self.check_compatibility(&[])
    }
}
