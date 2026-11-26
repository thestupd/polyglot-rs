use polyglot_core::{
    CompatibilityResult, CompatibilityStatus, CompositionResult, CompositionStrategy, Error,
    PolyglotInput, Registry, Result, StrategyRequirements,
};

pub struct Append {
    registry: Registry,
    max_output_bytes: usize,
}

impl Append {
    pub fn new(registry: Registry, max_output_bytes: usize) -> Self {
        Self {
            registry,
            max_output_bytes,
        }
    }
}

impl CompositionStrategy for Append {
    fn name(&self) -> &'static str {
        "append"
    }

    fn requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            description: "Append streams in input order, subject to native layout constraints"
                .into(),
            min_inputs: 2,
            requires_native_handlers: false,
            limitations: vec![
                "Concatenation alone is not verified polyglot support".into(),
                "Known handlers may prohibit prefixes/suffixes or require alignment".into(),
            ],
        }
    }

    fn check_compatibility(&self, inputs: &[PolyglotInput]) -> Result<CompatibilityResult> {
        if !(2..=256).contains(&inputs.len()) {
            return Err(Error::InvalidInput(
                "Append requires between 2 and 256 inputs".into(),
            ));
        }
        let total = inputs.iter().try_fold(0usize, |size, i| {
            size.checked_add(i.data.len())
                .ok_or_else(|| Error::LimitExceeded("Append size overflow".into()))
        })?;
        let mut prefix = 0usize;
        let mut limitations = self.requirements().limitations;
        for (index, input) in inputs.iter().enumerate() {
            let suffix = total - prefix - input.data.len();
            match self.registry.get(&input.format) {
                Some(handler) => match handler.append_capability() {
                    Some(capability) => {
                        if (index > 0 && !capability.permits_prefix())
                            || (index + 1 < inputs.len() && !capability.permits_suffix())
                        {
                            return Ok(CompatibilityResult::unsupported(format!("{} requires its complete stream at byte zero with no extra bytes under the current native parser profile", input.format)));
                        }
                        let alignment = capability.alignment();
                        if alignment == 0 {
                            return Err(Error::InvalidInput(
                                "Append alignment must be positive".into(),
                            ));
                        }
                        if !prefix.is_multiple_of(alignment)
                            || !suffix.is_multiple_of(alignment)
                            || !input.data.len().is_multiple_of(alignment)
                        {
                            return Ok(CompatibilityResult::unsupported(format!(
                                "{} requires {alignment}-byte aligned append regions",
                                input.format
                            )));
                        }
                    }
                    None => limitations.push(format!(
                        "{} has no declared append capability; whole-output validation is required",
                        input.format
                    )),
                },
                None => limitations.push(format!(
                    "{} is opaque and has no native validator; creation requires validate: false",
                    input.format
                )),
            }
            prefix += input.data.len();
        }
        Ok(CompatibilityResult::conditional("append", "A binary candidate can be assembled; parser compatibility and preservation of decoded content are not established", limitations))
    }

    fn check_formats(&self, formats: &[String]) -> Result<CompatibilityResult> {
        // Only positions/capabilities can be assessed without bytes. Alignment
        // remains a condition on actual input and neighboring stream lengths.
        let inputs: Vec<_> = formats
            .iter()
            .map(|format| PolyglotInput {
                format: format.clone(),
                data: vec![],
                label: None,
            })
            .collect();
        self.check_compatibility(&inputs)
    }

    fn compose(&self, inputs: &[PolyglotInput]) -> Result<CompositionResult> {
        let compatibility = self.check_compatibility(inputs)?;
        if compatibility.status == CompatibilityStatus::Unsupported {
            return Err(Error::UnsupportedCombination(
                compatibility.explanation.unwrap_or_default(),
            ));
        }
        let size = inputs.iter().try_fold(0usize, |size, i| {
            size.checked_add(i.data.len())
                .ok_or_else(|| Error::LimitExceeded("Append size overflow".into()))
        })?;
        if size > self.max_output_bytes {
            return Err(Error::LimitExceeded("Output exceeds maxOutputBytes".into()));
        }
        let mut data = Vec::new();
        data.try_reserve_exact(size)
            .map_err(|_| Error::LimitExceeded("Cannot allocate append output".into()))?;
        for input in inputs {
            data.extend_from_slice(&input.data);
        }
        Ok(CompositionResult::candidate(
            data,
            self.name(),
            compatibility,
        ))
    }
}
