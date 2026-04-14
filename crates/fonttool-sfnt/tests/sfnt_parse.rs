use fonttool_sfnt::parse_sfnt;

const TEST_TTF_BYTES: &[u8] = include_bytes!("../../../testdata/OpenSans-Regular.ttf");
const TEST_OTF_BYTES: &[u8] = include_bytes!("../../../testdata/cff-static.otf");

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
