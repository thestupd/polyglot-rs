use crate::{handler, malformed};
use polyglot_core::{FormatInfo, Limits, Result, ValidationResult};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub struct MpegTs;
impl polyglot_core::AppendCapability for MpegTs {
    fn permits_prefix(&self) -> bool {
        true
    }
    fn permits_suffix(&self) -> bool {
        true
    }
    fn alignment(&self) -> usize {
        PACKET
    }
}
const PACKET: usize = 188;
handler!(
    MpegTs,
    "mpeg-ts",
    |data: &[u8]| data.len() >= PACKET * 3 && (0..3).all(|i| data[i * PACKET] == 0x47),
    parse
);

/// ISO/IEC 13818-1 CRC (no reflection, no final xor).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[derive(Default)]
struct Section {
    bytes: Vec<u8>,
}

impl Section {
    // PSI sections are bounded to 1024 bytes regardless of advertised lengths.
    fn feed(&mut self, mut payload: &[u8], start: bool) -> Result<Vec<Vec<u8>>> {
        let mut sections = vec![];
        if start {
            let (&pointer, rest) = payload
                .split_first()
                .ok_or_else(|| malformed("Missing PSI pointer field"))?;
            let pointer = pointer as usize;
            let prefix = rest
                .get(..pointer)
                .ok_or_else(|| malformed("PSI pointer exceeds payload"))?;
            if !self.bytes.is_empty() {
                self.consume(prefix, &mut sections, false)?;
                if !self.bytes.is_empty() {
                    return Err(malformed("Incomplete PSI section before new section start"));
                }
            }
            payload = &rest[pointer..];
        } else if self.bytes.is_empty() {
            return Ok(sections); // capture may begin mid-section
        }
        self.consume(payload, &mut sections, start)?;
        Ok(sections)
    }

    fn consume(
        &mut self,
        mut bytes: &[u8],
        out: &mut Vec<Vec<u8>>,
        mut may_start: bool,
    ) -> Result<()> {
        while !bytes.is_empty() {
            if self.bytes.is_empty() && (!may_start || bytes[0] == 0xff) {
                return Ok(());
            }
            let needed = if self.bytes.len() < 3 {
                3 - self.bytes.len()
            } else {
                let size = 3 + ((((self.bytes[1] & 15) as usize) << 8) | self.bytes[2] as usize);
                if !(7..=1024).contains(&size) {
                    return Err(malformed("Invalid PSI section length"));
                }
                size - self.bytes.len()
            };
            let count = needed.min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.bytes.len() >= 3 {
                let size = 3 + ((((self.bytes[1] & 15) as usize) << 8) | self.bytes[2] as usize);
                if !(7..=1024).contains(&size) {
                    return Err(malformed("Invalid PSI section length"));
                }
                if self.bytes.len() == size {
                    out.push(std::mem::take(&mut self.bytes));
                    may_start = true;
                }
            }
        }
        Ok(())
    }
}

fn descriptors(data: &[u8]) -> Result<()> {
    let mut i = 0;
    while i < data.len() {
        let size = *data
            .get(i + 1)
            .ok_or_else(|| malformed("Truncated MPEG-TS descriptor"))? as usize;
        i += size + 2;
        if i > data.len() {
            return Err(malformed("Descriptor length exceeds table"));
        }
    }
    Ok(())
}

fn table(
    section: &[u8],
    pid: u16,
    programs: &mut BTreeMap<u16, u16>,
    streams: &mut BTreeSet<u16>,
) -> Result<()> {
    if section.len() < 12
        || section[1] & 0xf0 != 0xb0
        || section[5] & 0xc0 != 0xc0
        || section[6] > section[7]
        || crc32(section) != 0
    {
        return Err(malformed("Invalid PSI syntax, section numbering or CRC"));
    }
    if section[5] & 1 == 0 {
        return Ok(());
    } // next table, not currently applicable
    let end = section.len() - 4;
    if pid == 0 {
        if section[0] != 0 || !(end - 8).is_multiple_of(4) {
            return Err(malformed("Invalid PAT layout"));
        }
        for entry in section[8..end].as_chunks::<4>().0 {
            if entry[2] & 0xe0 != 0xe0 {
                return Err(malformed("Invalid PAT reserved bits"));
            }
            let program = u16::from_be_bytes([entry[0], entry[1]]);
            let target = (((entry[2] & 31) as u16) << 8) | entry[3] as u16;
            if program != 0 {
                if !(0x10..0x1fff).contains(&target) {
                    return Err(malformed("Invalid PAT program map PID"));
                }
                programs.insert(program, target);
            }
        }
    } else {
        if section[0] != 2 || end < 12 || section[8] & 0xe0 != 0xe0 || section[10] & 0xf0 != 0xf0 {
            return Err(malformed("Invalid PMT layout"));
        }
        let program = u16::from_be_bytes([section[3], section[4]]);
        if programs.get(&program) != Some(&pid) {
            return Err(malformed("PMT program does not match PAT"));
        }
        let info_len = (((section[10] & 15) as usize) << 8) | section[11] as usize;
        let mut offset = 12 + info_len;
        if offset > end {
            return Err(malformed("PMT program descriptors exceed section"));
        }
        descriptors(&section[12..offset])?;
        while offset < end {
            let entry = section
                .get(offset..offset + 5)
                .ok_or_else(|| malformed("Truncated PMT stream"))?;
            if offset + 5 > end || entry[1] & 0xe0 != 0xe0 || entry[3] & 0xf0 != 0xf0 {
                return Err(malformed("Invalid PMT stream header"));
            }
            let stream_pid = (((entry[1] & 31) as u16) << 8) | entry[2] as u16;
            if !(0x10..0x1fff).contains(&stream_pid) {
                return Err(malformed("Invalid elementary stream PID"));
            }
            streams.insert(stream_pid);
            let length = (((entry[3] & 15) as usize) << 8) | entry[4] as usize;
            let next = offset + 5 + length;
            if next > end {
                return Err(malformed("PMT stream descriptors exceed section"));
            }
            descriptors(&section[offset + 5..next])?;
            offset = next;
        }
    }
    Ok(())
}

fn adaptation(packet: &[u8], control: u8) -> Result<(usize, bool)> {
    if control & 2 == 0 {
        return Ok((4, false));
    }
    let size = packet[4] as usize;
    let end = 5 + size;
    if end > PACKET || (control == 2 && end != PACKET) || (control == 3 && end == PACKET) {
        return Err(malformed("Invalid MPEG-TS adaptation field length"));
    }
    if size == 0 {
        return Ok((end, false));
    }
    let flags = packet[5];
    let mut offset = 6;
    for flag in [0x10, 0x08] {
        if flags & flag != 0 {
            offset += 6; // PCR / OPCR
            if offset > end {
                return Err(malformed("Truncated PCR/OPCR"));
            }
            let clock = &packet[offset - 6..offset];
            let extension = (((clock[4] & 1) as u16) << 8) | clock[5] as u16;
            if clock[4] & 0x7e != 0x7e || extension >= 300 {
                return Err(malformed("Invalid PCR/OPCR encoding"));
            }
        }
    }
    if flags & 4 != 0 {
        offset += 1;
    }
    for flag in [2, 1] {
        if flags & flag != 0 {
            if offset >= end {
                return Err(malformed("Truncated adaptation subfield"));
            }
            offset += 1 + packet[offset] as usize;
        }
    }
    if offset > end || packet[offset..end].iter().any(|&b| b != 0xff) {
        return Err(malformed("Invalid adaptation subfield length or stuffing"));
    }
    Ok((end, flags & 0x80 != 0))
}

fn parse(data: &[u8], _: Limits) -> Result<FormatInfo> {
    if data.is_empty() || !data.len().is_multiple_of(PACKET) {
        return Err(malformed("MPEG-TS must contain complete 188-byte packets"));
    }
    let mut previous: BTreeMap<u16, (u8, usize, bool)> = BTreeMap::new();
    let mut sections: BTreeMap<u16, Section> = BTreeMap::new();
    let mut programs = BTreeMap::new();
    let mut streams = BTreeSet::new();
    let mut pids = BTreeSet::new();
    let (mut pat, mut pmt, mut scrambled) = (false, BTreeSet::new(), false);
    for (index, packet) in data.as_chunks::<PACKET>().0.iter().enumerate() {
        if packet[0] != 0x47 || packet[1] & 0x80 != 0 {
            return Err(malformed(format!(
                "Packet {index}: sync byte or transport error"
            )));
        }
        let pid = (((packet[1] & 31) as u16) << 8) | packet[2] as u16;
        pids.insert(pid);
        let control = (packet[3] >> 4) & 3;
        if control == 0 || packet[3] >> 6 == 1 {
            return Err(malformed("Reserved transport control value"));
        }
        let encrypted = packet[3] >> 6 != 0;
        scrambled |= encrypted;
        let (offset, discontinuity) = adaptation(packet, control)?;
        let counter = packet[3] & 15;
        let has_payload = control & 1 != 0;
        if discontinuity {
            sections.remove(&pid);
        }
        let mut duplicate = false;
        if pid != 0x1fff {
            if let Some(&(last, prev_index, repeated)) = previous.get(&pid) {
                let expected = if has_payload { (last + 1) & 15 } else { last };
                if !discontinuity && counter != expected {
                    duplicate = has_payload
                        && counter == last
                        && !repeated
                        && packet == &data[prev_index * PACKET..(prev_index + 1) * PACKET];
                    if !duplicate {
                        return Err(malformed(format!(
                            "Packet {index}: continuity counter mismatch on PID {pid}"
                        )));
                    }
                }
            }
            previous.insert(pid, (counter, index, duplicate));
        }
        if !has_payload || duplicate || encrypted {
            continue;
        }
        if pid == 0 || programs.values().any(|&map_pid| map_pid == pid) {
            for section in sections
                .entry(pid)
                .or_default()
                .feed(&packet[offset..], packet[1] & 0x40 != 0)?
            {
                table(&section, pid, &mut programs, &mut streams)?;
                if pid == 0 {
                    pat = true;
                } else {
                    pmt.insert(pid);
                }
            }
        }
    }
    let mut info = FormatInfo::new("mpeg-ts");
    if !pat {
        info.validation.warnings.push(
            "No complete PAT observed; this may be a partial capture or null-only stream".into(),
        );
    }
    if programs.values().any(|pid| !pmt.contains(pid)) {
        info.validation
            .warnings
            .push("Not every PAT program has a complete PMT in this capture".into());
    }
    if sections.values().any(|s| !s.bytes.is_empty()) {
        info.validation
            .warnings
            .push("Capture ends with an incomplete PSI section".into());
    }
    if scrambled {
        info.validation
            .warnings
            .push("Scrambled payloads were not inspected".into());
    }
    info.validation.warnings.push("Transport structure validation only: PES, codec decoding, timing and table-version transitions are not validated".into());
    info.metadata.insert("packetSize".into(), json!(PACKET));
    info.metadata
        .insert("packets".into(), json!(data.len() / PACKET));
    info.metadata.insert("pids".into(), json!(pids));
    info.metadata.insert(
        "programs".into(),
        json!(programs.keys().collect::<Vec<_>>()),
    );
    info.metadata.insert("streamPids".into(), json!(streams));
    info.metadata.insert("hasPat".into(), json!(pat));
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_known_check_value() {
        assert_eq!(crc32(b"123456789"), 0x0376_e6e7);
    }
    #[test]
    fn section_reassembles_and_checks_pointer() {
        let mut s = Section::default();
        assert!(s.feed(&[0, 0, 0xb0, 9, 1], true).unwrap().is_empty());
        let result = s.feed(&[2, 3, 4, 5, 6, 7, 8, 9], false).unwrap();
        assert_eq!(result[0].len(), 12);
        assert!(s.feed(&[255], true).is_err());
    }
}
