use crate::{handler, malformed};
use polyglot_core::{Cursor, Error, FormatInfo, Limits, Result, ValidationResult};
use serde_json::json;
use std::io::Cursor as IoCursor;

pub struct Png;
impl polyglot_core::AppendCapability for Png {
    fn permits_prefix(&self) -> bool {
        false
    }
    fn permits_suffix(&self) -> bool {
        false
    }
}
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
handler!(Png, "png", |data: &[u8]| data.starts_with(SIGNATURE), parse);

fn parse(data: &[u8], limits: Limits) -> Result<FormatInfo> {
    let mut cursor = Cursor::new(data);
    if cursor.take(8)? != SIGNATURE {
        return Err(malformed("Invalid PNG signature"));
    }
    let mut info = FormatInfo::new("png");
    let (mut chunks, mut palette, mut idat, mut idat_ended, mut animated) =
        (0, false, false, false, false);
    let (mut color, mut depth) = (0, 0);
    loop {
        let length = cursor.be_u32()? as usize;
        if length > 0x7fff_ffff {
            return Err(malformed("PNG chunk length exceeds 31-bit limit"));
        }
        let kind = cursor.take(4)?;
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(malformed("Invalid PNG chunk type"));
        }
        let payload = cursor.take(length)?;
        let crc = cursor.be_u32()?;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(payload);
        if hasher.finalize() != crc {
            return Err(malformed("PNG chunk CRC mismatch"));
        }
        if chunks == 0 && kind != b"IHDR" {
            return Err(malformed("IHDR must be the first chunk"));
        }
        chunks += 1;
        if idat && kind != b"IDAT" {
            idat_ended = true;
        }
        match kind {
            b"IHDR" => {
                if chunks != 1 || length != 13 {
                    return Err(malformed("Invalid or duplicate IHDR"));
                }
                let mut header = Cursor::new(payload);
                let width = header.be_u32()?;
                let height = header.be_u32()?;
                depth = header.byte()?;
                color = header.byte()?;
                let valid_depth = match color {
                    0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                    2 | 4 | 6 => matches!(depth, 8 | 16),
                    3 => matches!(depth, 1 | 2 | 4 | 8),
                    _ => false,
                };
                if width == 0
                    || height == 0
                    || width > 0x7fff_ffff
                    || height > 0x7fff_ffff
                    || !valid_depth
                    || header.byte()? != 0
                    || header.byte()? != 0
                    || header.byte()? > 1
                {
                    return Err(malformed(
                        "Invalid PNG dimensions, color type, bit depth or encoding method",
                    ));
                }
                info.metadata.insert("width".into(), json!(width));
                info.metadata.insert("height".into(), json!(height));
                info.metadata.insert("bitDepth".into(), json!(depth));
                info.metadata.insert("colorType".into(), json!(color));
            }
            b"PLTE" => {
                if palette
                    || idat
                    || length == 0
                    || length > 768
                    || !length.is_multiple_of(3)
                    || matches!(color, 0 | 4)
                    || (color == 3 && length / 3 > (1usize << depth))
                {
                    return Err(malformed("Invalid PNG palette or palette order"));
                }
                palette = true;
            }
            b"IDAT" => {
                if idat_ended || (color == 3 && !palette) {
                    return Err(malformed("Invalid IDAT order or missing palette"));
                }
                idat = true;
            }
            b"IEND" => {
                if length != 0 || !idat {
                    return Err(malformed("Invalid IEND or missing IDAT"));
                }
                if cursor.remaining() != 0 {
                    return Err(malformed("Trailing bytes after PNG IEND"));
                }
                break;
            }
            b"acTL" | b"fcTL" | b"fdAT" => animated = true,
            _ if kind[0].is_ascii_uppercase() => {
                return Err(malformed("Unknown critical PNG chunk"))
            }
            _ => {}
        }
    }
    // The initial handler validates static PNGs. Do not silently validate only
    // the default image of an APNG while claiming its animation is valid.
    if animated {
        return Err(malformed(
            "APNG validation is not implemented; only static PNG is supported",
        ));
    }
    let mut decoder = ::png::Decoder::new_with_limits(
        IoCursor::new(data),
        ::png::Limits {
            bytes: limits.max_decoded_bytes,
        },
    );
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder
        .read_info()
        .map_err(|e| malformed(format!("PNG decoder: {e}")))?;
    let expected = reader
        .output_buffer_size()
        .ok_or_else(|| Error::LimitExceeded("PNG decoded size overflow".into()))?;
    if expected > limits.max_decoded_bytes {
        return Err(Error::LimitExceeded("PNG exceeds maxDecodedBytes".into()));
    }
    // Row decoding avoids allocating a full image based on untrusted dimensions.
    while reader
        .next_row()
        .map_err(|e| malformed(format!("PNG decoder: {e}")))?
        .is_some()
    {}
    reader
        .finish()
        .map_err(|e| malformed(format!("PNG decoder: {e}")))?;
    info.metadata.insert("chunks".into(), json!(chunks));
    info.validation
        .warnings
        .push("Ancillary metadata semantics are not exhaustively validated".into());
    Ok(info)
}
