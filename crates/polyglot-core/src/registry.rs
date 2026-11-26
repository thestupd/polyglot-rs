use crate::{Error, FormatHandler, InspectionResult, Limits, Result, ValidationResult};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct Registry {
    handlers: BTreeMap<String, Arc<dyn FormatHandler>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, handler: impl FormatHandler + 'static) -> Result<()> {
        let name = handler.name();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(Error::InvalidInput(
                "Format names must contain lowercase ASCII letters, digits or hyphens".into(),
            ));
        }
        if self.handlers.contains_key(name) {
            return Err(Error::DuplicateFormat(name.into()));
        }
        self.handlers.insert(name.into(), Arc::new(handler));
        Ok(())
    }

    pub fn names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<&dyn FormatHandler> {
        self.handlers.get(name).map(Arc::as_ref)
    }

    pub fn inspect(&self, data: &[u8], limits: Limits) -> Result<InspectionResult> {
        limits.check_input(data.len())?;
        let mut metadata = BTreeMap::new();
        for (name, handler) in &self.handlers {
            if handler.detect(data) {
                metadata.insert(name.clone(), handler.inspect_with_limits(data, limits)?);
            }
        }
        Ok(InspectionResult {
            formats: metadata.keys().cloned().collect(),
            metadata,
            size_bytes: data.len(),
        })
    }

    /// Unknown formats fail explicitly; known interpretations are checked independently.
    pub fn validate(
        &self,
        data: &[u8],
        formats: &[String],
        limits: Limits,
    ) -> Result<Vec<ValidationResult>> {
        limits.check_input(data.len())?;
        if formats.is_empty() {
            return Err(Error::InvalidInput(
                "At least one format is required".into(),
            ));
        }
        formats
            .iter()
            .map(|name| match self.handlers.get(name) {
                Some(handler) => handler.validate_with_limits(data, limits),
                None => Ok(ValidationResult::invalid(
                    name,
                    "No registered native validator for this format",
                )),
            })
            .collect()
    }
}
