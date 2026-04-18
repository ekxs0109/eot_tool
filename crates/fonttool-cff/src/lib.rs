//! CFF/CFF2 conversion boundary for the Rust rewrite.

mod inspect;
mod subset;
mod variation;

use std::path::Path;

pub use inspect::{inspect_otf_font, CffError, CffFontKind};
pub use subset::{
    serialize_subset_otf, subset_static_cff, subset_variable_cff2, OtfSubsetResult,
};
pub use variation::{parse_variation_axes, VariationAxisValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtfSubsetRequest<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub text: &'a str,
    pub variation_axes: Option<&'a str>,
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
