//! HarfBuzz-facing boundary for subset execution.

use core::fmt;
use std::path::Path;
use std::process::Command;

use fonttool_subset::{SubsetPlan, SubsetWarnings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySubsetRequest<'a> {
    pub input_path: &'a Path,
    pub output_path: &'a Path,
    pub plan: &'a SubsetPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarfbuzzSubsetError {
    LegacyBinaryFailed(String),
    InvalidUtf8Path,
    EmptyPlan,
    Io(String),
}

impl fmt::Display for HarfbuzzSubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarfbuzzSubsetError::LegacyBinaryFailed(stderr) => {
                write!(f, "legacy subset backend failed: {stderr}")
            }
            HarfbuzzSubsetError::InvalidUtf8Path => {
                f.write_str("subset paths must be valid utf-8 for the legacy adapter")
            }
            HarfbuzzSubsetError::EmptyPlan => {
                f.write_str("subset plan must include at least one glyph")
            }
            HarfbuzzSubsetError::Io(message) => f.write_str(message),
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

    let input = request
        .input_path
        .to_str()
        .ok_or(HarfbuzzSubsetError::InvalidUtf8Path)?;
    let output = request
        .output_path
        .to_str()
        .ok_or(HarfbuzzSubsetError::InvalidUtf8Path)?;
    let glyph_ids = request
        .plan
        .included_glyph_ids()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",");

    let legacy_binary = Path::new("build/fonttool");
    if !legacy_binary.exists() {
        return Err(HarfbuzzSubsetError::Io(format!(
            "legacy subset binary is missing at {}",
            legacy_binary.display()
        )));
    }

    let output = Command::new(legacy_binary)
        .arg("subset")
        .arg(input)
        .arg(output)
        .arg("--glyph-ids")
        .arg(glyph_ids)
        .output()
        .map_err(|error| HarfbuzzSubsetError::Io(format!("failed to launch legacy subset backend: {error}")))?;

    if !output.status.success() {
        return Err(HarfbuzzSubsetError::LegacyBinaryFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(SubsetWarnings {
        dropped_hdmx: stderr.contains("warning: unsupported HDMX in subset path; dropping table"),
        dropped_vdmx: stderr.contains(
            "warning: unsupported VDMX in MTX encode/subset path; dropping table",
        ),
    })
}
