//! Built-in format handlers. These do not depend on Node.js.
mod gif;
mod jpeg;
mod mpeg_ts;
mod png;

pub use gif::Gif;
pub use jpeg::Jpeg;
pub use mpeg_ts::MpegTs;
pub use png::Png;
use polyglot_core::{Error, FormatInfo, Registry, Result, ValidationResult};

pub fn builtin_registry() -> Result<Registry> {
    let mut registry = Registry::new();
    registry.register(Png)?;
    registry.register(Gif)?;
    registry.register(Jpeg)?;
    registry.register(MpegTs)?;
    Ok(registry)
}

fn malformed(message: impl Into<String>) -> Error {
    Error::Malformed(message.into())
}

fn report(format: &str, parsed: Result<FormatInfo>) -> Result<FormatInfo> {
    match parsed {
        Ok(info) => Ok(info),
        Err(Error::Malformed(message)) => {
            let mut info = FormatInfo::new(format);
            info.validation = ValidationResult::invalid(format, message);
            Ok(info)
        }
        Err(error) => Err(error),
    }
}

// Shared trait boilerplate keeps normal and limit-aware inspection consistent.
macro_rules! handler {
    ($handler:ty, $name:literal, $detect:expr, $parse:ident) => {
        impl polyglot_core::FormatHandler for $handler {
            fn name(&self) -> &'static str {
                $name
            }
            fn append_capability(&self) -> Option<&dyn polyglot_core::AppendCapability> {
                Some(self)
            }
            fn detect(&self, data: &[u8]) -> bool {
                ($detect)(data)
            }
            fn validate(&self, data: &[u8]) -> Result<ValidationResult> {
                self.validate_with_limits(data, Limits::default())
            }
            fn inspect(&self, data: &[u8]) -> Result<FormatInfo> {
                self.inspect_with_limits(data, Limits::default())
            }
            fn validate_with_limits(
                &self,
                data: &[u8],
                limits: Limits,
            ) -> Result<ValidationResult> {
                Ok(self.inspect_with_limits(data, limits)?.validation)
            }
            fn inspect_with_limits(&self, data: &[u8], limits: Limits) -> Result<FormatInfo> {
                limits.check_input(data.len())?;
                crate::report($name, $parse(data, limits))
            }
        }
    };
}
pub(crate) use handler;
