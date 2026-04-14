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
pub struct OwnedSfntTable {
    pub tag: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSfntFont {
    version_tag: u32,
    tables: Vec<OwnedSfntTable>,
}

impl OwnedSfntFont {
    #[must_use]
    pub fn new(version_tag: u32) -> Self {
        Self {
            version_tag,
            tables: Vec::new(),
        }
    }

    #[must_use]
    pub fn version_tag(&self) -> u32 {
        self.version_tag
    }

    #[must_use]
    pub fn tables(&self) -> &[OwnedSfntTable] {
        &self.tables
    }

    #[must_use]
    pub fn table(&self, tag: u32) -> Option<&OwnedSfntTable> {
        self.tables.iter().find(|table| table.tag == tag)
    }

    pub fn add_table(&mut self, tag: u32, data: Vec<u8>) {
        if let Some(table) = self.tables.iter_mut().find(|table| table.tag == tag) {
            table.data = data;
            return;
        }

        self.tables.push(OwnedSfntTable { tag, data });
    }

    pub fn remove_table(&mut self, tag: u32) -> Option<OwnedSfntTable> {
        let index = self.tables.iter().position(|table| table.tag == tag)?;
        Some(self.tables.remove(index))
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializeError {
    EmptyFont,
    TooManyTables,
    FontTooLarge,
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializeError::EmptyFont => f.write_str("sfnt must contain at least one table"),
            SerializeError::TooManyTables => f.write_str("sfnt contains too many tables"),
            SerializeError::FontTooLarge => f.write_str("sfnt output is too large"),
        }
    }
}

impl std::error::Error for SerializeError {}

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

#[must_use]
pub fn load_sfnt(bytes: &[u8]) -> Result<OwnedSfntFont, ParseError> {
    let parsed = parse_sfnt(bytes)?;
    let mut font = OwnedSfntFont::new(parsed.version_tag());

    for record in parsed.table_directory().entries() {
        let start = record.offset as usize;
        let end = start + record.length as usize;
        let data = if record.length == 0 {
            Vec::new()
        } else {
            bytes[start..end].to_vec()
        };
        font.add_table(record.tag, data);
    }

    Ok(font)
}

#[must_use]
pub fn serialize_sfnt(font: &OwnedSfntFont) -> Result<Vec<u8>, SerializeError> {
    if font.tables.is_empty() {
        return Err(SerializeError::EmptyFont);
    }

    let num_tables = u16::try_from(font.tables.len()).map_err(|_| SerializeError::TooManyTables)?;
    let mut tables = font.tables.clone();
    tables.sort_by_key(|table| table.tag);

    let header_size = SFNT_HEADER_SIZE
        .checked_add(
            tables
                .len()
                .checked_mul(SFNT_TABLE_RECORD_SIZE)
                .ok_or(SerializeError::FontTooLarge)?,
        )
        .ok_or(SerializeError::FontTooLarge)?;

    let mut total_size = header_size;
    for table in &tables {
        if !table.data.is_empty() {
            total_size = total_size
                .checked_add(align4(table.data.len()))
                .ok_or(SerializeError::FontTooLarge)?;
        }
    }

    let mut output = vec![0u8; total_size];
    output[..4].copy_from_slice(&font.version_tag.to_be_bytes());
    output[4..6].copy_from_slice(&num_tables.to_be_bytes());

    let search_range_shift = highest_bit(num_tables as usize);
    let search_range = 1usize << search_range_shift;
    output[6..8].copy_from_slice(&((search_range * 16) as u16).to_be_bytes());
    output[8..10].copy_from_slice(&(search_range_shift as u16).to_be_bytes());
    output[10..12]
        .copy_from_slice(&(((num_tables as usize - search_range) * 16) as u16).to_be_bytes());

    let mut data_offset = header_size;
    let mut head_entry_offset = None;
    let mut head_table_offset = 0usize;
    let mut head_table_length = 0usize;

    for (index, table) in tables.iter().enumerate() {
        let entry_offset = SFNT_HEADER_SIZE + index * SFNT_TABLE_RECORD_SIZE;
        output[entry_offset..entry_offset + 4].copy_from_slice(&table.tag.to_be_bytes());

        if table.data.is_empty() {
            continue;
        }

        let checksum = calc_checksum(&table.data);
        output[entry_offset + 4..entry_offset + 8].copy_from_slice(&checksum.to_be_bytes());
        output[entry_offset + 8..entry_offset + 12].copy_from_slice(
            &(u32::try_from(data_offset).map_err(|_| SerializeError::FontTooLarge)?).to_be_bytes(),
        );
        output[entry_offset + 12..entry_offset + 16].copy_from_slice(
            &(u32::try_from(table.data.len()).map_err(|_| SerializeError::FontTooLarge)?)
                .to_be_bytes(),
        );

        output[data_offset..data_offset + table.data.len()].copy_from_slice(&table.data);

        if table.tag == u32::from_be_bytes(*b"head") && table.data.len() >= 12 {
            head_entry_offset = Some(entry_offset);
            head_table_offset = data_offset;
            head_table_length = table.data.len();
            output[head_table_offset + 8..head_table_offset + 12]
                .copy_from_slice(&0u32.to_be_bytes());
            let head_checksum =
                calc_checksum(&output[head_table_offset..head_table_offset + head_table_length]);
            output[entry_offset + 4..entry_offset + 8]
                .copy_from_slice(&head_checksum.to_be_bytes());
        }

        data_offset += align4(table.data.len());
    }

    if let Some(entry_offset) = head_entry_offset {
        let head_checksum =
            calc_checksum(&output[head_table_offset..head_table_offset + head_table_length]);
        output[entry_offset + 4..entry_offset + 8].copy_from_slice(&head_checksum.to_be_bytes());

        let font_checksum = calc_checksum(&output);
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(font_checksum);
        output[head_table_offset + 8..head_table_offset + 12]
            .copy_from_slice(&adjustment.to_be_bytes());
    }

    Ok(output)
}

fn calc_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let padded_len = align4(data.len());

    for chunk_start in (0..padded_len).step_by(4) {
        let mut value = 0u32;
        for offset in 0..4 {
            value <<= 8;
            value |= u32::from(*data.get(chunk_start + offset).unwrap_or(&0));
        }
        sum = sum.wrapping_add(value);
    }

    sum
}

fn align4(length: usize) -> usize {
    (length + 3) & !3
}

fn highest_bit(value: usize) -> usize {
    let mut result = 0usize;
    let mut current = value;
    while current > 1 {
        current >>= 1;
        result += 1;
    }
    result
}
