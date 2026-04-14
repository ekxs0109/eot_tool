//! Shared SFNT models and table directory logic.

use core::fmt;

use fonttool_bytes::{ByteError, ByteReader};

pub const SFNT_VERSION_TRUETYPE: u32 = 0x0001_0000;
pub const SFNT_VERSION_OTTO: u32 = u32::from_be_bytes(*b"OTTO");
const SFNT_HEADER_SIZE: usize = 12;
const SFNT_TABLE_RECORD_SIZE: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfntFont {
    version_tag: u32,
    table_directory: SfntTableDirectory,
}

impl SfntFont {
    #[must_use]
    pub fn version_tag(&self) -> u32 {
        self.version_tag
    }

    #[must_use]
    pub fn table_directory(&self) -> &SfntTableDirectory {
        &self.table_directory
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfntTableDirectory {
    records: Vec<SfntTableRecord>,
}

impl SfntTableDirectory {
    #[must_use]
    pub fn new(records: Vec<SfntTableRecord>) -> Self {
        Self { records }
    }

    #[must_use]
    pub fn entries(&self) -> &[SfntTableRecord] {
        &self.records
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfntTableRecord {
    pub tag: u32,
    pub checksum: u32,
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TruncatedHeader,
    InvalidVersionTag(u32),
    TruncatedDirectory,
    InvalidTableRange { tag: u32 },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::TruncatedHeader => f.write_str("sfnt header is truncated"),
            ParseError::InvalidVersionTag(tag) => {
                write!(f, "unsupported sfnt version tag: 0x{tag:08x}")
            }
            ParseError::TruncatedDirectory => f.write_str("sfnt table directory is truncated"),
            ParseError::InvalidTableRange { tag } => {
                write!(f, "table range is invalid for tag 0x{tag:08x}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

#[must_use]
pub fn parse_sfnt(bytes: &[u8]) -> Result<SfntFont, ParseError> {
    let mut reader = ByteReader::new(bytes);

    if bytes.len() < SFNT_HEADER_SIZE {
        return Err(ParseError::TruncatedHeader);
    }

    let version_tag = reader
        .read_u32_be()
        .map_err(|_: ByteError| ParseError::TruncatedHeader)?;
    if version_tag != SFNT_VERSION_TRUETYPE && version_tag != SFNT_VERSION_OTTO {
        return Err(ParseError::InvalidVersionTag(version_tag));
    }

    let num_tables = reader
        .read_u16_be()
        .map_err(|_: ByteError| ParseError::TruncatedHeader)? as usize;
    reader
        .skip(6)
        .map_err(|_: ByteError| ParseError::TruncatedHeader)?;

    let directory_len = num_tables
        .checked_mul(SFNT_TABLE_RECORD_SIZE)
        .and_then(|size| SFNT_HEADER_SIZE.checked_add(size))
        .ok_or(ParseError::TruncatedDirectory)?;

    if bytes.len() < directory_len {
        return Err(ParseError::TruncatedDirectory);
    }

    let mut table_directory = Vec::with_capacity(num_tables);
    for _ in 0..num_tables {
        let tag = reader
            .read_u32_be()
            .map_err(|_: ByteError| ParseError::TruncatedDirectory)?;
        let checksum = reader
            .read_u32_be()
            .map_err(|_: ByteError| ParseError::TruncatedDirectory)?;
        let offset = reader
            .read_u32_be()
            .map_err(|_: ByteError| ParseError::TruncatedDirectory)?;
        let length = reader
            .read_u32_be()
            .map_err(|_: ByteError| ParseError::TruncatedDirectory)?;

        if length > 0 && (offset as usize) < directory_len {
            return Err(ParseError::InvalidTableRange { tag });
        }

        let end = offset
            .checked_add(length)
            .ok_or(ParseError::InvalidTableRange { tag })?;
        if end as usize > bytes.len() {
            return Err(ParseError::InvalidTableRange { tag });
        }

        table_directory.push(SfntTableRecord {
            tag,
            checksum,
            offset,
            length,
        });
    }

    Ok(SfntFont {
        version_tag,
        table_directory: SfntTableDirectory::new(table_directory),
    })
}
