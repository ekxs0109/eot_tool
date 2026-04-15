//! Rust-owned subset planning for the rewrite.

use core::fmt;

use fonttool_sfnt::OwnedSfntFont;

const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_CVT: u32 = u32::from_be_bytes(*b"cvt ");
const TAG_DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");
const SUBSET_GID_NOT_INCLUDED: u16 = u16::MAX;
const CMAP_FORMAT_4: u16 = 4;
const CMAP_FORMAT_12: u16 = 12;
const GLYF_COMPOSITE_ARG_WORDS: u16 = 0x0001;
const GLYF_COMPOSITE_HAVE_SCALE: u16 = 0x0008;
const GLYF_COMPOSITE_MORE_COMPONENTS: u16 = 0x0020;
const GLYF_COMPOSITE_HAVE_XY_SCALE: u16 = 0x0040;
const GLYF_COMPOSITE_HAVE_TWO_BY_TWO: u16 = 0x0080;
const GLYF_COMPOSITE_HAVE_INSTRUCTIONS: u16 = 0x0100;

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

pub fn subset_owned_font(
    font: OwnedSfntFont,
    request: &GlyphIdRequest,
) -> Result<(OwnedSfntFont, SubsetWarnings), SubsetError> {
    let plan = plan_glyph_subset(&font, request, false)?;
    let mut warnings = SubsetWarnings::default();
    let mut output = font.clone();

    update_maxp_num_glyphs(&mut output, plan.output_num_glyphs())?;
    update_hhea_and_hmtx_tables(&font, &plan, &mut output)?;
    update_glyf_and_loca_tables(&font, &plan, &mut output)?;
    update_cmap_table(&font, &plan, &mut output)?;
    apply_output_table_policy(&mut output, &mut warnings);

    Ok((output, warnings))
}

fn update_maxp_num_glyphs(
    font: &mut OwnedSfntFont,
    num_glyphs: u16,
) -> Result<(), SubsetError> {
    let mut maxp = font
        .remove_table(TAG_MAXP)
        .ok_or(SubsetError::MissingMaxp)?
        .data;

    if maxp.len() < 6 {
        return Err(SubsetError::TruncatedMaxp);
    }

    maxp[4..6].copy_from_slice(&num_glyphs.to_be_bytes());
    font.add_table(TAG_MAXP, maxp);
    Ok(())
}

fn update_hhea_and_hmtx_tables(
    input: &OwnedSfntFont,
    plan: &SubsetPlan,
    output: &mut OwnedSfntFont,
) -> Result<(), SubsetError> {
    let hhea = input
        .table(TAG_HHEA)
        .ok_or(SubsetError::MissingMaxp)?
        .data
        .clone();
    if hhea.len() < 36 {
        return Err(SubsetError::TruncatedMaxp);
    }

    let num_output_glyphs = plan.output_num_glyphs();
    let mut metrics = Vec::with_capacity(usize::from(num_output_glyphs));
    for output_gid in 0..num_output_glyphs {
        let source_gid = source_gid_for_output_gid(plan, output_gid);
        metrics.push(read_hmetric(input, source_gid)?);
    }

    let mut num_hmetrics = num_output_glyphs;
    if num_output_glyphs > 0 {
        let last_advance = metrics[usize::from(num_output_glyphs - 1)].advance_width;
        while num_hmetrics > 1
            && metrics[usize::from(num_hmetrics - 2)].advance_width == last_advance
        {
            num_hmetrics -= 1;
        }
    }

    let mut hmtx = Vec::with_capacity(
        usize::from(num_hmetrics) * 4 + usize::from(num_output_glyphs - num_hmetrics) * 2,
    );
    for metric in metrics.iter().take(usize::from(num_hmetrics)) {
        hmtx.extend_from_slice(&metric.advance_width.to_be_bytes());
        hmtx.extend_from_slice(&metric.left_side_bearing.to_be_bytes());
    }
    for metric in metrics.iter().skip(usize::from(num_hmetrics)) {
        hmtx.extend_from_slice(&metric.left_side_bearing.to_be_bytes());
    }

    let mut rebuilt_hhea = hhea;
    rebuilt_hhea[34..36].copy_from_slice(&num_hmetrics.to_be_bytes());
    output.remove_table(TAG_HHEA);
    output.remove_table(TAG_HMTX);
    output.add_table(TAG_HHEA, rebuilt_hhea);
    output.add_table(TAG_HMTX, hmtx);
    Ok(())
}

fn update_glyf_and_loca_tables(
    input: &OwnedSfntFont,
    plan: &SubsetPlan,
    output: &mut OwnedSfntFont,
) -> Result<(), SubsetError> {
    let source_glyf = input
        .table(TAG_GLYF)
        .ok_or(SubsetError::MissingMaxp)?
        .data
        .as_slice();
    let mut loca = Vec::new();
    let index_to_loca_format = read_index_to_loca_format(input)?;
    let output_num_glyphs = plan.output_num_glyphs();
    let mut glyf_data = Vec::with_capacity(source_glyf.len());

    match index_to_loca_format {
        0 => loca.resize(usize::from(output_num_glyphs + 1) * 2, 0),
        1 => loca.resize(usize::from(output_num_glyphs + 1) * 4, 0),
        _ => return Err(SubsetError::TruncatedMaxp),
    }

    for output_gid in 0..output_num_glyphs {
        let source_gid = source_gid_for_output_gid(plan, output_gid);
        let (glyph_offset, glyph_length) = read_glyph_range(input, source_gid)?;

        match index_to_loca_format {
            0 => {
                let offset = usize::from(output_gid) * 2;
                let loca_value = u16::try_from(glyf_data.len() / 2)
                    .map_err(|_| SubsetError::TruncatedMaxp)?;
                loca[offset..offset + 2].copy_from_slice(&loca_value.to_be_bytes());
            }
            1 => {
                let offset = usize::from(output_gid) * 4;
                let loca_value =
                    u32::try_from(glyf_data.len()).map_err(|_| SubsetError::TruncatedMaxp)?;
                loca[offset..offset + 4].copy_from_slice(&loca_value.to_be_bytes());
            }
            _ => unreachable!(),
        }

        if glyph_length == 0 {
            continue;
        }

        let glyph_slice = &source_glyf[glyph_offset..glyph_offset + glyph_length];
        let glyph = renumber_glyph_data(glyph_slice, plan)?;
        glyf_data.extend_from_slice(&glyph);
    }

    match index_to_loca_format {
        0 => {
            let offset = usize::from(output_num_glyphs) * 2;
            let loca_value = u16::try_from(glyf_data.len() / 2)
                .map_err(|_| SubsetError::TruncatedMaxp)?;
            loca[offset..offset + 2].copy_from_slice(&loca_value.to_be_bytes());
        }
        1 => {
            let offset = usize::from(output_num_glyphs) * 4;
            let loca_value =
                u32::try_from(glyf_data.len()).map_err(|_| SubsetError::TruncatedMaxp)?;
            loca[offset..offset + 4].copy_from_slice(&loca_value.to_be_bytes());
        }
        _ => unreachable!(),
    }

    output.remove_table(TAG_GLYF);
    output.remove_table(TAG_LOCA);
    output.add_table(TAG_GLYF, glyf_data);
    output.add_table(TAG_LOCA, loca);
    Ok(())
}

fn update_cmap_table(
    input: &OwnedSfntFont,
    plan: &SubsetPlan,
    output: &mut OwnedSfntFont,
) -> Result<(), SubsetError> {
    let cmap = input.table(TAG_CMAP).ok_or(SubsetError::MissingMaxp)?.data.as_slice();
    let entries = collect_cmap_entries(cmap, plan)?;
    let rebuilt = build_cmap_table(&entries);
    output.remove_table(TAG_CMAP);
    output.add_table(TAG_CMAP, rebuilt);
    Ok(())
}

fn source_gid_for_output_gid(plan: &SubsetPlan, output_gid: u16) -> u16 {
    if plan.keep_gids() {
        output_gid
    } else {
        plan.included_glyph_ids()[usize::from(output_gid)]
    }
}

#[derive(Debug, Clone, Copy)]
struct Hmetric {
    advance_width: u16,
    left_side_bearing: i16,
}

#[derive(Debug, Clone, Copy)]
struct CmapEntry {
    codepoint: u32,
    glyph_id: u16,
}

fn read_index_to_loca_format(font: &OwnedSfntFont) -> Result<i16, SubsetError> {
    let head = font
        .table(TAG_HEAD)
        .ok_or(SubsetError::MissingMaxp)?
        .data
        .as_slice();
    if head.len() < 52 {
        return Err(SubsetError::TruncatedMaxp);
    }
    let format = i16::from_be_bytes([head[50], head[51]]);
    if format != 0 && format != 1 {
        return Err(SubsetError::TruncatedMaxp);
    }
    Ok(format)
}

fn read_num_glyphs(font: &OwnedSfntFont) -> Result<u16, SubsetError> {
    let maxp = font
        .table(TAG_MAXP)
        .ok_or(SubsetError::MissingMaxp)?
        .data
        .as_slice();
    if maxp.len() < 6 {
        return Err(SubsetError::TruncatedMaxp);
    }
    Ok(u16::from_be_bytes([maxp[4], maxp[5]]))
}

fn read_num_hmetrics(font: &OwnedSfntFont) -> Result<u16, SubsetError> {
    let hhea = font
        .table(TAG_HHEA)
        .ok_or(SubsetError::MissingMaxp)?
        .data
        .as_slice();
    if hhea.len() < 36 {
        return Err(SubsetError::TruncatedMaxp);
    }
    Ok(u16::from_be_bytes([hhea[34], hhea[35]]))
}

fn read_hmetric(font: &OwnedSfntFont, glyph_id: u16) -> Result<Hmetric, SubsetError> {
    let hmtx = font.table(TAG_HMTX).ok_or(SubsetError::MissingMaxp)?.data.as_slice();
    let num_glyphs = read_num_glyphs(font)?;
    let number_of_hmetrics = read_num_hmetrics(font)?;
    if glyph_id >= num_glyphs {
        return Err(SubsetError::GlyphIdOutOfRange(glyph_id));
    }
    if number_of_hmetrics == 0 || number_of_hmetrics > num_glyphs {
        return Err(SubsetError::TruncatedMaxp);
    }
    let hmtx_length = usize::from(number_of_hmetrics) * 4
        + usize::from(num_glyphs - number_of_hmetrics) * 2;
    if hmtx.len() < hmtx_length {
        return Err(SubsetError::TruncatedMaxp);
    }

    let glyph_index = usize::from(glyph_id);
    if glyph_id < number_of_hmetrics {
        let offset = glyph_index * 4;
        Ok(Hmetric {
            advance_width: u16::from_be_bytes([hmtx[offset], hmtx[offset + 1]]),
            left_side_bearing: i16::from_be_bytes([hmtx[offset + 2], hmtx[offset + 3]]),
        })
    } else {
        let last_hmetric = usize::from(number_of_hmetrics - 1) * 4;
        let lsb_offset = usize::from(number_of_hmetrics) * 4
            + usize::from(glyph_id - number_of_hmetrics) * 2;
        Ok(Hmetric {
            advance_width: u16::from_be_bytes([hmtx[last_hmetric], hmtx[last_hmetric + 1]]),
            left_side_bearing: i16::from_be_bytes([hmtx[lsb_offset], hmtx[lsb_offset + 1]]),
        })
    }
}

fn read_glyph_range(font: &OwnedSfntFont, glyph_id: u16) -> Result<(usize, usize), SubsetError> {
    let loca = font.table(TAG_LOCA).ok_or(SubsetError::MissingMaxp)?.data.as_slice();
    let glyf = font.table(TAG_GLYF).ok_or(SubsetError::MissingMaxp)?.data.as_slice();
    let num_glyphs = read_num_glyphs(font)?;
    if glyph_id >= num_glyphs {
        return Err(SubsetError::GlyphIdOutOfRange(glyph_id));
    }

    match read_index_to_loca_format(font)? {
        0 => {
            let offset = usize::from(glyph_id) * 2;
            if offset + 4 > loca.len() {
                return Err(SubsetError::TruncatedMaxp);
            }
            let start = usize::from(u16::from_be_bytes([loca[offset], loca[offset + 1]])) * 2;
            let end = usize::from(u16::from_be_bytes([loca[offset + 2], loca[offset + 3]])) * 2;
            if end < start || end > glyf.len() {
                return Err(SubsetError::TruncatedMaxp);
            }
            Ok((start, end))
        }
        1 => {
            let offset = usize::from(glyph_id) * 4;
            if offset + 8 > loca.len() {
                return Err(SubsetError::TruncatedMaxp);
            }
            let start = u32::from_be_bytes([loca[offset], loca[offset + 1], loca[offset + 2], loca[offset + 3]]) as usize;
            let end = u32::from_be_bytes([loca[offset + 4], loca[offset + 5], loca[offset + 6], loca[offset + 7]]) as usize;
            if end < start || end > glyf.len() {
                return Err(SubsetError::TruncatedMaxp);
            }
            Ok((start, end))
        }
        _ => Err(SubsetError::TruncatedMaxp),
    }
}

fn renumber_glyph_data(glyph_data: &[u8], plan: &SubsetPlan) -> Result<Vec<u8>, SubsetError> {
    if glyph_data.is_empty() {
        return Ok(Vec::new());
    }
    if glyph_data.len() < 10 {
        return Err(SubsetError::TruncatedMaxp);
    }

    let mut copy = glyph_data.to_vec();
    let contour_count = i16::from_be_bytes([copy[0], copy[1]]);
    if plan.keep_gids() || contour_count >= 0 {
        return Ok(copy);
    }

    let mut position = 10usize;
    let mut flags;
    loop {
        if position + 4 > copy.len() {
            return Err(SubsetError::TruncatedMaxp);
        }
        flags = u16::from_be_bytes([copy[position], copy[position + 1]]);
        let component_gid = u16::from_be_bytes([copy[position + 2], copy[position + 3]]);
        let new_component_gid = plan
            .old_to_new_gid()
            .get(usize::from(component_gid))
            .copied()
            .unwrap_or(SUBSET_GID_NOT_INCLUDED);
        if new_component_gid == SUBSET_GID_NOT_INCLUDED {
            return Err(SubsetError::GlyphIdOutOfRange(component_gid));
        }
        copy[position + 2..position + 4].copy_from_slice(&new_component_gid.to_be_bytes());
        position += 4;

        position += if flags & GLYF_COMPOSITE_ARG_WORDS != 0 { 4 } else { 2 };
        if flags & GLYF_COMPOSITE_HAVE_SCALE != 0 {
            position += 2;
        } else if flags & GLYF_COMPOSITE_HAVE_XY_SCALE != 0 {
            position += 4;
        } else if flags & GLYF_COMPOSITE_HAVE_TWO_BY_TWO != 0 {
            position += 8;
        }
        if position > copy.len() {
            return Err(SubsetError::TruncatedMaxp);
        }

        if flags & GLYF_COMPOSITE_MORE_COMPONENTS == 0 {
            break;
        }
    }

    if flags & GLYF_COMPOSITE_HAVE_INSTRUCTIONS != 0 {
        if position + 2 > copy.len() {
            return Err(SubsetError::TruncatedMaxp);
        }
        let instruction_length = usize::from(u16::from_be_bytes([copy[position], copy[position + 1]]));
        position += 2 + instruction_length;
        if position > copy.len() {
            return Err(SubsetError::TruncatedMaxp);
        }
    }

    Ok(copy)
}

fn collect_cmap_entries(cmap: &[u8], plan: &SubsetPlan) -> Result<Vec<CmapEntry>, SubsetError> {
    let (format, offset) = best_cmap_subtable(cmap)?;
    let mut entries = Vec::new();

    match format {
        CMAP_FORMAT_4 => collect_cmap_entries_format4(cmap, offset, plan, &mut entries)?,
        CMAP_FORMAT_12 => collect_cmap_entries_format12(cmap, offset, plan, &mut entries)?,
        _ => return Err(SubsetError::TruncatedMaxp),
    }

    Ok(entries)
}

fn best_cmap_subtable(cmap: &[u8]) -> Result<(u16, usize), SubsetError> {
    if cmap.len() < 4 {
        return Err(SubsetError::TruncatedMaxp);
    }
    let num_tables = u16::from_be_bytes([cmap[2], cmap[3]]) as usize;
    if cmap.len() < 4 + num_tables * 8 {
        return Err(SubsetError::TruncatedMaxp);
    }

    let mut best_score = -1i32;
    let mut best_format = 0u16;
    let mut best_offset = 0usize;

    for index in 0..num_tables {
        let record_offset = 4 + index * 8;
        let platform_id = u16::from_be_bytes([cmap[record_offset], cmap[record_offset + 1]]);
        let encoding_id = u16::from_be_bytes([cmap[record_offset + 2], cmap[record_offset + 3]]);
        let subtable_offset = u32::from_be_bytes([
            cmap[record_offset + 4],
            cmap[record_offset + 5],
            cmap[record_offset + 6],
            cmap[record_offset + 7],
        ]) as usize;
        if subtable_offset + 2 > cmap.len() {
            continue;
        }
        let format = u16::from_be_bytes([cmap[subtable_offset], cmap[subtable_offset + 1]]);
        let score = match format {
            CMAP_FORMAT_12 if platform_id == 3 && encoding_id == 10 => 4,
            CMAP_FORMAT_12 if platform_id == 0 => 3,
            CMAP_FORMAT_4 if platform_id == 3 && encoding_id == 1 => 2,
            CMAP_FORMAT_4 if platform_id == 0 => 1,
            _ => -1,
        };
        if score > best_score {
            best_score = score;
            best_format = format;
            best_offset = subtable_offset;
        }
    }

    if best_score < 0 {
        return Err(SubsetError::TruncatedMaxp);
    }

    Ok((best_format, best_offset))
}

fn collect_cmap_entries_format4(
    cmap: &[u8],
    offset: usize,
    plan: &SubsetPlan,
    entries: &mut Vec<CmapEntry>,
) -> Result<(), SubsetError> {
    if offset + 16 > cmap.len() {
        return Err(SubsetError::TruncatedMaxp);
    }
    let length = usize::from(u16::from_be_bytes([cmap[offset + 2], cmap[offset + 3]]));
    if offset + length > cmap.len() || length < 16 {
        return Err(SubsetError::TruncatedMaxp);
    }
    let seg_count = usize::from(u16::from_be_bytes([cmap[offset + 6], cmap[offset + 7]]) / 2);
    let end_codes_offset = offset + 14;
    let start_codes_offset = end_codes_offset + seg_count * 2 + 2;
    let id_deltas_offset = start_codes_offset + seg_count * 2;
    let id_range_offsets_offset = id_deltas_offset + seg_count * 2;
    if id_range_offsets_offset + seg_count * 2 > offset + length {
        return Err(SubsetError::TruncatedMaxp);
    }

    for i in 0..seg_count {
        let start_code = u16::from_be_bytes([cmap[start_codes_offset + i * 2], cmap[start_codes_offset + i * 2 + 1]]);
        let end_code = u16::from_be_bytes([cmap[end_codes_offset + i * 2], cmap[end_codes_offset + i * 2 + 1]]);
        if start_code == 0xFFFF && end_code == 0xFFFF {
            break;
        }
        if end_code < start_code {
            return Err(SubsetError::TruncatedMaxp);
        }

        let id_delta = i16::from_be_bytes([
            cmap[id_deltas_offset + i * 2],
            cmap[id_deltas_offset + i * 2 + 1],
        ]);
        let id_range_offset = u16::from_be_bytes([
            cmap[id_range_offsets_offset + i * 2],
            cmap[id_range_offsets_offset + i * 2 + 1],
        ]);

        for codepoint in u32::from(start_code)..=u32::from(end_code) {
            let glyph_id = if id_range_offset == 0 {
                ((codepoint as i32 + id_delta as i32) & 0xFFFF) as u16
            } else {
                let glyph_index_pos = id_range_offsets_offset
                    + i * 2
                    + usize::from(id_range_offset)
                    + usize::try_from(codepoint - u32::from(start_code))
                        .map_err(|_| SubsetError::TruncatedMaxp)?
                        * 2;
                if glyph_index_pos + 2 > offset + length {
                    continue;
                }
                let glyph = u16::from_be_bytes([cmap[glyph_index_pos], cmap[glyph_index_pos + 1]]);
                if glyph == 0 {
                    0
                } else {
                    ((u32::from(glyph) + id_delta as i32 as u32) & 0xFFFF) as u16
                }
            };

            if glyph_id != 0
                && usize::from(glyph_id) < plan.old_to_new_gid().len()
                && plan.old_to_new_gid()[usize::from(glyph_id)] != SUBSET_GID_NOT_INCLUDED
            {
                append_cmap_entry(entries, codepoint, plan.old_to_new_gid()[usize::from(glyph_id)]);
            }

            if codepoint == 0xFFFF {
                break;
            }
        }
    }

    Ok(())
}

fn collect_cmap_entries_format12(
    cmap: &[u8],
    offset: usize,
    plan: &SubsetPlan,
    entries: &mut Vec<CmapEntry>,
) -> Result<(), SubsetError> {
    if offset + 16 > cmap.len() {
        return Err(SubsetError::TruncatedMaxp);
    }
    let length = u32::from_be_bytes([cmap[offset + 4], cmap[offset + 5], cmap[offset + 6], cmap[offset + 7]]) as usize;
    let num_groups = u32::from_be_bytes([cmap[offset + 12], cmap[offset + 13], cmap[offset + 14], cmap[offset + 15]]) as usize;
    if offset + length > cmap.len() || length < 16 + num_groups * 12 {
        return Err(SubsetError::TruncatedMaxp);
    }

    for group_index in 0..num_groups {
        let group_offset = offset + 16 + group_index * 12;
        let start_char = u32::from_be_bytes([
            cmap[group_offset],
            cmap[group_offset + 1],
            cmap[group_offset + 2],
            cmap[group_offset + 3],
        ]);
        let end_char = u32::from_be_bytes([
            cmap[group_offset + 4],
            cmap[group_offset + 5],
            cmap[group_offset + 6],
            cmap[group_offset + 7],
        ]);
        let start_glyph_id = u32::from_be_bytes([
            cmap[group_offset + 8],
            cmap[group_offset + 9],
            cmap[group_offset + 10],
            cmap[group_offset + 11],
        ]);
        if end_char < start_char {
            return Err(SubsetError::TruncatedMaxp);
        }

        for codepoint in start_char..=end_char {
            let glyph_id = start_glyph_id + (codepoint - start_char);
            if glyph_id != 0
                && usize::try_from(glyph_id)
                    .ok()
                    .is_some_and(|index| index < plan.old_to_new_gid().len())
            {
                let index = usize::try_from(glyph_id).map_err(|_| SubsetError::TruncatedMaxp)?;
                let mapped = plan.old_to_new_gid()[index];
                if mapped != SUBSET_GID_NOT_INCLUDED {
                    append_cmap_entry(entries, codepoint, mapped);
                }
            }
            if codepoint == 0xFFFF_FFFF {
                break;
            }
        }
    }

    Ok(())
}

fn append_cmap_entry(entries: &mut Vec<CmapEntry>, codepoint: u32, glyph_id: u16) {
    if let Some(last) = entries.last_mut() {
        if last.codepoint == codepoint {
            last.glyph_id = glyph_id;
            return;
        }
    }
    entries.push(CmapEntry { codepoint, glyph_id });
}

fn build_cmap_table(entries: &[CmapEntry]) -> Vec<u8> {
    let format4 = build_cmap_format4(entries);
    let has_format12 = entries.iter().any(|entry| entry.codepoint > 0xFFFF);

    if has_format12 {
        let format12 = build_cmap_format12(entries);
        let header_len = 4 + 2 * 8;
        let format12_offset = header_len + format4.len();
        let mut cmap = vec![0; header_len + format4.len() + format12.len()];

        write_u16_be(&mut cmap, 2, 2);
        write_u16_be(&mut cmap, 4, 3);
        write_u16_be(&mut cmap, 6, 1);
        write_u32_be(&mut cmap, 8, u32::try_from(header_len).unwrap_or(u32::MAX));
        write_u16_be(&mut cmap, 12, 3);
        write_u16_be(&mut cmap, 14, 10);
        write_u32_be(
            &mut cmap,
            16,
            u32::try_from(format12_offset).unwrap_or(u32::MAX),
        );
        copy_into(&format4, &mut cmap, header_len);
        copy_into(&format12, &mut cmap, format12_offset);
        cmap
    } else {
        let header_len = 4 + 8;
        let mut cmap = vec![0; header_len + format4.len()];

        write_u16_be(&mut cmap, 2, 1);
        write_u16_be(&mut cmap, 4, 3);
        write_u16_be(&mut cmap, 6, 1);
        write_u32_be(&mut cmap, 8, u32::try_from(header_len).unwrap_or(u32::MAX));
        write_u16_be(&mut cmap, 12, 3);
        write_u16_be(&mut cmap, 14, 1);
        write_u32_be(&mut cmap, 16, u32::try_from(header_len).unwrap_or(u32::MAX));
        copy_into(&format4, &mut cmap, header_len);
        cmap
    }
}

fn build_cmap_format4(entries: &[CmapEntry]) -> Vec<u8> {
    let mut segments: Vec<CmapFormat4Segment> = Vec::new();
    for entry in entries {
        if entry.codepoint > 0xFFFE {
            continue;
        }
        let delta = ((entry.glyph_id as i32 - entry.codepoint as i32) & 0xFFFF) as i16;
        if let Some(last) = segments.last_mut() {
            if last.end_code + 1 == entry.codepoint as u16 && last.delta == delta {
                last.end_code = entry.codepoint as u16;
                continue;
            }
        }
        segments.push(CmapFormat4Segment {
            start_code: entry.codepoint as u16,
            end_code: entry.codepoint as u16,
            delta,
        });
    }

    let seg_count = u16::try_from(segments.len() + 1).unwrap_or(u16::MAX);
    let seg_count_x2 = seg_count * 2;
    let search_power = highest_power_of_two_at_most(seg_count);
    let entry_selector = floor_log2_positive(search_power);
    let search_range = search_power * 2;
    let length = 16 + usize::from(seg_count) * 8;
    let mut format4 = vec![0; length];

    write_u16_be(&mut format4, 0, CMAP_FORMAT_4);
    write_u16_be(&mut format4, 2, u16::try_from(length).unwrap_or(u16::MAX));
    write_u16_be(&mut format4, 6, seg_count_x2);
    write_u16_be(&mut format4, 8, u16::try_from(search_range).unwrap_or(u16::MAX));
    write_u16_be(&mut format4, 10, u16::try_from(entry_selector).unwrap_or(u16::MAX));
    write_u16_be(
        &mut format4,
        12,
        seg_count_x2 - u16::try_from(search_range).unwrap_or(u16::MAX),
    );
    for (index, segment) in segments.iter().enumerate() {
        write_u16_be(&mut format4, 14 + index * 2, segment.end_code);
        write_u16_be(
            &mut format4,
            16 + usize::from(seg_count) * 2 + index * 2,
            segment.start_code,
        );
        write_i16_be(&mut format4, 16 + usize::from(seg_count) * 4 + index * 2, segment.delta);
    }
    write_u16_be(&mut format4, 14 + segments.len() * 2, 0xFFFF);
    write_u16_be(
        &mut format4,
        16 + usize::from(seg_count) * 2 + segments.len() * 2,
        0xFFFF,
    );
    write_u16_be(
        &mut format4,
        16 + usize::from(seg_count) * 4 + segments.len() * 2,
        1,
    );

    format4
}

fn build_cmap_format12(entries: &[CmapEntry]) -> Vec<u8> {
    let mut groups: Vec<CmapFormat12Group> = Vec::new();
    for entry in entries {
        if let Some(last) = groups.last_mut() {
            if last.end_code + 1 == entry.codepoint
                && last.start_glyph_id + (last.end_code - last.start_code) + 1
                    == u32::from(entry.glyph_id)
            {
                last.end_code = entry.codepoint;
                continue;
            }
        }
        groups.push(CmapFormat12Group {
            start_code: entry.codepoint,
            end_code: entry.codepoint,
            start_glyph_id: entry.glyph_id as u32,
        });
    }

    let length = 16 + groups.len() * 12;
    let mut format12 = vec![0; length];
    write_u16_be(&mut format12, 0, CMAP_FORMAT_12);
    write_u32_be(&mut format12, 4, u32::try_from(length).unwrap_or(u32::MAX));
    write_u32_be(&mut format12, 12, u32::try_from(groups.len()).unwrap_or(u32::MAX));
    for (index, group) in groups.iter().enumerate() {
        let group_offset = 16 + index * 12;
        write_u32_be(&mut format12, group_offset, group.start_code);
        write_u32_be(&mut format12, group_offset + 4, group.end_code);
        write_u32_be(&mut format12, group_offset + 8, group.start_glyph_id);
    }

    format12
}

#[derive(Debug, Clone, Copy)]
struct CmapFormat4Segment {
    start_code: u16,
    end_code: u16,
    delta: i16,
}

#[derive(Debug, Clone, Copy)]
struct CmapFormat12Group {
    start_code: u32,
    end_code: u32,
    start_glyph_id: u32,
}

fn copy_into(src: &[u8], dst: &mut [u8], offset: usize) {
    dst[offset..offset + src.len()].copy_from_slice(src);
}

fn write_u16_be(buf: &mut Vec<u8>, offset: usize, value: u16) {
    if buf.len() < offset + 2 {
        buf.resize(offset + 2, 0);
    }
    buf[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_i16_be(buf: &mut Vec<u8>, offset: usize, value: i16) {
    write_u16_be(buf, offset, value as u16);
}

fn write_u32_be(buf: &mut Vec<u8>, offset: usize, value: u32) {
    if buf.len() < offset + 4 {
        buf.resize(offset + 4, 0);
    }
    buf[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn highest_power_of_two_at_most(value: u16) -> u16 {
    let mut result = 1u16;
    while result <= value / 2 {
        result <<= 1;
    }
    result
}

fn floor_log2_positive(value: u16) -> u16 {
    let mut result = 0u16;
    let mut current = value;
    while current > 1 {
        current >>= 1;
        result += 1;
    }
    result
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
