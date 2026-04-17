use fonttool_sfnt::parse_sfnt;
use fonttool_sfnt::ParseError;

const TEST_TTF_BYTES: &[u8] = include_bytes!("../../../testdata/OpenSans-Regular.ttf");
const TEST_OTF_BYTES: &[u8] = include_bytes!("../../../testdata/cff-static.otf");

fn build_sfnt(version_tag: u32, records: &[(u32, u32, u32, u32)], total_len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; total_len];
    bytes[..4].copy_from_slice(&version_tag.to_be_bytes());
    bytes[4..6].copy_from_slice(&(records.len() as u16).to_be_bytes());

    for (index, &(tag, checksum, offset, length)) in records.iter().enumerate() {
        let base = 12 + index * 16;
        if base + 4 <= bytes.len() {
            bytes[base..base + 4].copy_from_slice(&tag.to_be_bytes());
        }
        if base + 8 <= bytes.len() {
            bytes[base + 4..base + 8].copy_from_slice(&checksum.to_be_bytes());
        }
        if base + 12 <= bytes.len() {
            bytes[base + 8..base + 12].copy_from_slice(&offset.to_be_bytes());
        }
        if base + 16 <= bytes.len() {
            bytes[base + 12..base + 16].copy_from_slice(&length.to_be_bytes());
        }
    }

    bytes
}

#[test]
fn parses_truetype_sfnt_header() {
    let font = parse_sfnt(TEST_TTF_BYTES).unwrap();
    assert_eq!(font.version_tag(), 0x0001_0000);
}

#[test]
fn parses_otto_sfnt_header() {
    let font = parse_sfnt(TEST_OTF_BYTES).unwrap();
    assert_eq!(font.version_tag(), u32::from_be_bytes(*b"OTTO"));
}

#[test]
fn rejects_truncated_header() {
    let bytes = vec![0u8; 11];

    let err = parse_sfnt(&bytes).unwrap_err();
    assert_eq!(err, ParseError::TruncatedHeader);
}

#[test]
fn rejects_truncated_directory() {
    let bytes = build_sfnt(0x0001_0000, &[(u32::from_be_bytes(*b"head"), 0, 28, 4)], 27);

    let err = parse_sfnt(&bytes).unwrap_err();
    assert_eq!(err, ParseError::TruncatedDirectory);
}

#[test]
fn rejects_out_of_bounds_table_range() {
    let bytes = build_sfnt(
        0x0001_0000,
        &[(u32::from_be_bytes(*b"head"), 0, 32, 12)],
        40,
    );

    let err = parse_sfnt(&bytes).unwrap_err();
    assert_eq!(
        err,
        ParseError::InvalidTableRange {
            tag: u32::from_be_bytes(*b"head")
        }
    );
}

#[test]
fn rejects_table_offset_into_directory() {
    let bytes = build_sfnt(0x0001_0000, &[(u32::from_be_bytes(*b"head"), 0, 20, 4)], 64);

    let err = parse_sfnt(&bytes).unwrap_err();
    assert_eq!(
        err,
        ParseError::InvalidTableRange {
            tag: u32::from_be_bytes(*b"head")
        }
    );
}
