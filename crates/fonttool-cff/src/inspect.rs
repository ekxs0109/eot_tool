use core::fmt;

use fonttool_sfnt::load_sfnt;

const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CffFontKind {
    pub is_cff_flavor: bool,
    pub is_variable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CffError {
    MissingTextSelection,
    VariationRejectedForStaticInput,
    InvalidInput(String),
    InvalidVariationAxis(String),
    EncodeFailed(String),
    SubsetFailed(String),
}

impl fmt::Display for CffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CffError::MissingTextSelection => {
                f.write_str("subset currently requires --text for OTF input")
            }
            CffError::VariationRejectedForStaticInput => {
                f.write_str("variation arguments require a variable CFF2 input")
            }
            CffError::InvalidInput(message)
            | CffError::InvalidVariationAxis(message)
            | CffError::EncodeFailed(message)
            | CffError::SubsetFailed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CffError {}

pub fn inspect_otf_font(bytes: &[u8]) -> Result<CffFontKind, CffError> {
    let font = load_sfnt(bytes)
        .map_err(|error| CffError::InvalidInput(format!("invalid SFNT: {error}")))?;

    let has_cff = font.table(TAG_CFF).is_some();
    let has_cff2 = font.table(TAG_CFF2).is_some();
    let has_fvar = font.table(TAG_FVAR).is_some();

    Ok(CffFontKind {
        is_cff_flavor: has_cff || has_cff2,
        is_variable: has_cff2 && has_fvar,
    })
}
