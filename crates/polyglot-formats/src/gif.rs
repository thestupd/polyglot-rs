use crate::{handler, malformed};
use polyglot_core::{Cursor, Error, FormatInfo, Limits, Result, ValidationResult};
use serde_json::json;
use std::{io::Cursor as IoCursor, num::NonZeroU64};

pub struct Gif;
impl polyglot_core::AppendCapability for Gif {
    fn permits_prefix(&self) -> bool {
        false
    }
    fn permits_suffix(&self) -> bool {
        false
    }
}
handler!(
    Gif,
    "gif",
    |data: &[u8]| data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
    parse
);

fn sub_blocks(cursor: &mut Cursor<'_>) -> Result<()> {
    loop {
        let len = cursor.byte()? as usize;
        if len == 0 {
            return Ok(());
        }
        cursor.take(len)?;
    }
}

fn parse(data: &[u8], limits: Limits) -> Result<FormatInfo> {
    let mut c = Cursor::new(data);
    let version = c.take(6)?;
    if version != b"GIF87a" && version != b"GIF89a" {
        return Err(malformed("Invalid GIF signature"));
    }
    let width = c.le_u16()?;
    let height = c.le_u16()?;
    if width == 0 || height == 0 {
        return Err(malformed("GIF screen dimensions must be nonzero"));
    }
    let packed = c.byte()?;
    let background = c.byte()?;
    c.byte()?; // pixel aspect ratio
    let global_colors = if packed & 0x80 != 0 {
        1usize << ((packed & 7) + 1)
    } else {
        0
    };
    if global_colors > 0 {
        if background as usize >= global_colors {
            return Err(malformed("GIF background index exceeds palette"));
        }
        c.take(global_colors * 3)?;
    }
    let (mut frames, mut decoded) = (0usize, 0usize);
    loop {
        match c.byte()? {
            0x3b => {
                if c.remaining() != 0 {
                    return Err(malformed("Trailing bytes after GIF trailer"));
                }
                break;
            }
            0x2c => {
                let left = c.le_u16()? as u32;
                let top = c.le_u16()? as u32;
                let w = c.le_u16()? as u32;
                let h = c.le_u16()? as u32;
                let flags = c.byte()?;
                if w == 0
                    || h == 0
                    || left + w > width as u32
                    || top + h > height as u32
                    || flags & 0x18 != 0
                {
                    return Err(malformed("Invalid GIF frame dimensions or reserved bits"));
                }
                let colors = if flags & 0x80 != 0 {
                    let count = 1usize << ((flags & 7) + 1);
                    c.take(count * 3)?;
                    count
                } else {
                    global_colors
                };
                if colors == 0 {
                    return Err(malformed("GIF frame has no color table"));
                }
                if !(2..=8).contains(&c.byte()?) {
                    return Err(malformed("Invalid GIF LZW minimum code size"));
                }
                sub_blocks(&mut c)?;
                decoded =
                    decoded
                        .checked_add((w as usize).checked_mul(h as usize).ok_or_else(|| {
                            Error::LimitExceeded("GIF frame size overflow".into())
                        })?)
                        .ok_or_else(|| Error::LimitExceeded("GIF decoded size overflow".into()))?;
                if decoded > limits.max_decoded_bytes {
                    return Err(Error::LimitExceeded("GIF exceeds maxDecodedBytes".into()));
                }
                frames += 1;
            }
            0x21 => {
                if version == b"GIF87a" {
                    return Err(malformed("GIF87a cannot contain GIF89a extensions"));
                }
                match c.byte()? {
                    0xf9 => {
                        if c.byte()? != 4 {
                            return Err(malformed("Invalid graphic control block size"));
                        }
                        let control = c.take(4)?;
                        if control[0] & 0xe0 != 0 || ((control[0] >> 2) & 7) > 3 || c.byte()? != 0 {
                            return Err(malformed("Invalid GIF graphic control extension"));
                        }
                    }
                    0xff => {
                        if c.byte()? != 11 {
                            return Err(malformed("Invalid GIF application extension size"));
                        }
                        c.take(11)?;
                        sub_blocks(&mut c)?;
                    }
                    0x01 => {
                        if c.byte()? != 12 || global_colors == 0 {
                            return Err(malformed("Invalid GIF plain text extension"));
                        }
                        c.take(12)?;
                        sub_blocks(&mut c)?;
                    }
                    0xfe => sub_blocks(&mut c)?,
                    _ => return Err(malformed("Unknown GIF extension")),
                }
            }
            _ => return Err(malformed("Invalid GIF block introducer")),
        }
    }
    // Streams without raster frames (e.g. plain text) are outside this handler.
    if frames == 0 {
        return Err(malformed("GIF contains no raster frames"));
    }
    let mut options = ::gif::DecodeOptions::new();
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    options.set_memory_limit(::gif::MemoryLimit::Bytes(
        NonZeroU64::new(limits.max_decoded_bytes as u64)
            .ok_or_else(|| Error::InvalidInput("Invalid decoded byte limit".into()))?,
    ));
    let mut decoder = options
        .read_info(IoCursor::new(data))
        .map_err(|e| malformed(format!("GIF decoder: {e}")))?;
    let mut decoded_frames = 0;
    while let Some(frame) = decoder
        .read_next_frame()
        .map_err(|e| malformed(format!("GIF decoder: {e}")))?
    {
        if frame.buffer.len() != frame.width as usize * frame.height as usize {
            return Err(malformed(
                "GIF decoded pixel count does not match frame dimensions",
            ));
        }
        // gif validates palette availability; explicitly check decoded indices.
        let colors = frame
            .palette
            .as_ref()
            .map(|p| p.len() / 3)
            .unwrap_or(global_colors);
        if frame.buffer.iter().any(|&index| index as usize >= colors) {
            return Err(malformed("GIF decoded palette index out of range"));
        }
        decoded_frames += 1;
    }
    if decoded_frames != frames {
        return Err(malformed("GIF decoder frame count mismatch"));
    }
    let mut info = FormatInfo::new("gif");
    info.metadata.insert("width".into(), json!(width));
    info.metadata.insert("height".into(), json!(height));
    info.metadata.insert("frames".into(), json!(frames));
    info.metadata.insert(
        "version".into(),
        json!(if version == b"GIF87a" { "87a" } else { "89a" }),
    );
    info.validation
        .warnings
        .push("Plain text rendering and application extension semantics are not validated".into());
    Ok(info)
}
