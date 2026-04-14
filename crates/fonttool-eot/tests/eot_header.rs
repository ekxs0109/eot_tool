use fonttool_eot::{parse_eot_header, EotHeaderError};

const FIXTURE_BYTES: &[u8] = include_bytes!("../../../testdata/wingdings3.eot");

fn fixture_bytes() -> &'static [u8] {
    FIXTURE_BYTES
}

#[test]
fn parses_eot_header_lengths_and_flags() {
    let header = parse_eot_header(fixture_bytes()).unwrap();

    assert!(header.header_length > 0);
    assert!(header.flags & 0x4 != 0);
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
