use std::process::Command;

#[test]
fn workspace_cli_smoke_is_registered() {
    let status = Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .arg("--help")
        .status()
        .expect("fonttool binary should launch");

    assert!(status.success(), "expected --help to exit successfully");
}
