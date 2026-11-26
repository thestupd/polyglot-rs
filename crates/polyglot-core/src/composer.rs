use crate::{
    CompatibilityResult, CompatibilityStatus, CompositionStrategy, Error, Limits, PolyglotInput,
    Registry, Result, StrategyRequirements, ValidationResult,
};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct CompositionOptions {
    pub strategy: String,
    pub validate: bool,
    pub limits: Limits,
    pub max_output_bytes: usize,
}

impl Default for CompositionOptions {
    fn default() -> Self {
        Self {
            strategy: "auto".into(),
            validate: true,
            limits: Limits::default(),
            max_output_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputReport {
    pub format: String,
    pub label: Option<String>,
    pub size_bytes: usize,
    pub validation: Option<ValidationResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionReport {
    pub strategy: String,
    pub compatibility: CompatibilityResult,
    pub inputs: Vec<InputReport>,
    pub validation: Vec<ValidationResult>,
    pub validated: bool,
    pub size_bytes: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CompositionResult {
    pub data: Vec<u8>,
    pub report: CompositionReport,
}

impl CompositionResult {
    /// A candidate remains unvalidated until Composer independently checks it.
    pub fn candidate(data: Vec<u8>, strategy: &str, compatibility: CompatibilityResult) -> Self {
        Self {
            report: CompositionReport {
                strategy: strategy.into(),
                compatibility,
                inputs: vec![],
                validation: vec![],
                validated: false,
                size_bytes: data.len(),
                warnings: vec![],
            },
            data,
        }
    }
}

pub struct Composer {
    registry: Registry,
    strategies: Vec<Box<dyn CompositionStrategy>>,
}

fn check_formats<'a>(formats: impl Iterator<Item = &'a str>) -> Result<()> {
    for name in formats {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(Error::InvalidInput(
                "Format names must contain lowercase ASCII letters, digits or hyphens".into(),
            ));
        }
    }
    Ok(())
}

impl Composer {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            strategies: vec![],
        }
    }

    pub fn register_strategy(
        &mut self,
        strategy: impl CompositionStrategy + 'static,
    ) -> Result<()> {
        let name = strategy.name();
        check_formats(std::iter::once(name))?;
        if name == "auto" {
            return Err(Error::InvalidInput(
                "auto is reserved for strategy selection".into(),
            ));
        }
        if self.strategies.iter().any(|s| s.name() == name) {
            return Err(Error::DuplicateStrategy(name.into()));
        }
        self.strategies.push(Box::new(strategy));
        Ok(())
    }

    pub fn strategies(&self) -> Vec<(String, StrategyRequirements)> {
        self.strategies
            .iter()
            .map(|s| (s.name().into(), s.requirements()))
            .collect()
    }

    fn candidates(
        &self,
        formats: &[String],
        inputs: Option<&[PolyglotInput]>,
        strategy: &str,
    ) -> Result<Vec<(usize, CompatibilityResult)>> {
        if !(2..=256).contains(&formats.len()) {
            return Err(Error::InvalidInput(
                "Composition requires between 2 and 256 inputs".into(),
            ));
        }
        check_formats(formats.iter().map(String::as_str))?;
        if strategy != "auto" && !self.strategies.iter().any(|s| s.name() == strategy) {
            return Err(Error::UnknownStrategy(strategy.into()));
        }
        self.strategies
            .iter()
            .enumerate()
            .filter(|(_, s)| strategy == "auto" || s.name() == strategy)
            .map(|(index, s)| {
                let requirements = s.requirements();
                if formats.len() < requirements.min_inputs
                    || (requirements.requires_native_handlers
                        && formats
                            .iter()
                            .any(|format| self.registry.get(format).is_none()))
                {
                    return Ok((
                        index,
                        CompatibilityResult::unsupported(
                            "Strategy input-count or native-handler requirements are not met",
                        ),
                    ));
                }
                let result = match inputs {
                    Some(inputs) => s.check_compatibility(inputs)?,
                    None => s.check_formats(formats)?,
                };
                Ok((index, result))
            })
            .collect()
    }

    /// Format-only query: it cannot prove that particular bytes will compose.
    pub fn check_compatibility(
        &self,
        formats: &[String],
        strategy: &str,
    ) -> Result<CompatibilityResult> {
        let candidates = self.candidates(formats, None, strategy)?;
        let status = candidates
            .iter()
            .map(|(_, c)| c.status)
            .max()
            .unwrap_or(CompatibilityStatus::Unsupported);
        let mut result = CompatibilityResult { status, strategies: vec![], limitations: vec![], explanation: Some("Format-only analysis; input bytes and target parser behavior still require validation".into()) };
        for (index, candidate) in candidates {
            if candidate.status == CompatibilityStatus::Unsupported {
                if status == CompatibilityStatus::Unsupported {
                    result.limitations.push(format!(
                        "{}: {}",
                        self.strategies[index].name(),
                        candidate.explanation.unwrap_or_default()
                    ));
                }
            } else {
                result.strategies.push(self.strategies[index].name().into());
                result.limitations.extend(candidate.limitations);
            }
        }
        Ok(result)
    }

    pub fn compose(
        &self,
        inputs: &[PolyglotInput],
        options: &CompositionOptions,
    ) -> Result<CompositionResult> {
        options.limits.check_input(0)?;
        if options.max_output_bytes == 0 {
            return Err(Error::InvalidInput(
                "maxOutputBytes must be positive".into(),
            ));
        }
        let total = inputs.iter().try_fold(0usize, |total, input| {
            total
                .checked_add(input.data.len())
                .ok_or_else(|| Error::LimitExceeded("Total input size overflow".into()))
        })?;
        options.limits.check_input(total)?;
        let ordered_formats: Vec<_> = inputs.iter().map(|i| i.format.clone()).collect();
        let mut candidates = self.candidates(&ordered_formats, Some(inputs), &options.strategy)?;
        candidates.retain(|(_, c)| c.status != CompatibilityStatus::Unsupported);
        // Stable ordering: supported before conditional, then registration order.
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.1.status));
        if candidates.is_empty() {
            return Err(Error::UnsupportedCombination(
                "No registered strategy can compose these inputs under the current parser profiles"
                    .into(),
            ));
        }
        let mut input_reports = vec![];
        for input in inputs {
            let validation = if options.validate {
                let reports = self.registry.validate(
                    &input.data,
                    std::slice::from_ref(&input.format),
                    options.limits,
                )?;
                let report = reports.into_iter().next().ok_or_else(|| {
                    Error::InvalidComposition("Missing input validation report".into())
                })?;
                if !report.valid {
                    return Err(Error::InvalidComposition(format!(
                        "Input {}: {}",
                        input.format,
                        report.errors.join("; ")
                    )));
                }
                Some(report)
            } else {
                None
            };
            input_reports.push(InputReport {
                format: input.format.clone(),
                label: input.label.clone(),
                size_bytes: input.data.len(),
                validation,
            });
        }
        let formats: Vec<_> = inputs
            .iter()
            .map(|i| i.format.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut last_error = None;
        for (index, compatibility) in candidates {
            let strategy = &self.strategies[index];
            let mut candidate = match strategy.compose(inputs) {
                Ok(candidate) => candidate,
                Err(error @ (Error::InvalidComposition(_) | Error::UnsupportedCombination(_))) => {
                    last_error = Some(error);
                    continue;
                }
                Err(error) => return Err(error),
            };
            if candidate.data.len() > options.max_output_bytes {
                return Err(Error::LimitExceeded("Output exceeds maxOutputBytes".into()));
            }
            let validation = if options.validate {
                // The output has its own size budget; input limits do not reject
                // valid strategy overhead after it has already been bounded.
                let output_limits = Limits {
                    max_input_bytes: options.max_output_bytes,
                    ..options.limits
                };
                let reports = self
                    .registry
                    .validate(&candidate.data, &formats, output_limits)?;
                if reports.iter().any(|r| !r.valid) {
                    last_error = Some(Error::InvalidComposition(
                        reports
                            .iter()
                            .filter(|r| !r.valid)
                            .map(|r| format!("{}: {}", r.format, r.errors.join("; ")))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ));
                    continue;
                }
                reports
            } else {
                vec![]
            };
            candidate.report = CompositionReport {
                strategy: strategy.name().into(),
                compatibility,
                inputs: input_reports,
                validation,
                validated: options.validate,
                size_bytes: candidate.data.len(),
                warnings: vec![
                    "Native validation is not independent parser or decoded-content verification"
                        .into(),
                ],
            };
            if !options.validate {
                candidate.report.warnings.push(
                    "Validation was disabled; this is an unvalidated binary candidate".into(),
                );
            }
            if formats.len() < 2 {
                candidate
                    .report
                    .warnings
                    .push("Inputs do not request multiple distinct format interpretations".into());
            }
            return Ok(candidate);
        }
        Err(last_error.unwrap_or_else(|| {
            Error::UnsupportedCombination("No strategy produced a valid candidate".into())
        }))
    }
}
