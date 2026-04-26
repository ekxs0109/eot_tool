//! EOT container parsing and header modeling.

use core::convert::TryFrom;
use core::fmt;

pub const EOT_FIXED_HEADER_SIZE: usize = 82;
const EOT_VERSION_20001: u32 = 0x0002_0001;
const EOT_VERSION_20002: u32 = 0x0002_0002;
const EOT_MAGIC_NUMBER: u16 = 0x504c;
const EOT_FLAG_COMPRESSED: u32 = 0x0000_0004;
const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const EOT_ROOT_STRING_XOR_KEY: u32 = 0x5047_5342;
const NAME_PLATFORM_WINDOWS: u16 = 3;
const NAME_ENCODING_UNICODE_BMP: u16 = 1;
const NAME_LANGUAGE_EN_US: u16 = 0x0409;
const NAME_LANGUAGE_JA_JP: u16 = 0x0411;
const NAME_LANGUAGE_KO_KR: u16 = 0x0412;
const NAME_LANGUAGE_ZH_TW: u16 = 0x0404;
const NAME_LANGUAGE_ZH_CN: u16 = 0x0804;
const NAME_ID_FAMILY: u16 = 1;
const NAME_ID_STYLE: u16 = 2;
const NAME_ID_FULL: u16 = 4;
const NAME_ID_VERSION: u16 = 5;
const EOT_CHARSET_ANSI: u8 = 0;
const EOT_CHARSET_DEFAULT: u8 = 1;
const EOT_CHARSET_SHIFTJIS: u8 = 128;
const EOT_CHARSET_HANGEUL: u8 = 129;
const EOT_CHARSET_JOHAB: u8 = 130;
const EOT_CHARSET_GB2312: u8 = 134;
const EOT_CHARSET_CHINESEBIG5: u8 = 136;
const EOT_CHARSET_GREEK: u8 = 161;
const EOT_CHARSET_TURKISH: u8 = 162;
const EOT_CHARSET_VIETNAMESE: u8 = 163;
const EOT_CHARSET_HEBREW: u8 = 177;
const EOT_CHARSET_ARABIC: u8 = 178;
const EOT_CHARSET_BALTIC: u8 = 186;
const EOT_CHARSET_RUSSIAN: u8 = 204;
const EOT_CHARSET_THAI: u8 = 222;
const EOT_CHARSET_EASTEUROPE: u8 = 238;

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
pub enum EotEncodeError {
    PayloadTooLarge,
}

impl fmt::Display for EotEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EotEncodeError::PayloadTooLarge => f.write_str("encoded EOT payload is too large"),
        }
    }
}

impl std::error::Error for EotEncodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EotVersion {
    V1,
    V2,
}

impl EotVersion {
    fn raw(self) -> u32 {
        match self {
            Self::V1 => EOT_VERSION_20001,
            Self::V2 => EOT_VERSION_20002,
        }
    }

    fn includes_v20002_trailer(self) -> bool {
        matches!(self, Self::V2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EotBuildOptions {
    pub version: EotVersion,
    pub apply_ppt_xor: bool,
}

impl From<bool> for EotBuildOptions {
    fn from(apply_ppt_xor: bool) -> Self {
        Self {
            version: EotVersion::V2,
            apply_ppt_xor,
        }
    }
}

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
pub fn build_eot_file(
    head_table: &[u8],
    os2_table: &[u8],
    name_table: &[u8],
    payload: &[u8],
    options: impl Into<EotBuildOptions>,
) -> Result<Vec<u8>, EotEncodeError> {
    let options = options.into();
    let charset = derive_eot_charset(os2_table);
    let family_name = extract_family_name_utf16le(name_table, charset);
    let style_name = extract_name_utf16le(name_table, NAME_ID_STYLE);
    let version_name = extract_name_utf16le(name_table, NAME_ID_VERSION);
    let full_name = extract_name_utf16le(name_table, NAME_ID_FULL);

    let mut payload = payload.to_vec();
    if options.apply_ppt_xor {
        for byte in &mut payload {
            *byte ^= 0x50;
        }
    }

    let header_len = EOT_FIXED_HEADER_SIZE
        .checked_add(2 + family_name.len())
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_add(2 + style_name.len()))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_add(2 + version_name.len()))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_add(2 + full_name.len()))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| {
            if options.version.includes_v20002_trailer() {
                value.checked_add(20)
            } else {
                Some(value)
            }
        })
        .ok_or(EotEncodeError::PayloadTooLarge)?;
    let total_size = header_len
        .checked_add(payload.len())
        .ok_or(EotEncodeError::PayloadTooLarge)?;

    let mut output = Vec::with_capacity(total_size);
    push_u32_le(
        &mut output,
        u32::try_from(total_size).map_err(|_| EotEncodeError::PayloadTooLarge)?,
    );
    push_u32_le(
        &mut output,
        u32::try_from(payload.len()).map_err(|_| EotEncodeError::PayloadTooLarge)?,
    );
    push_u32_le(&mut output, options.version.raw());
    push_u32_le(
        &mut output,
        EOT_FLAG_COMPRESSED
            | if options.apply_ppt_xor {
                EOT_FLAG_PPT_XOR
            } else {
                0
            },
    );

    if os2_table.len() >= 42 {
        output.extend_from_slice(&os2_table[32..42]);
    } else {
        output.extend_from_slice(&[0u8; 10]);
    }

    output.push(charset);
    output.push(derive_eot_italic_byte(head_table, os2_table));

    push_u32_le(
        &mut output,
        if os2_table.len() >= 6 {
            u32::from(u16::from_be_bytes([os2_table[4], os2_table[5]]))
        } else {
            0
        },
    );
    push_u16_le(
        &mut output,
        if os2_table.len() >= 10 {
            u16::from_be_bytes([os2_table[8], os2_table[9]])
        } else {
            0
        },
    );
    push_u16_le(&mut output, EOT_MAGIC_NUMBER);

    for range_index in 0..4 {
        push_u32_le(
            &mut output,
            if os2_table.len() >= 58 {
                let base = 42 + range_index * 4;
                u32::from_be_bytes([
                    os2_table[base],
                    os2_table[base + 1],
                    os2_table[base + 2],
                    os2_table[base + 3],
                ])
            } else {
                0
            },
        );
    }

    let os2_version = read_be_u16(os2_table, 0).unwrap_or(0);
    let (code_page_range1, code_page_range2) = if os2_version >= 1 && os2_table.len() >= 86 {
        (
            u32::from_be_bytes([os2_table[78], os2_table[79], os2_table[80], os2_table[81]]),
            u32::from_be_bytes([os2_table[82], os2_table[83], os2_table[84], os2_table[85]]),
        )
    } else {
        (0x0000_0001, 0x0000_0000)
    };
    push_u32_le(&mut output, code_page_range1);
    push_u32_le(&mut output, code_page_range2);

    push_u32_le(
        &mut output,
        if head_table.len() >= 12 {
            u32::from_be_bytes([head_table[8], head_table[9], head_table[10], head_table[11]])
        } else {
            0
        },
    );

    for _ in 0..4 {
        push_u32_le(&mut output, 0);
    }

    push_u16_le(&mut output, 0);
    push_length_prefixed_bytes(&mut output, &family_name)?;
    push_u16_le(&mut output, 0);
    push_length_prefixed_bytes(&mut output, &style_name)?;
    push_u16_le(&mut output, 0);
    push_length_prefixed_bytes(&mut output, &version_name)?;
    push_u16_le(&mut output, 0);
    push_length_prefixed_bytes(&mut output, &full_name)?;
    push_u16_le(&mut output, 0);
    push_u16_le(&mut output, 0);

    if options.version.includes_v20002_trailer() {
        push_u32_le(&mut output, EOT_ROOT_STRING_XOR_KEY);
        push_u32_le(&mut output, 0);
        push_u16_le(&mut output, 0);
        push_u16_le(&mut output, 0);
        push_u32_le(&mut output, 0);
        push_u32_le(&mut output, 0);
    }

    output.extend_from_slice(&payload);
    Ok(output)
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

fn push_u16_le(dst: &mut Vec<u8>, value: u16) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(dst: &mut Vec<u8>, value: u32) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn read_u16_le(bytes: &[u8], offset: &mut usize) -> Result<u16, EotHeaderError> {
    let bytes = read_bytes(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(bytes: &[u8], offset: &mut usize) -> Result<u32, EotHeaderError> {
    let bytes = read_bytes(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
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

fn push_length_prefixed_bytes(dst: &mut Vec<u8>, bytes: &[u8]) -> Result<(), EotEncodeError> {
    let length = u16::try_from(bytes.len()).map_err(|_| EotEncodeError::PayloadTooLarge)?;
    push_u16_le(dst, length);
    dst.extend_from_slice(bytes);
    Ok(())
}

fn derive_eot_charset(os2_table: &[u8]) -> u8 {
    let os2_version = read_be_u16(os2_table, 0).unwrap_or(0);
    if os2_version < 1 || os2_table.len() < 86 {
        return EOT_CHARSET_DEFAULT;
    }

    let code_page_range1 =
        u32::from_be_bytes([os2_table[78], os2_table[79], os2_table[80], os2_table[81]]);
    let code_page_range2 =
        u32::from_be_bytes([os2_table[82], os2_table[83], os2_table[84], os2_table[85]]);

    let has_code_page = |bit: u8| -> bool {
        if bit < 32 {
            code_page_range1 & (1u32 << bit) != 0
        } else {
            code_page_range2 & (1u32 << (bit - 32)) != 0
        }
    };

    if has_code_page(18) {
        return EOT_CHARSET_GB2312;
    }
    if has_code_page(17) {
        return EOT_CHARSET_SHIFTJIS;
    }
    if has_code_page(19) {
        return EOT_CHARSET_HANGEUL;
    }
    if has_code_page(21) {
        return EOT_CHARSET_JOHAB;
    }
    if has_code_page(20) {
        return EOT_CHARSET_CHINESEBIG5;
    }
    if has_code_page(16) {
        return EOT_CHARSET_THAI;
    }
    if has_code_page(0) {
        return EOT_CHARSET_ANSI;
    }
    if has_code_page(8) {
        return EOT_CHARSET_VIETNAMESE;
    }
    if has_code_page(7) {
        return EOT_CHARSET_BALTIC;
    }
    if has_code_page(6) {
        return EOT_CHARSET_ARABIC;
    }
    if has_code_page(5) {
        return EOT_CHARSET_HEBREW;
    }
    if has_code_page(4) {
        return EOT_CHARSET_TURKISH;
    }
    if has_code_page(3) {
        return EOT_CHARSET_GREEK;
    }
    if has_code_page(2) {
        return EOT_CHARSET_RUSSIAN;
    }
    if has_code_page(1) {
        return EOT_CHARSET_EASTEUROPE;
    }

    EOT_CHARSET_DEFAULT
}

fn derive_eot_italic_byte(head_table: &[u8], os2_table: &[u8]) -> u8 {
    let os2_italic =
        os2_table.len() >= 64 && u16::from_be_bytes([os2_table[62], os2_table[63]]) & 1 != 0;
    let head_italic = read_be_u16(head_table, 44).is_some_and(|mac_style| mac_style & 0x0002 != 0);

    if os2_italic || head_italic {
        u8::MAX
    } else {
        0
    }
}

fn extract_family_name_utf16le(name_table: &[u8], charset: u8) -> Vec<u8> {
    let mut preferred_languages = Vec::new();
    match charset {
        EOT_CHARSET_GB2312 => preferred_languages.push(NAME_LANGUAGE_ZH_CN),
        EOT_CHARSET_CHINESEBIG5 => preferred_languages.push(NAME_LANGUAGE_ZH_TW),
        EOT_CHARSET_SHIFTJIS => preferred_languages.push(NAME_LANGUAGE_JA_JP),
        EOT_CHARSET_HANGEUL | EOT_CHARSET_JOHAB => preferred_languages.push(NAME_LANGUAGE_KO_KR),
        _ => {}
    }

    extract_name_utf16le_with_fallbacks(name_table, NAME_ID_FAMILY, &preferred_languages)
}

fn extract_name_utf16le(name_table: &[u8], name_id: u16) -> Vec<u8> {
    extract_name_utf16le_with_fallbacks(name_table, name_id, &[])
}

fn extract_name_utf16le_with_fallbacks(
    name_table: &[u8],
    name_id: u16,
    preferred_languages: &[u16],
) -> Vec<u8> {
    for language_id in preferred_languages
        .iter()
        .copied()
        .chain([NAME_LANGUAGE_EN_US])
    {
        if let Some(bytes) = find_name_record(
            name_table,
            NAME_PLATFORM_WINDOWS,
            NAME_ENCODING_UNICODE_BMP,
            language_id,
            name_id,
        ) {
            return utf16be_to_utf16le(bytes);
        }
    }

    if let Some(bytes) = find_name_record_any_language(
        name_table,
        NAME_PLATFORM_WINDOWS,
        NAME_ENCODING_UNICODE_BMP,
        name_id,
    ) {
        return utf16be_to_utf16le(bytes);
    }

    Vec::new()
}

fn utf16be_to_utf16le(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() % 2 != 0 {
        return Vec::new();
    }

    let mut utf16le = bytes.to_vec();
    for chunk in utf16le.chunks_exact_mut(2) {
        chunk.swap(0, 1);
    }
    utf16le
}

fn find_name_record(
    name_table: &[u8],
    platform_id: u16,
    encoding_id: u16,
    language_id: u16,
    name_id: u16,
) -> Option<&[u8]> {
    let count = usize::from(read_be_u16(name_table, 2)?);
    let storage_offset = usize::from(read_be_u16(name_table, 4)?);

    for index in 0..count {
        let record_offset = 6 + index * 12;
        let record = name_table.get(record_offset..record_offset + 12)?;
        let record_platform = u16::from_be_bytes([record[0], record[1]]);
        let record_encoding = u16::from_be_bytes([record[2], record[3]]);
        let record_language = u16::from_be_bytes([record[4], record[5]]);
        let record_name = u16::from_be_bytes([record[6], record[7]]);

        if record_platform != platform_id
            || record_encoding != encoding_id
            || record_language != language_id
            || record_name != name_id
        {
            continue;
        }

        let length = usize::from(u16::from_be_bytes([record[8], record[9]]));
        let offset = usize::from(u16::from_be_bytes([record[10], record[11]]));
        let start = storage_offset.checked_add(offset)?;
        let end = start.checked_add(length)?;
        return name_table.get(start..end);
    }

    None
}

fn find_name_record_any_language(
    name_table: &[u8],
    platform_id: u16,
    encoding_id: u16,
    name_id: u16,
) -> Option<&[u8]> {
    let count = usize::from(read_be_u16(name_table, 2)?);
    let storage_offset = usize::from(read_be_u16(name_table, 4)?);

    for index in 0..count {
        let record_offset = 6 + index * 12;
        let record = name_table.get(record_offset..record_offset + 12)?;
        let record_platform = u16::from_be_bytes([record[0], record[1]]);
        let record_encoding = u16::from_be_bytes([record[2], record[3]]);
        let record_name = u16::from_be_bytes([record[6], record[7]]);

        if record_platform != platform_id
            || record_encoding != encoding_id
            || record_name != name_id
        {
            continue;
        }

        let length = usize::from(u16::from_be_bytes([record[8], record[9]]));
        let offset = usize::from(u16::from_be_bytes([record[10], record[11]]));
        let start = storage_offset.checked_add(offset)?;
        let end = start.checked_add(length)?;
        return name_table.get(start..end);
    }

    None
}
