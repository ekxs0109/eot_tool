mod support;

use std::fs;

use fonttool_sfnt::load_sfnt;

const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_POST: u32 = u32::from_be_bytes(*b"post");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");

fn table_bytes<'a>(font: &'a fonttool_sfnt::OwnedSfntFont, tag: u32, name: &str) -> &'a [u8] {
    font.table(tag)
        .unwrap_or_else(|| panic!("expected {name} table"))
        .data
        .as_slice()
}

fn read_u16_be(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64_be(bytes: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

#[test]
fn encode_static_cff_input_to_eot() {
    let roundtrip = support::encode_static_cff_to_roundtrip_ttf();
    let decoded_bytes = fs::read(roundtrip.font_path()).expect("decoded font should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded font should parse");
    assert!(
        sfnt.table(TAG_GLYF).is_some(),
        "decoded output should contain glyf"
    );
}

#[test]
fn roundtrip_otf_fixture_preserves_expected_post_and_hhea_fields() {
    let roundtrip = support::encode_otf_to_roundtrip_ttf(
        "testdata/aipptfonts/\u{9999}\u{8549}Plus__20220301185701917366.otf",
    );
    let decoded_bytes = fs::read(roundtrip.font_path()).expect("decoded font should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded font should parse");

    let post = table_bytes(&sfnt, TAG_POST, "post");
    let hhea = table_bytes(&sfnt, TAG_HHEA, "hhea");

    assert_eq!(read_u32_be(post, 0), 0x0003_0000);
    assert_eq!(i16::from_be_bytes([post[8], post[9]]), -75);
    assert_eq!(i16::from_be_bytes([post[10], post[11]]), 50);
    assert_eq!(read_u16_be(hhea, 34), 4518);
}

#[test]
fn roundtrip_otf_fixture_writes_nonzero_head_checksum_and_timestamps() {
    let roundtrip = support::encode_otf_to_roundtrip_ttf(
        "testdata/aipptfonts/\u{9999}\u{8549}Plus__20220301185701917366.otf",
    );
    let decoded_bytes = fs::read(roundtrip.font_path()).expect("decoded font should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded font should parse");

    let head = table_bytes(&sfnt, TAG_HEAD, "head");
    let checksum_adjustment = read_u32_be(head, 8);
    let created = read_u64_be(head, 20);
    let modified = read_u64_be(head, 28);

    assert_ne!(checksum_adjustment, 0);
    assert_ne!(created, 0);
    assert_eq!(created, modified);
}

#[test]
fn subset_cff2_variable_input_with_variation() {
    let output_path = support::temp_fntdata();
    let decoded_path = support::temp_ttf();

    let output = support::run_fonttool([
        "subset",
        "testdata/cff2-variable.otf",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        "--text",
        "ABC",
        "--variation",
        "wght=700",
    ]);

    assert!(
        output.status.success(),
        "expected subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    support::decode_with_legacy(&output_path, &decoded_path);
    let decoded_bytes = fs::read(&decoded_path).expect("decoded subset should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded subset should parse");
    assert!(
        sfnt.table(TAG_GLYF).is_some(),
        "subset output should contain glyf"
    );
    assert!(
        sfnt.table(TAG_MAXP).is_some(),
        "subset output should contain maxp"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
}
