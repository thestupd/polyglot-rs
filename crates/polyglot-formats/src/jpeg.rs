use crate::{handler, malformed};
use polyglot_core::{Cursor, FormatInfo, Limits, Result, ValidationResult};
use serde_json::json;

pub struct Jpeg;
impl polyglot_core::AppendCapability for Jpeg {
    fn permits_prefix(&self) -> bool {
        false
    }
    fn permits_suffix(&self) -> bool {
        false
    }
}
handler!(
    Jpeg,
    "jpeg",
    |data: &[u8]| data.starts_with(&[0xff, 0xd8, 0xff]),
    parse
);

/// Markers that stand alone with no length field: TEM and the restart markers.
fn standalone(marker: u8) -> bool {
    marker == 0x01 || (0xd0..=0xd7).contains(&marker)
}

/// Start-of-frame markers carrying frame dimensions (excluding DHT/JPG/DAC).
fn is_sof(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc))
}

/// Skip entropy-coded scan data to the next real marker. FF00 is a stuffed
/// literal and FFD0..FFD7 are in-scan restart markers; neither ends the scan.
fn skip_scan(data: &[u8], mut offset: usize) -> Result<usize> {
    loop {
        while offset < data.len() && data[offset] != 0xff {
            offset += 1;
        }
        let next = *data
            .get(offset + 1)
            .ok_or_else(|| malformed("Truncated JPEG scan data"))?;
        if next == 0x00 || next == 0xff || standalone(next) {
            offset += if next == 0xff { 1 } else { 2 };
        } else {
            return Ok(offset); // a real marker begins here
        }
    }
}

fn parse(data: &[u8], _: Limits) -> Result<FormatInfo> {
    let mut c = Cursor::new(data);
    if c.be_u16()? != 0xffd8 {
        return Err(malformed("Invalid JPEG SOI marker"));
    }
    let mut info = FormatInfo::new("jpeg");
    let (mut segments, mut saw_sof, mut saw_sos) = (0usize, false, false);
    loop {
        let mut byte = c.byte()?;
        if byte != 0xff {
            return Err(malformed("Expected a JPEG marker"));
        }
        while byte == 0xff {
            byte = c.byte()?; // consume fill bytes before the marker code
        }
        let marker = byte;
        if marker == 0xd8 {
            return Err(malformed("Unexpected second JPEG SOI"));
        }
        if marker == 0xd9 {
            if c.remaining() != 0 {
                return Err(malformed("Trailing bytes after JPEG EOI"));
            }
            break;
        }
        if standalone(marker) {
            continue;
        }
        let length = c.be_u16()? as usize;
        if length < 2 {
            return Err(malformed("Invalid JPEG segment length"));
        }
        let payload = c.take(length - 2)?;
        segments += 1;
        if is_sof(marker) {
            if saw_sof {
                return Err(malformed("Multiple JPEG start-of-frame markers"));
            }
            if payload.len() < 6 {
                return Err(malformed("Truncated JPEG frame header"));
            }
            let height = u16::from_be_bytes([payload[1], payload[2]]);
            let width = u16::from_be_bytes([payload[3], payload[4]]);
            let components = payload[5];
            if width == 0 || height == 0 || !matches!(components, 1 | 3 | 4) {
                return Err(malformed("Invalid JPEG dimensions or component count"));
            }
            if payload.len() != 6 + components as usize * 3 {
                return Err(malformed(
                    "JPEG frame header does not match component count",
                ));
            }
            info.metadata.insert("width".into(), json!(width));
            info.metadata.insert("height".into(), json!(height));
            info.metadata.insert("components".into(), json!(components));
            saw_sof = true;
        }
        if marker == 0xda {
            if !saw_sof {
                return Err(malformed("JPEG scan precedes its frame header"));
            }
            saw_sos = true;
            let next = skip_scan(data, c.position())?;
            c.take(next - c.position())?;
        }
    }
    if !saw_sof || !saw_sos {
        return Err(malformed("JPEG is missing a frame header or scan"));
    }
    info.metadata.insert("segments".into(), json!(segments));
    info.validation.warnings.push(
        "JPEG carriers are validated structurally; entropy-coded pixel data is not decoded".into(),
    );
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skip_scan_passes_stuffing_and_restart_markers() {
        // entropy bytes, a stuffed FF00, a restart FFD0, then the real EOI FFD9
        let data = [0x12, 0xff, 0x00, 0x34, 0xff, 0xd0, 0x56, 0xff, 0xd9];
        assert_eq!(skip_scan(&data, 0).unwrap(), 7);
    }
}
