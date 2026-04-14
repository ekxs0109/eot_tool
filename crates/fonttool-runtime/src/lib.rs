//! Runtime-facing abstractions shared by Rust-native and WASM entry points.

use core::fmt;
use std::path::{Path, PathBuf};

pub use fonttool_cff::CffError;

use fonttool_cff::{
    encode_otf_with_legacy_backend, inspect_otf_font, subset_otf_with_legacy_backend,
    OtfSubsetRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeThreadMode {
    SingleThread,
    Pthreads,
}

impl RuntimeThreadMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeThreadMode::SingleThread => "single-thread",
            RuntimeThreadMode::Pthreads => "pthreads",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeDiagnostics {
    pub requested_threads: usize,
    pub effective_threads: usize,
    pub resolved_mode: String,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Eot,
    Fntdata,
}

impl OutputKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            OutputKind::Eot => "eot",
            OutputKind::Fntdata => "fntdata",
        }
    }

    #[must_use]
    pub fn file_extension(self) -> &'static str {
        match self {
            OutputKind::Eot => "eot",
            OutputKind::Fntdata => "fntdata",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertRequest<'a> {
    pub input_path: &'a Path,
    pub output_kind: OutputKind,
    pub variation_axes: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertResult {
    pub data: Vec<u8>,
    pub diagnostics: RuntimeDiagnostics,
    pub output_kind: OutputKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidUtf8Path,
    Io(String),
    Backend(String),
    Cff(CffError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidUtf8Path => f.write_str("path must be valid utf-8"),
            RuntimeError::Io(message) => f.write_str(message),
            RuntimeError::Backend(message) => f.write_str(message),
            RuntimeError::Cff(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<CffError> for RuntimeError {
    fn from(value: CffError) -> Self {
        RuntimeError::Cff(value)
    }
}

#[must_use]
pub fn runtime_thread_mode() -> RuntimeThreadMode {
    if cfg!(target_feature = "atomics") {
        RuntimeThreadMode::Pthreads
    } else {
        RuntimeThreadMode::SingleThread
    }
}

#[must_use]
pub fn default_runtime_diagnostics() -> RuntimeDiagnostics {
    RuntimeDiagnostics {
        requested_threads: 0,
        effective_threads: 0,
        resolved_mode: match runtime_thread_mode() {
            RuntimeThreadMode::SingleThread => "single".to_string(),
            RuntimeThreadMode::Pthreads => "threaded".to_string(),
        },
        fallback_reason: None,
    }
}

pub fn convert_otf_to_embedded_font(
    request: ConvertRequest<'_>,
) -> Result<ConvertResult, RuntimeError> {
    let temp_output = temp_runtime_output_path(request.output_kind);
    let input_path = resolve_workspace_path(request.input_path);
    run_conversion_request(request, &input_path, &temp_output)?;
    let data = std::fs::read(&temp_output)
        .map_err(|error| RuntimeError::Io(format!("failed to read runtime output: {error}")))?;
    let _ = std::fs::remove_file(&temp_output);

    Ok(ConvertResult {
        data,
        diagnostics: default_runtime_diagnostics(),
        output_kind: request.output_kind,
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn resolve_workspace_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root().join(path)
    }
}

fn run_conversion_request(
    request: ConvertRequest<'_>,
    input_path: &Path,
    output_path: &Path,
) -> Result<(), RuntimeError> {
    if let Some(variation_axes) = request.variation_axes {
        let font_bytes = std::fs::read(input_path)
            .map_err(|error| RuntimeError::Io(format!("failed to read OTF input: {error}")))?;
        let kind = inspect_otf_font(&font_bytes)?;
        if !kind.is_cff_flavor {
            return Err(RuntimeError::Backend(
                "runtime bridge expects OTF/CFF or OTF/CFF2 input".to_string(),
            ));
        }
        if !kind.is_variable {
            return Err(RuntimeError::Cff(
                CffError::VariationRejectedForStaticInput,
            ));
        }

        return subset_otf_with_legacy_backend(OtfSubsetRequest {
            input_path,
            output_path,
            text: "ABC",
            variation_axes: Some(variation_axes),
        })
        .map_err(RuntimeError::from);
    }

    encode_otf_with_legacy_backend(input_path, output_path).map_err(RuntimeError::from)
}

fn temp_runtime_output_path(output_kind: OutputKind) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-runtime-{}-{unique}.{}",
        std::process::id(),
        output_kind.file_extension()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reports_runtime_mode_string() {
        assert!(matches!(
            runtime_thread_mode().as_str(),
            "single-thread" | "pthreads"
        ));
    }

    #[test]
    fn converts_static_otf_through_runtime_bridge() {
        let result = convert_otf_to_embedded_font(ConvertRequest {
            input_path: Path::new("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: None,
        })
        .expect("runtime bridge should encode static cff");

        assert!(!result.data.is_empty(), "runtime bridge should produce bytes");
        assert_eq!(result.output_kind, OutputKind::Eot);
    }

    #[test]
    fn rejects_variation_axes_for_static_otf() {
        let error = convert_otf_to_embedded_font(ConvertRequest {
            input_path: Path::new("testdata/cff-static.otf"),
            output_kind: OutputKind::Fntdata,
            variation_axes: Some("wght=700"),
        })
        .expect_err("static CFF input should reject variation axes");

        assert_eq!(
            error,
            RuntimeError::Cff(CffError::VariationRejectedForStaticInput)
        );
    }
}
