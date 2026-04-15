mod support;

use std::fs;

use fonttool_sfnt::load_sfnt;

const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");

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
