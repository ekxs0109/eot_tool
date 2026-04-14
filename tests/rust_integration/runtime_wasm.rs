use std::path::{Path, PathBuf};

use fonttool_runtime::{
    convert_otf_to_embedded_font, default_runtime_diagnostics, runtime_thread_mode, ConvertRequest,
    OutputKind, RuntimeError,
};
use fonttool_wasm::{
    wasm_convert_otf_to_embedded_font, wasm_runtime_get_diagnostics, wasm_runtime_thread_mode,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn fixture(path: &str) -> PathBuf {
    workspace_root().join(path)
}

#[test]
fn runtime_mode_matches_supported_strings() {
    assert!(matches!(
        runtime_thread_mode().as_str(),
        "single-thread" | "pthreads"
    ));
    assert!(matches!(
        wasm_runtime_thread_mode().as_str(),
        "single-thread" | "pthreads"
    ));
}

#[test]
fn runtime_and_wasm_default_diagnostics_stay_in_sync() {
    let runtime = default_runtime_diagnostics();
    let wasm = wasm_runtime_get_diagnostics();

    assert_eq!(runtime, wasm);
    assert_eq!(runtime.requested_threads, 0);
    assert_eq!(runtime.effective_threads, 0);
    assert!(matches!(runtime.resolved_mode.as_str(), "single" | "threaded"));
    assert_eq!(
        runtime.fallback_reason.as_deref(),
        Some("runtime scheduling diagnostics are not yet available in the Rust bridge")
    );
}

#[test]
fn runtime_bridge_converts_static_cff_fixture() {
    let result = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: None,
    })
    .expect("runtime bridge should convert static cff input");

    assert!(!result.data.is_empty(), "runtime bridge should emit bytes");
    assert_eq!(result.output_kind, OutputKind::Eot);
    assert_eq!(result.diagnostics, default_runtime_diagnostics());
}

#[test]
fn wasm_bridge_converts_static_cff_fixture() {
    let result = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: None,
    })
    .expect("wasm bridge should convert static cff input");

    assert!(!result.data.is_empty(), "wasm bridge should emit bytes");
    assert_eq!(result.output_kind, OutputKind::Fntdata);
}

#[test]
fn runtime_bridge_rejects_variation_axes_for_static_input() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: Some("wght=700"),
    })
    .expect_err("static cff input should reject variation request");

    assert_eq!(
        error,
        RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
    );
}

#[test]
fn wasm_bridge_surfaces_runtime_validation_errors() {
    let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: Some("wght=700"),
    })
    .expect_err("wasm bridge should surface runtime validation errors");

    assert!(matches!(
        error,
        RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
    ));
}

#[test]
fn runtime_bridge_rejects_variable_font_conversion_for_now() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff2-variable.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: Some("wght=700"),
    })
    .expect_err("variable conversion should stay explicitly unsupported");

    assert_eq!(
        error,
        RuntimeError::Backend("runtime bridge does not yet support variable-font conversion".to_string())
    );
}

#[test]
fn wasm_bridge_rejects_variable_font_conversion_for_now() {
    let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff2-variable.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: Some("wght=700"),
    })
    .expect_err("variable conversion should stay explicitly unsupported");

    assert_eq!(
        error,
        RuntimeError::Backend("runtime bridge does not yet support variable-font conversion".to_string())
    );
}
