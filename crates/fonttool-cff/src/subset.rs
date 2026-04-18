use crate::{CffError, VariationAxisValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtfSubsetResult {
    pub sfnt_bytes: Vec<u8>,
}

pub fn subset_static_cff(_bytes: &[u8], _text: &str) -> Result<OtfSubsetResult, CffError> {
    Err(CffError::SubsetFailed(
        "static CFF subset not implemented yet".to_string(),
    ))
}

pub fn subset_variable_cff2(
    _bytes: &[u8],
    _text: &str,
    _axes: &[VariationAxisValue],
) -> Result<OtfSubsetResult, CffError> {
    Err(CffError::SubsetFailed(
        "variable CFF2 subset not implemented yet".to_string(),
    ))
}

pub fn serialize_subset_otf(result: OtfSubsetResult) -> Result<Vec<u8>, CffError> {
    Ok(result.sfnt_bytes)
}
