//! Rust-facing WASM entry points built on top of the runtime crate.

use fonttool_runtime::{
    convert_otf_to_embedded_font, default_runtime_diagnostics, runtime_thread_mode, ConvertRequest,
    ConvertResult, RuntimeDiagnostics, RuntimeError, RuntimeThreadMode,
};

pub use fonttool_runtime::{OutputKind as WasmOutputKind, RuntimeError as WasmRuntimeError};

#[must_use]
pub fn wasm_runtime_thread_mode() -> RuntimeThreadMode {
    runtime_thread_mode()
}

#[must_use]
pub fn wasm_runtime_get_diagnostics() -> RuntimeDiagnostics {
    default_runtime_diagnostics()
}

pub fn wasm_convert_otf_to_embedded_font(
    request: ConvertRequest<'_>,
) -> Result<ConvertResult, RuntimeError> {
    convert_otf_to_embedded_font(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fonttool_runtime::OutputKind;
    use std::path::{Path, PathBuf};

    fn fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root should exist")
            .join(path)
    }

    #[test]
    fn exposes_runtime_mode() {
        assert!(matches!(
            wasm_runtime_thread_mode().as_str(),
            "single-thread" | "pthreads"
        ));
    }

    #[test]
    fn exposes_runtime_diagnostics() {
        let diagnostics = wasm_runtime_get_diagnostics();
        assert!(diagnostics.requested_threads >= 1);
        assert_eq!(diagnostics.effective_threads, 0);
        assert!(matches!(
            diagnostics.resolved_mode.as_str(),
            "single" | "threaded"
        ));
        assert_ne!(
            diagnostics.fallback_reason.as_deref(),
            Some("runtime scheduling diagnostics are not yet available in the Rust bridge")
        );
    }

    #[test]
    fn converts_static_cff_to_eot() {
        let result = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: None,
        })
        .expect("wasm bridge should convert static CFF input to EOT");

        assert_eq!(result.output_kind, OutputKind::Eot);
        assert!(!result.data.is_empty(), "wasm bridge should return encoded bytes");
    }

    #[test]
    fn converts_variable_cff2_to_fntdata() {
        let result = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff2-variable.otf"),
            output_kind: OutputKind::Fntdata,
            variation_axes: Some("wght=700"),
        })
        .expect("wasm bridge should materialize variable CFF2 input to .fntdata");

        assert_eq!(result.output_kind, OutputKind::Fntdata);
        assert!(
            !result.data.is_empty(),
            "wasm bridge should return encoded .fntdata bytes"
        );
    }

    #[test]
    fn surfaces_runtime_validation_errors() {
        let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: Some("wght=700"),
        })
        .expect_err("wasm bridge should surface runtime validation failures");

        assert!(matches!(
            error,
            RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
        ));
    }
}
