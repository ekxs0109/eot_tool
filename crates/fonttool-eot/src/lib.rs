//! EOT container parsing and header modeling.

use core::convert::TryFrom;
use core::fmt;

pub const EOT_FIXED_HEADER_SIZE: usize = 82;
const EOT_VERSION_20002: u32 = 0x0002_0002;
const EOT_MAGIC_NUMBER: u16 = 0x504c;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EotHeaderError {
    Truncated,
    InvalidMagic,
    InvalidPadding { field: &'static str },
    InvalidStringLength { field: &'static str },
    InvalidSizeMetadata,
}

impl fmt::Display for EotHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EotHeaderError::Truncated => f.write_str("EOT header is truncated"),
            EotHeaderError::InvalidMagic => f.write_str("invalid EOT magic number"),
            EotHeaderError::InvalidPadding { field } => {
                write!(f, "invalid EOT padding in {field}")
            }
            EotHeaderError::InvalidStringLength { field } => {
                write!(f, "invalid EOT string length in {field}")
            }
            EotHeaderError::InvalidSizeMetadata => f.write_str("invalid EOT size metadata"),
        }
    }
}

impl std::error::Error for EotHeaderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EotHeader<'a> {
    pub eot_size: u32,
    pub font_data_size: u32,
    pub version: u32,
    pub flags: u32,
    pub panose: [u8; 10],
    pub charset: u8,
    pub italic: u8,
    pub weight: u32,
    pub fs_type: u16,
    pub magic_number: u16,
    pub unicode_range: [u32; 4],
    pub code_page_range: [u32; 2],
    pub check_sum_adjustment: u32,
    pub reserved: [u32; 4],
    pub padding1: u16,
    pub family_name: &'a [u8],
    pub padding2: u16,
    pub style_name: &'a [u8],
    pub padding3: u16,
    pub version_name: &'a [u8],
    pub padding4: u16,
    pub full_name: &'a [u8],
    pub padding5: u16,
    pub root_string: &'a [u8],
    pub root_string_checksum: u32,
    pub eudc_code_page: u32,
    pub padding6: u16,
    pub signature_size: u16,
    pub signature: &'a [u8],
    pub eudc_flags: u32,
    pub eudc_font_size: u32,
    pub header_length: u32,
}

#[must_use]
pub fn parse_eot_header<'a>(bytes: &'a [u8]) -> Result<EotHeader<'a>, EotHeaderError> {
    if bytes.len() < EOT_FIXED_HEADER_SIZE {
        return Err(EotHeaderError::Truncated);
    }

    let mut offset = 0usize;
    let eot_size = read_u32_le(bytes, &mut offset)?;
    let font_data_size = read_u32_le(bytes, &mut offset)?;
    let version = read_u32_le(bytes, &mut offset)?;
    let flags = read_u32_le(bytes, &mut offset)?;
    let panose = read_array(bytes, &mut offset)?;
    let charset = read_u8(bytes, &mut offset)?;
    let italic = read_u8(bytes, &mut offset)?;
    let weight = read_u32_le(bytes, &mut offset)?;
    let fs_type = read_u16_le(bytes, &mut offset)?;
    let magic_number = read_u16_le(bytes, &mut offset)?;
    let unicode_range = read_u32_array::<4>(bytes, &mut offset)?;
    let code_page_range = read_u32_array::<2>(bytes, &mut offset)?;
    let check_sum_adjustment = read_u32_le(bytes, &mut offset)?;
    let reserved = read_u32_array::<4>(bytes, &mut offset)?;
    let padding1 = read_u16_le(bytes, &mut offset)?;

    if magic_number != EOT_MAGIC_NUMBER {
        return Err(EotHeaderError::InvalidMagic);
    }

    let family_name = read_length_prefixed_bytes(bytes, &mut offset, "family_name")?;
    let padding2 = read_zero_padding(bytes, &mut offset, "padding2")?;
    let style_name = read_length_prefixed_bytes(bytes, &mut offset, "style_name")?;
    let padding3 = read_zero_padding(bytes, &mut offset, "padding3")?;
    let version_name = read_length_prefixed_bytes(bytes, &mut offset, "version_name")?;
    let padding4 = read_zero_padding(bytes, &mut offset, "padding4")?;
    let full_name = read_length_prefixed_bytes(bytes, &mut offset, "full_name")?;
    let padding5 = read_zero_padding(bytes, &mut offset, "padding5")?;
    let root_string = read_length_prefixed_bytes(bytes, &mut offset, "root_string")?;

    let mut root_string_checksum = 0u32;
    let mut eudc_code_page = 0u32;
    let mut padding6 = 0u16;
    let mut signature_size = 0u16;
    let mut signature = &[][..];
    let mut eudc_flags = 0u32;
    let mut eudc_font_size = 0u32;

    if version == EOT_VERSION_20002 {
        root_string_checksum = read_u32_le(bytes, &mut offset)?;
        eudc_code_page = read_u32_le(bytes, &mut offset)?;
        padding6 = read_zero_padding(bytes, &mut offset, "padding6")?;
        signature_size = read_u16_le(bytes, &mut offset)?;
        signature = read_bytes(bytes, &mut offset, signature_size as usize)?;
        eudc_flags = read_u32_le(bytes, &mut offset)?;
        eudc_font_size = read_u32_le(bytes, &mut offset)?;
        let _ = read_bytes(bytes, &mut offset, eudc_font_size as usize)?;
    }

    let header_length = u32::try_from(offset).map_err(|_| EotHeaderError::InvalidSizeMetadata)?;

    if eot_size < font_data_size {
        return Err(EotHeaderError::InvalidSizeMetadata);
    }

    if header_length != eot_size - font_data_size {
        return Err(EotHeaderError::InvalidSizeMetadata);
    }

    let declared_eot_size =
        usize::try_from(eot_size).map_err(|_| EotHeaderError::InvalidSizeMetadata)?;
    if bytes.len() < declared_eot_size {
        return Err(EotHeaderError::InvalidSizeMetadata);
    }

    Ok(EotHeader {
        eot_size,
        font_data_size,
        version,
        flags,
        panose,
        charset,
        italic,
        weight,
        fs_type,
        magic_number,
        unicode_range,
        code_page_range,
        check_sum_adjustment,
        reserved,
        padding1,
        family_name,
        padding2,
        style_name,
        padding3,
        version_name,
        padding4,
        full_name,
        padding5,
        root_string,
        root_string_checksum,
        eudc_code_page,
        padding6,
        signature_size,
        signature,
        eudc_flags,
        eudc_font_size,
        header_length,
    })
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> Result<u8, EotHeaderError> {
    let value = *bytes.get(*offset).ok_or(EotHeaderError::Truncated)?;
    *offset += 1;
    Ok(value)
}

fn read_u16_le(bytes: &[u8], offset: &mut usize) -> Result<u16, EotHeaderError> {
    let bytes = read_bytes(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(bytes: &[u8], offset: &mut usize) -> Result<u32, EotHeaderError> {
    let bytes = read_bytes(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_array(bytes: &[u8], offset: &mut usize) -> Result<[u8; 10], EotHeaderError> {
    let bytes = read_bytes(bytes, offset, 10)?;
    let mut array = [0u8; 10];
    array.copy_from_slice(bytes);
    Ok(array)
}

fn read_u32_array<const N: usize>(
    bytes: &[u8],
    offset: &mut usize,
) -> Result<[u32; N], EotHeaderError> {
    let mut values = [0u32; N];
    let mut index = 0usize;

    while index < N {
        values[index] = read_u32_le(bytes, offset)?;
        index += 1;
    }

    Ok(values)
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    count: usize,
) -> Result<&'a [u8], EotHeaderError> {
    let end = offset.checked_add(count).ok_or(EotHeaderError::Truncated)?;
    let slice = bytes.get(*offset..end).ok_or(EotHeaderError::Truncated)?;
    *offset = end;
    Ok(slice)
}

fn read_length_prefixed_bytes<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    field: &'static str,
) -> Result<&'a [u8], EotHeaderError> {
    let size = read_u16_le(bytes, offset)?;
    if size & 1 != 0 {
        return Err(EotHeaderError::InvalidStringLength { field });
    }
    read_bytes(bytes, offset, size as usize)
}

fn read_zero_padding(
    bytes: &[u8],
    offset: &mut usize,
    field: &'static str,
) -> Result<u16, EotHeaderError> {
    let value = read_u16_le(bytes, offset)?;
    if value != 0 {
        return Err(EotHeaderError::InvalidPadding { field });
    }
    Ok(value)
}
