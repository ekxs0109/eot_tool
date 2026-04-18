use allsorts::{
    binary::read::ReadScope,
    font_data::FontData,
    subset::{self, CmapTarget, SubsetProfile},
    tables::{variable_fonts::fvar::FvarTable, Fixed, FontTableProvider, MaxpTable},
    tag, variations,
};

use crate::{
    inspect_otf_font,
    variation::{axis_value_for_tag, VariationAxisValue},
    CffError,
};

pub fn instantiate_variable_cff2(
    bytes: &[u8],
    axes: &[VariationAxisValue],
) -> Result<Vec<u8>, CffError> {
    let kind = inspect_otf_font(bytes)?;
    if !kind.is_variable {
        return Err(CffError::VariationRejectedForStaticInput);
    }

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid variable font: {error}")))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid variable font: {error}")))?;

    let fvar_data = provider
        .read_table_data(tag::FVAR)
        .map_err(|error| CffError::InvalidInput(format!("invalid variable font: {error}")))?;
    let fvar = ReadScope::new(&fvar_data)
        .read::<FvarTable<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid fvar table: {error}")))?;

    let user_tuple = user_tuple(&fvar, axes);
    let (instantiated, _) = variations::instance(&provider, &user_tuple)
        .map_err(|error| map_variation_error(error, kind.is_variable))?;

    materialize_static_cff(&instantiated)
}

fn user_tuple(fvar: &FvarTable<'_>, axes: &[VariationAxisValue]) -> Vec<Fixed> {
    fvar.axes()
        .map(|axis| {
            axis_value_for_tag(axes, axis.axis_tag.to_be_bytes())
                .map(|axis_value| Fixed::from(axis_value.value))
                .unwrap_or(axis.default_value)
        })
        .collect()
}

fn map_variation_error(error: variations::VariationError, is_variable_input: bool) -> CffError {
    match error {
        variations::VariationError::NotVariableFont if !is_variable_input => {
            CffError::VariationRejectedForStaticInput
        }
        other => CffError::InvalidInput(format!("failed to instantiate variable font: {other}")),
    }
}

fn materialize_static_cff(bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid instantiated font: {error}")))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid instantiated font: {error}")))?;
    let maxp_data = provider
        .read_table_data(tag::MAXP)
        .map_err(|error| CffError::InvalidInput(format!("invalid instantiated font: {error}")))?;
    let maxp = ReadScope::new(&maxp_data)
        .read::<MaxpTable>()
        .map_err(|error| CffError::InvalidInput(format!("invalid maxp table: {error}")))?;
    let glyph_ids = (0..maxp.num_glyphs).collect::<Vec<u16>>();

    subset::subset(
        &provider,
        &glyph_ids,
        &SubsetProfile::Minimal,
        CmapTarget::Unrestricted,
    )
    .map_err(|error| CffError::InvalidInput(format!("failed to materialize static CFF: {error}")))
}
