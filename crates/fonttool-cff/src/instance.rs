use allsorts::{
    binary::read::ReadScope,
    binary::write::{WriteBinary, WriteBuffer},
    cff::{
        cff2::{OutputFormat, CFF2},
        CFF,
    },
    font_data::FontData,
    tables::{variable_fonts::fvar::FvarTable, Fixed, FontTableProvider},
    tag, variations,
};

use crate::{inspect_otf_font, variation::VariationAxisValue, CffError};
use fonttool_sfnt::{load_sfnt, serialize_sfnt};

const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");

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

    let user_tuple = user_tuple(&fvar, axes)?;
    let (instantiated, _) = variations::instance(&provider, &user_tuple)
        .map_err(|error| map_variation_error(error, kind.is_variable))?;

    materialize_static_cff(&instantiated)
}

fn user_tuple(fvar: &FvarTable<'_>, axes: &[VariationAxisValue]) -> Result<Vec<Fixed>, CffError> {
    let axis_records = fvar.axes().collect::<Vec<_>>();
    let mut user_tuple = axis_records
        .iter()
        .map(|axis| axis.default_value)
        .collect::<Vec<_>>();

    for axis in axes {
        let Some(index) = axis_records
            .iter()
            .position(|record| record.axis_tag.to_be_bytes() == axis.tag)
        else {
            let tag = std::str::from_utf8(&axis.tag).unwrap_or("????");
            return Err(CffError::InvalidVariationAxis(format!(
                "unknown variation axis tag `{tag}`"
            )));
        };
        user_tuple[index] = Fixed::from(axis.value);
    }

    Ok(user_tuple)
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

    if provider.has_table(TAG_CFF) && !provider.has_table(TAG_CFF2) {
        return Ok(bytes.to_vec());
    }

    let cff2_data = provider
        .read_table_data(TAG_CFF2)
        .map_err(|error| CffError::InvalidInput(format!("invalid instantiated font: {error}")))?;
    let cff2 = ReadScope::new(&cff2_data)
        .read::<CFF2<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid CFF2 table: {error}")))?;
    let glyph_ids = (0..cff2.char_strings_index.len() as u16).collect::<Vec<u16>>();
    let cff_subset = cff2
        .subset_to_cff(&glyph_ids, &provider, true, OutputFormat::Type1OrCid)
        .map_err(|error| {
            CffError::InvalidInput(format!("failed to convert CFF2 to CFF: {error}"))
        })?;

    let mut cff_data = WriteBuffer::new();
    CFF::write(&mut cff_data, &cff_subset.into()).map_err(|error| {
        CffError::InvalidInput(format!("failed to serialize CFF table: {error}"))
    })?;

    let mut font = load_sfnt(bytes)
        .map_err(|error| CffError::InvalidInput(format!("invalid instantiated SFNT: {error}")))?;
    font.remove_table(TAG_CFF2);
    font.add_table(TAG_CFF, cff_data.into_inner());
    font.remove_table(tag::FVAR);
    font.remove_table(tag::AVAR);
    font.remove_table(tag::HVAR);
    font.remove_table(tag::MVAR);
    font.remove_table(tag::CVAR);
    font.remove_table(tag::GVAR);

    serialize_sfnt(&font).map_err(|error| {
        CffError::InvalidInput(format!("failed to serialize materialized SFNT: {error}"))
    })
}
