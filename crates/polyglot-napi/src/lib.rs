//! Private transport layer. JSON carries small reports, never binary payloads.
use napi::{bindgen_prelude::*, Env, Task};
use napi_derive::napi;
use polyglot_core::Limits;

fn native_error(error: impl std::fmt::Display) -> napi::Error {
    napi::Error::new(Status::GenericFailure, error.to_string())
}

pub struct Analyze {
    data: Vec<u8>,
    formats: Option<Vec<String>>,
    limits: Limits,
}

#[napi]
impl Task for Analyze {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<String> {
        let registry = polyglot_formats::builtin_registry().map_err(native_error)?;
        let result = if let Some(formats) = &self.formats {
            serde_json::to_string(
                &registry
                    .validate(&self.data, formats, self.limits)
                    .map_err(native_error)?,
            )
        } else {
            serde_json::to_string(
                &registry
                    .inspect(&self.data, self.limits)
                    .map_err(native_error)?,
            )
        };
        result.map_err(native_error)
    }

    fn resolve(&mut self, _: Env, output: String) -> napi::Result<String> {
        Ok(output)
    }
}

fn task(
    data: Buffer,
    formats: Option<Vec<String>>,
    max_input_bytes: u32,
    max_decoded_bytes: u32,
) -> napi::Result<AsyncTask<Analyze>> {
    let limits = Limits {
        max_input_bytes: max_input_bytes as usize,
        max_decoded_bytes: max_decoded_bytes as usize,
    };
    limits.check_input(data.len()).map_err(native_error)?;
    // Snapshot once before returning to JS. A Buffer reference alone would let
    // callers mutate the bytes while the libuv worker reads them.
    Ok(AsyncTask::new(Analyze {
        data: data.to_vec(),
        formats,
        limits,
    }))
}

#[napi]
pub fn inspect_native(
    data: Buffer,
    max_input_bytes: u32,
    max_decoded_bytes: u32,
) -> napi::Result<AsyncTask<Analyze>> {
    task(data, None, max_input_bytes, max_decoded_bytes)
}

#[napi]
pub fn validate_native(
    data: Buffer,
    formats: Vec<String>,
    max_input_bytes: u32,
    max_decoded_bytes: u32,
) -> napi::Result<AsyncTask<Analyze>> {
    task(data, Some(formats), max_input_bytes, max_decoded_bytes)
}

#[napi]
pub fn native_formats() -> napi::Result<Vec<String>> {
    Ok(polyglot_formats::builtin_registry()
        .map_err(native_error)?
        .names())
}

#[napi(object)]
pub struct NativeInput {
    pub format: String,
    pub data: Buffer,
    pub label: Option<String>,
}

#[napi(object)]
pub struct NativeComposition {
    pub data: Buffer,
    pub report_json: String,
}

pub struct Compose {
    inputs: Vec<polyglot_core::PolyglotInput>,
    options: polyglot_core::CompositionOptions,
}

#[napi]
impl Task for Compose {
    type Output = polyglot_core::CompositionResult;
    type JsValue = NativeComposition;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        polyglot_strategies::builtin_composer(
            polyglot_formats::builtin_registry().map_err(native_error)?,
            self.options.max_output_bytes,
        )
        .map_err(native_error)?
        .compose(&self.inputs, &self.options)
        .map_err(native_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(NativeComposition {
            data: output.data.into(),
            report_json: serde_json::to_string(&output.report).map_err(native_error)?,
        })
    }
}

#[napi]
pub fn create_native(
    inputs: Vec<NativeInput>,
    strategy: String,
    validate: bool,
    max_input_bytes: u32,
    max_decoded_bytes: u32,
    max_output_bytes: u32,
) -> napi::Result<AsyncTask<Compose>> {
    if !(2..=256).contains(&inputs.len()) {
        return Err(native_error(polyglot_core::Error::InvalidInput(
            "Composition requires between 2 and 256 inputs".into(),
        )));
    }
    let limits = Limits {
        max_input_bytes: max_input_bytes as usize,
        max_decoded_bytes: max_decoded_bytes as usize,
    };
    let total = inputs.iter().try_fold(0usize, |sum, input| {
        sum.checked_add(input.data.len()).ok_or_else(|| {
            native_error(polyglot_core::Error::LimitExceeded(
                "Total input size overflow".into(),
            ))
        })
    })?;
    limits.check_input(total).map_err(native_error)?;
    if max_output_bytes == 0 {
        return Err(native_error(polyglot_core::Error::InvalidInput(
            "maxOutputBytes must be positive".into(),
        )));
    }
    // Validate the aggregate budget before making any owned input snapshots.
    let inputs = inputs
        .into_iter()
        .map(|input| polyglot_core::PolyglotInput {
            format: input.format,
            data: input.data.to_vec(),
            label: input.label,
        })
        .collect();
    Ok(AsyncTask::new(Compose {
        inputs,
        options: polyglot_core::CompositionOptions {
            strategy,
            validate,
            limits,
            max_output_bytes: max_output_bytes as usize,
        },
    }))
}

pub struct CheckCompatibility {
    formats: Vec<String>,
    strategy: String,
}

#[napi]
impl Task for CheckCompatibility {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<String> {
        let composer = polyglot_strategies::builtin_composer(
            polyglot_formats::builtin_registry().map_err(native_error)?,
            64 * 1024 * 1024,
        )
        .map_err(native_error)?;
        serde_json::to_string(
            &composer
                .check_compatibility(&self.formats, &self.strategy)
                .map_err(native_error)?,
        )
        .map_err(native_error)
    }

    fn resolve(&mut self, _: Env, output: String) -> napi::Result<String> {
        Ok(output)
    }
}

#[napi]
pub fn check_compatibility_native(
    formats: Vec<String>,
    strategy: String,
) -> napi::Result<AsyncTask<CheckCompatibility>> {
    if !(2..=256).contains(&formats.len()) {
        return Err(native_error(polyglot_core::Error::InvalidInput(
            "Compatibility requires between 2 and 256 formats".into(),
        )));
    }
    Ok(AsyncTask::new(CheckCompatibility { formats, strategy }))
}

#[napi]
pub fn strategies_native() -> napi::Result<String> {
    let composer = polyglot_strategies::builtin_composer(
        polyglot_formats::builtin_registry().map_err(native_error)?,
        64 * 1024 * 1024,
    )
    .map_err(native_error)?;
    serde_json::to_string(&composer.strategies()).map_err(native_error)
}
