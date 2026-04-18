use std::collections::BTreeSet;

use allsorts::{
    binary::read::ReadScope,
    font::read_cmap_subtable,
    font_data::FontData,
    subset::{subset, CmapTarget, SubsetProfile},
    tables::{cmap::Cmap, FontTableProvider},
    tag,
};

use crate::{inspect_otf_font, instantiate_variable_cff2, CffError, VariationAxisValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtfSubsetResult {
    pub sfnt_bytes: Vec<u8>,
}

pub fn subset_static_cff(bytes: &[u8], text: &str) -> Result<OtfSubsetResult, CffError> {
    let kind = inspect_otf_font(bytes)?;
    if !kind.is_cff_flavor || kind.is_variable {
        return Err(CffError::SubsetFailed(
            "static CFF subset requires a static CFF input".to_string(),
        ));
    }

    subset_otf_bytes(bytes, text)
}

pub fn subset_variable_cff2(
    bytes: &[u8],
    text: &str,
    axes: &[VariationAxisValue],
) -> Result<OtfSubsetResult, CffError> {
    let kind = inspect_otf_font(bytes)?;
    if !kind.is_variable {
        return Err(CffError::VariationRejectedForStaticInput);
    }

    let materialized = instantiate_variable_cff2(bytes, axes)?;
    subset_otf_bytes(&materialized, text)
}

pub fn serialize_subset_otf(result: OtfSubsetResult) -> Result<Vec<u8>, CffError> {
    Ok(result.sfnt_bytes)
}

fn subset_otf_bytes(bytes: &[u8], text: &str) -> Result<OtfSubsetResult, CffError> {
    let glyph_ids = plan_text_glyph_ids(bytes, text)?;
    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF font: {error}")))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF font: {error}")))?;

    let subset_bytes = subset(
        &provider,
        &glyph_ids,
        &SubsetProfile::Minimal,
        CmapTarget::Unicode,
    )
    .or_else(|_| {
        subset(
            &provider,
            &glyph_ids,
            &SubsetProfile::Minimal,
            CmapTarget::Unrestricted,
        )
    })
    .map_err(|error| CffError::SubsetFailed(format!("failed to subset OTF font: {error}")))?;

    Ok(OtfSubsetResult {
        sfnt_bytes: subset_bytes,
    })
}

fn plan_text_glyph_ids(bytes: &[u8], text: &str) -> Result<Vec<u16>, CffError> {
    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF font: {error}")))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF font: {error}")))?;
    let cmap_data = provider
        .read_table_data(tag::CMAP)
        .map_err(|error| CffError::InvalidInput(format!("invalid cmap table: {error}")))?;
    let cmap = ReadScope::new(&cmap_data)
        .read::<Cmap<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid cmap table: {error}")))?;
    let (_, cmap_subtable) = read_cmap_subtable(&cmap)
        .map_err(|error| CffError::InvalidInput(format!("invalid cmap table: {error}")))?
        .ok_or_else(|| CffError::SubsetFailed("font does not contain a usable cmap".to_string()))?;

    let mut planned = vec![0u16];
    let mut seen = BTreeSet::from([0u16]);

    for codepoint in text.chars().map(u32::from) {
        let Some(glyph_id) = cmap_subtable.map_glyph(codepoint).map_err(|error| {
            CffError::InvalidInput(format!("failed to read cmap entry: {error}"))
        })?
        else {
            continue;
        };
        if seen.insert(glyph_id) {
            planned.push(glyph_id);
        }
    }

    Ok(planned)
}
