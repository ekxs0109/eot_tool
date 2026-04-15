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
    fn rejects_static_cff_conversion_until_phase3() {
        let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: None,
        })
        .expect_err("static CFF conversion should stay outside the Phase 1 wasm boundary");

        assert_eq!(
            error,
            RuntimeError::Cff(fonttool_runtime::CffError::EncodeDeferredToPhase3)
        );
    }

    #[test]
    fn rejects_fntdata_output_until_phase2() {
        let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Fntdata,
            variation_axes: None,
        })
        .expect_err(".fntdata output should stay outside the Phase 1 wasm boundary");

        assert_eq!(
            error,
            RuntimeError::Backend(
                "PowerPoint-compatible .fntdata output remains Phase 2-owned; use the archived native binary for compatibility flows".to_string()
            )
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

    #[test]
    fn rejects_variable_font_conversion_until_runtime_bridge_grows_full_support() {
        let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff2-variable.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: Some("wght=700"),
        })
        .expect_err("variable conversion should stay explicitly unsupported");

        assert_eq!(
            error,
            RuntimeError::Backend(
                "runtime bridge does not yet support variable-font conversion".to_string()
            )
        );
    }
}
