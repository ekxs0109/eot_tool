//! Rust-owned subset planning for the rewrite.

use core::fmt;

use fonttool_sfnt::OwnedSfntFont;

const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_CVT: u32 = u32::from_be_bytes(*b"cvt ");
const TAG_DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");
const SUBSET_GID_NOT_INCLUDED: u16 = u16::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePolicy {
    Keep,
    Reencode,
    DropWithWarning,
}

#[must_use]
pub fn table_policy_for_tag(tag: u32) -> TablePolicy {
    match tag {
        TAG_DSIG | TAG_VDMX => TablePolicy::DropWithWarning,
        TAG_CVT | TAG_HDMX => TablePolicy::Reencode,
        _ => TablePolicy::Keep,
    }
}

#[must_use]
pub fn subset_table_policy_for_tag(tag: u32) -> TablePolicy {
    match tag {
        TAG_DSIG | TAG_HDMX | TAG_VDMX => TablePolicy::DropWithWarning,
        _ => TablePolicy::Keep,
    }
}

#[must_use]
pub fn should_copy_encode_block1_table(tag: u32) -> bool {
    !matches!(table_policy_for_tag(tag), TablePolicy::DropWithWarning)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphIdRequest {
    glyph_ids: Vec<u16>,
}

impl GlyphIdRequest {
    pub fn parse_csv(csv: &str) -> Result<Self, SubsetError> {
        let mut glyph_ids = Vec::new();

        for raw_part in csv.split(',') {
            let part = raw_part.trim();
            if part.is_empty() {
                return Err(SubsetError::InvalidGlyphIdList);
            }

            let glyph_id = part
                .parse::<u16>()
                .map_err(|_| SubsetError::InvalidGlyphIdList)?;
            glyph_ids.push(glyph_id);
        }

        if glyph_ids.is_empty() {
            return Err(SubsetError::InvalidGlyphIdList);
        }

        Ok(Self { glyph_ids })
    }

    #[must_use]
    pub fn glyph_ids(&self) -> &[u16] {
        &self.glyph_ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsetPlan {
    included_glyph_ids: Vec<u16>,
    old_to_new_gid: Vec<u16>,
    keep_gids: bool,
}

impl SubsetPlan {
    #[must_use]
    pub fn included_glyph_ids(&self) -> &[u16] {
        &self.included_glyph_ids
    }

    #[must_use]
    pub fn output_num_glyphs(&self) -> u16 {
        self.included_glyph_ids.len().try_into().unwrap_or(u16::MAX)
    }

    #[must_use]
    pub fn keep_gids(&self) -> bool {
        self.keep_gids
    }

    #[must_use]
    pub fn old_to_new_gid(&self) -> &[u16] {
        &self.old_to_new_gid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubsetWarnings {
    pub dropped_hdmx: bool,
    pub dropped_vdmx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubsetError {
    InvalidGlyphIdList,
    MissingMaxp,
    TruncatedMaxp,
    GlyphIdOutOfRange(u16),
}

impl fmt::Display for SubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubsetError::InvalidGlyphIdList => f.write_str("invalid --glyph-ids list"),
            SubsetError::MissingMaxp => f.write_str("required table `maxp` is missing"),
            SubsetError::TruncatedMaxp => f.write_str("maxp table is truncated"),
            SubsetError::GlyphIdOutOfRange(glyph_id) => {
                write!(f, "glyph id {glyph_id} is out of range for this font")
            }
        }
    }
}

impl std::error::Error for SubsetError {}

pub fn plan_glyph_subset(
    font: &OwnedSfntFont,
    request: &GlyphIdRequest,
    keep_gids: bool,
) -> Result<SubsetPlan, SubsetError> {
    let maxp = font.table(TAG_MAXP).ok_or(SubsetError::MissingMaxp)?;
    if maxp.data.len() < 6 {
        return Err(SubsetError::TruncatedMaxp);
    }

    let total_input_glyphs = usize::from(u16::from_be_bytes([maxp.data[4], maxp.data[5]]));
    let mut included = vec![false; total_input_glyphs];
    if total_input_glyphs > 0 {
        included[0] = true;
    }

    for &glyph_id in request.glyph_ids() {
        let index = usize::from(glyph_id);
        if index >= total_input_glyphs {
            return Err(SubsetError::GlyphIdOutOfRange(glyph_id));
        }
        included[index] = true;
    }

    let mut included_glyph_ids = Vec::new();
    let mut old_to_new_gid = vec![SUBSET_GID_NOT_INCLUDED; total_input_glyphs];
    let mut next_output_gid = 0u16;

    for (glyph_id, is_included) in included.into_iter().enumerate() {
        if !is_included {
            continue;
        }

        let glyph_id_u16 = glyph_id as u16;
        included_glyph_ids.push(glyph_id_u16);
        old_to_new_gid[glyph_id] = if keep_gids {
            glyph_id_u16
        } else {
            let mapped = next_output_gid;
            next_output_gid = next_output_gid.saturating_add(1);
            mapped
        };
    }

    Ok(SubsetPlan {
        included_glyph_ids,
        old_to_new_gid,
        keep_gids,
    })
}

pub fn apply_output_table_policy(font: &mut OwnedSfntFont, warnings: &mut SubsetWarnings) {
    for tag in [TAG_DSIG, TAG_HDMX, TAG_VDMX] {
        if subset_table_policy_for_tag(tag) != TablePolicy::DropWithWarning {
            continue;
        }

        if font.remove_table(tag).is_none() {
            continue;
        }

        match tag {
            TAG_HDMX => warnings.dropped_hdmx = true,
            TAG_VDMX => warnings.dropped_vdmx = true,
            TAG_DSIG => {}
            _ => unreachable!("subset output table policy only handles known tags"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_output_table_policy, should_copy_encode_block1_table, subset_table_policy_for_tag,
        table_policy_for_tag, SubsetWarnings, TablePolicy,
    };
    use fonttool_sfnt::{OwnedSfntFont, SFNT_VERSION_TRUETYPE};

    const TAG_CVT: u32 = u32::from_be_bytes(*b"cvt ");
    const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
    const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
    const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");

    #[test]
    fn table_policy_classifies_extra_tables() {
        assert_eq!(table_policy_for_tag(TAG_CVT), TablePolicy::Reencode);
        assert_eq!(table_policy_for_tag(TAG_HDMX), TablePolicy::Reencode);
        assert_eq!(table_policy_for_tag(TAG_VDMX), TablePolicy::DropWithWarning);
    }

    #[test]
    fn table_policy_keeps_other_tables() {
        assert_eq!(table_policy_for_tag(TAG_NAME), TablePolicy::Keep);
    }

    #[test]
    fn subset_table_policy_classifies_extra_tables() {
        assert_eq!(subset_table_policy_for_tag(TAG_CVT), TablePolicy::Keep);
        assert_eq!(
            subset_table_policy_for_tag(TAG_HDMX),
            TablePolicy::DropWithWarning
        );
        assert_eq!(
            subset_table_policy_for_tag(TAG_VDMX),
            TablePolicy::DropWithWarning
        );
    }

    #[test]
    fn apply_output_table_policy_drops_warning_tables_and_keeps_cvt() {
        let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
        font.add_table(TAG_CVT, vec![0x00, 0x10]);
        font.add_table(TAG_HDMX, vec![0x00, 0x01]);
        font.add_table(TAG_VDMX, vec![0x00, 0x02]);

        let mut warnings = SubsetWarnings::default();
        apply_output_table_policy(&mut font, &mut warnings);

        assert!(font.table(TAG_CVT).is_some(), "cvt should be retained");
        assert!(font.table(TAG_HDMX).is_none(), "hdmx should be dropped");
        assert!(font.table(TAG_VDMX).is_none(), "VDMX should be dropped");
        assert!(warnings.dropped_hdmx, "hdmx drop should set a warning");
        assert!(warnings.dropped_vdmx, "VDMX drop should set a warning");
    }

    #[test]
    fn encode_block1_copy_policy_only_keeps_keep_tables() {
        assert!(should_copy_encode_block1_table(TAG_NAME));
        assert!(should_copy_encode_block1_table(TAG_CVT));
        assert!(should_copy_encode_block1_table(TAG_HDMX));
        assert!(!should_copy_encode_block1_table(TAG_VDMX));
    }
}
