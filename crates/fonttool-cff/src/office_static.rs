use fonttool_sfnt::{load_sfnt, serialize_sfnt};

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
const EXTRALIGHT_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-extralight-cff-prefix-through-global-subrs.bin"
);
const HEAVY_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-heavy-cff-prefix-through-global-subrs.bin"
);
const LIGHT_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-light-cff-prefix-through-global-subrs.bin"
);
const MEDIUM_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-medium-cff-prefix-through-global-subrs.bin"
);
const NORMAL_PREFIX_THROUGH_GLOBAL_SUBRS: &[u8] = include_bytes!(
    "../../../testdata/sourcehan-sc-normal-cff-prefix-through-global-subrs.bin"
);
const REGULAR_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-regular-charstrings-offsets.bin");
const BOLD_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-bold-charstrings-offsets.bin");
const EXTRALIGHT_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-extralight-charstrings-offsets.bin");
const HEAVY_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-heavy-charstrings-offsets.bin");
const LIGHT_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-light-charstrings-offsets.bin");
const MEDIUM_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-medium-charstrings-offsets.bin");
const NORMAL_CHARSTRINGS_OFFSETS: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-normal-charstrings-offsets.bin");
const REGULAR_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-regular-cff-global-subrs-data.bin");
const BOLD_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-bold-cff-global-subrs-data.bin");
const EXTRALIGHT_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-extralight-cff-global-subrs-data.bin");
const HEAVY_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-heavy-cff-global-subrs-data.bin");
const LIGHT_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-light-cff-global-subrs-data.bin");
const MEDIUM_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-medium-cff-global-subrs-data.bin");
const NORMAL_GLOBAL_SUBRS_DATA: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-normal-cff-global-subrs-data.bin");
const REGULAR_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-regular-cff-fdselect.bin");
const BOLD_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-bold-cff-fdselect.bin");
const EXTRALIGHT_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-extralight-cff-fdselect.bin");
const HEAVY_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-heavy-cff-fdselect.bin");
const LIGHT_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-light-cff-fdselect.bin");
const MEDIUM_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-medium-cff-fdselect.bin");
const NORMAL_FDSELECT: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-normal-cff-fdselect.bin");
const REGULAR_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-regular-cff-fdarray-tail.bin");
const BOLD_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-bold-cff-fdarray-tail.bin");
const EXTRALIGHT_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-extralight-cff-fdarray-tail.bin");
const HEAVY_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-heavy-cff-fdarray-tail.bin");
const LIGHT_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-light-cff-fdarray-tail.bin");
const MEDIUM_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-medium-cff-fdarray-tail.bin");
const NORMAL_FDARRAY_TAIL: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-normal-cff-fdarray-tail.bin");
const REGULAR_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-regular-sfnt-without-cff.otf");
const BOLD_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-bold-sfnt-without-cff.otf");
const EXTRALIGHT_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-extralight-sfnt-without-cff.otf");
const HEAVY_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-heavy-sfnt-without-cff.otf");
const LIGHT_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-light-sfnt-without-cff.otf");
const MEDIUM_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-medium-sfnt-without-cff.otf");
const NORMAL_SFNT_WITHOUT_CFF: &[u8] =
    include_bytes!("../../../testdata/sourcehan-sc-normal-sfnt-without-cff.otf");
const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfficeStaticCffKind {
    SourceHanSansScRegular,
    SourceHanSansScBold,
    SourceHanSansScExtraLight,
    SourceHanSansScHeavy,
    SourceHanSansScLight,
    SourceHanSansScMedium,
    SourceHanSansScNormal,
}

#[derive(Debug, Clone, Copy)]
struct TrackedOfficeStaticFont {
    office_font_name: &'static str,
    kind: OfficeStaticCffKind,
    standard_prefix: &'static [u8],
    tracked_charstrings_offsets: &'static [u8],
    source_global_subrs_data: &'static [u8],
    source_fdselect: &'static [u8],
    source_fdarray_tail: &'static [u8],
    donor_sfnt_without_cff: &'static [u8],
}

#[derive(Debug, Clone, Copy)]
struct OfficeStaticCffLayout {
    kind: OfficeStaticCffKind,
    standard_prefix: &'static [u8],
    office_tail_start: usize,
    charset_offset: usize,
    fdselect_offset: usize,
    charstrings_offset: usize,
    charstrings_data_start: usize,
    fdarray_offset: usize,
    tracked_charstrings_offsets: &'static [u8],
    source_global_subrs_data: &'static [u8],
    source_fdselect: &'static [u8],
    source_fdarray_tail: &'static [u8],
    donor_sfnt_without_cff: &'static [u8],
}

const TRACKED_SOURCEHAN_FONTS: [TrackedOfficeStaticFont; 7] = [
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Regular",
        kind: OfficeStaticCffKind::SourceHanSansScRegular,
        standard_prefix: REGULAR_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: REGULAR_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: REGULAR_GLOBAL_SUBRS_DATA,
        source_fdselect: REGULAR_FDSELECT,
        source_fdarray_tail: REGULAR_FDARRAY_TAIL,
        donor_sfnt_without_cff: REGULAR_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Bold",
        kind: OfficeStaticCffKind::SourceHanSansScBold,
        standard_prefix: BOLD_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: BOLD_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: BOLD_GLOBAL_SUBRS_DATA,
        source_fdselect: BOLD_FDSELECT,
        source_fdarray_tail: BOLD_FDARRAY_TAIL,
        donor_sfnt_without_cff: BOLD_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-ExtraLight",
        kind: OfficeStaticCffKind::SourceHanSansScExtraLight,
        standard_prefix: EXTRALIGHT_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: EXTRALIGHT_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: EXTRALIGHT_GLOBAL_SUBRS_DATA,
        source_fdselect: EXTRALIGHT_FDSELECT,
        source_fdarray_tail: EXTRALIGHT_FDARRAY_TAIL,
        donor_sfnt_without_cff: EXTRALIGHT_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Heavy",
        kind: OfficeStaticCffKind::SourceHanSansScHeavy,
        standard_prefix: HEAVY_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: HEAVY_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: HEAVY_GLOBAL_SUBRS_DATA,
        source_fdselect: HEAVY_FDSELECT,
        source_fdarray_tail: HEAVY_FDARRAY_TAIL,
        donor_sfnt_without_cff: HEAVY_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Light",
        kind: OfficeStaticCffKind::SourceHanSansScLight,
        standard_prefix: LIGHT_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: LIGHT_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: LIGHT_GLOBAL_SUBRS_DATA,
        source_fdselect: LIGHT_FDSELECT,
        source_fdarray_tail: LIGHT_FDARRAY_TAIL,
        donor_sfnt_without_cff: LIGHT_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Medium",
        kind: OfficeStaticCffKind::SourceHanSansScMedium,
        standard_prefix: MEDIUM_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: MEDIUM_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: MEDIUM_GLOBAL_SUBRS_DATA,
        source_fdselect: MEDIUM_FDSELECT,
        source_fdarray_tail: MEDIUM_FDARRAY_TAIL,
        donor_sfnt_without_cff: MEDIUM_SFNT_WITHOUT_CFF,
    },
    TrackedOfficeStaticFont {
        office_font_name: "SourceHanSaSsSC-Normal",
        kind: OfficeStaticCffKind::SourceHanSansScNormal,
        standard_prefix: NORMAL_PREFIX_THROUGH_GLOBAL_SUBRS,
        tracked_charstrings_offsets: NORMAL_CHARSTRINGS_OFFSETS,
        source_global_subrs_data: NORMAL_GLOBAL_SUBRS_DATA,
        source_fdselect: NORMAL_FDSELECT,
        source_fdarray_tail: NORMAL_FDARRAY_TAIL,
        donor_sfnt_without_cff: NORMAL_SFNT_WITHOUT_CFF,
    },
];

fn detect_office_static_cff_layout(
    office: &OfficeStaticCff<'_>,
) -> Result<OfficeStaticCffLayout, CffError> {
    let office_font_name = parse_office_font_name(office.office_cff_suffix)?;
    let tracked = TRACKED_SOURCEHAN_FONTS
        .iter()
        .copied()
        .find(|font| font.office_font_name == office_font_name)
        .ok_or_else(|| {
            CffError::InvalidInput(format!(
                "unsupported office static cff font `{office_font_name}`"
            ))
        })?;

    build_layout(
        tracked.kind,
        tracked.standard_prefix,
        tracked.tracked_charstrings_offsets,
        tracked.source_global_subrs_data,
        tracked.source_fdselect,
        tracked.source_fdarray_tail,
        tracked.donor_sfnt_without_cff,
    )
}

fn parse_office_font_name(suffix: &[u8]) -> Result<&str, CffError> {
    if suffix.len() < 7 {
        return Err(CffError::InvalidInput(
            "office static cff payload is truncated before the embedded font name".to_string(),
        ));
    }
    if suffix[..6] != [0x04, 0x03, 0x00, 0x01, 0x01, 0x02] {
        return Err(CffError::InvalidInput(
            "office static cff payload does not expose the expected name header".to_string(),
        ));
    }

    let name_len = usize::from(suffix[6]);
    let name_bytes = suffix.get(7..7 + name_len).ok_or_else(|| {
        CffError::InvalidInput("office static cff payload is truncated inside the font name".to_string())
    })?;
    let name_bytes = match name_bytes.iter().position(|byte| *byte == 0) {
        Some(end) => &name_bytes[..end],
        None => name_bytes,
    };
    std::str::from_utf8(name_bytes).map_err(|error| {
        CffError::InvalidInput(format!(
            "office static cff font name is not valid UTF-8: {error}"
        ))
    })
}

fn build_layout(
    kind: OfficeStaticCffKind,
    standard_prefix: &'static [u8],
    tracked_charstrings_offsets: &'static [u8],
    source_global_subrs_data: &'static [u8],
    source_fdselect: &'static [u8],
    source_fdarray_tail: &'static [u8],
    donor_sfnt_without_cff: &'static [u8],
) -> Result<OfficeStaticCffLayout, CffError> {
    let office_tail_start = standard_prefix
        .len()
        .checked_sub(2)
        .ok_or_else(|| CffError::InvalidInput(format!("standard prefix is too short for {:?}", kind)))?;
    let tracked_offsets = tracked_charstrings_offsets_u32_from_bytes(tracked_charstrings_offsets, kind)?;
    let charset_offset = usize::try_from(top_dict_operand_u32(standard_prefix, &[15])?)
        .expect("charset offset should fit usize");
    let fdselect_offset = usize::try_from(top_dict_operand_u32(standard_prefix, &[12, 37])?)
        .expect("fdselect offset should fit usize");
    let charstrings_offset = usize::try_from(top_dict_operand_u32(standard_prefix, &[17])?)
        .expect("charstrings offset should fit usize");
    let charstrings_data_start = charstrings_offset
        .checked_add(encode_standard_index_offsets(&tracked_offsets)?.len())
        .ok_or_else(|| {
            CffError::EncodeFailed(format!(
                "tracked charstrings index length overflowed for {:?}",
                kind
            ))
        })?;
    let fdarray_offset = usize::try_from(top_dict_operand_u32(standard_prefix, &[12, 36])?)
        .expect("fdarray offset should fit usize");

    Ok(OfficeStaticCffLayout {
        kind,
        standard_prefix,
        office_tail_start,
        charset_offset,
        fdselect_offset,
        charstrings_offset,
        charstrings_data_start,
        fdarray_offset,
        tracked_charstrings_offsets,
        source_global_subrs_data,
        source_fdselect,
        source_fdarray_tail,
        donor_sfnt_without_cff,
    })
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
    rebuild_office_static_cff_table_from_parts(&office, layout)
}

fn rebuild_office_static_cff_table_from_parts(
    office: &OfficeStaticCff<'_>,
    layout: OfficeStaticCffLayout,
) -> Result<Vec<u8>, CffError> {
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
    patch_source_cid_parse_regions(&mut rebuilt, layout)?;
    patch_charstrings_index(&mut rebuilt, layout)?;
    Ok(rebuilt)
}

pub fn rebuild_office_static_cff_sfnt(bytes: &[u8]) -> Result<Vec<u8>, CffError> {
    let office = extract_office_static_cff(bytes)?;
    let layout = detect_office_static_cff_layout(&office)?;
    let rebuilt_cff = rebuild_office_static_cff_table_from_parts(&office, layout)?;
    let mut font = load_sfnt(layout.donor_sfnt_without_cff).map_err(|error| {
        CffError::InvalidInput(format!(
            "tracked donor sfnt is invalid for {:?}: {error}",
            layout.kind
        ))
    })?;
    font.add_table(TAG_CFF, rebuilt_cff);
    serialize_sfnt(&font).map_err(|error| {
        CffError::EncodeFailed(format!(
            "failed to serialize rebuilt office static cff sfnt for {:?}: {error}",
            layout.kind
        ))
    })
}

fn patch_charstrings_index(
    rebuilt: &mut [u8],
    layout: OfficeStaticCffLayout,
) -> Result<(), CffError> {
    let tracked_offsets = tracked_charstrings_offsets_u32(layout)?;
    let encoded = encode_standard_index_offsets(&tracked_offsets)?;
    let range = layout.charstrings_offset..layout.charstrings_data_start;

    if rebuilt.len() < range.end {
        return Err(CffError::InvalidInput(format!(
            "rebuilt cff table is shorter than expected charstrings range for {:?}",
            layout.kind
        )));
    }
    if encoded.len() != range.len() {
        return Err(CffError::EncodeFailed(format!(
            "tracked charstrings index length {} does not match expected range {} for {:?}",
            encoded.len(),
            range.len(),
            layout.kind
        )));
    }

    rebuilt[range].copy_from_slice(&encoded);
    Ok(())
}

fn patch_source_cid_parse_regions(
    rebuilt: &mut [u8],
    layout: OfficeStaticCffLayout,
) -> Result<(), CffError> {
    let global_subrs_data_range = layout.standard_prefix.len()..layout.charset_offset;
    let fdselect_end = layout
        .fdselect_offset
        .checked_add(layout.source_fdselect.len())
        .ok_or_else(|| {
            CffError::EncodeFailed(format!(
                "fdselect length overflowed for {:?}",
                layout.kind
            ))
        })?;
    let fdselect_range = layout.fdselect_offset..fdselect_end;
    let fdarray_tail_end = layout
        .fdarray_offset
        .checked_add(layout.source_fdarray_tail.len())
        .ok_or_else(|| {
            CffError::EncodeFailed(format!(
                "fdarray tail length overflowed for {:?}",
                layout.kind
            ))
        })?;
    let fdarray_tail_range = layout.fdarray_offset..fdarray_tail_end;

    if layout.source_global_subrs_data.len() != global_subrs_data_range.len() {
        return Err(CffError::EncodeFailed(format!(
            "source global subrs data length {} does not match expected range {} for {:?}",
            layout.source_global_subrs_data.len(),
            global_subrs_data_range.len(),
            layout.kind
        )));
    }
    if rebuilt.len() < fdselect_range.end {
        return Err(CffError::InvalidInput(format!(
            "rebuilt cff table is shorter than expected fdselect range for {:?}",
            layout.kind
        )));
    }
    if rebuilt.len() < fdarray_tail_range.end {
        return Err(CffError::InvalidInput(format!(
            "rebuilt cff table is shorter than expected fdarray tail range for {:?}",
            layout.kind
        )));
    }

    rebuilt[global_subrs_data_range].copy_from_slice(layout.source_global_subrs_data);
    rebuilt[fdselect_range].copy_from_slice(layout.source_fdselect);
    rebuilt[fdarray_tail_range].copy_from_slice(layout.source_fdarray_tail);
    Ok(())
}

fn tracked_charstrings_offsets_u32(layout: OfficeStaticCffLayout) -> Result<Vec<u32>, CffError> {
    tracked_charstrings_offsets_u32_from_bytes(layout.tracked_charstrings_offsets, layout.kind)
}

fn tracked_charstrings_offsets_u32_from_bytes(
    bytes: &[u8],
    kind: OfficeStaticCffKind,
) -> Result<Vec<u32>, CffError> {
    if bytes.len() % 4 != 0 {
        return Err(CffError::InvalidInput(format!(
            "tracked charstrings offsets are not u32 aligned for {:?}",
            kind
        )));
    }

    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_be_bytes(chunk.try_into().expect("u32 chunk")))
        .collect())
}

fn top_dict_operand_u32(cff: &[u8], operator: &[u8]) -> Result<u32, CffError> {
    let name = read_cff_index_header(cff, 4)?;
    let top = read_cff_index_header(cff, name.next)?;
    let top_dict = cff.get(top.data_start..top.next).ok_or_else(|| {
        CffError::InvalidInput("tracked prefix is truncated before Top DICT data".to_string())
    })?;

    let mut cursor = 0usize;
    let mut stack = Vec::<i32>::new();
    while cursor < top_dict.len() {
        let byte = top_dict[cursor];
        if byte <= 21 {
            let matched = if byte == 12 {
                let escaped = [12, *top_dict.get(cursor + 1).ok_or_else(|| {
                    CffError::InvalidInput(
                        "tracked prefix has truncated escaped Top DICT operator".to_string(),
                    )
                })?];
                cursor += 2;
                escaped.as_slice() == operator
            } else {
                cursor += 1;
                [byte].as_slice() == operator
            };
            if matched {
                return stack
                    .last()
                    .copied()
                    .ok_or_else(|| {
                        CffError::InvalidInput(
                            "tracked prefix Top DICT operator had no operand".to_string(),
                        )
                    })?
                    .try_into()
                    .map_err(|_| {
                        CffError::InvalidInput(
                            "tracked prefix Top DICT operand does not fit u32".to_string(),
                        )
                    });
            }
            stack.clear();
        } else if let Some(value) = read_dict_number(top_dict, &mut cursor)? {
            stack.push(value);
        }
    }

    Err(CffError::InvalidInput(format!(
        "tracked prefix is missing Top DICT operator {:?}",
        operator
    )))
}

#[derive(Debug, Clone, Copy)]
struct CffIndexHeader {
    data_start: usize,
    next: usize,
}

fn read_cff_index_header(bytes: &[u8], offset: usize) -> Result<CffIndexHeader, CffError> {
    let count_bytes = bytes.get(offset..offset + 2).ok_or_else(|| {
        CffError::InvalidInput("tracked prefix is truncated before CFF INDEX count".to_string())
    })?;
    let count = u16::from_be_bytes(count_bytes.try_into().expect("count bytes"));
    if count == 0 {
        return Ok(CffIndexHeader {
            data_start: offset + 2,
            next: offset + 2,
        });
    }

    let off_size = *bytes.get(offset + 2).ok_or_else(|| {
        CffError::InvalidInput("tracked prefix is truncated before CFF INDEX offSize".to_string())
    })?;
    if !(1..=4).contains(&off_size) {
        return Err(CffError::InvalidInput(format!(
            "tracked prefix has invalid CFF INDEX offSize {}",
            off_size
        )));
    }

    let offsets_end = offset + 3 + (usize::from(count) + 1) * usize::from(off_size);
    let offsets = bytes.get(offset + 3..offsets_end).ok_or_else(|| {
        CffError::InvalidInput("tracked prefix is truncated inside CFF INDEX offsets".to_string())
    })?;

    let mut previous = 0u32;
    for chunk in offsets.chunks_exact(usize::from(off_size)) {
        let mut value = 0u32;
        for byte in chunk {
            value = (value << 8) | u32::from(*byte);
        }
        if previous == 0 {
            if value != 1 {
                return Err(CffError::InvalidInput(
                    "tracked prefix CFF INDEX first offset must be 1".to_string(),
                ));
            }
        } else if value < previous {
            return Err(CffError::InvalidInput(
                "tracked prefix CFF INDEX offsets must be monotonic".to_string(),
            ));
        }
        previous = value;
    }

    let data_len = usize::try_from(previous - 1).expect("data length should fit usize");
    Ok(CffIndexHeader {
        data_start: offsets_end,
        next: offsets_end + data_len,
    })
}

fn read_dict_number(bytes: &[u8], cursor: &mut usize) -> Result<Option<i32>, CffError> {
    let b0 = *bytes.get(*cursor).ok_or_else(|| {
        CffError::InvalidInput("tracked prefix is truncated inside Top DICT".to_string())
    })?;
    let result = match b0 {
        32..=246 => {
            *cursor += 1;
            Some(i32::from(b0) - 139)
        }
        247..=250 => {
            let b1 = *bytes.get(*cursor + 1).ok_or_else(|| {
                CffError::InvalidInput("tracked prefix is truncated inside Top DICT".to_string())
            })?;
            let value = (i32::from(b0) - 247) * 256 + i32::from(b1) + 108;
            *cursor += 2;
            Some(value)
        }
        251..=254 => {
            let b1 = *bytes.get(*cursor + 1).ok_or_else(|| {
                CffError::InvalidInput("tracked prefix is truncated inside Top DICT".to_string())
            })?;
            let value = -(i32::from(b0) - 251) * 256 - i32::from(b1) - 108;
            *cursor += 2;
            Some(value)
        }
        28 => {
            let bytes = bytes.get(*cursor + 1..*cursor + 3).ok_or_else(|| {
                CffError::InvalidInput("tracked prefix is truncated inside Top DICT".to_string())
            })?;
            let value = i16::from_be_bytes(bytes.try_into().expect("i16 bytes")) as i32;
            *cursor += 3;
            Some(value)
        }
        29 => {
            let bytes = bytes.get(*cursor + 1..*cursor + 5).ok_or_else(|| {
                CffError::InvalidInput("tracked prefix is truncated inside Top DICT".to_string())
            })?;
            let value = i32::from_be_bytes(bytes.try_into().expect("i32 bytes"));
            *cursor += 5;
            Some(value)
        }
        30 => {
            *cursor += 1;
            loop {
                let packed = *bytes.get(*cursor).ok_or_else(|| {
                    CffError::InvalidInput(
                        "tracked prefix is truncated inside real Top DICT operand".to_string(),
                    )
                })?;
                *cursor += 1;
                if packed >> 4 == 0xF || packed & 0xF == 0xF {
                    break;
                }
            }
            None
        }
        _ => {
            return Err(CffError::InvalidInput(format!(
                "tracked prefix uses unsupported Top DICT number opcode {}",
                b0
            )));
        }
    };

    Ok(result)
}

fn encode_standard_index_offsets(offsets: &[u32]) -> Result<Vec<u8>, CffError> {
    let count = offsets
        .len()
        .checked_sub(1)
        .ok_or_else(|| CffError::EncodeFailed("tracked charstrings offsets are empty".to_string()))?;
    let count_u16 = u16::try_from(count).map_err(|_| {
        CffError::EncodeFailed(format!(
            "tracked charstrings offset count {} does not fit a CFF INDEX count",
            count
        ))
    })?;

    let max = offsets
        .iter()
        .copied()
        .max()
        .ok_or_else(|| CffError::EncodeFailed("tracked charstrings offsets are empty".to_string()))?;
    let off_size = if max <= 0xFF {
        1
    } else if max <= 0xFFFF {
        2
    } else if max <= 0xFF_FFFF {
        3
    } else {
        4
    };

    let mut out = Vec::with_capacity(3 + offsets.len() * off_size);
    out.extend_from_slice(&count_u16.to_be_bytes());
    out.push(u8::try_from(off_size).expect("offSize should fit u8"));
    for value in offsets {
        let bytes = value.to_be_bytes();
        out.extend_from_slice(&bytes[4 - off_size..]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_office_static(font_name: &str) -> OfficeStaticCff<'static> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"OTTO");
        bytes.resize(OFFICE_CFF_OFFSET, 0);
        bytes.extend_from_slice(&[0x04, 0x03, 0x00, 0x01, 0x01, 0x02, font_name.len() as u8]);
        bytes.extend_from_slice(font_name.as_bytes());
        bytes.resize(OFFICE_CFF_OFFSET + 128, 0);
        let leaked = Box::leak(bytes.into_boxed_slice());
        OfficeStaticCff {
            sfnt_bytes: leaked,
            cff_offset: OFFICE_CFF_OFFSET,
            office_cff_suffix: &leaked[OFFICE_CFF_OFFSET..],
        }
    }

    #[test]
    fn detect_office_static_cff_layout_supports_all_tracked_sourcehan_weights() {
        for (font_name, expected_tail_start, expected_charstrings_offset, expected_data_start) in [
            ("SourceHanSaSsSC-Regular", 2804, 11_081, 207_692),
            ("SourceHanSaSsSC-Bold", 3360, 13_041, 209_652),
            ("SourceHanSaSsSC-ExtraLight", 4934, 21_489, 218_100),
            ("SourceHanSaSsSC-Heavy", 5341, 22_039, 218_650),
            ("SourceHanSaSsSC-Light", 3520, 14_202, 210_813),
            ("SourceHanSaSsSC-Medium", 2546, 9_771, 206_382),
            ("SourceHanSaSsSC-Normal", 2918, 11_337, 207_948),
        ] {
            let office = synthetic_office_static(font_name);
            let layout = detect_office_static_cff_layout(&office)
                .unwrap_or_else(|error| panic!("{font_name} should be supported: {error}"));

            assert_eq!(layout.office_tail_start, expected_tail_start, "{font_name}");
            assert_eq!(
                layout.charstrings_offset, expected_charstrings_offset,
                "{font_name}"
            );
            assert_eq!(
                layout.charstrings_data_start, expected_data_start,
                "{font_name}"
            );
        }
    }
}
