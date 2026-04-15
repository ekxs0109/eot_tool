use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-{label}-{}-{unique}.{extension}",
        std::process::id()
    ))
}

#[allow(dead_code)]
pub fn temp_fntdata() -> PathBuf {
    temp_path("otf-convert", "fntdata")
}

pub fn temp_ttf() -> PathBuf {
    temp_path("otf-convert", "ttf")
}

pub fn temp_eot() -> PathBuf {
    temp_path("otf-convert", "eot")
}

pub fn run_fonttool<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("fonttool binary should launch")
}

#[allow(dead_code)]
pub fn run_python<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(workspace_root().join("build/venv/bin/python"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("python binary should launch")
}

fn run_legacy_fonttool<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(workspace_root().join("build/fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("legacy fonttool binary should launch")
}

pub fn decode_with_legacy(input: &Path, output: &Path) {
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

#[allow(dead_code)]
pub fn save_ttf_with_fonttools(input: &Path, output: &Path) {
    let save = run_python([
        OsStr::new("-c"),
        OsStr::new(
            "from fontTools.ttLib import TTFont; import sys; \
             font = TTFont(sys.argv[1]); font.save(sys.argv[2]); font.close()",
        ),
        input.as_os_str(),
        output.as_os_str(),
    ]);

    assert!(
        save.status.success(),
        "expected fonttools save to succeed, stderr: {}",
        String::from_utf8_lossy(&save.stderr)
    );
}

#[allow(dead_code)]
pub fn run_fonttools_parity(left: &Path, right: &Path) -> Output {
    run_python([
        OsStr::new("tests/test_fonttools_parity.py"),
        left.as_os_str(),
        right.as_os_str(),
    ])
}

pub struct StaticCffRoundtrip {
    eot_path: PathBuf,
    roundtrip_path: PathBuf,
}

impl StaticCffRoundtrip {
    pub fn font_path(&self) -> &Path {
        &self.roundtrip_path
    }
}

impl Drop for StaticCffRoundtrip {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.eot_path);
        let _ = fs::remove_file(&self.roundtrip_path);
    }
}

pub fn encode_static_cff_to_roundtrip_ttf() -> StaticCffRoundtrip {
    encode_otf_to_roundtrip_ttf("testdata/cff-static.otf")
}

pub fn encode_otf_to_roundtrip_ttf(input_path: &str) -> StaticCffRoundtrip {
    let output_path = temp_eot();
    let decoded_path = temp_ttf();

    let output = run_fonttool([
        "encode",
        input_path,
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

    StaticCffRoundtrip {
        eot_path: output_path,
        roundtrip_path: decoded_path,
    }
}
