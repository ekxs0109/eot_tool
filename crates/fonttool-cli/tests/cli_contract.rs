use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::parse_eot_header;
use fonttool_sfnt::{load_sfnt, parse_sfnt, SFNT_VERSION_OTTO, SFNT_VERSION_TRUETYPE};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
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

fn temp_path(stem: &str, extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-cli-contract-{stem}-{}-{unique}.{extension}",
        std::process::id()
    ))
}

fn assert_sfnt_has_tables(path: &Path, tables: &[u32]) {
    let bytes = fs::read(path).expect("decoded font should exist");
    parse_sfnt(&bytes).expect("decoded font should parse as sfnt");

    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");
    for table in tables {
        assert!(
            font.table(*table).is_some(),
            "expected table {:08X} in {}",
            table,
            path.display()
        );
    }
}

fn write_obfuscated_fixture_copy(source_path: &Path, dest_path: &Path) {
    let mut bytes = fs::read(source_path).expect("fixture should be readable");
    let header = parse_eot_header(&bytes).expect("fixture should parse as EOT");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;

    let flags = u32::from_le_bytes(bytes[12..16].try_into().expect("flags should exist"));
    bytes[12..16].copy_from_slice(&(flags | EOT_FLAG_PPT_XOR).to_le_bytes());
    for byte in &mut bytes[payload_start..payload_end] {
        *byte ^= 0x50;
    }

    fs::write(dest_path, bytes).expect("obfuscated fixture copy should be writable");
}

#[test]
fn help_succeeds_and_lists_top_level_commands() {
    let output = run_fonttool(["--help"]);

    assert!(output.status.success(), "expected --help to succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: fonttool <COMMAND>"),
        "expected usage banner, stdout: {stdout}"
    );
    assert!(
        stdout.contains("encode <INPUT> <OUTPUT>"),
        "expected encode command in help, stdout: {stdout}"
    );
    assert!(
        stdout.contains("decode <INPUT> <OUTPUT>"),
        "expected decode command in help, stdout: {stdout}"
    );
    assert!(
        stdout.contains("subset <INPUT> <OUTPUT>"),
        "expected subset command in help, stdout: {stdout}"
    );
}

#[test]
fn no_command_exits_success_and_prints_help() {
    let output = run_fonttool(std::iter::empty::<&str>());

    assert!(
        output.status.success(),
        "expected empty invocation to succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: fonttool <COMMAND>"),
        "expected help output for empty invocation, stdout: {stdout}"
    );
}

#[test]
fn unknown_command_exits_with_status_2_and_reports_name() {
    let output = run_fonttool(["nonesuch"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected clap-style usage error exit code"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command `nonesuch`"),
        "expected unknown-command error, stderr: {stderr}"
    );
}

#[test]
fn decode_without_required_args_exits_with_status_2_and_contract_error() {
    let output = run_fonttool(["decode"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("decode expects INPUT and OUTPUT paths"),
        "expected decode contract error, stderr: {stderr}"
    );
}

#[test]
fn encode_without_required_args_exits_with_status_2_and_contract_error() {
    let output = run_fonttool(["encode"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("encode expects INPUT and OUTPUT paths"),
        "expected encode contract error, stderr: {stderr}"
    );
}

#[test]
fn decode_real_fixture_creates_parseable_output_file() {
    let output_path = temp_path("decode-font1", "otf");

    let output = run_fonttool([
        "decode",
        "testdata/font1.fntdata",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected decode fixture path to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.is_file(),
        "expected decode to create output file"
    );

    let bytes = fs::read(&output_path).expect("decoded output should be readable");
    let font = load_sfnt(&bytes).expect("decoded output should load as sfnt");
    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);
    assert_sfnt_has_tables(
        &output_path,
        &[TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_NAME, TAG_CMAP],
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn encode_then_decode_cff_static_otf_roundtrip_produces_required_tables() {
    let encoded_path = temp_path("cff-static-roundtrip", "eot");
    let decoded_path = temp_path("cff-static-roundtrip", "otf");

    let encode_output = run_fonttool([
        "encode",
        "testdata/cff-static.otf",
        encoded_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        encode_output.status.success(),
        "expected CFF encode to succeed, stderr: {}",
        String::from_utf8_lossy(&encode_output.stderr)
    );
    assert!(
        encoded_path.is_file(),
        "expected encode to create an EOT output file"
    );

    let decode_output = run_fonttool([
        "decode",
        encoded_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        decoded_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        decode_output.status.success(),
        "expected CFF roundtrip decode to succeed, stderr: {}",
        String::from_utf8_lossy(&decode_output.stderr)
    );
    assert!(
        decoded_path.is_file(),
        "expected decode to create roundtrip output"
    );

    let roundtrip_bytes = fs::read(&decoded_path).expect("roundtrip output should be readable");
    let roundtrip_font = load_sfnt(&roundtrip_bytes).expect("roundtrip output should load as sfnt");
    assert_eq!(
        roundtrip_font.version_tag(),
        SFNT_VERSION_TRUETYPE,
        "expected current CFF roundtrip to decode as a TrueType sfnt"
    );
    assert_sfnt_has_tables(
        &decoded_path,
        &[
            TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_NAME, TAG_CMAP, TAG_GLYF, TAG_LOCA,
        ],
    );

    let _ = fs::remove_file(encoded_path);
    let _ = fs::remove_file(decoded_path);
}

#[test]
fn decode_obfuscated_fixture_copy_succeeds_for_supported_fntdata_fixture() {
    let input_path = temp_path("font1-obfuscated", "fntdata");
    let output_path = temp_path("font1-obfuscated", "otf");

    write_obfuscated_fixture_copy(
        &workspace_root().join("testdata/font1.fntdata"),
        &input_path,
    );

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
        "expected decode of obfuscated fixture copy to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.is_file(),
        "expected decode to create output for obfuscated copy"
    );
    assert_sfnt_has_tables(
        &output_path,
        &[TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_NAME, TAG_CMAP],
    );

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn subset_missing_flag_value_is_rejected() {
    let output = run_fonttool([
        "subset",
        "in.ttf",
        "out.eot",
        "--variation",
        "wght=700",
        "--glyph-ids",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset flag is missing a value"),
        "expected missing-value error, stderr: {stderr}"
    );
}

#[test]
fn subset_duplicate_selection_mode_is_rejected() {
    let output = run_fonttool([
        "subset",
        "in.ttf",
        "out.eot",
        "--glyph-ids",
        "1,2",
        "--text",
        "ab",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset accepts only one selection mode"),
        "expected duplicate-selection error, stderr: {stderr}"
    );
}

#[test]
fn subset_missing_selection_mode_is_rejected() {
    let output = run_fonttool(["subset", "in.ttf", "out.eot", "--variation", "wght=700"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset requires either --glyph-ids or --text"),
        "expected missing-selection-mode error, stderr: {stderr}"
    );
}

#[test]
fn subset_static_otf_with_variation_is_rejected_by_current_contract() {
    let output = run_fonttool([
        "subset",
        "testdata/cff-static.otf",
        "out.eot",
        "--text",
        "ABC",
        "--variation",
        "wght=700",
    ]);

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("variation arguments require a variable CFF2 input"),
        "expected static OTF variation rejection, stderr: {stderr}"
    );
}

#[test]
fn subset_otf_without_text_is_rejected_by_current_contract() {
    let output = run_fonttool([
        "subset",
        "testdata/cff-static.otf",
        "out.eot",
        "--glyph-ids",
        "1,2",
    ]);

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset currently requires --text for OTF input"),
        "expected missing-text rejection for OTF input, stderr: {stderr}"
    );
}

#[test]
fn subset_non_otf_with_text_only_is_rejected_by_current_contract() {
    let output = run_fonttool([
        "subset",
        "testdata/OpenSans-Regular.ttf",
        "out.eot",
        "--text",
        "ABC",
    ]);

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset currently only supports --glyph-ids for non-OTF input"),
        "expected non-OTF glyph-id-only rejection, stderr: {stderr}"
    );
}

#[test]
fn subset_unsupported_flag_is_rejected() {
    let output = run_fonttool(["subset", "in.ttf", "out.eot", "--bogus", "x"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported subset flag `--bogus`"),
        "expected unsupported-flag error, stderr: {stderr}"
    );
}
