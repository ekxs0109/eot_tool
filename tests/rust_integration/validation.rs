mod support;

use std::process::Command;

fn assert_python_success_or_skip_missing_fonttools(output: std::process::Output) {
    if output.status.success() {
        return;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code() == Some(2) && stderr.contains("fontTools is required") {
        eprintln!("skipping python font verification: {stderr}");
        return;
    }

    panic!("expected python verifier to succeed, stderr: {stderr}");
}

#[test]
fn python_verify_font_entrypoint_accepts_a_valid_font() {
    let font_path = support::fixture_path("testdata/OpenSans-Regular.ttf");
    let output = support::run_python([
        "tests/verify_font.py",
        font_path
            .to_str()
            .expect("fixture path should be valid utf-8"),
    ]);
    assert_python_success_or_skip_missing_fonttools(output);
}

#[cfg(target_os = "macos")]
#[test]
fn swift_coretext_probe_accepts_supported_decode_output_from_rust_harness() {
    let decoded_path = support::temp_ttf();
    let input_path = support::fixture_path("testdata/font1.fntdata");
    let decode = support::run_fonttool([
        "decode",
        input_path
            .to_str()
            .expect("fixture path should be valid utf-8"),
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
