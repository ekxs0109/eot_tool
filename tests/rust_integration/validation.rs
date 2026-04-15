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
fn swift_coretext_probe_accepts_supported_decode_output_from_rust_harness() {
    let decoded_path = support::temp_ttf();
    let decode = support::run_fonttool([
        "decode",
        "testdata/font1.fntdata",
        decoded_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        decode.status.success(),
        "expected Rust decode to succeed, stderr: {}",
        String::from_utf8_lossy(&decode.stderr)
    );

    let output = Command::new("swift")
        .args([
            "run",
            "--package-path",
            "tests/macos-swift",
            "FonttoolCoreTextProbe",
            decoded_path
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

    let _ = std::fs::remove_file(decoded_path);
}
