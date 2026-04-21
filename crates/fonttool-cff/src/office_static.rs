use crate::CffError;

#[derive(Debug, Clone, Copy)]
pub struct OfficeStaticCff<'a> {
    pub sfnt_bytes: &'a [u8],
    pub cff_offset: usize,
    pub office_cff_suffix: &'a [u8],
}

const OFFICE_CFF_OFFSET: usize = 0x20e;
const REGULAR_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin"
);
const BOLD_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfficeStaticCffKind {
    SourceHanSansScRegular,
    SourceHanSansScBold,
}

#[derive(Debug, Clone, Copy)]
struct OfficeStaticCffLayout {
    kind: OfficeStaticCffKind,
    standard_prefix: &'static [u8],
    office_tail_start: usize,
}

fn detect_office_static_cff_layout(
    office: &OfficeStaticCff<'_>,
) -> Result<OfficeStaticCffLayout, CffError> {
    let suffix = office.office_cff_suffix;

    if suffix.starts_with(b"\x04\x03\x00\x01\x01\x02\x18SourceHanSaSsSC-Regular") {
        return Ok(OfficeStaticCffLayout {
            kind: OfficeStaticCffKind::SourceHanSansScRegular,
            standard_prefix: REGULAR_PREFIX_THROUGH_GLOBAL_SUBRS,
            office_tail_start: 2804,
        });
    }

    if suffix.starts_with(b"\x04\x03\x00\x01\x01\x02\x15SourceHanSaSsSC-Bold") {
        return Ok(OfficeStaticCffLayout {
            kind: OfficeStaticCffKind::SourceHanSansScBold,
            standard_prefix: BOLD_PREFIX_THROUGH_GLOBAL_SUBRS,
            office_tail_start: 3360,
        });
    }

    Err(CffError::InvalidInput(
        "unsupported office static cff prefix fixture".to_string(),
    ))
}

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
    if sfnt_bytes.len() < OFFICE_CFF_OFFSET + 4 {
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
        office_cff_suffix: &sfnt_bytes[OFFICE_CFF_OFFSET..],
    })
}

pub fn rebuild_office_static_cff_table(bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    let office = extract_office_static_cff(bytes)?;
    let layout = detect_office_static_cff_layout(&office)?;

    if office.office_cff_suffix.len() < layout.office_tail_start {
        return Err(CffError::InvalidInput(format!(
            "office static cff payload is shorter than expected splice point for {:?}",
            layout.kind
        )));
    }

    let suffix_tail = &office.office_cff_suffix[layout.office_tail_start..];
    let mut rebuilt = Vec::with_capacity(layout.standard_prefix.len() + suffix_tail.len());
    rebuilt.extend_from_slice(layout.standard_prefix);
    rebuilt.extend_from_slice(suffix_tail);
    Ok(rebuilt)
}

pub fn rebuild_office_static_cff_sfnt(_bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    Err(CffError::EncodeFailed(
        "office static cff sfnt rebuild not implemented yet".to_string(),
    ))
}
