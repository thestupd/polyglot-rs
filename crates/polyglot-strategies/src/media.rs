//! Carry an arbitrary payload inside a PNG, GIF or JPEG image.
//!
//! The first input is the carrier image and is validated as that image. The
//! second input is the payload — any bytes in any format. The payload is stored
//! byte-for-byte and never parsed, so its internal format is the caller's
//! concern. The output opens as the image, and the report's payload region
//! gives the payload's byte range (length and offset) for byte-range access.
use polyglot_core::{
    CompatibilityResult, CompositionResult, CompositionStrategy, Error, PolyglotInput, Region,
    Result, StrategyRequirements,
};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
/// End of the IHDR chunk: signature (8) + length/type (8) + data (13) + CRC (4).
const PNG_IHDR_END: usize = 33;
/// Ancillary, private, reserved-bit-clear, safe-to-copy chunk type.
const PNG_CHUNK: &[u8; 4] = b"paTs";
const PNG_MAX_CHUNK: usize = 0x7fff_ffff;
const GIF_TRAILER: u8 = 0x3b;

#[derive(Clone, Copy)]
enum Carrier {
    Png,
    Gif,
    Jpeg,
}

impl Carrier {
    fn format(self) -> &'static str {
        match self {
            Carrier::Png => "png",
            Carrier::Gif => "gif",
            Carrier::Jpeg => "jpeg",
        }
    }
}

/// `png` strategy: the payload is a private ancillary PNG chunk right after IHDR.
pub struct PngCarrier {
    max_output_bytes: usize,
}

/// `gif` strategy: the payload follows the GIF trailer.
pub struct GifCarrier {
    max_output_bytes: usize,
}

/// `jpeg` strategy: the payload follows the JPEG end-of-image marker.
pub struct JpegCarrier {
    max_output_bytes: usize,
}

impl PngCarrier {
    pub fn new(max_output_bytes: usize) -> Self {
        Self { max_output_bytes }
    }
}
impl GifCarrier {
    pub fn new(max_output_bytes: usize) -> Self {
        Self { max_output_bytes }
    }
}
impl JpegCarrier {
    pub fn new(max_output_bytes: usize) -> Self {
        Self { max_output_bytes }
    }
}

fn limitations(carrier: Carrier) -> Vec<String> {
    vec![
        "The payload is stored byte-for-byte and never validated; only the carrier image is"
            .into(),
        "The payload region reports its byte range (length@offset) for byte-range access".into(),
        match carrier {
            Carrier::Png => "The payload is a private ancillary chunk (paTs) before the image \
                             data; the whole file stays a valid PNG and decoders skip the chunk"
                .into(),
            Carrier::Gif => "The payload follows the GIF trailer; decoders stop at the trailer, \
                             but strict validators that reject trailing bytes will not accept the file"
                .into(),
            Carrier::Jpeg => "The payload follows the JPEG EOI marker; decoders stop at EOI, \
                              but strict validators that reject trailing bytes will not accept the file"
                .into(),
        },
    ]
}

fn formats(inputs: &[PolyglotInput]) -> Vec<&str> {
    inputs.iter().map(|i| i.format.as_str()).collect()
}

/// The first input is the carrier image; the second is any other format.
fn check(name: &str, carrier: Carrier, formats: &[&str]) -> CompatibilityResult {
    match formats {
        [image, payload] if *image == carrier.format() && *payload != carrier.format() => {
            CompatibilityResult::supported(
                name,
                format!(
                    "The {} carrier is validated; the {payload} payload is carried unchanged",
                    carrier.format()
                ),
                limitations(carrier),
            )
        }
        _ => CompatibilityResult::unsupported(format!(
            "{name} requires a {} carrier as the first input and one payload of another format",
            carrier.format()
        )),
    }
}

fn reserve(size: usize, max_output_bytes: usize) -> Result<Vec<u8>> {
    if size > max_output_bytes {
        return Err(Error::LimitExceeded("Output exceeds maxOutputBytes".into()));
    }
    let mut data = Vec::new();
    data.try_reserve_exact(size)
        .map_err(|_| Error::LimitExceeded("Cannot allocate composition output".into()))?;
    Ok(data)
}

/// PNG CRC-32 (ISO 3309, reflected polynomial 0xedb88320).
fn png_crc(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for part in parts {
        for &byte in *part {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn compose(
    name: &str,
    carrier: Carrier,
    inputs: &[PolyglotInput],
    max_output_bytes: usize,
) -> Result<CompositionResult> {
    let compatibility = check(name, carrier, &formats(inputs));
    let ([image_input, payload_input], true) = (inputs, !compatibility.strategies.is_empty())
    else {
        return Err(Error::UnsupportedCombination(
            compatibility.explanation.unwrap_or_default(),
        ));
    };
    let (image, payload) = (&image_input.data, &payload_input.data);
    let overflow = || Error::LimitExceeded("Composition size overflow".into());
    let (data, offset) = match carrier {
        Carrier::Png => {
            if image.len() < PNG_IHDR_END
                || &image[..8] != PNG_SIGNATURE
                || image[8..12] != [0, 0, 0, 13]
                || &image[12..16] != b"IHDR"
            {
                return Err(Error::InvalidComposition(
                    "PNG carrier must begin with a signature and an IHDR chunk".into(),
                ));
            }
            if payload.len() > PNG_MAX_CHUNK {
                return Err(Error::LimitExceeded(
                    "Payload exceeds the PNG chunk size limit".into(),
                ));
            }
            let size = image
                .len()
                .checked_add(payload.len())
                .and_then(|size| size.checked_add(12))
                .ok_or_else(overflow)?;
            let mut data = reserve(size, max_output_bytes)?;
            data.extend_from_slice(&image[..PNG_IHDR_END]);
            data.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            data.extend_from_slice(PNG_CHUNK);
            data.extend_from_slice(payload);
            data.extend_from_slice(&png_crc(&[PNG_CHUNK, payload]).to_be_bytes());
            data.extend_from_slice(&image[PNG_IHDR_END..]);
            (data, PNG_IHDR_END + 8)
        }
        Carrier::Gif => {
            if !(image.starts_with(b"GIF87a") || image.starts_with(b"GIF89a"))
                || image.last() != Some(&GIF_TRAILER)
            {
                return Err(Error::InvalidComposition(
                    "GIF carrier must begin with a GIF signature and end with its trailer".into(),
                ));
            }
            let size = image
                .len()
                .checked_add(payload.len())
                .ok_or_else(overflow)?;
            let mut data = reserve(size, max_output_bytes)?;
            data.extend_from_slice(image);
            data.extend_from_slice(payload);
            (data, image.len())
        }
        Carrier::Jpeg => {
            if !image.starts_with(&[0xff, 0xd8, 0xff]) || !image.ends_with(&[0xff, 0xd9]) {
                return Err(Error::InvalidComposition(
                    "JPEG carrier must begin with a SOI marker and end with EOI".into(),
                ));
            }
            let size = image
                .len()
                .checked_add(payload.len())
                .ok_or_else(overflow)?;
            let mut data = reserve(size, max_output_bytes)?;
            data.extend_from_slice(image);
            data.extend_from_slice(payload);
            (data, image.len())
        }
    };
    // The PNG carrier spans the whole output; GIF/JPEG carriers end before the payload.
    let image_length = match carrier {
        Carrier::Png => data.len(),
        Carrier::Gif | Carrier::Jpeg => image.len(),
    };
    let regions = vec![
        Region {
            format: carrier.format().into(),
            offset: 0,
            length: image_length,
        },
        Region {
            format: payload_input.format.clone(),
            offset,
            length: payload.len(),
        },
    ];
    Ok(CompositionResult::candidate(data, name, compatibility).with_regions(regions))
}

fn requirements(carrier: Carrier) -> StrategyRequirements {
    StrategyRequirements {
        description: format!(
            "Carry an arbitrary payload inside a {} image; only the image is validated",
            carrier.format()
        ),
        min_inputs: 2,
        requires_native_handlers: false,
        limitations: limitations(carrier),
    }
}

macro_rules! strategy {
    ($type:ty, $name:literal, $carrier:expr) => {
        impl CompositionStrategy for $type {
            fn name(&self) -> &'static str {
                $name
            }
            fn requirements(&self) -> StrategyRequirements {
                requirements($carrier)
            }
            fn check_compatibility(&self, inputs: &[PolyglotInput]) -> Result<CompatibilityResult> {
                Ok(check($name, $carrier, &formats(inputs)))
            }
            fn check_formats(&self, formats: &[String]) -> Result<CompatibilityResult> {
                let formats: Vec<_> = formats.iter().map(String::as_str).collect();
                Ok(check($name, $carrier, &formats))
            }
            fn carried_inputs(&self, inputs: &[PolyglotInput]) -> Vec<usize> {
                // The payload (second input) is stored unchanged and not validated.
                if inputs.len() == 2 {
                    vec![1]
                } else {
                    vec![]
                }
            }
            fn compose(&self, inputs: &[PolyglotInput]) -> Result<CompositionResult> {
                compose($name, $carrier, inputs, self.max_output_bytes)
            }
        }
    };
}

strategy!(PngCarrier, "png", Carrier::Png);
strategy!(GifCarrier, "gif", Carrier::Gif);
strategy!(JpegCarrier, "jpeg", Carrier::Jpeg);

#[cfg(test)]
mod tests {
    use super::*;
    use polyglot_core::{CompatibilityStatus, CompositionOptions};

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn input(format: &str, data: Vec<u8>) -> PolyglotInput {
        PolyglotInput {
            format: format.into(),
            data,
            label: None,
        }
    }

    fn composer() -> polyglot_core::Composer {
        crate::builtin_composer(polyglot_formats::builtin_registry().unwrap(), 1 << 26).unwrap()
    }

    #[test]
    fn png_crc_matches_reference() {
        assert_eq!(png_crc(&[b"IEND"]), 0xae42_6082);
    }

    #[test]
    fn png_carrier_keeps_a_valid_png_and_carries_any_payload_unchanged() {
        // An arbitrary payload name, and even invalid transport bytes, are carried.
        for payload in [fixture("red.ts"), b"not a transport stream".to_vec()] {
            for payload_format in ["mpeg-ts", "segment", "data"] {
                let inputs = [
                    input("png", fixture("red.png")),
                    input(payload_format, payload.clone()),
                ];
                let result = composer()
                    .compose(&inputs, &CompositionOptions::default())
                    .unwrap();
                let report = &result.report;
                assert_eq!(report.strategy, "png");
                assert_eq!(report.compatibility.status, CompatibilityStatus::Supported);
                // Only the carrier is validated; the payload has no report.
                assert_eq!(report.validation.len(), 1);
                assert_eq!(report.validation[0].format, "png");
                assert!(report.validation[0].valid, "{report:?}");
                assert!(report.inputs[1].validation.is_none());
                assert!(report
                    .warnings
                    .iter()
                    .any(|w| w.contains("carried") || w.contains("Carried")));
                let region = &report.regions[1];
                assert_eq!(
                    (region.format.as_str(), region.offset),
                    (payload_format, 41)
                );
                assert_eq!(&result.data[41..][..region.length], &payload[..]);
            }
        }
    }

    #[test]
    fn gif_and_jpeg_carriers_append_the_payload_after_their_terminators() {
        let payload = fixture("red.ts");
        for (carrier, name) in [("gif", "black.gif"), ("jpeg", "red.jpg")] {
            let image = fixture(name);
            let inputs = [
                input(carrier, image.clone()),
                input("segment", payload.clone()),
            ];
            let result = composer()
                .compose(&inputs, &CompositionOptions::default())
                .unwrap();
            assert_eq!(
                result.report.compatibility.status,
                CompatibilityStatus::Supported
            );
            assert_eq!(result.data, [image.clone(), payload.clone()].concat());
            assert_eq!(
                result.report.regions,
                vec![
                    Region {
                        format: carrier.into(),
                        offset: 0,
                        length: image.len()
                    },
                    Region {
                        format: "segment".into(),
                        offset: image.len(),
                        length: payload.len()
                    },
                ]
            );
        }
    }

    #[test]
    fn carrier_must_be_first_and_a_valid_image() {
        // Payload first is not a carrier arrangement.
        let swapped = [
            input("segment", fixture("red.ts")),
            input("png", fixture("red.png")),
        ];
        assert_eq!(
            PngCarrier::new(1 << 20)
                .check_compatibility(&swapped)
                .unwrap()
                .status,
            CompatibilityStatus::Unsupported
        );
        // A corrupt carrier fails validation (the payload is still never checked).
        let mut png = fixture("red.png");
        png[20] ^= 0xff;
        let inputs = [input("png", png), input("segment", fixture("red.ts"))];
        assert!(matches!(
            composer().compose(&inputs, &CompositionOptions::default()),
            Err(Error::InvalidComposition(_))
        ));
    }

    #[test]
    fn output_budget_is_enforced_before_allocation() {
        let inputs = [
            input("gif", fixture("black.gif")),
            input("segment", fixture("red.ts")),
        ];
        assert!(matches!(
            GifCarrier::new(16).compose(&inputs),
            Err(Error::LimitExceeded(_))
        ));
    }
}
