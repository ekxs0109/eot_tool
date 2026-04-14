use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_sfnt::{parse_sfnt, SFNT_VERSION_OTTO};

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
fn decode_rejects_non_empty_extra_mtx_blocks() {
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
        "expected decode to fail for unsupported extra MTX blocks"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-empty extra MTX blocks are not supported"),
        "expected unsupported-block error, stderr: {stderr}"
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

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("slice should be 4 bytes"))
}

fn read_u24_be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}
