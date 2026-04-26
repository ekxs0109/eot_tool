use allsorts::{
    binary::read::ReadScope,
    font_data::FontData,
    tables::{FontTableProvider, SfntVersion},
};
use fonttool_sfnt::{serialize_sfnt, OwnedSfntFont};

use crate::CffError;

pub fn load_font_source(bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    let font_file = ReadScope::new(bytes)
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid font source: {error}")))?;

    if matches!(font_file, FontData::OpenType(_)) {
        return Ok(bytes.to_vec());
    }

    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid font source: {error}")))?;
    let tags = provider.table_tags().ok_or_else(|| {
        CffError::InvalidInput("materialized font source did not expose table tags".to_string())
    })?;

    let mut font = OwnedSfntFont::new(provider.sfnt_version());
    for tag in tags {
        let data = provider
            .table_data(tag)
            .map_err(|error| {
                CffError::InvalidInput(format!("failed to read source table: {error}"))
            })?
            .ok_or_else(|| {
                CffError::InvalidInput(format!(
                    "font source advertised missing table `{}`",
                    String::from_utf8_lossy(&tag.to_be_bytes())
                ))
            })?;
        font.add_table(tag, data.into_owned());
    }

    serialize_sfnt(&font).map_err(|error| {
        CffError::InvalidInput(format!("failed to serialize materialized SFNT: {error}"))
    })
}
