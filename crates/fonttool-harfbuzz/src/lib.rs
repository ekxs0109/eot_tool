//! HarfBuzz-facing boundary for subset execution.

use core::fmt;
use std::path::Path;

use fonttool_subset::{SubsetPlan, SubsetWarnings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySubsetRequest<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub plan: &'a SubsetPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarfbuzzSubsetError {
    EmptyPlan,
    DeferredToPhase2,
}

impl fmt::Display for HarfbuzzSubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarfbuzzSubsetError::EmptyPlan => {
                f.write_str("subset plan must include at least one glyph")
            }
            HarfbuzzSubsetError::DeferredToPhase2 => f.write_str(
                "subset execution for non-OTF input remains Phase 2-owned; use the archived native binary for compatibility flows",
            ),
        }
    }
}

impl std::error::Error for HarfbuzzSubsetError {}

pub fn run_subset_adapter(
    request: LegacySubsetRequest<'_>,
) -> Result<SubsetWarnings, HarfbuzzSubsetError> {
    if request.plan.included_glyph_ids().is_empty() {
        return Err(HarfbuzzSubsetError::EmptyPlan);
    }
    let _ = request.input_path;
    let _ = request.output_path;
    Err(HarfbuzzSubsetError::DeferredToPhase2)
}
