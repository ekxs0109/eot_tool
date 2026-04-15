//! CFF/CFF2 conversion boundary for the Rust rewrite.

use core::fmt;
use std::path::Path;

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
    MissingTextSelection,
    VariationRejectedForStaticInput,
    InvalidInput(String),
    EncodeDeferredToPhase3,
    SubsetDeferredToPhase3,
}

impl fmt::Display for CffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CffError::MissingTextSelection => {
                f.write_str("subset currently requires --text for OTF input")
            }
            CffError::VariationRejectedForStaticInput => {
                f.write_str("variation arguments require a variable CFF2 input")
            }
            CffError::InvalidInput(message) => f.write_str(message),
            CffError::EncodeDeferredToPhase3 => f.write_str(
                "OTF(CFF/CFF2) encode remains Phase 3-owned; use the archived native binary for compatibility flows",
            ),
            CffError::SubsetDeferredToPhase3 => f.write_str(
                "OTF(CFF/CFF2) subset remains Phase 3-owned; use the archived native binary for compatibility flows",
            ),
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
    let font = load_sfnt(bytes)
        .map_err(|error| CffError::InvalidInput(format!("invalid SFNT: {error}")))?;
    Ok(CffFontKind {
        is_cff_flavor: font.table(TAG_CFF).is_some() || font.table(TAG_CFF2).is_some(),
        is_variable: font.table(TAG_CFF2).is_some() && font.table(TAG_FVAR).is_some(),
    })
}

pub fn encode_otf_with_legacy_backend(
    _input_path: &Path,
    _output_path: &Path,
) -> Result<(), CffError> {
    Err(CffError::EncodeDeferredToPhase3)
}

pub fn subset_otf_with_legacy_backend(request: OtfSubsetRequest<'_>) -> Result<(), CffError> {
    if request.text.is_empty() {
        return Err(CffError::MissingTextSelection);
    }

    let _ = request.variation_axes;
    Err(CffError::SubsetDeferredToPhase3)
}
