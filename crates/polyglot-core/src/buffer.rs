use crate::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_input_bytes: usize,
    /// Per operation, summed across GIF frames / PNG animation frames.
    pub max_decoded_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_decoded_bytes: 64 * 1024 * 1024,
        }
    }
}

impl Limits {
    pub fn check_input(self, size: usize) -> Result<()> {
        if self.max_input_bytes == 0 || self.max_decoded_bytes == 0 {
            return Err(Error::InvalidInput("Limits must be positive".into()));
        }
        if size > self.max_input_bytes {
            return Err(Error::LimitExceeded("Input exceeds maxInputBytes".into()));
        }
        Ok(())
    }
}

/// Borrowed cursor. All offsets and lengths are checked before slicing.
pub struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }
    pub fn position(&self) -> usize {
        self.offset
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }
    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| Error::Malformed("Offset overflow".into()))?;
        let bytes = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| Error::Malformed(format!("Truncated data at byte {}", self.offset)))?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn be_u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn le_u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn be_u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_rejects_overflow_and_truncation_without_advancing() {
        let mut c = Cursor::new(&[1, 2]);
        assert_eq!(c.byte().unwrap(), 1);
        assert!(c.take(usize::MAX).is_err());
        assert!(c.take(2).is_err());
        assert_eq!(c.position(), 1);
        assert_eq!(c.byte().unwrap(), 2);
    }
}
