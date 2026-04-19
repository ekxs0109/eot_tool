mod support;

use std::fs;

fn isolated_cwd(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fonttool-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("isolated cwd should be creatable");
    path
}

#[test]
fn encode_woff_truetype_source_to_eot_succeeds() {
    let output_path = support::temp_eot();
    let decoded_path = support::temp_ttf();
    let cwd = isolated_cwd("woff-encode-cwd");
    let input_path = support::fixture_path("testdata/OpenSans-Regular.woff");

    let output = support::run_fonttool_in_dir(
        [
            "encode",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
        ],
        &cwd,
    );

    assert!(
        output.status.success(),
        "expected WOFF TrueType encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.exists(), "encode should create an EOT output");
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    support::assert_true_type_glyf_output(&decoded_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn convert_woff2_truetype_source_to_ttf_succeeds() {
    let output_path = support::temp_ttf();
    let cwd = isolated_cwd("woff2-convert-cwd");
    let input_path = support::fixture_path("testdata/OpenSans-Regular.woff2");

    let output = support::run_fonttool_in_dir(
        [
            "convert",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--to",
            "ttf",
        ],
        &cwd,
    );

    assert!(
        output.status.success(),
        "expected WOFF2 TrueType convert to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.exists(), "convert should create a TTF output");
    support::assert_true_type_glyf_output(&output_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}
