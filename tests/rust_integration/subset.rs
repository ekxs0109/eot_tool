use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_sfnt::load_sfnt;

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
        "fonttool-subset-{}-{unique}.eot",
        std::process::id()
    ))
}

fn temp_ttf() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-subset-decoded-{}-{unique}.ttf",
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

fn maxp_num_glyphs(path: &Path) -> u16 {
    let bytes = fs::read(path).expect("decoded subset output should be readable");
    let sfnt = load_sfnt(&bytes).expect("decoded subset output should parse");
    let maxp = sfnt.table(TAG_MAXP).expect("subset output should contain maxp");
    u16::from_be_bytes([maxp.data[4], maxp.data[5]])
}

#[test]
fn subset_keeps_requested_glyphs_and_updates_num_glyphs() {
    let output_path = temp_eot();
    let decoded_path = temp_ttf();

    let output = run_fonttool([
        "subset",
        "testdata/wingdings3.eot",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        "--glyph-ids",
        "0,1,2",
    ]);

    assert!(
        output.status.success(),
        "expected subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decode = run_fonttool([
        "decode",
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
        decoded_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    let decode = if decode.status.success() {
        decode
    } else {
        Command::new(workspace_root().join("build/fonttool"))
            .args([
                "decode",
                output_path
                    .to_str()
                    .expect("temp path should be valid utf-8"),
                decoded_path
                    .to_str()
                    .expect("temp path should be valid utf-8"),
            ])
            .current_dir(workspace_root())
            .output()
            .expect("legacy fonttool decode should launch")
    };

    assert!(
        decode.status.success(),
        "expected decode of subset output to succeed, stderr: {}",
        String::from_utf8_lossy(&decode.stderr)
    );

    assert_eq!(maxp_num_glyphs(&decoded_path), 3);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: unsupported HDMX in subset path; dropping table"),
        "expected HDMX warning, stderr: {stderr}"
    );
    assert!(
        stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "expected VDMX warning, stderr: {stderr}"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
}
