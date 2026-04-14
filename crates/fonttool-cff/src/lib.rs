//! CFF/CFF2 conversion boundary for the Rust rewrite.

use core::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use fonttool_sfnt::load_sfnt;

const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtfSubsetRequest<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub text: &'a str,
    pub variation_axes: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CffError {
    InvalidUtf8Path,
    MissingTextSelection,
    VariationRejectedForStaticInput,
    InvalidInput(String),
    LegacyBinaryFailed(String),
    Io(String),
}

impl fmt::Display for CffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CffError::InvalidUtf8Path => f.write_str("path must be valid utf-8"),
            CffError::MissingTextSelection => {
                f.write_str("Task 7 currently only supports --text for OTF subset input")
            }
            CffError::VariationRejectedForStaticInput => {
                f.write_str("variation arguments require a variable CFF2 input")
            }
            CffError::InvalidInput(message) => f.write_str(message),
            CffError::LegacyBinaryFailed(message) => f.write_str(message),
            CffError::Io(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CffError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CffFontKind {
    pub is_cff_flavor: bool,
    pub is_variable: bool,
}

pub fn inspect_otf_font(bytes: &[u8]) -> Result<CffFontKind, CffError> {
    let font = load_sfnt(bytes).map_err(|error| CffError::InvalidInput(format!("invalid SFNT: {error}")))?;
    Ok(CffFontKind {
        is_cff_flavor: font.table(TAG_CFF).is_some() || font.table(TAG_CFF2).is_some(),
        is_variable: font.table(TAG_CFF2).is_some() && font.table(TAG_FVAR).is_some(),
    })
}

pub fn encode_otf_with_legacy_backend(
    input_path: &Path,
    output_path: &Path,
) -> Result<(), CffError> {
    run_legacy_fonttool([
        "encode",
        path_to_utf8(input_path)?,
        path_to_utf8(output_path)?,
    ])
}

pub fn subset_otf_with_legacy_backend(
    request: OtfSubsetRequest<'_>,
) -> Result<(), CffError> {
    if request.text.is_empty() {
        return Err(CffError::MissingTextSelection);
    }

    let mut args = vec![
        "subset".to_string(),
        path_to_utf8(request.input_path)?.to_string(),
        path_to_utf8(request.output_path)?.to_string(),
        "--text".to_string(),
        request.text.to_string(),
    ];
    if let Some(variation_axes) = request.variation_axes {
        args.push("--variation".to_string());
        args.push(variation_axes.to_string());
    }

    run_legacy_fonttool(args)
}

fn path_to_utf8(path: &Path) -> Result<&str, CffError> {
    path.to_str().ok_or(CffError::InvalidUtf8Path)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn run_legacy_fonttool<I, S>(args: I) -> Result<(), CffError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let legacy_binary = workspace_root().join("build/fonttool");
    if !legacy_binary.exists() {
        return Err(CffError::Io(format!(
            "legacy fonttool binary is missing at {}",
            legacy_binary.display()
        )));
    }

    let output = Command::new(&legacy_binary)
        .args(args)
        .output()
        .map_err(|error| CffError::Io(format!("failed to launch legacy fonttool backend: {error}")))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(CffError::LegacyBinaryFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}
