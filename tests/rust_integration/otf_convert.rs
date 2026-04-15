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
fn encode_static_cff_input_is_explicitly_phase3_owned() {
    let output_path = support::temp_eot();
    let cwd = isolated_cwd("otf-phase3-cwd");
    let input_path = support::workspace_root().join("testdata/cff-static.otf");

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

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("OTF(CFF/CFF2) encode remains Phase 3-owned"),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output_path.exists(),
        "encode should not create output while the OTF chain is deferred"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn encode_otf_parity_fixture_is_explicitly_phase3_owned() {
    let output_path = support::temp_eot();
    let cwd = isolated_cwd("otf-parity-phase3-cwd");
    let fixture = support::otf_parity_fixture();

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

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("OTF(CFF/CFF2) encode remains Phase 3-owned"),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output_path.exists(),
        "encode should not create output while the OTF chain is deferred"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn subset_cff2_variable_input_is_explicitly_phase3_owned() {
    let output_path = support::temp_fntdata();
    let cwd = isolated_cwd("otf-subset-phase3-cwd");
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

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("OTF(CFF/CFF2) subset remains Phase 3-owned"),
        "expected explicit Phase 3 boundary, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output_path.exists(),
        "subset should not create output while the OTF chain is deferred"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(cwd);
}
