//! Shared byte-level parsing primitives.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteError {
    Truncated,
}

impl fmt::Display for ByteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ByteError::Truncated => f.write_str("buffer is truncated"),
        }
    }
}

impl std::error::Error for ByteError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    #[must_use]
    pub fn position(&self) -> usize {
        self.offset
    }

    pub fn skip(&mut self, count: usize) -> Result<(), ByteError> {
        self.read_bytes(count).map(|_| ())
    }

    pub fn read_u8(&mut self) -> Result<u8, ByteError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    pub fn read_u16_be(&mut self) -> Result<u16, ByteError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32_be(&mut self) -> Result<u32, ByteError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], ByteError> {
        let end = self.offset.checked_add(count).ok_or(ByteError::Truncated)?;
        let slice = self.bytes.get(self.offset..end).ok_or(ByteError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N], ByteError> {
        let bytes = self.read_bytes(N)?;
        let mut array = [0u8; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }
}
