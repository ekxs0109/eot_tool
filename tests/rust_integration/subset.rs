mod support;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    support::workspace_root()
}

fn temp_eot() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-subset-{}-{unique}.eot",
        std::process::id()
    ))
}

#[test]
fn subset_non_otf_input_is_explicitly_phase2_owned_without_shellout() {
    let output_path = temp_eot();
    let isolated_cwd = std::env::temp_dir().join(format!(
        "fonttool-subset-cwd-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");

    let input_path = workspace_root().join("testdata/font1.fntdata");
    let output = support::run_fonttool_in_dir(
        [
            "subset",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,1",
        ],
        &isolated_cwd,
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "subset should fail with an explicit deferred-boundary error, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset execution for non-OTF input remains Phase 2-owned"),
        "expected explicit Phase 2 boundary, stderr: {stderr}"
    );
    assert!(
        !output_path.exists(),
        "subset should not create an output while the path is deferred"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}
