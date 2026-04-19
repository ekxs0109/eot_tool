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
fn encode_static_cff_input_is_rust_owned() {
    let output_path = support::temp_eot();
    let decoded_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-encode-cwd");
    let input_path = support::workspace_root().join("testdata/cff-static.otf");
    let source_bytes = fs::read(&input_path).expect("fixture should be readable");

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
        "expected static CFF encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "encode should create output for static CFF input"
    );
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    support::assert_decoded_otto_preserves_office_style_static_cff_tables(
        &decoded_path,
        &source_bytes,
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn encode_otf_parity_fixture_is_rust_owned() {
    let output_path = support::temp_eot();
    let decoded_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-parity-encode-cwd");
    let fixture = support::otf_parity_fixture();
    let source_bytes = fs::read(&fixture).expect("fixture should be readable");

    let output = support::run_fonttool_in_dir(
        [
            "encode",
            fixture
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
        "expected OTF parity fixture encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "encode should create output for the OTF parity fixture"
    );
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    support::assert_decoded_otto_preserves_office_style_static_cff_tables(
        &decoded_path,
        &source_bytes,
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn subset_static_cff_text_input_succeeds() {
    let output_path = support::temp_fntdata();
    let decoded_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-static-subset-cwd");
    let input_path = support::workspace_root().join("testdata/cff-static.otf");

    let output = support::run_fonttool_in_dir(
        [
            "subset",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--text",
            ".",
        ],
        &cwd,
    );

    assert!(
        output.status.success(),
        "expected static CFF subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "subset should create output for static CFF input"
    );
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    support::assert_decoded_otto_static_cff_output(&decoded_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn subset_cff2_variable_input_materializes_static_cff_output() {
    let output_path = support::temp_fntdata();
    let decoded_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-variable-subset-cwd");
    let input_path = support::workspace_root().join("testdata/cff2-variable.otf");

    let output = support::run_fonttool_in_dir(
        [
            "subset",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--text",
            "ABC",
            "--variation",
            "wght=700",
        ],
        &cwd,
    );

    assert!(
        output.status.success(),
        "expected variable CFF2 subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "subset should create output for variable CFF2 input"
    );
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    support::assert_decoded_otto_static_cff_output(&decoded_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn convert_static_cff_input_to_ttf_requires_explicit_command() {
    let output_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-static-convert-cwd");
    let input_path = support::workspace_root().join("testdata/cff-static.otf");

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
        "expected explicit convert to ttf to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "convert should create output for static CFF input"
    );
    support::assert_true_type_glyf_output(&output_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn convert_variable_cff2_input_materializes_then_converts_to_ttf() {
    let output_path = support::temp_ttf();
    let cwd = isolated_cwd("otf-variable-convert-cwd");
    let input_path = support::workspace_root().join("testdata/cff2-variable.otf");

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
            "--variation",
            "wght=700",
        ],
        &cwd,
    );

    assert!(
        output.status.success(),
        "expected variable CFF2 convert to ttf to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.exists(),
        "convert should create output for variable CFF2 input"
    );
    support::assert_true_type_glyf_output(&output_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}
