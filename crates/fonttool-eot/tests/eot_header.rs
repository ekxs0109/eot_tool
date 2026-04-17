use fonttool_eot::{
    build_eot_file, parse_eot_header, EotBuildOptions, EotHeaderError, EotVersion,
};

const FIXTURE_BYTES: &[u8] = include_bytes!("../../../testdata/wingdings3.eot");

fn fixture_bytes() -> &'static [u8] {
    FIXTURE_BYTES
}

#[test]
fn parses_eot_header_lengths_and_flags() {
    let header = parse_eot_header(fixture_bytes()).unwrap();

    assert_eq!(header.version, 0x0002_0002);
    assert_eq!(header.magic_number, 0x504c);
    assert_eq!(header.header_length, 202);
    assert_eq!(
        header.eot_size - header.font_data_size,
        header.header_length
    );
    assert!(header.flags & 0x4 != 0);
    assert_eq!(header.signature_size, 0);
    assert_eq!(header.eudc_font_size, 0);
    assert!(header.root_string.is_empty());
}

#[test]
fn rejects_truncated_header() {
    let bytes = [0u8; 81];

    let err = parse_eot_header(&bytes).unwrap_err();
    assert_eq!(err, EotHeaderError::Truncated);
}

#[test]
fn rejects_invalid_magic() {
    let mut bytes = [0u8; 82];
    bytes[34] = 0x34;
    bytes[35] = 0x12;

    let err = parse_eot_header(&bytes).unwrap_err();
    assert_eq!(err, EotHeaderError::InvalidMagic);
}

#[test]
fn rejects_invalid_size_metadata() {
    let mut bytes = fixture_bytes().to_vec();
    bytes[0..4].copy_from_slice(&0u32.to_le_bytes());

    let err = parse_eot_header(&bytes).unwrap_err();
    assert_eq!(err, EotHeaderError::InvalidSizeMetadata);
}

#[test]
fn rejects_truncated_to_declared_header_length() {
    let full = parse_eot_header(fixture_bytes()).unwrap();
    let truncated = &fixture_bytes()[..full.header_length as usize];

    let err = parse_eot_header(truncated).unwrap_err();
    assert_eq!(err, EotHeaderError::InvalidSizeMetadata);
}

#[test]
fn parses_v20002_trailer_fields() {
    let mut bytes = [0u8; 512];
    let signature = [0xde, 0xad, 0xbe, 0xef];
    let eudc_font_data = [0x11, 0x22, 0x33];
    let header_length = build_synthetic_v20002_header(&mut bytes, &signature, &eudc_font_data, 5);

    let header = parse_eot_header(&bytes[..header_length + 5]).unwrap();

    assert_eq!(header.signature_size, signature.len() as u16);
    assert_eq!(header.signature, &signature);
    assert_eq!(header.eudc_font_size, eudc_font_data.len() as u32);
    assert_eq!(header.eudc_flags, 0x99aa_bbcc);
}

#[test]
fn build_v1_header_omits_v20002_trailer_fields() {
    let bytes = build_synthetic_v1_header_with_payload(8);
    let header = parse_eot_header(&bytes).unwrap();

    assert_eq!(header.version, 0x0002_0001);
    assert_eq!(header.signature_size, 0);
    assert_eq!(header.eudc_font_size, 0);
    assert_eq!(header.root_string_checksum, 0);
}

fn build_synthetic_v20002_header(
    dst: &mut [u8],
    signature: &[u8],
    eudc_font_data: &[u8],
    payload_size: u32,
) -> usize {
    dst.fill(0);
    let mut offset = 82usize;

    offset = append_length_prefixed_ascii(dst, offset, "Family");
    write_u16_le(dst, offset, 0);
    offset += 2;
    offset = append_length_prefixed_ascii(dst, offset, "Style");
    write_u16_le(dst, offset, 0);
    offset += 2;
    offset = append_length_prefixed_ascii(dst, offset, "Version");
    write_u16_le(dst, offset, 0);
    offset += 2;
    offset = append_length_prefixed_ascii(dst, offset, "Full");
    write_u16_le(dst, offset, 0);
    offset += 2;
    write_u16_le(dst, offset, 0);
    offset += 2;

    write_u32_le(dst, offset, 0x1122_3344);
    offset += 4;
    write_u32_le(dst, offset, 0x5566_7788);
    offset += 4;
    write_u16_le(dst, offset, 0);
    offset += 2;
    write_u16_le(dst, offset, signature.len() as u16);
    offset += 2;
    if !signature.is_empty() {
        dst[offset..offset + signature.len()].copy_from_slice(signature);
        offset += signature.len();
    }
    write_u32_le(dst, offset, 0x99aa_bbcc);
    offset += 4;
    write_u32_le(dst, offset, eudc_font_data.len() as u32);
    offset += 4;
    if !eudc_font_data.is_empty() {
        dst[offset..offset + eudc_font_data.len()].copy_from_slice(eudc_font_data);
        offset += eudc_font_data.len();
    }
    if payload_size > 0 {
        dst[offset..offset + payload_size as usize].fill(0x5a);
    }

    write_u32_le(dst, 0, (offset as u32) + payload_size);
    write_u32_le(dst, 4, payload_size);
    write_u32_le(dst, 8, 0x0002_0002);
    write_u32_le(dst, 12, 0x4);
    write_u32_le(dst, 28, 400);
    write_u16_le(dst, 32, 0);
    write_u16_le(dst, 34, 0x504c);

    offset
}

fn build_synthetic_v1_header_with_payload(payload_size: usize) -> Vec<u8> {
    let payload = vec![0x5a; payload_size];
    build_eot_file(
        &synthetic_head_table(),
        &synthetic_os2_table(),
        &synthetic_name_table(),
        &payload,
        EotBuildOptions {
            version: EotVersion::V1,
            apply_ppt_xor: false,
        },
    )
    .expect("synthetic eot should build")
}

fn synthetic_head_table() -> Vec<u8> {
    let mut bytes = vec![0u8; 12];
    bytes[8..12].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    bytes
}

fn synthetic_os2_table() -> Vec<u8> {
    vec![0u8; 86]
}

fn synthetic_name_table() -> Vec<u8> {
    let records = [
        (1u16, utf16be_ascii("Family")),
        (2u16, utf16be_ascii("Regular")),
        (5u16, utf16be_ascii("Version 1.0")),
        (4u16, utf16be_ascii("Family Regular")),
    ];
    let storage_offset = 6 + records.len() * 12;
    let mut bytes = vec![0u8; storage_offset];
    bytes[0..2].copy_from_slice(&0u16.to_be_bytes());
    bytes[2..4].copy_from_slice(&(records.len() as u16).to_be_bytes());
    bytes[4..6].copy_from_slice(&(storage_offset as u16).to_be_bytes());

    let mut string_offset = 0usize;
    for (index, (name_id, value)) in records.iter().enumerate() {
        let record_offset = 6 + index * 12;
        bytes[record_offset..record_offset + 2].copy_from_slice(&3u16.to_be_bytes());
        bytes[record_offset + 2..record_offset + 4].copy_from_slice(&1u16.to_be_bytes());
        bytes[record_offset + 4..record_offset + 6].copy_from_slice(&0x0409u16.to_be_bytes());
        bytes[record_offset + 6..record_offset + 8].copy_from_slice(&name_id.to_be_bytes());
        bytes[record_offset + 8..record_offset + 10]
            .copy_from_slice(&(value.len() as u16).to_be_bytes());
        bytes[record_offset + 10..record_offset + 12]
            .copy_from_slice(&(string_offset as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        string_offset += value.len();
    }

    bytes
}

fn utf16be_ascii(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        bytes.push(0);
        bytes.push(byte);
    }
    bytes
}

fn write_u16_le(dst: &mut [u8], offset: usize, value: u16) {
    dst[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_le(dst: &mut [u8], offset: usize, value: u32) {
    dst[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_utf16le_ascii(dst: &mut [u8], offset: usize, text: &str) {
    for (index, byte) in text.bytes().enumerate() {
        dst[offset + index * 2] = byte;
        dst[offset + index * 2 + 1] = 0;
    }
}

fn append_length_prefixed_ascii(dst: &mut [u8], offset: usize, text: &str) -> usize {
    let length = (text.len() * 2) as u16;
    write_u16_le(dst, offset, length);
    write_utf16le_ascii(dst, offset + 2, text);
    offset + 2 + length as usize
}
