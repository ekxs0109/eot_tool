use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn validation_python() -> PathBuf {
    let venv_python = workspace_root().join("build/venv/bin/python");
    if venv_python.exists() {
        venv_python
    } else {
        PathBuf::from("python3")
    }
}

#[test]
fn python_verify_font_entrypoint_accepts_a_valid_font() {
    let output = Command::new(validation_python())
        .args(["tests/verify_font.py", "testdata/OpenSans-Regular.ttf"])
        .current_dir(workspace_root())
        .output()
        .expect("python3 should launch");

    assert!(
        output.status.success(),
        "expected python verifier to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
