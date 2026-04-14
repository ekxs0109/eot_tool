use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_sfnt::load_sfnt;

const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn temp_eot() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-otf-convert-{}-{unique}.eot",
        std::process::id()
    ))
}

fn temp_fntdata() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-otf-convert-{}-{unique}.fntdata",
        std::process::id()
    ))
}

fn temp_ttf() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-otf-convert-{}-{unique}.ttf",
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

fn run_legacy_fonttool<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(workspace_root().join("build/fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("legacy fonttool binary should launch")
}

fn decode_with_legacy(input: &Path, output: &Path) {
    let decode = run_legacy_fonttool([
        "decode",
        input.to_str().expect("path should be valid utf-8"),
        output.to_str().expect("path should be valid utf-8"),
    ]);

    assert!(
        decode.status.success(),
        "expected legacy decode to succeed, stderr: {}",
        String::from_utf8_lossy(&decode.stderr)
    );
}

#[test]
fn encode_static_cff_input_to_eot() {
    let output_path = temp_eot();
    let decoded_path = temp_ttf();

    let output = run_fonttool([
        "encode",
        "testdata/cff-static.otf",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    decode_with_legacy(&output_path, &decoded_path);
    let decoded_bytes = fs::read(&decoded_path).expect("decoded font should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded font should parse");
    assert!(sfnt.table(TAG_GLYF).is_some(), "decoded output should contain glyf");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
}

#[test]
fn subset_cff2_variable_input_with_variation() {
    let output_path = temp_fntdata();
    let decoded_path = temp_ttf();

    let output = run_fonttool([
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

    decode_with_legacy(&output_path, &decoded_path);
    let decoded_bytes = fs::read(&decoded_path).expect("decoded subset should be readable");
    let sfnt = load_sfnt(&decoded_bytes).expect("decoded subset should parse");
    assert!(sfnt.table(TAG_GLYF).is_some(), "subset output should contain glyf");
    assert!(sfnt.table(TAG_MAXP).is_some(), "subset output should contain maxp");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
}
