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
    use std::path::Path;
    use fonttool_runtime::OutputKind;

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
        assert!(matches!(diagnostics.resolved_mode.as_str(), "single" | "threaded"));
    }

    #[test]
    fn converts_otf_via_wasm_bridge() {
        let result = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: Path::new("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: None,
        })
        .expect("wasm bridge should convert static cff otf");

        assert!(!result.data.is_empty(), "wasm bridge should return bytes");
        assert_eq!(result.output_kind, OutputKind::Eot);
    }

    #[test]
    fn surfaces_runtime_validation_errors() {
        let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
            input_path: Path::new("testdata/cff-static.otf"),
            output_kind: OutputKind::Fntdata,
            variation_axes: Some("wght=700"),
        })
        .expect_err("wasm bridge should surface runtime validation failures");

        assert!(matches!(
            error,
            RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
        ));
    }
}
