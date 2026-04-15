mod support;

use std::process::Command;

#[test]
fn python_verify_font_entrypoint_accepts_a_valid_font() {
    let output = support::run_python(["tests/verify_font.py", "testdata/OpenSans-Regular.ttf"]);

    assert!(
        output.status.success(),
        "expected python verifier to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn swift_coretext_probe_accepts_static_cff_roundtrip_from_rust_harness() {
    let roundtrip = support::encode_static_cff_to_roundtrip_ttf();

    let output = Command::new("swift")
        .args([
            "run",
            "--package-path",
            "tests/macos-swift",
            "FonttoolCoreTextProbe",
            roundtrip
                .font_path()
                .to_str()
                .expect("temp path should be valid utf-8"),
        ])
        .current_dir(support::workspace_root())
        .output()
        .expect("swift should launch");

    assert!(
        output.status.success(),
        "expected Swift CoreText probe to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "coretext font accepted",
        "unexpected Swift CoreText probe stdout"
    );
}
