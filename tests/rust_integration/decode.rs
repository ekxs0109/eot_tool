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

fn assert_otf_or_ttf_header_matches_fixture(path: &Path) {
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
fn decode_font1_fntdata_writes_coretext_acceptable_font() {
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
    assert_otf_or_ttf_header_matches_fixture(&output_path);

    let _ = fs::remove_file(output_path);
}
