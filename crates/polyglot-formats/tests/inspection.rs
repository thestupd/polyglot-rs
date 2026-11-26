use polyglot_core::{FormatHandler, Limits, Registry};
use polyglot_formats::{builtin_registry, Gif, MpegTs, Png};

const PNG: &[u8] = include_bytes!("../../../tests/fixtures/red.png");
const GIF: &[u8] = include_bytes!("../../../tests/fixtures/black.gif");
const TS: &[u8] = include_bytes!("../../../tests/fixtures/red.ts");

#[test]
fn detect_inspect_and_validate_real_fixtures() {
    let registry = builtin_registry().unwrap();
    for (format, data) in [("png", PNG), ("gif", GIF), ("mpeg-ts", TS)] {
        let original = data.to_vec();
        let result = registry.inspect(data, Limits::default()).unwrap();
        assert_eq!(result.formats, vec![format]);
        assert!(
            result.metadata[format].validation.valid,
            "{:?}",
            result.metadata[format]
        );
        assert_eq!(data, original);
        let independent = registry
            .validate(data, &[format.into(), "opaque".into()], Limits::default())
            .unwrap();
        assert!(independent[0].valid);
        assert!(!independent[1].valid);
    }
    assert_eq!(Png.inspect(PNG).unwrap().metadata["width"], 2);
    assert_eq!(Gif.inspect(GIF).unwrap().metadata["frames"], 1);
    assert_eq!(MpegTs.inspect(TS).unwrap().metadata["hasPat"], true);
    assert!(!MpegTs.inspect(TS).unwrap().metadata["streamPids"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn candidate_detection_does_not_imply_validity() {
    let registry = builtin_registry().unwrap();
    for (format, data) in [
        ("png", &PNG[..8]),
        ("gif", &GIF[..6]),
        ("mpeg-ts", &TS[..188 * 3 + 1]),
    ] {
        let result = registry.inspect(data, Limits::default()).unwrap();
        assert!(result.formats.contains(&format.into()));
        assert!(!result.metadata[format].validation.valid);
    }
    for data in [b"".as_slice(), b"not a binary format", &[0x47; 188 * 3]] {
        assert!(!Png.detect(data));
        assert!(!Gif.detect(data));
    }
}

#[test]
fn every_image_truncation_is_invalid() {
    for (handler, data) in [
        (&Png as &dyn FormatHandler, PNG),
        (&Gif as &dyn FormatHandler, GIF),
    ] {
        for end in 0..data.len() {
            assert!(
                !handler.validate(&data[..end]).unwrap().valid,
                "{} prefix {end}",
                handler.name()
            );
        }
    }
}

#[test]
fn malformed_and_random_data_never_panics() {
    let handlers: [&dyn FormatHandler; 3] = [&Png, &Gif, &MpegTs];
    let mut random = 0x1234_5678u32;
    for size in 0..1024 {
        let data: Vec<_> = (0..size)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u8
            })
            .collect();
        for handler in handlers {
            let result = handler.validate(&data).unwrap();
            assert!(!result.valid);
        }
    }
    // Exercise deep parsers as well as signature failures.
    for (handler, source) in [
        (&Png as &dyn FormatHandler, PNG),
        (&Gif as &dyn FormatHandler, GIF),
        (&MpegTs as &dyn FormatHandler, TS),
    ] {
        for index in 0..source.len() {
            let mut corrupt = source.to_vec();
            corrupt[index] ^= 0xff;
            let _ = handler.inspect(&corrupt);
        }
    }
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut result = (data.len() as u32).to_be_bytes().to_vec();
    result.extend_from_slice(kind);
    result.extend_from_slice(data);
    let crc = crc32fast::hash(&result[4..]);
    result.extend_from_slice(&crc.to_be_bytes());
    result
}

#[test]
fn png_checks_crc_chunk_order_lengths_and_compressed_pixels() {
    let mut corrupt = PNG.to_vec();
    corrupt[29] ^= 1;
    assert!(!Png.validate(&corrupt).unwrap().valid);
    corrupt = PNG.to_vec();
    corrupt[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(!Png.validate(&corrupt).unwrap().valid);
    let ihdr = &PNG[8..33];
    let mut missing = PNG[..33].to_vec();
    missing.extend(png_chunk(b"IEND", &[]));
    assert!(!Png.validate(&missing).unwrap().valid);
    let mut bad_deflate = PNG[..33].to_vec();
    bad_deflate.extend(png_chunk(b"IDAT", b"not compressed pixels"));
    bad_deflate.extend(png_chunk(b"IEND", &[]));
    assert!(!Png.validate(&bad_deflate).unwrap().valid);
    let mut duplicate = PNG[..8].to_vec();
    duplicate.extend_from_slice(ihdr);
    duplicate.extend_from_slice(ihdr);
    duplicate.extend_from_slice(&PNG[33..]);
    assert!(!Png.validate(&duplicate).unwrap().valid);
    let mut trailing = PNG.to_vec();
    trailing.push(0);
    assert!(!Png.validate(&trailing).unwrap().valid);
}

#[test]
fn gif_checks_palettes_bounds_extensions_and_lzw() {
    for (index, value) in [(6, 0), (24, 2), (28, 0x18), (29, 12), (31, 0xff), (34, 0)] {
        let mut bad = GIF.to_vec();
        bad[index] = value;
        assert!(!Gif.validate(&bad).unwrap().valid, "byte {index}");
    }
    let mut with_comment = GIF[..19].to_vec();
    with_comment.extend_from_slice(b"\x21\xfe\x03abc\x00");
    with_comment.extend_from_slice(&GIF[19..]);
    assert!(Gif.validate(&with_comment).unwrap().valid);
    let mut truncated_comment = GIF[..19].to_vec();
    truncated_comment.extend_from_slice(b"\x21\xfe\xff");
    assert!(!Gif.validate(&truncated_comment).unwrap().valid);
}

fn null_packets(count: usize) -> Vec<u8> {
    let mut data = vec![0xff; 188 * count];
    for p in data.as_chunks_mut::<188>().0 {
        p[..4].copy_from_slice(&[0x47, 0x1f, 0xff, 0x10]);
    }
    data
}

#[test]
fn mpeg_ts_checks_sync_alignment_control_continuity_and_psi_crc() {
    let null = null_packets(3);
    assert!(MpegTs.detect(&null));
    assert!(MpegTs.validate(&null).unwrap().valid);
    for (index, value) in [(0, 0), (1, 0x9f), (3, 0), (3, 0x50), (3, 0x30)] {
        let mut bad = null.clone();
        bad[index] = value;
        assert!(
            !MpegTs.validate(&bad).unwrap().valid,
            "byte {index} = {value}"
        );
    }
    assert!(!MpegTs.validate(&null[..null.len() - 1]).unwrap().valid);
    let mut continuity = null.clone();
    for (i, p) in continuity.as_chunks_mut::<188>().0.iter_mut().enumerate() {
        p[1] = 1;
        p[2] = 0;
        p[3] = 0x10 | if i == 2 { 7 } else { i as u8 };
    }
    assert!(!MpegTs.validate(&continuity).unwrap().valid);
    let mut bad_crc = TS.to_vec();
    let packet = bad_crc
        .as_chunks_mut::<188>()
        .0
        .iter_mut()
        .find(|p| p[1] & 31 == 0 && p[2] == 0)
        .unwrap();
    packet[15] ^= 1;
    assert!(!MpegTs.validate(&bad_crc).unwrap().valid);
}

#[test]
fn limits_and_registry_contracts() {
    let registry = builtin_registry().unwrap();
    assert!(registry
        .inspect(
            PNG,
            Limits {
                max_input_bytes: 1,
                ..Limits::default()
            }
        )
        .is_err());
    assert!(registry
        .validate(
            GIF,
            &["gif".into()],
            Limits {
                max_decoded_bytes: 0,
                ..Limits::default()
            }
        )
        .is_err());
    assert!(registry
        .inspect(
            PNG,
            Limits {
                max_decoded_bytes: 1,
                ..Limits::default()
            }
        )
        .map(|r| !r.metadata["png"].validation.valid)
        .unwrap_or(true));
    assert!(registry.validate(PNG, &[], Limits::default()).is_err());
    let mut registry = Registry::new();
    registry.register(Png).unwrap();
    assert!(registry.register(Png).is_err());
    assert_eq!(registry.names(), vec!["png"]);
}
