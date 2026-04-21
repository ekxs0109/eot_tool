use crate::CffError;

#[derive(Debug, Clone, Copy)]
pub struct OfficeStaticCff<'a> {
    pub sfnt_bytes: &'a [u8],
    pub cff_offset: usize,
    pub cff_bytes: &'a [u8],
}

const OFFICE_CFF_OFFSET: usize = 0x20e;

pub fn extract_office_static_cff(bytes: &[u8]) -> Result<OfficeStaticCff<'_>, CffError> {
    let sfnt_bytes = if bytes.first() == Some(&0) && bytes.get(1..5) == Some(b"OTTO".as_slice()) {
        &bytes[1..]
    } else {
        bytes
    };

    if !sfnt_bytes.starts_with(b"OTTO") {
        return Err(CffError::InvalidInput(
            "office static cff block1 must begin with OTTO".to_string(),
        ));
    }
    if sfnt_bytes.len() <= OFFICE_CFF_OFFSET + 4 {
        return Err(CffError::InvalidInput(
            "office static cff block1 is truncated before the cff payload".to_string(),
        ));
    }
    if sfnt_bytes[OFFICE_CFF_OFFSET..OFFICE_CFF_OFFSET + 4] != [0x04, 0x03, 0x00, 0x01] {
        return Err(CffError::InvalidInput(
            "office static cff payload does not expose the expected cff prefix".to_string(),
        ));
    }

    Ok(OfficeStaticCff {
        sfnt_bytes,
        cff_offset: OFFICE_CFF_OFFSET,
        cff_bytes: &sfnt_bytes[OFFICE_CFF_OFFSET..],
    })
}

pub fn rebuild_office_static_cff_table(_bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    Err(CffError::EncodeFailed(
        "office static cff table rebuild not implemented yet".to_string(),
    ))
}

pub fn rebuild_office_static_cff_sfnt(_bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    Err(CffError::EncodeFailed(
        "office static cff sfnt rebuild not implemented yet".to_string(),
    ))
}
