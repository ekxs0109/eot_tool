use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_sfnt::{load_sfnt, parse_sfnt, SFNT_VERSION_OTTO, SFNT_VERSION_TRUETYPE};

const RAW_SFNT_HEADER_LENGTH: usize = 100;
const EOT_FLAGS_RANGE: std::ops::Range<usize> = 12..16;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn temp_out() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-decode-{}-{unique}.otf",
        std::process::id()
    ))
}

fn temp_in(suffix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-decode-input-{}-{unique}.{suffix}",
        std::process::id()
    ))
}

fn run_fonttool<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("fonttool binary should launch")
}

fn assert_otto_header_matches_fixture(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert!(
        bytes.len() >= 4,
        "decoded font should contain an sfnt version header"
    );
    assert_eq!(&bytes[..4], b"OTTO");

    let sfnt = parse_sfnt(&bytes).expect("decoded font should parse as sfnt");
    assert_eq!(sfnt.version_tag(), SFNT_VERSION_OTTO);
}

fn assert_variable_cff2_output(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");

    assert_eq!(&bytes[..4], b"OTTO");
    assert!(
        font.table(u32::from_be_bytes(*b"CFF2")).is_some(),
        "decoded output should preserve the CFF2 table"
    );
    assert!(
        font.table(u32::from_be_bytes(*b"fvar")).is_some(),
        "decoded output should preserve variation axes"
    );
}

fn assert_truetype_roundtrip_ready(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert!(
        bytes.len() >= 4,
        "decoded font should contain an sfnt version header"
    );
    assert_eq!(&bytes[..4], &SFNT_VERSION_TRUETYPE.to_be_bytes());

    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");
    let glyf = font
        .table(u32::from_be_bytes(*b"glyf"))
        .expect("decoded font should contain glyf");
    let loca = font
        .table(u32::from_be_bytes(*b"loca"))
        .expect("decoded font should contain loca");

    assert!(
        !glyf.data.is_empty(),
        "decoded TrueType fixture should expose real glyf bytes"
    );
    assert!(
        !loca.data.is_empty(),
        "decoded TrueType fixture should expose real loca bytes"
    );
}

fn assert_truetype_output_matches_fixture(path: &Path, expected: &[u8]) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert_eq!(
        bytes, expected,
        "raw-SFNT payload decode should reproduce the original TrueType bytes"
    );
    assert_truetype_roundtrip_ready(path);
}

#[test]
fn decode_font1_fntdata_writes_otto_sfnt() {
    let output_path = temp_out();

    let output = run_fonttool([
        "decode",
        "testdata/font1.fntdata",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected decode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_otto_header_matches_fixture(&output_path);

    let _ = fs::remove_file(output_path);
}

#[test]
fn decode_otto_cff2_variable_fixture_writes_variable_otto_output() {
    let input_path = workspace_root().join("testdata/otto-cff2-variable.fntdata");
    let output_path = temp_out();

    let output = run_fonttool([
        "decode",
        input_path.to_str().expect("fixture path should be valid utf-8"),
        output_path.to_str().expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected decode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_variable_cff2_output(&output_path);

    let _ = fs::remove_file(output_path);
}

#[test]
fn decode_pptx_fixture_fntdata_reconstructs_roundtrip_ready_truetype() {
    let output_path = temp_out();
    let reencoded_path = temp_in("eot");

    let output = run_fonttool([
        "decode",
        "build/pptx_case7/ppt/fonts/font1.fntdata",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected decode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_truetype_roundtrip_ready(&output_path);

    let encode_output = run_fonttool([
        "encode",
        output_path
            .to_str()
            .expect("decoded path should be valid utf-8"),
        reencoded_path
            .to_str()
            .expect("re-encoded path should be valid utf-8"),
    ]);
    assert!(
        encode_output.status.success(),
        "expected decoded TrueType fixture to re-encode, stderr: {}",
        String::from_utf8_lossy(&encode_output.stderr)
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(reencoded_path);
}

#[test]
fn decode_raw_sfnt_payload_eot_writes_truetype_output() {
    let input_path = temp_in("eot");
    let output_path = temp_out();
    let sfnt_bytes = fs::read(workspace_root().join("testdata/OpenSans-Regular.ttf"))
        .expect("fixture sfnt should be readable");
    let fixture = build_raw_sfnt_payload_eot(&sfnt_bytes);
    fs::write(&input_path, fixture).expect("synthetic raw-sfnt eot should be writable");

    let output = run_fonttool([
        "decode",
        input_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected raw-sfnt EOT decode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_truetype_output_matches_fixture(&output_path, &sfnt_bytes);

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn decode_xor_raw_sfnt_payload_eot_writes_truetype_output() {
    let input_path = temp_in("eot");
    let output_path = temp_out();
    let sfnt_bytes = fs::read(workspace_root().join("testdata/OpenSans-Regular.ttf"))
        .expect("fixture sfnt should be readable");
    let mut fixture = build_raw_sfnt_payload_eot(&sfnt_bytes);

    fixture[EOT_FLAGS_RANGE].copy_from_slice(&0x1000_0000u32.to_le_bytes());
    let payload_start = RAW_SFNT_HEADER_LENGTH;
    for byte in &mut fixture[payload_start..] {
        *byte ^= 0x50;
    }

    fs::write(&input_path, fixture).expect("synthetic xor raw-sfnt eot should be writable");

    let output = run_fonttool([
        "decode",
        input_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected xor raw-sfnt EOT decode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_truetype_output_matches_fixture(&output_path, &sfnt_bytes);

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn decode_rejects_incomplete_extra_mtx_blocks_for_truetype_reconstruction() {
    let input_path = temp_in("fntdata");
    let output_path = temp_out();
    let fixture = build_fixture_with_non_empty_block3();

    fs::write(&input_path, fixture).expect("mutated fixture should be writable");

    let output = run_fonttool([
        "decode",
        input_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        !output.status.success(),
        "expected decode to fail for incomplete TrueType reconstruction blocks"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("current Rust MTX decode requires both block2 and block3"),
        "expected incomplete-block error, stderr: {stderr}"
    );

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(output_path);
}

fn build_fixture_with_non_empty_block3() -> Vec<u8> {
    let mut bytes = fs::read(workspace_root().join("testdata/font1.fntdata"))
        .expect("fixture should be readable");
    let eot_size = read_u32_le(&bytes[0..4]) as usize;
    let font_data_size = read_u32_le(&bytes[4..8]) as usize;
    let header_length = eot_size - font_data_size;
    let payload_start = header_length;
    let payload_end = payload_start + font_data_size;
    let payload = &bytes[payload_start..payload_end];

    assert_eq!(payload[0], 3, "fixture should use 3 MTX blocks");

    let offset_block3 = read_u24_be(&payload[7..10]) as usize;
    let block3_start = payload_start + offset_block3;
    let block3_end = payload_end;

    let non_empty_block3 = [0x00, 0x00, 0x08, 0x2A, 0x2A, 0x89, 0x80, 0xA8, 0x0C, 0x20];
    bytes.splice(block3_start..block3_end, non_empty_block3);

    let new_font_data_size = font_data_size - (block3_end - block3_start) + non_empty_block3.len();
    let new_eot_size = header_length + new_font_data_size;

    bytes[0..4].copy_from_slice(&(new_eot_size as u32).to_le_bytes());
    bytes[4..8].copy_from_slice(&(new_font_data_size as u32).to_le_bytes());

    bytes
}

fn build_raw_sfnt_payload_eot(sfnt_bytes: &[u8]) -> Vec<u8> {
    let eot_size = RAW_SFNT_HEADER_LENGTH + sfnt_bytes.len();
    let mut bytes = vec![0u8; eot_size];

    bytes[0..4].copy_from_slice(&(eot_size as u32).to_le_bytes());
    bytes[4..8].copy_from_slice(&(sfnt_bytes.len() as u32).to_le_bytes());
    bytes[8..12].copy_from_slice(&0x0002_0001u32.to_le_bytes());
    bytes[EOT_FLAGS_RANGE].copy_from_slice(&0u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&300u32.to_le_bytes());
    bytes[34..36].copy_from_slice(&0x504cu16.to_le_bytes());
    bytes[RAW_SFNT_HEADER_LENGTH..].copy_from_slice(sfnt_bytes);

    bytes
}

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("slice should be 4 bytes"))
}

fn read_u24_be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}
