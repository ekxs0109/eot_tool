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

#[allow(dead_code)]
pub fn otf_parity_fixture() -> PathBuf {
    for relative in [
        "testdata/aipptfonts/香蕉Plus__20220301185701917366.otf",
        "testdata/20220301185701917366.otf",
    ] {
        let path = workspace_root().join(relative);
        if path.exists() {
            return path;
        }
    }

    panic!("expected OTF parity fixture to exist in a known testdata location");
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

#[allow(dead_code)]
pub fn temp_ttf() -> PathBuf {
    temp_path("otf-convert", "ttf")
}

#[allow(dead_code)]
pub fn temp_eot() -> PathBuf {
    temp_path("otf-convert", "eot")
}

#[allow(dead_code)]
pub fn run_fonttool<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_fonttool_in_dir(args, &workspace_root())
}

pub fn run_fonttool_in_dir<I, S>(args: I, current_dir: &Path) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("fonttool binary should launch")
}

#[allow(dead_code)]
pub fn run_python<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let workspace = workspace_root();
    let candidate_roots = std::iter::once(workspace.clone()).chain(
        fs::read_dir(workspace.join(".worktrees"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path())),
    );
    let python = candidate_roots
        .map(|root| root.join("build/venv/bin/python"))
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("python3"));

    Command::new(python)
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("python binary should launch")
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
