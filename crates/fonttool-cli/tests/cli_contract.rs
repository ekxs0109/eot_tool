use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn run_fonttool<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("fonttool binary should launch")
}

#[test]
fn help_succeeds_and_lists_top_level_commands() {
    let output = run_fonttool(["--help"]);

    assert!(output.status.success(), "expected --help to succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: fonttool <COMMAND>"),
        "expected usage banner, stdout: {stdout}"
    );
    assert!(
        stdout.contains("encode <INPUT> <OUTPUT>"),
        "expected encode command in help, stdout: {stdout}"
    );
    assert!(
        stdout.contains("decode <INPUT> <OUTPUT>"),
        "expected decode command in help, stdout: {stdout}"
    );
    assert!(
        stdout.contains("subset <INPUT> <OUTPUT>"),
        "expected subset command in help, stdout: {stdout}"
    );
}

#[test]
fn no_command_exits_success_and_prints_help() {
    let output = run_fonttool(std::iter::empty::<&str>());

    assert!(
        output.status.success(),
        "expected empty invocation to succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: fonttool <COMMAND>"),
        "expected help output for empty invocation, stdout: {stdout}"
    );
}

#[test]
fn unknown_command_exits_with_status_2_and_reports_name() {
    let output = run_fonttool(["nonesuch"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected clap-style usage error exit code"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown command `nonesuch`"),
        "expected unknown-command error, stderr: {stderr}"
    );
}

#[test]
fn decode_without_required_args_exits_with_status_2_and_contract_error() {
    let output = run_fonttool(["decode"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("decode expects INPUT and OUTPUT paths"),
        "expected decode contract error, stderr: {stderr}"
    );
}

#[test]
fn encode_without_required_args_exits_with_status_2_and_contract_error() {
    let output = run_fonttool(["encode"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("encode expects INPUT and OUTPUT paths"),
        "expected encode contract error, stderr: {stderr}"
    );
}

#[test]
fn subset_missing_flag_value_is_rejected() {
    let output = run_fonttool([
        "subset",
        "in.ttf",
        "out.eot",
        "--variation",
        "wght=700",
        "--glyph-ids",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset flag is missing a value"),
        "expected missing-value error, stderr: {stderr}"
    );
}

#[test]
fn subset_duplicate_selection_mode_is_rejected() {
    let output = run_fonttool([
        "subset",
        "in.ttf",
        "out.eot",
        "--glyph-ids",
        "1,2",
        "--text",
        "ab",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset accepts only one selection mode"),
        "expected duplicate-selection error, stderr: {stderr}"
    );
}

#[test]
fn subset_missing_selection_mode_is_rejected() {
    let output = run_fonttool([
        "subset",
        "in.ttf",
        "out.eot",
        "--variation",
        "wght=700",
    ]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subset requires either --glyph-ids or --text"),
        "expected missing-selection-mode error, stderr: {stderr}"
    );
}

#[test]
fn subset_unsupported_flag_is_rejected() {
    let output = run_fonttool(["subset", "in.ttf", "out.eot", "--bogus", "x"]);

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported subset flag `--bogus`"),
        "expected unsupported-flag error, stderr: {stderr}"
    );
}
