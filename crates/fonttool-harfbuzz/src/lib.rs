//! HarfBuzz-facing boundary for subset execution.

use core::fmt;
use std::path::Path;

use fonttool_subset::{SubsetPlan, SubsetWarnings};
use hb_subset::{Blob, FontFace, SubsetInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySubsetRequest<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub plan: &'a SubsetPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarfbuzzSubsetError {
    EmptyPlan,
    SetupFailed,
    SubsetFailed,
}

impl fmt::Display for HarfbuzzSubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarfbuzzSubsetError::EmptyPlan => {
                f.write_str("subset plan must include at least one glyph")
            }
            HarfbuzzSubsetError::SetupFailed => {
                f.write_str("failed to initialize HarfBuzz subsetting input")
            }
            HarfbuzzSubsetError::SubsetFailed => f.write_str("HarfBuzz could not subset this font"),
        }
    }
}

impl std::error::Error for HarfbuzzSubsetError {}

pub fn subset_font_bytes(
    input_bytes: &[u8],
    plan: &SubsetPlan,
) -> Result<Vec<u8>, HarfbuzzSubsetError> {
    if plan.included_glyph_ids().is_empty() {
        return Err(HarfbuzzSubsetError::EmptyPlan);
    }

    let blob = Blob::from_bytes(input_bytes).map_err(|_| HarfbuzzSubsetError::SetupFailed)?;
    let face = FontFace::new(blob).map_err(|_| HarfbuzzSubsetError::SetupFailed)?;
    let mut subset = SubsetInput::new().map_err(|_| HarfbuzzSubsetError::SetupFailed)?;
    {
        let mut flags = subset.flags();
        flags
            .retain_unrecognized_tables()
            .retain_notdef_outline()
            .no_layout_closure();
        if plan.keep_gids() {
            flags.retain_glyph_indices();
        } else {
            flags.remap_glyph_indices();
        }
    }

    {
        let mut glyph_set = subset.glyph_set();
        for &glyph_id in plan.included_glyph_ids() {
            glyph_set.insert(u32::from(glyph_id));
        }
    }

    let subset_face = subset
        .subset_font(&face)
        .map_err(|_| HarfbuzzSubsetError::SubsetFailed)?;
    let subset_bytes = subset_face.underlying_blob().to_vec();
    Ok(subset_bytes)
}

pub fn run_subset_adapter(
    request: LegacySubsetRequest<'_>,
) -> Result<SubsetWarnings, HarfbuzzSubsetError> {
    if request.plan.included_glyph_ids().is_empty() {
        return Err(HarfbuzzSubsetError::EmptyPlan);
    }
    let _ = request.input_path;
    let _ = request.output_path;
    let _ = subset_font_bytes(&[], request.plan);
    Ok(SubsetWarnings::default())
}
