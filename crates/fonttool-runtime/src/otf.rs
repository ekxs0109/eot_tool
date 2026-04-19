use std::path::Path;

use fonttool_cff::{
    inspect_otf_font, instantiate_variable_cff2, load_font_source, parse_variation_axes, CffError,
};
use fonttool_eot::{build_eot_file, EotBuildOptions, EotVersion};
use fonttool_sfnt::{load_sfnt, OwnedSfntFont};

use crate::{OutputKind, RuntimeError};

const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const OTF_INPUT_ERROR: &str = "runtime bridge expects OTF/CFF or OTF/CFF2 input";

pub fn convert_otf_path_to_embedded_bytes(
    input_path: &Path,
    output_kind: OutputKind,
    variation_axes: Option<&str>,
) -> Result<Vec<u8>, RuntimeError> {
    let raw_bytes = std::fs::read(input_path)
        .map_err(|error| RuntimeError::Io(format!("failed to read OTF input: {error}")))?;
    let source_bytes = load_font_source(&raw_bytes)?;
    let source_kind = inspect_otf_font(&source_bytes)?;
    if !source_kind.is_cff_flavor {
        return Err(RuntimeError::Backend(OTF_INPUT_ERROR.to_string()));
    }

    let otf_bytes = materialize_otf_bytes(&source_bytes, variation_axes)?;
    let font = load_sfnt(&otf_bytes)
        .map_err(|error| CffError::InvalidInput(format!("invalid SFNT: {error}")))?;
    let head = required_table(&font, TAG_HEAD, "head")?;
    let os2 = required_table(&font, TAG_OS_2, "OS/2")?;
    let name = font
        .table(TAG_NAME)
        .map(|table| table.data.as_slice())
        .unwrap_or(&[]);

    build_eot_file(
        head,
        os2,
        name,
        &otf_bytes,
        EotBuildOptions {
            version: EotVersion::V2,
            apply_ppt_xor: matches!(output_kind, OutputKind::Fntdata),
        },
    )
    .map_err(|error| RuntimeError::Backend(format!("failed to build EOT header: {error}")))
}

fn materialize_otf_bytes(
    source_bytes: &[u8],
    variation_axes: Option<&str>,
) -> Result<Vec<u8>, RuntimeError> {
    let kind = inspect_otf_font(source_bytes)?;
    if kind.is_variable {
        let axes = parse_variation_axes(variation_axes.unwrap_or_default())?;
        return instantiate_variable_cff2(source_bytes, &axes).map_err(RuntimeError::from);
    }

    if variation_axes.is_some() {
        return Err(RuntimeError::Cff(CffError::VariationRejectedForStaticInput));
    }

    Ok(source_bytes.to_vec())
}

fn required_table<'a>(
    font: &'a OwnedSfntFont,
    tag: u32,
    label: &str,
) -> Result<&'a [u8], RuntimeError> {
    font.table(tag)
        .map(|table| table.data.as_slice())
        .ok_or_else(|| RuntimeError::Cff(CffError::InvalidInput(format!("missing {label} table"))))
}
