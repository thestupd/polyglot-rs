use polyglot_core::{
    CompatibilityResult, CompatibilityStatus, Composer, CompositionOptions, CompositionResult,
    CompositionStrategy, Error, FormatHandler, FormatInfo, Limits, PolyglotInput, Registry, Result,
    ValidationResult,
};
use polyglot_strategies::{builtin_composer, Append};

fn input(format: &str, data: &[u8]) -> PolyglotInput {
    PolyglotInput {
        format: format.into(),
        data: data.into(),
        label: None,
    }
}

// Synthetic native grammars exercise extension contracts without claiming any
// production format pairing is externally verified.
struct ToyFormat {
    name: &'static str,
    exact: bool,
}
impl FormatHandler for ToyFormat {
    fn name(&self) -> &'static str {
        self.name
    }
    fn detect(&self, data: &[u8]) -> bool {
        match self.name {
            "alpha" => data.first() == Some(&b'A') && (!self.exact || data.len() == 1),
            "beta" => data.last() == Some(&b'B'),
            _ => false,
        }
    }
    fn validate(&self, data: &[u8]) -> Result<ValidationResult> {
        Ok(if self.detect(data) {
            ValidationResult::valid(self.name)
        } else {
            ValidationResult::invalid(self.name, "Grammar mismatch")
        })
    }
    fn inspect(&self, data: &[u8]) -> Result<FormatInfo> {
        let mut info = FormatInfo::new(self.name);
        info.validation = self.validate(data)?;
        Ok(info)
    }
}

fn registry(exact: bool) -> Registry {
    let mut r = Registry::new();
    r.register(ToyFormat {
        name: "alpha",
        exact,
    })
    .unwrap();
    r.register(ToyFormat {
        name: "beta",
        exact: false,
    })
    .unwrap();
    r
}

#[test]
fn custom_native_formats_compose_without_changing_the_engine() {
    let composer = builtin_composer(registry(false), 1024).unwrap();
    let mut inputs = vec![input("alpha", b"A"), input("beta", b"B")];
    inputs[0].label = Some("host".into());
    let result = composer
        .compose(&inputs, &CompositionOptions::default())
        .unwrap();
    assert_eq!(result.data, b"AB");
    assert_eq!(inputs[0].data, b"A");
    assert!(result.report.validated);
    assert!(result.report.validation.iter().all(|v| v.valid));
    assert_eq!(result.report.validation.len(), 2);
    assert_eq!(result.report.inputs[0].label.as_deref(), Some("host"));
    assert_eq!(
        result.report.compatibility.status,
        CompatibilityStatus::Conditional
    );
    assert_eq!(
        result.data,
        composer
            .compose(&inputs, &CompositionOptions::default())
            .unwrap()
            .data
    );
}

#[test]
fn opaque_inputs_are_accepted_only_as_explicitly_unvalidated_candidates() {
    let composer = builtin_composer(Registry::new(), 1024).unwrap();
    let inputs = [input("opaque-a", &[0, 255]), input("opaque-b", &[42])];
    assert!(matches!(
        composer.compose(&inputs, &CompositionOptions::default()),
        Err(Error::InvalidComposition(_))
    ));
    let options = CompositionOptions {
        validate: false,
        ..CompositionOptions::default()
    };
    let result = composer.compose(&inputs, &options).unwrap();
    assert_eq!(result.data, [0, 255, 42]);
    assert!(!result.report.validated);
    assert!(result.report.validation.is_empty());
    assert!(result.report.inputs.iter().all(|i| i.validation.is_none()));
    assert_eq!(
        result.report.compatibility.status,
        CompatibilityStatus::Conditional
    );
}

#[test]
fn whole_output_validation_is_mandatory_by_default() {
    let composer = builtin_composer(registry(true), 1024).unwrap();
    let inputs = [input("alpha", b"A"), input("beta", b"B")];
    assert!(matches!(
        composer.compose(&inputs, &CompositionOptions::default()),
        Err(Error::InvalidComposition(_))
    ));
    let inputs = [input("alpha", b"bad"), input("beta", b"B")];
    assert!(matches!(
        composer.compose(&inputs, &CompositionOptions::default()),
        Err(Error::InvalidComposition(_))
    ));
}

#[test]
fn built_in_profiles_reject_image_append_and_placeholder_algorithms() {
    let composer = builtin_composer(polyglot_formats::builtin_registry().unwrap(), 1024).unwrap();
    // A carrier image as the first input + any payload second is supported
    // through the carrier's named strategy; append never promotes the pair.
    for (formats, strategy) in [
        (["png", "mpeg-ts"], "png"),
        (["gif", "mpeg-ts"], "gif"),
        (["jpeg", "mpeg-ts"], "jpeg"),
        (["png", "segment"], "png"),
        (["jpeg", "zip"], "jpeg"),
    ] {
        let formats = formats.map(String::from);
        let result = composer.check_compatibility(&formats, "auto").unwrap();
        assert_eq!(result.status, CompatibilityStatus::Supported);
        assert_eq!(result.strategies, vec![strategy.to_string()]);
        let append = composer.check_compatibility(&formats, "append").unwrap();
        assert_eq!(append.status, CompatibilityStatus::Unsupported);
    }
    // The carrier must come first; a payload-first pair is not a carrier.
    for formats in [["mpeg-ts", "png"], ["zip", "jpeg"]] {
        let formats = formats.map(String::from);
        let result = composer.check_compatibility(&formats, "auto").unwrap();
        assert_ne!(result.status, CompatibilityStatus::Supported);
    }
    for name in ["embed", "overlap"] {
        let result = composer
            .check_compatibility(&["opaque-a".into(), "opaque-b".into()], name)
            .unwrap();
        assert_eq!(result.status, CompatibilityStatus::Unsupported);
        assert!(matches!(
            composer.compose(
                &[input("opaque-a", b"a"), input("opaque-b", b"b")],
                &CompositionOptions {
                    strategy: name.into(),
                    validate: false,
                    ..CompositionOptions::default()
                }
            ),
            Err(Error::UnsupportedCombination(_))
        ));
    }
    let result = composer
        .check_compatibility(&["mpeg-ts".into(), "mpeg-ts".into()], "append")
        .unwrap();
    assert_eq!(result.status, CompatibilityStatus::Conditional);
}

#[test]
fn limits_and_strategy_selection_fail_before_returning_unsafe_output() {
    let composer = builtin_composer(registry(false), 1).unwrap();
    let inputs = [input("alpha", b"A"), input("beta", b"B")];
    assert!(matches!(
        composer.compose(
            &inputs,
            &CompositionOptions {
                max_output_bytes: 1,
                ..CompositionOptions::default()
            }
        ),
        Err(Error::LimitExceeded(_))
    ));
    assert!(matches!(
        composer.compose(
            &inputs,
            &CompositionOptions {
                limits: Limits {
                    max_input_bytes: 1,
                    ..Limits::default()
                },
                ..CompositionOptions::default()
            }
        ),
        Err(Error::LimitExceeded(_))
    ));
    assert!(matches!(
        composer.compose(
            &inputs,
            &CompositionOptions {
                strategy: "absent".into(),
                ..CompositionOptions::default()
            }
        ),
        Err(Error::UnknownStrategy(_))
    ));
    assert!(matches!(
        composer.compose(&inputs[..1], &CompositionOptions::default()),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        composer.compose(
            &inputs,
            &CompositionOptions {
                max_output_bytes: 0,
                ..CompositionOptions::default()
            }
        ),
        Err(Error::InvalidInput(_))
    ));
    let mut composer = Composer::new(registry(false));
    composer
        .register_strategy(Append::new(registry(false), 1024))
        .unwrap();
    assert!(matches!(
        composer.register_strategy(Append::new(registry(false), 1024)),
        Err(Error::DuplicateStrategy(_))
    ));
}

struct BrokenCandidate;
impl CompositionStrategy for BrokenCandidate {
    fn name(&self) -> &'static str {
        "broken"
    }
    fn check_compatibility(&self, inputs: &[PolyglotInput]) -> Result<CompatibilityResult> {
        if inputs.iter().any(|input| input.data.is_empty()) {
            return Err(Error::InvalidInput(
                "Byte-aware checker requires actual input bytes".into(),
            ));
        }
        Ok(CompatibilityResult::conditional(
            "broken",
            "Test fallback",
            vec![],
        ))
    }
    fn compose(&self, _: &[PolyglotInput]) -> Result<CompositionResult> {
        let mut result = CompositionResult::candidate(
            b"not-valid".to_vec(),
            "forged",
            CompatibilityResult::conditional("forged", "Test untrusted report", vec![]),
        );
        result.report.validated = true;
        Ok(result)
    }
}

#[test]
fn format_queries_do_not_fabricate_empty_binary_inputs() {
    let mut composer = Composer::new(registry(false));
    composer.register_strategy(BrokenCandidate).unwrap();
    let result = composer
        .check_compatibility(&["alpha".into(), "beta".into()], "auto")
        .unwrap();
    assert_eq!(result.status, CompatibilityStatus::Conditional);
    assert_eq!(result.strategies, vec!["broken"]);
    assert!(result.limitations.is_empty());
}

#[test]
fn auto_falls_back_after_failed_validation_and_rebuilds_reports() {
    let r = registry(false);
    let mut composer = Composer::new(r.clone());
    composer.register_strategy(BrokenCandidate).unwrap();
    composer.register_strategy(Append::new(r, 1024)).unwrap();
    let inputs = [input("alpha", b"A"), input("beta", b"B")];
    let result = composer
        .compose(&inputs, &CompositionOptions::default())
        .unwrap();
    assert_eq!(result.report.strategy, "append");
    assert_eq!(result.data, b"AB");
    assert!(matches!(
        composer.compose(
            &inputs,
            &CompositionOptions {
                strategy: "broken".into(),
                ..CompositionOptions::default()
            }
        ),
        Err(Error::InvalidComposition(_))
    ));
}

#[test]
fn mpeg_ts_reassembly_is_validated_but_not_claimed_as_a_polyglot() {
    let data = include_bytes!("../../../tests/fixtures/red.ts");
    let split = 188 * 4;
    let composer =
        builtin_composer(polyglot_formats::builtin_registry().unwrap(), 1024 * 1024).unwrap();
    let result = composer
        .compose(
            &[
                input("mpeg-ts", &data[..split]),
                input("mpeg-ts", &data[split..]),
            ],
            &CompositionOptions::default(),
        )
        .unwrap();
    assert_eq!(result.data, data);
    assert!(result.report.validated);
    assert_eq!(result.report.validation.len(), 1);
    assert_eq!(
        result.report.compatibility.status,
        CompatibilityStatus::Conditional
    );
    assert!(result
        .report
        .warnings
        .iter()
        .any(|w| w.contains("multiple distinct")));
    assert!(matches!(
        composer.compose(
            &[
                input("mpeg-ts", &data[..split - 1]),
                input("mpeg-ts", &data[split - 1..])
            ],
            &CompositionOptions::default()
        ),
        Err(Error::UnsupportedCombination(_))
    ));
}
