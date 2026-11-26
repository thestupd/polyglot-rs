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

/// Byte range in the output where one format interpretation lives. When a
/// strategy declares a region, that format is validated over the region only.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub format: String,
    pub offset: usize,
    pub length: usize,
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
    /// Empty when every interpretation spans the complete output.
    pub regions: Vec<Region>,
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
                regions: vec![],
                warnings: vec![],
            },
            data,
        }
    }

    /// Declare where each interpretation lives in the candidate output.
    pub fn with_regions(mut self, regions: Vec<Region>) -> Self {
        self.report.regions = regions;
        self
    }
}

fn region_bytes<'a>(data: &'a [u8], regions: &[Region], format: &str) -> Result<&'a [u8]> {
    match regions.iter().find(|r| r.format == format) {
        None => Ok(data),
        Some(r) => r
            .offset
            .checked_add(r.length)
            .and_then(|end| data.get(r.offset..end))
            .ok_or_else(|| Error::InvalidComposition(format!("{format} region is out of bounds"))),
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
        // Validation reports are cached across candidate strategies. A strategy's
        // carried payload inputs are stored unchanged and never validated, so an
        // input validated for one candidate may be opaque payload for another.
        let mut input_validation: Vec<Option<ValidationResult>> = vec![None; inputs.len()];
        let distinct_formats = inputs
            .iter()
            .map(|i| i.format.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let mut last_error = None;
        'candidates: for (index, compatibility) in candidates {
            let strategy = &self.strategies[index];
            let carried = strategy.carried_inputs(inputs);
            // Validate every input the strategy does not carry as opaque payload.
            if options.validate {
                for (position, input) in inputs.iter().enumerate() {
                    if carried.contains(&position) {
                        continue;
                    }
                    if input_validation[position].is_none() {
                        let report = self
                            .registry
                            .validate(
                                &input.data,
                                std::slice::from_ref(&input.format),
                                options.limits,
                            )?
                            .into_iter()
                            .next()
                            .ok_or_else(|| {
                                Error::InvalidComposition("Missing input validation report".into())
                            })?;
                        input_validation[position] = Some(report);
                    }
                    if let Some(report) = &input_validation[position] {
                        if !report.valid {
                            last_error = Some(Error::InvalidComposition(format!(
                                "Input {}: {}",
                                input.format,
                                report.errors.join("; ")
                            )));
                            continue 'candidates;
                        }
                    }
                }
            }
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
            // Bounds are enforced even when validation is disabled.
            let regions = &candidate.report.regions;
            for region in regions {
                region_bytes(&candidate.data, regions, &region.format)?;
            }
            // Output validation covers every non-carried distinct format, each
            // checked within its declared region. Carried payloads are skipped.
            let carried_formats: BTreeSet<&str> = carried
                .iter()
                .filter_map(|&position| inputs.get(position))
                .map(|i| i.format.as_str())
                .collect();
            let output_formats: Vec<String> = inputs
                .iter()
                .enumerate()
                .filter(|(position, _)| !carried.contains(position))
                .map(|(_, i)| i.format.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let validation = if options.validate {
                // The output has its own size budget; input limits do not reject
                // valid strategy overhead after it has already been bounded.
                let output_limits = Limits {
                    max_input_bytes: options.max_output_bytes,
                    ..options.limits
                };
                let mut reports = Vec::with_capacity(output_formats.len());
                for format in &output_formats {
                    let bytes = region_bytes(&candidate.data, &candidate.report.regions, format)?;
                    reports.extend(self.registry.validate(
                        bytes,
                        std::slice::from_ref(format),
                        output_limits,
                    )?);
                }
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
            let input_reports: Vec<InputReport> = inputs
                .iter()
                .enumerate()
                .map(|(position, input)| InputReport {
                    format: input.format.clone(),
                    label: input.label.clone(),
                    size_bytes: input.data.len(),
                    validation: if carried.contains(&position) {
                        None
                    } else {
                        input_validation[position].clone()
                    },
                })
                .collect();
            let regions = std::mem::take(&mut candidate.report.regions);
            let region_warning = !regions.is_empty();
            candidate.report = CompositionReport {
                strategy: strategy.name().into(),
                compatibility,
                inputs: input_reports,
                validation,
                validated: options.validate,
                size_bytes: candidate.data.len(),
                regions,
                warnings: vec![
                    "Native validation is not independent parser or decoded-content verification"
                        .into(),
                ],
            };
            if region_warning {
                candidate
                    .report
                    .warnings
                    .push("Each interpretation was validated within its declared region".into());
            }
            if !carried_formats.is_empty() {
                let mut names: Vec<&str> = carried_formats.into_iter().collect();
                names.sort_unstable();
                candidate.report.warnings.push(format!(
                    "Carried payload stored unchanged and not validated: {}",
                    names.join(", ")
                ));
            }
            if !options.validate {
                candidate.report.warnings.push(
                    "Validation was disabled; this is an unvalidated binary candidate".into(),
                );
            }
            if distinct_formats < 2 {
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
