use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use allsorts::{
    binary::read::ReadScope,
    cff::{outline::CFFOutlines, CFF},
    font::{Font, GlyphTableFlags},
    font_data::FontData,
    outline::{OutlineBuilder, OutlineSink},
    pathfinder_geometry::{line_segment::LineSegment2F, vector::Vector2F},
    tables::{FontTableProvider, SfntVersion},
    tag,
};
use fonttool_cff::office_static::{
    extract_office_static_cff, rebuild_office_static_cff_sfnt, rebuild_office_static_cff_table,
};
use fonttool_cff::{convert_otf_to_ttf, inspect_otf_font};
use fonttool_eot::parse_eot_header;
use fonttool_mtx::{decompress_lz_with_limit, parse_mtx_container};
use fonttool_sfnt::{load_sfnt, SFNT_VERSION_OTTO};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn shared_repo_root() -> Option<PathBuf> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .ok()?;
    let worktrees_dir = workspace.parent()?;
    if worktrees_dir.file_name() != Some(OsStr::new(".worktrees")) {
        return None;
    }
    Some(worktrees_dir.parent()?.to_path_buf())
}

fn optional_fixture(relative: &str) -> Option<PathBuf> {
    let workspace_path = fixture(relative);
    if workspace_path.exists() {
        return Some(workspace_path);
    }

    let shared = shared_repo_root()?.join(relative);
    if shared.exists() {
        Some(shared)
    } else {
        None
    }
}

fn decode_block1_from_fntdata_path(path: &Path) -> Vec<u8> {
    let input = fs::read(path).expect("fixture should be readable");
    let header = parse_eot_header(&input).expect("fixture should contain a valid eot header");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let mut payload = input[payload_start..payload_end].to_vec();
    if header.flags & EOT_FLAG_PPT_XOR != 0 {
        for byte in &mut payload {
            *byte ^= 0x50;
        }
    }

    let container =
        parse_mtx_container(&payload).expect("fixture should contain a valid MTX container");
    let copy_limit = usize::try_from(container.copy_dist).expect("copy_dist should fit usize");
    decompress_lz_with_limit(container.block1, copy_limit).expect("block1 should decompress")
}

fn decode_block1_from_fntdata(relative: &str) -> Vec<u8> {
    let path = fixture(relative);
    decode_block1_from_fntdata_path(&path)
}

fn tracked_bytes(relative: &str) -> Vec<u8> {
    fs::read(fixture(relative)).expect("tracked fixture should be readable")
}

fn tracked_u32_be(relative: &str) -> Vec<u32> {
    let bytes = tracked_bytes(relative);
    assert_eq!(bytes.len() % 4, 0, "tracked u32 fixture should be aligned");
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_be_bytes(chunk.try_into().expect("u32 chunk")))
        .collect()
}

struct CffIndexHeader {
    count: u16,
    off_size: u8,
    data_start: usize,
    next: usize,
}

fn read_index_header(bytes: &[u8], offset: usize) -> CffIndexHeader {
    let count = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
    if count == 0 {
        return CffIndexHeader {
            count: 0,
            off_size: 0,
            data_start: offset + 2,
            next: offset + 2,
        };
    }

    let off_size = bytes[offset + 2];
    assert!(
        (1..=4).contains(&off_size),
        "invalid offSize {} at offset {}",
        off_size,
        offset
    );

    let offsets_end = offset + 3 + (usize::from(count) + 1) * usize::from(off_size);
    let mut previous = 0u32;
    for chunk in bytes[offset + 3..offsets_end].chunks_exact(usize::from(off_size)) {
        let mut value = 0u32;
        for byte in chunk {
            value = (value << 8) | u32::from(*byte);
        }
        if previous == 0 {
            assert_eq!(value, 1, "first CFF offset must be 1");
        } else {
            assert!(value >= previous, "CFF offsets must be monotonic");
        }
        previous = value;
    }

    let data_len = usize::try_from(previous - 1).expect("CFF data length should fit usize");
    CffIndexHeader {
        count,
        off_size,
        data_start: offsets_end,
        next: offsets_end + data_len,
    }
}

fn read_dict_number(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
    let b0 = bytes[*cursor];
    match b0 {
        32..=246 => {
            *cursor += 1;
            Some(i32::from(b0) - 139)
        }
        247..=250 => {
            let value = (i32::from(b0) - 247) * 256 + i32::from(bytes[*cursor + 1]) + 108;
            *cursor += 2;
            Some(value)
        }
        251..=254 => {
            let value = -(i32::from(b0) - 251) * 256 - i32::from(bytes[*cursor + 1]) - 108;
            *cursor += 2;
            Some(value)
        }
        28 => {
            let value = i16::from_be_bytes([bytes[*cursor + 1], bytes[*cursor + 2]]) as i32;
            *cursor += 3;
            Some(value)
        }
        29 => {
            let value = i32::from_be_bytes([
                bytes[*cursor + 1],
                bytes[*cursor + 2],
                bytes[*cursor + 3],
                bytes[*cursor + 4],
            ]);
            *cursor += 5;
            Some(value)
        }
        30 => {
            *cursor += 1;
            loop {
                let packed = bytes[*cursor];
                *cursor += 1;
                if packed >> 4 == 0xF || packed & 0xF == 0xF {
                    break;
                }
            }
            None
        }
        _ => panic!("unsupported DICT number opcode {b0} at {}", *cursor),
    }
}

fn top_dict_operand_u32(cff: &[u8], operator: &[u8]) -> u32 {
    let name = read_index_header(cff, 4);
    let top = read_index_header(cff, name.next);
    let top_dict = &cff[top.data_start..top.next];

    let mut cursor = 0usize;
    let mut stack = Vec::<i32>::new();
    while cursor < top_dict.len() {
        let byte = top_dict[cursor];
        if byte <= 21 {
            let matched = if byte == 12 {
                let escaped = [12, top_dict[cursor + 1]];
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
                    .expect("operator should have an operand")
                    .try_into()
                    .expect("operand should fit u32");
            }
            stack.clear();
        } else if let Some(value) = read_dict_number(top_dict, &mut cursor) {
            stack.push(value);
        }
    }

    panic!("missing Top DICT operator {:?}", operator);
}

fn read_charstrings_offsets(cff: &[u8]) -> Vec<u32> {
    let charstrings_offset = top_dict_operand_u32(cff, &[17]) as usize;
    let index = read_index_header(cff, charstrings_offset);
    let off_size = usize::from(index.off_size);
    let count = usize::from(index.count);
    let offsets_start = charstrings_offset + 3;
    let offsets_end = offsets_start + (count + 1) * off_size;

    cff[offsets_start..offsets_end]
        .chunks_exact(off_size)
        .map(|chunk| {
            let mut value = 0u32;
            for byte in chunk {
                value = (value << 8) | u32::from(*byte);
            }
            value
        })
        .collect()
}

fn assert_rebuilt_office_tail_slice(
    rebuilt: &[u8],
    office: &fonttool_cff::office_static::OfficeStaticCff<'_>,
    prefix_len: usize,
    rebuilt_offset: usize,
    len: usize,
) {
    assert!(
        rebuilt_offset >= prefix_len,
        "rebuilt offset {} must be inside the spliced office tail after prefix {}",
        rebuilt_offset,
        prefix_len
    );

    let office_tail_start = prefix_len
        .checked_sub(2)
        .expect("tracked standard prefix should include the two-byte splice overlap");
    let office_offset = office_tail_start + (rebuilt_offset - prefix_len);

    assert_eq!(
        &rebuilt[rebuilt_offset..rebuilt_offset + len],
        &office.office_cff_suffix[office_offset..office_offset + len]
    );
}

fn assert_source_backed_parse_contract(
    block1: &[u8],
    prefix_relative: &str,
    global_subrs_relative: &str,
    fdselect_relative: &str,
    fdarray_tail_relative: &str,
    expected_global_header: (u16, u8, usize),
) {
    let office = extract_office_static_cff(block1).expect("fixture should decode");
    let rebuilt = rebuild_office_static_cff_table(block1).expect("fixture should rebuild");
    let prefix = tracked_bytes(prefix_relative);
    let source_global_subrs_data = tracked_bytes(global_subrs_relative);
    let source_fdselect = tracked_bytes(fdselect_relative);
    let source_fdarray_tail = tracked_bytes(fdarray_tail_relative);

    assert!(rebuilt.starts_with(&prefix));

    let charset_offset = top_dict_operand_u32(&rebuilt, &[15]) as usize;
    let fdselect_range = fdselect_format3_range(&rebuilt);
    let charstrings_offset = top_dict_operand_u32(&rebuilt, &[17]) as usize;
    let charstrings = read_index_header(&rebuilt, charstrings_offset);
    let fdarray_offset = top_dict_operand_u32(&rebuilt, &[12, 36]) as usize;
    let fdarray_tail_end = fdarray_offset + source_fdarray_tail.len();

    assert_eq!(
        &rebuilt[prefix.len()..charset_offset],
        source_global_subrs_data.as_slice()
    );
    assert_eq!(
        &rebuilt[fdarray_offset..fdarray_tail_end],
        source_fdarray_tail.as_slice()
    );
    assert_eq!(
        &rebuilt[fdselect_range.0..fdselect_range.1],
        source_fdselect.as_slice()
    );

    assert_rebuilt_office_tail_slice(
        &rebuilt,
        &office,
        prefix.len(),
        charset_offset,
        fdselect_range.0 - charset_offset,
    );
    assert_rebuilt_office_tail_slice(&rebuilt, &office, prefix.len(), charstrings.data_start, 32);

    let name = read_index_header(&rebuilt, 4);
    let top = read_index_header(&rebuilt, name.next);
    let string = read_index_header(&rebuilt, top.next);
    let global = read_index_header(&rebuilt, string.next);

    assert_eq!((name.count, name.off_size), (1, 1));
    assert_eq!((top.count, top.off_size), (1, 1));
    assert_eq!((string.count, string.off_size), (23, 2));
    assert_eq!(
        (global.count, global.off_size, global.data_start),
        expected_global_header
    );
}

fn sfnt_table_bytes(sfnt: &[u8], tag: u32) -> Vec<u8> {
    let font = load_sfnt(sfnt).expect("sfnt should parse");
    font.table(tag)
        .unwrap_or_else(|| panic!("missing table {:?}", tag.to_be_bytes()))
        .data
        .clone()
}

#[derive(Default)]
struct NullOutlineSink;

impl OutlineSink for NullOutlineSink {
    fn move_to(&mut self, _to: Vector2F) {}

    fn line_to(&mut self, _to: Vector2F) {}

    fn quadratic_curve_to(&mut self, _ctrl: Vector2F, _to: Vector2F) {}

    fn cubic_curve_to(&mut self, _ctrl: LineSegment2F, _to: Vector2F) {}

    fn close(&mut self) {}
}

fn first_outline_visit_error(sfnt: &[u8]) -> Option<(u16, Option<String>, String)> {
    let scope = ReadScope::new(sfnt);
    let font_file = scope
        .read::<FontData<'_>>()
        .expect("sfnt should parse as FontData");
    let provider = font_file
        .table_provider(0)
        .expect("sfnt should provide a table provider");
    let font = Font::new(provider).expect("sfnt should build an allsorts Font");

    assert!(
        font.glyph_table_flags.contains(GlyphTableFlags::CFF),
        "diagnostic expects a static CFF font"
    );
    assert_eq!(
        font.font_table_provider.sfnt_version(),
        tag::OTTO,
        "diagnostic expects OTTO input"
    );

    let glyph_ids = (0..font.num_glyphs()).collect::<Vec<_>>();
    let glyph_names = font
        .glyph_names(&glyph_ids)
        .into_iter()
        .map(|name| name.into_owned())
        .collect::<Vec<_>>();
    let cff_data = font
        .font_table_provider
        .read_table_data(tag::CFF)
        .expect("CFF table should be readable")
        .into_owned();
    let cff = ReadScope::new(&cff_data)
        .read::<CFF<'_>>()
        .expect("CFF table should parse");
    let mut builder = CFFOutlines { table: &cff };

    for glyph_id in glyph_ids {
        let glyph_name = glyph_names.get(glyph_id as usize).cloned();
        let mut sink = NullOutlineSink;
        if let Err(error) = builder.visit(glyph_id, None, &mut sink) {
            return Some((glyph_id, glyph_name, error.to_string()));
        }
    }

    None
}

fn patch_rebuilt_cff_from_source_ranges(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    ranges: &[(usize, usize)],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for (start, end) in ranges {
        patched_cff[*start..*end].copy_from_slice(&source_cff[*start..*end]);
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn cff_charstring_range(cff: &[u8], glyph_id: u16) -> (usize, usize) {
    let charstrings_offset = top_dict_operand_u32(cff, &[17]) as usize;
    let index = read_index_header(cff, charstrings_offset);
    let offsets = read_charstrings_offsets(cff);
    let glyph_index = usize::from(glyph_id);
    let start = index.data_start + usize::try_from(offsets[glyph_index] - 1).expect("start offset");
    let end = index.data_start + usize::try_from(offsets[glyph_index + 1] - 1).expect("end offset");
    (start, end)
}

fn patch_rebuilt_cff_from_source_glyphs(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    glyph_ids: &[u16],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for glyph_id in glyph_ids {
        let (rebuilt_start, rebuilt_end) = cff_charstring_range(&patched_cff, *glyph_id);
        let (source_start, source_end) = cff_charstring_range(source_cff, *glyph_id);
        assert_eq!(
            rebuilt_end - rebuilt_start,
            source_end - source_start,
            "glyph {} charstring length should remain stable for in-place patching",
            glyph_id
        );
        patched_cff[rebuilt_start..rebuilt_end]
            .copy_from_slice(&source_cff[source_start..source_end]);
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn patch_rebuilt_cff_from_source_glyph_slices(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    slices: &[(u16, usize, usize)],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for (glyph_id, start_in_glyph, end_in_glyph) in slices {
        let (rebuilt_start, rebuilt_end) = cff_charstring_range(&patched_cff, *glyph_id);
        let (source_start, source_end) = cff_charstring_range(source_cff, *glyph_id);
        let glyph_len = rebuilt_end - rebuilt_start;
        assert_eq!(
            glyph_len,
            source_end - source_start,
            "glyph {} charstring length should remain stable for slice patching",
            glyph_id
        );
        assert!(
            start_in_glyph <= end_in_glyph && *end_in_glyph <= glyph_len,
            "glyph {} patch slice [{}, {}) should fit glyph length {}",
            glyph_id,
            start_in_glyph,
            end_in_glyph,
            glyph_len
        );
        patched_cff[rebuilt_start + *start_in_glyph..rebuilt_start + *end_in_glyph]
            .copy_from_slice(
                &source_cff[source_start + *start_in_glyph..source_start + *end_in_glyph],
            );
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn patch_rebuilt_cff_from_source_glyph_prefixes(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    prefixes: &[(u16, usize)],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for (glyph_id, prefix_len) in prefixes {
        let (rebuilt_start, rebuilt_end) = cff_charstring_range(&patched_cff, *glyph_id);
        let (source_start, source_end) = cff_charstring_range(source_cff, *glyph_id);
        assert_eq!(
            rebuilt_end - rebuilt_start,
            source_end - source_start,
            "glyph {} charstring length should remain stable for prefix patching",
            glyph_id
        );
        patched_cff[rebuilt_start..rebuilt_start + *prefix_len]
            .copy_from_slice(&source_cff[source_start..source_start + *prefix_len]);
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn looks_like_kind_swapped_lead_byte(source: u8, rebuilt: u8) -> bool {
    let source_num2 = (0xf7..=0xfe).contains(&source);
    let rebuilt_num2 = (0xf7..=0xfe).contains(&rebuilt);
    let source_op = source <= 31;
    let rebuilt_op = rebuilt <= 31;

    (source_num2 && rebuilt_num2)
        || (source_num2 && rebuilt_op)
        || (source_op && rebuilt_num2)
        || source == 0x13
        || rebuilt == 0x13
}

fn patch_rebuilt_cff_from_source_kind_swapped_leads(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    glyph_ids: &[u16],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for glyph_id in glyph_ids {
        let (rebuilt_start, rebuilt_end) = cff_charstring_range(&patched_cff, *glyph_id);
        let (source_start, source_end) = cff_charstring_range(source_cff, *glyph_id);
        let glyph_len = rebuilt_end - rebuilt_start;
        assert_eq!(
            glyph_len,
            source_end - source_start,
            "glyph {} charstring length should remain stable for lead-byte patching",
            glyph_id
        );

        let rebuilt_bytes = patched_cff[rebuilt_start..rebuilt_end].to_vec();
        let source_bytes = &source_cff[source_start..source_end];
        for index in 0..glyph_len.saturating_sub(1) {
            if source_bytes[index] == rebuilt_bytes[index] {
                continue;
            }
            if source_bytes[index + 1] != rebuilt_bytes[index + 1] {
                continue;
            }
            if !looks_like_kind_swapped_lead_byte(source_bytes[index], rebuilt_bytes[index]) {
                continue;
            }
            patched_cff[rebuilt_start + index] = source_bytes[index];
        }
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn token_head_len(span: &Type2TokenSpan) -> usize {
    match span.role {
        Type2TokenRole::MaskBytes => 0,
        Type2TokenRole::Operator | Type2TokenRole::Hintmask | Type2TokenRole::Cntrmask => span.len,
        Type2TokenRole::Number1
        | Type2TokenRole::Number2Pos
        | Type2TokenRole::Number2Neg
        | Type2TokenRole::ShortInt
        | Type2TokenRole::Fixed16_16 => 1,
    }
}

fn patch_rebuilt_cff_from_source_token_start_heads(
    rebuilt_sfnt: &[u8],
    source_cff: &[u8],
    glyph_ids: &[u16],
) -> Vec<u8> {
    let mut patched_cff = sfnt_table_bytes(rebuilt_sfnt, TAG_CFF);
    for glyph_id in glyph_ids {
        let (rebuilt_start, rebuilt_end) = cff_charstring_range(&patched_cff, *glyph_id);
        let (source_start, source_end) = cff_charstring_range(source_cff, *glyph_id);
        let glyph_len = rebuilt_end - rebuilt_start;
        assert_eq!(
            glyph_len,
            source_end - source_start,
            "glyph {} charstring length should remain stable for token-head patching",
            glyph_id
        );

        let source_bytes = &source_cff[source_start..source_end];
        for span in scan_type2_token_spans(source_bytes) {
            let head_len = token_head_len(&span);
            if head_len == 0 {
                continue;
            }
            let rebuilt_slice =
                &mut patched_cff[rebuilt_start + span.offset..rebuilt_start + span.offset + head_len];
            let source_slice = &source_bytes[span.offset..span.offset + head_len];
            if rebuilt_slice != source_slice {
                rebuilt_slice.copy_from_slice(source_slice);
            }
        }
    }

    let mut font = load_sfnt(rebuilt_sfnt).expect("rebuilt sfnt should parse");
    font.add_table(TAG_CFF, patched_cff);
    fonttool_sfnt::serialize_sfnt(&font).expect("patched sfnt should serialize")
}

fn cff_charstring_bytes(cff: &[u8], glyph_id: u16) -> Vec<u8> {
    let (start, end) = cff_charstring_range(cff, glyph_id);
    cff[start..end].to_vec()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Type2Token {
    Number(i32),
    Operator(&'static str),
    Mask(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type2TokenRole {
    Number1,
    Number2Pos,
    Number2Neg,
    ShortInt,
    Fixed16_16,
    Operator,
    Hintmask,
    Cntrmask,
    MaskBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Type2TokenSpan {
    offset: usize,
    len: usize,
    role: Type2TokenRole,
    operator: Option<&'static str>,
}

fn type2_operator_name(byte: u8) -> &'static str {
    match byte {
        1 => "hstem",
        3 => "vstem",
        4 => "vmoveto",
        5 => "rlineto",
        6 => "hlineto",
        7 => "vlineto",
        8 => "rrcurveto",
        10 => "callsubr",
        11 => "return",
        14 => "endchar",
        18 => "hstemhm",
        19 => "hintmask",
        20 => "cntrmask",
        21 => "rmoveto",
        22 => "hmoveto",
        23 => "vstemhm",
        24 => "rcurveline",
        25 => "rlinecurve",
        26 => "vvcurveto",
        27 => "hhcurveto",
        29 => "callgsubr",
        30 => "vhcurveto",
        31 => "hvcurveto",
        _ => panic!("unsupported Type 2 operator {byte}"),
    }
}

fn type2_escape_operator_name(byte: u8) -> &'static str {
    match byte {
        0 => "dotsection",
        1 => "vstem3",
        2 => "hstem3",
        3 => "and",
        4 => "or",
        5 => "not",
        9 => "abs",
        10 => "add",
        11 => "sub",
        12 => "div",
        14 => "neg",
        15 => "eq",
        18 => "drop",
        20 => "put",
        21 => "get",
        22 => "ifelse",
        23 => "random",
        24 => "mul",
        26 => "sqrt",
        27 => "dup",
        28 => "exch",
        29 => "index",
        30 => "roll",
        34 => "hflex",
        35 => "flex",
        36 => "hflex1",
        37 => "flex1",
        _ => panic!("unsupported escaped Type 2 operator 12 {byte}"),
    }
}

fn flush_type2_numbers(tokens: &mut Vec<Type2Token>, stack: &mut Vec<i32>) {
    tokens.extend(stack.drain(..).map(Type2Token::Number));
}

fn parse_type2_charstring(bytes: &[u8]) -> Vec<Type2Token> {
    let mut cursor = 0usize;
    let mut stack = Vec::<i32>::new();
    let mut tokens = Vec::<Type2Token>::new();
    let mut stem_hints = 0usize;

    while cursor < bytes.len() {
        let b0 = bytes[cursor];
        match b0 {
            32..=246 => {
                stack.push(i32::from(b0) - 139);
                cursor += 1;
            }
            247..=250 => {
                stack.push((i32::from(b0) - 247) * 256 + i32::from(bytes[cursor + 1]) + 108);
                cursor += 2;
            }
            251..=254 => {
                stack.push(-(i32::from(b0) - 251) * 256 - i32::from(bytes[cursor + 1]) - 108);
                cursor += 2;
            }
            28 => {
                stack.push(i16::from_be_bytes([bytes[cursor + 1], bytes[cursor + 2]]) as i32);
                cursor += 3;
            }
            255 => {
                stack.push(i32::from_be_bytes([
                    bytes[cursor + 1],
                    bytes[cursor + 2],
                    bytes[cursor + 3],
                    bytes[cursor + 4],
                ]));
                cursor += 5;
            }
            12 => {
                flush_type2_numbers(&mut tokens, &mut stack);
                tokens.push(Type2Token::Operator(type2_escape_operator_name(
                    bytes[cursor + 1],
                )));
                cursor += 2;
            }
            1 | 3 | 18 | 23 => {
                let stem_count = if stack.len() % 2 == 0 {
                    stack.len() / 2
                } else {
                    (stack.len() - 1) / 2
                };
                stem_hints += stem_count;
                flush_type2_numbers(&mut tokens, &mut stack);
                tokens.push(Type2Token::Operator(type2_operator_name(b0)));
                cursor += 1;
            }
            19 | 20 => {
                flush_type2_numbers(&mut tokens, &mut stack);
                tokens.push(Type2Token::Operator(type2_operator_name(b0)));
                cursor += 1;
                let mask_len = (stem_hints + 7) / 8;
                tokens.push(Type2Token::Mask(bytes[cursor..cursor + mask_len].to_vec()));
                cursor += mask_len;
            }
            4..=11 | 14 | 21 | 22 | 24..=27 | 29..=31 => {
                flush_type2_numbers(&mut tokens, &mut stack);
                tokens.push(Type2Token::Operator(type2_operator_name(b0)));
                cursor += 1;
            }
            _ => panic!("unsupported Type 2 byte {b0} at {cursor}"),
        }
    }

    flush_type2_numbers(&mut tokens, &mut stack);
    tokens
}

fn scan_type2_token_spans(bytes: &[u8]) -> Vec<Type2TokenSpan> {
    let mut cursor = 0usize;
    let mut stack_len = 0usize;
    let mut stem_hints = 0usize;
    let mut spans = Vec::<Type2TokenSpan>::new();

    while cursor < bytes.len() {
        let start = cursor;
        let b0 = bytes[cursor];
        match b0 {
            32..=246 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 1,
                    role: Type2TokenRole::Number1,
                    operator: None,
                });
                stack_len += 1;
                cursor += 1;
            }
            247..=250 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 2,
                    role: Type2TokenRole::Number2Pos,
                    operator: None,
                });
                stack_len += 1;
                cursor += 2;
            }
            251..=254 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 2,
                    role: Type2TokenRole::Number2Neg,
                    operator: None,
                });
                stack_len += 1;
                cursor += 2;
            }
            28 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 3,
                    role: Type2TokenRole::ShortInt,
                    operator: None,
                });
                stack_len += 1;
                cursor += 3;
            }
            255 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 5,
                    role: Type2TokenRole::Fixed16_16,
                    operator: None,
                });
                stack_len += 1;
                cursor += 5;
            }
            12 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 2,
                    role: Type2TokenRole::Operator,
                    operator: Some(type2_escape_operator_name(bytes[cursor + 1])),
                });
                stack_len = 0;
                cursor += 2;
            }
            1 | 3 | 18 | 23 => {
                let stem_count = if stack_len % 2 == 0 {
                    stack_len / 2
                } else {
                    (stack_len - 1) / 2
                };
                stem_hints += stem_count;
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 1,
                    role: Type2TokenRole::Operator,
                    operator: Some(type2_operator_name(b0)),
                });
                stack_len = 0;
                cursor += 1;
            }
            19 | 20 => {
                let role = if b0 == 19 {
                    Type2TokenRole::Hintmask
                } else {
                    Type2TokenRole::Cntrmask
                };
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 1,
                    role,
                    operator: Some(type2_operator_name(b0)),
                });
                stack_len = 0;
                cursor += 1;
                let mask_len = (stem_hints + 7) / 8;
                if mask_len != 0 {
                    spans.push(Type2TokenSpan {
                        offset: cursor,
                        len: mask_len,
                        role: Type2TokenRole::MaskBytes,
                        operator: None,
                    });
                }
                cursor += mask_len;
            }
            4..=11 | 14 | 21 | 22 | 24..=27 | 29..=31 => {
                spans.push(Type2TokenSpan {
                    offset: start,
                    len: 1,
                    role: Type2TokenRole::Operator,
                    operator: Some(type2_operator_name(b0)),
                });
                stack_len = 0;
                cursor += 1;
            }
            _ => panic!("unsupported Type 2 byte {b0} at {cursor}"),
        }
    }

    spans
}

fn first_type2_draw_byte_offset(bytes: &[u8]) -> usize {
    let mut cursor = 0usize;
    let mut stack = Vec::<i32>::new();
    let mut stem_hints = 0usize;

    while cursor < bytes.len() {
        let b0 = bytes[cursor];
        match b0 {
            32..=246 => {
                stack.push(i32::from(b0) - 139);
                cursor += 1;
            }
            247..=250 => {
                stack.push((i32::from(b0) - 247) * 256 + i32::from(bytes[cursor + 1]) + 108);
                cursor += 2;
            }
            251..=254 => {
                stack.push(-(i32::from(b0) - 251) * 256 - i32::from(bytes[cursor + 1]) - 108);
                cursor += 2;
            }
            28 => {
                stack.push(i16::from_be_bytes([bytes[cursor + 1], bytes[cursor + 2]]) as i32);
                cursor += 3;
            }
            255 => {
                stack.push(i32::from_be_bytes([
                    bytes[cursor + 1],
                    bytes[cursor + 2],
                    bytes[cursor + 3],
                    bytes[cursor + 4],
                ]));
                cursor += 5;
            }
            12 => {
                stack.clear();
                cursor += 2;
            }
            1 | 3 | 18 | 23 => {
                let stem_count = if stack.len() % 2 == 0 {
                    stack.len() / 2
                } else {
                    (stack.len() - 1) / 2
                };
                stem_hints += stem_count;
                stack.clear();
                cursor += 1;
            }
            19 | 20 => {
                stack.clear();
                cursor += 1 + (stem_hints + 7) / 8;
            }
            4..=8 | 21 | 22 | 24..=27 | 30 | 31 => return cursor + 1,
            10 | 11 | 14 | 29 => {
                stack.clear();
                cursor += 1;
            }
            _ => panic!("unsupported Type 2 byte {b0} at {cursor}"),
        }
    }

    bytes.len()
}

fn fdselect_format3_fd_for_glyph(cff: &[u8], glyph_id: u16) -> u8 {
    let fdselect_offset = top_dict_operand_u32(cff, &[12, 37]) as usize;
    assert_eq!(cff[fdselect_offset], 3, "expected FDSelect format 3");
    let nranges = usize::from(u16::from_be_bytes([
        cff[fdselect_offset + 1],
        cff[fdselect_offset + 2],
    ]));
    let ranges_start = fdselect_offset + 3;
    let sentinel_offset = ranges_start + nranges * 3;
    let sentinel = u16::from_be_bytes([cff[sentinel_offset], cff[sentinel_offset + 1]]);

    for range_index in 0..nranges {
        let entry = ranges_start + range_index * 3;
        let start = u16::from_be_bytes([cff[entry], cff[entry + 1]]);
        let fd = cff[entry + 2];
        let end = if range_index + 1 < nranges {
            let next = entry + 3;
            u16::from_be_bytes([cff[next], cff[next + 1]])
        } else {
            sentinel
        };
        if glyph_id >= start && glyph_id < end {
            return fd;
        }
    }

    panic!("glyph {} is not covered by FDSelect format 3", glyph_id);
}

fn fdselect_format3_range(cff: &[u8]) -> (usize, usize) {
    let fdselect_offset = top_dict_operand_u32(cff, &[12, 37]) as usize;
    assert_eq!(cff[fdselect_offset], 3, "expected FDSelect format 3");
    let nranges = usize::from(u16::from_be_bytes([
        cff[fdselect_offset + 1],
        cff[fdselect_offset + 2],
    ]));
    let len = 1 + 2 + nranges * 3 + 2;
    (fdselect_offset, fdselect_offset + len)
}

#[test]
fn extract_office_static_cff_finds_regular_fixture_payload() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let office = extract_office_static_cff(&block1).expect("regular fixture should be recognized");

    assert!(office.sfnt_bytes.starts_with(b"OTTO"));
    assert_eq!(office.cff_offset, 0x20e);
    assert!(office
        .office_cff_suffix
        .starts_with(&[0x04, 0x03, 0x00, 0x01]));
}

#[test]
fn extract_office_static_cff_finds_bold_fixture_payload() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let office = extract_office_static_cff(&block1).expect("bold fixture should be recognized");

    assert!(office.sfnt_bytes.starts_with(b"OTTO"));
    assert_eq!(office.cff_offset, 0x20e);
    assert!(office
        .office_cff_suffix
        .starts_with(&[0x04, 0x03, 0x00, 0x01]));
}

#[test]
fn extract_office_static_cff_accepts_minimal_exact_length_payload() {
    let mut bytes = vec![0; 0x20e + 4];
    bytes[..4].copy_from_slice(b"OTTO");
    bytes[0x20e..0x20e + 4].copy_from_slice(&[0x04, 0x03, 0x00, 0x01]);

    let office = extract_office_static_cff(&bytes).expect("minimal payload should be accepted");

    assert_eq!(office.cff_offset, 0x20e);
    assert_eq!(office.office_cff_suffix, &[0x04, 0x03, 0x00, 0x01]);
}

#[test]
fn extract_office_static_cff_normalizes_single_leading_nul_before_otto() {
    let mut bytes = vec![0; 1 + 0x20e + 4];
    bytes[1..5].copy_from_slice(b"OTTO");
    bytes[1 + 0x20e..1 + 0x20e + 4].copy_from_slice(&[0x04, 0x03, 0x00, 0x01]);

    let office =
        extract_office_static_cff(&bytes).expect("leading nul-prefixed payload should normalize");

    assert_eq!(&office.sfnt_bytes[..4], b"OTTO");
    assert_eq!(office.cff_offset, 0x20e);
    assert_eq!(office.office_cff_suffix, &[0x04, 0x03, 0x00, 0x01]);
}

#[test]
fn office_static_raw_suffix_is_not_a_direct_standard_name_index() {
    for relative in [
        "testdata/otto-cff-office.fntdata",
        "testdata/presentation1-font2-bold.fntdata",
    ] {
        let block1 = decode_block1_from_fntdata(relative);
        let office = extract_office_static_cff(&block1).expect("fixture should decode");
        let raw_name_off_size = office.office_cff_suffix[6];

        assert!(raw_name_off_size > 4);
        assert_eq!(office.office_cff_suffix[4], 0x01);
        assert_eq!(office.office_cff_suffix[5], 0x02);
    }
}

#[test]
fn tracked_charstrings_reference_fixtures_match_known_landmarks() {
    let regular = tracked_u32_be("testdata/sourcehan-sc-regular-charstrings-offsets.bin");
    let bold = tracked_u32_be("testdata/sourcehan-sc-bold-charstrings-offsets.bin");

    assert_eq!(regular.len(), 65_536);
    assert_eq!(bold.len(), 65_536);
    assert_eq!(
        &regular[..10],
        &[1, 77, 80, 114, 163, 257, 368, 504, 658, 687]
    );
    assert_eq!(&bold[..10], &[1, 77, 80, 129, 149, 247, 370, 507, 668, 679]);
    assert_eq!(
        &regular[regular.len() - 3..],
        &[14_059_736, 14_059_738, 14_059_740]
    );
    assert_eq!(
        &bold[bold.len() - 3..],
        &[14_575_011, 14_575_013, 14_575_015]
    );
}

#[test]
fn rebuild_office_static_cff_table_restores_regular_charstrings_offsets() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt = rebuild_office_static_cff_table(&block1).expect("regular fixture should rebuild");
    let actual = read_charstrings_offsets(&rebuilt);
    let expected = tracked_u32_be("testdata/sourcehan-sc-regular-charstrings-offsets.bin");

    assert_eq!(actual, expected);
}

#[test]
fn rebuild_office_static_cff_table_restores_bold_charstrings_offsets() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let rebuilt = rebuild_office_static_cff_table(&block1).expect("bold fixture should rebuild");
    let actual = read_charstrings_offsets(&rebuilt);
    let expected = tracked_u32_be("testdata/sourcehan-sc-bold-charstrings-offsets.bin");

    assert_eq!(actual, expected);
}

fn assert_rebuilt_fdselect_matches_tracked_fixture(block1: &[u8], tracked_fdselect_relative: &str) {
    let rebuilt = rebuild_office_static_cff_table(block1).expect("fixture should rebuild");
    let expected = tracked_bytes(tracked_fdselect_relative);
    let fdselect_range = fdselect_format3_range(&rebuilt);

    assert_eq!(
        &rebuilt[fdselect_range.0..fdselect_range.1],
        expected.as_slice()
    );
}

#[test]
fn rebuild_office_static_cff_table_restores_regular_fdselect() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    assert_rebuilt_fdselect_matches_tracked_fixture(
        &block1,
        "testdata/sourcehan-sc-regular-cff-fdselect.bin",
    );
}

#[test]
fn rebuild_office_static_cff_table_restores_bold_fdselect() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    assert_rebuilt_fdselect_matches_tracked_fixture(
        &block1,
        "testdata/sourcehan-sc-bold-cff-fdselect.bin",
    );
}

#[test]
fn rebuild_office_static_cff_table_restores_regular_prefix_and_source_backed_parse_regions() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    assert_source_backed_parse_contract(
        &block1,
        "testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin",
        "testdata/sourcehan-sc-regular-cff-global-subrs-data.bin",
        "testdata/sourcehan-sc-regular-cff-fdselect.bin",
        "testdata/sourcehan-sc-regular-cff-fdarray-tail.bin",
        (932, 2, 2806),
    );
}

#[test]
fn rebuild_office_static_cff_table_restores_bold_prefix_and_source_backed_parse_regions() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    assert_source_backed_parse_contract(
        &block1,
        "testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin",
        "testdata/sourcehan-sc-bold-cff-global-subrs-data.bin",
        "testdata/sourcehan-sc-bold-cff-fdselect.bin",
        "testdata/sourcehan-sc-bold-cff-fdarray-tail.bin",
        (1240, 2, 3362),
    );
}

#[test]
fn rebuild_office_static_cff_table_supports_local_sourcehan_family_cases_if_present() {
    let Some(_root_case) = optional_fixture("testdata/OTF_CFF/SourceHanSansSC/full.pptx") else {
        return;
    };

    for (relative_fntdata, expected_prefix, expected_offsets, expected_fdselect) in [
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font1.fntdata",
            "testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-regular-charstrings-offsets.bin",
            "testdata/sourcehan-sc-regular-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font2.fntdata",
            "testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-bold-charstrings-offsets.bin",
            "testdata/sourcehan-sc-bold-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font3.fntdata",
            "testdata/sourcehan-sc-extralight-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-extralight-charstrings-offsets.bin",
            "testdata/sourcehan-sc-extralight-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font4.fntdata",
            "testdata/sourcehan-sc-heavy-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-heavy-charstrings-offsets.bin",
            "testdata/sourcehan-sc-heavy-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font5.fntdata",
            "testdata/sourcehan-sc-light-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-light-charstrings-offsets.bin",
            "testdata/sourcehan-sc-light-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font6.fntdata",
            "testdata/sourcehan-sc-medium-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-medium-charstrings-offsets.bin",
            "testdata/sourcehan-sc-medium-cff-fdselect.bin",
        ),
        (
            "testdata/OTF_CFF/SourceHanSansSC/embedded_fonts/font7.fntdata",
            "testdata/sourcehan-sc-normal-cff-prefix-through-global-subrs.bin",
            "testdata/sourcehan-sc-normal-charstrings-offsets.bin",
            "testdata/sourcehan-sc-normal-cff-fdselect.bin",
        ),
    ] {
        let path = optional_fixture(relative_fntdata).expect("local SourceHan case should exist");
        let block1 = decode_block1_from_fntdata_path(&path);
        let rebuilt =
            rebuild_office_static_cff_table(&block1).expect("local SourceHan case should rebuild");
        let prefix = tracked_bytes(expected_prefix);
        let offsets = tracked_u32_be(expected_offsets);
        let fdselect = tracked_bytes(expected_fdselect);
        let fdselect_range = fdselect_format3_range(&rebuilt);

        assert!(rebuilt.starts_with(&prefix), "{relative_fntdata}");
        assert_eq!(
            read_charstrings_offsets(&rebuilt),
            offsets,
            "{relative_fntdata}"
        );
        assert_eq!(
            &rebuilt[fdselect_range.0..fdselect_range.1],
            fdselect.as_slice(),
            "{relative_fntdata}"
        );
    }
}

#[test]
fn rebuild_office_static_cff_sfnt_materializes_regular_fixture_into_parseable_otto() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_cff =
        rebuild_office_static_cff_table(&block1).expect("regular fixture should rebuild CFF");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild SFNT");
    let donor = tracked_bytes("testdata/sourcehan-sc-regular-sfnt-without-cff.otf");

    let donor_font = load_sfnt(&donor).expect("tracked donor shell should parse");
    let rebuilt_font = load_sfnt(&rebuilt_sfnt).expect("rebuilt SFNT should parse");

    assert_eq!(rebuilt_font.version_tag(), SFNT_VERSION_OTTO);
    assert_eq!(rebuilt_font.tables().len(), donor_font.tables().len() + 1);
    assert_eq!(
        rebuilt_font
            .table(TAG_CFF)
            .expect("rebuilt SFNT should contain CFF")
            .data,
        rebuilt_cff
    );

    for donor_table in donor_font.tables() {
        let rebuilt_table = rebuilt_font
            .table(donor_table.tag)
            .expect("rebuilt SFNT should preserve donor tables");
        if donor_table.tag == TAG_HEAD {
            assert_eq!(
                &rebuilt_table.data[..8],
                &donor_table.data[..8],
                "head prefix"
            );
            assert_eq!(
                &rebuilt_table.data[12..],
                &donor_table.data[12..],
                "head suffix"
            );
            continue;
        }
        assert_eq!(
            rebuilt_table.data,
            donor_table.data,
            "tag {:?}",
            donor_table.tag.to_be_bytes()
        );
    }

    let kind = inspect_otf_font(&rebuilt_sfnt).expect("rebuilt SFNT should inspect as OTF/CFF");
    assert!(kind.is_cff_flavor);
    assert!(!kind.is_variable);
}

#[test]
fn rebuild_office_static_cff_sfnt_advances_regular_fixture_past_parse_failure() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild SFNT");
    let error = convert_otf_to_ttf(&rebuilt_sfnt, &[])
        .expect_err("regular rebuilt SFNT should still expose a deeper outline blocker");

    assert!(
        error.to_string().contains(
            "failed to visit glyph outline: an invalid amount of items are in an arguments stack"
        ),
        "{error}"
    );
}

#[test]
fn rebuild_office_static_cff_sfnt_materializes_bold_fixture_into_parseable_otto() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("bold fixture should rebuild SFNT");
    let donor = tracked_bytes("testdata/sourcehan-sc-bold-sfnt-without-cff.otf");

    let donor_font = load_sfnt(&donor).expect("tracked bold donor shell should parse");
    let rebuilt_font = load_sfnt(&rebuilt_sfnt).expect("rebuilt bold SFNT should parse");
    let kind = inspect_otf_font(&rebuilt_sfnt).expect("rebuilt bold SFNT should inspect");

    assert_eq!(rebuilt_font.version_tag(), SFNT_VERSION_OTTO);
    assert_eq!(rebuilt_font.tables().len(), donor_font.tables().len() + 1);
    assert!(kind.is_cff_flavor);
    assert!(!kind.is_variable);
}

#[test]
fn local_regular_source_backfill_from_global_subrs_data_start_unblocks_convert_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let global_subrs_data_start =
        tracked_bytes("testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin").len();
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[(global_subrs_data_start, source_cff.len())],
    );

    convert_otf_to_ttf(&patched, &[]).expect("source-backed control should convert");
}

#[test]
fn local_regular_source_backfill_of_globalsubrs_and_fdarray_tail_still_fails_outline_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let global_subrs_data_start =
        tracked_bytes("testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin").len();
    let charset_offset = top_dict_operand_u32(&source_cff, &[15]) as usize;
    let fdarray_offset = top_dict_operand_u32(&source_cff, &[12, 36]) as usize;
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (global_subrs_data_start, charset_offset),
            (fdarray_offset, source_cff.len()),
        ],
    );
    let error = convert_otf_to_ttf(&patched, &[])
        .expect_err("charstrings data should still block outline conversion");

    assert!(
        error.to_string().contains(
            "failed to visit glyph outline: an invalid amount of items are in an arguments stack"
        ),
        "{error}"
    );
}

#[test]
fn rebuild_office_static_cff_sfnt_regular_first_outline_failure_is_notdef() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let failure = first_outline_visit_error(&rebuilt_sfnt)
        .expect("current bounded rebuild should still expose an outline failure");

    assert_eq!(failure.0, 0);
    assert_eq!(failure.1.as_deref(), Some(".notdef"));
    assert!(
        failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        failure.2
    );
}

#[test]
fn local_regular_current_rebuilt_notdef_charstring_still_differs_from_source_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let rebuilt_charstring = cff_charstring_bytes(&rebuilt_cff, 0);
    let source_charstring = cff_charstring_bytes(&source_cff, 0);

    assert_eq!(rebuilt_charstring.len(), source_charstring.len());
    assert_ne!(rebuilt_charstring, source_charstring);
}

#[test]
fn local_regular_current_rebuilt_notdef_keeps_prefix_then_diverges_midstream_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let rebuilt_tokens = parse_type2_charstring(&cff_charstring_bytes(&rebuilt_cff, 0));
    let source_tokens = parse_type2_charstring(&cff_charstring_bytes(&source_cff, 0));

    assert_eq!(&rebuilt_tokens[..28], &source_tokens[..28]);
    assert_eq!(
        &source_tokens[28..36],
        &[
            Type2Token::Number(-313),
            Type2Token::Number(403),
            Type2Token::Number(313),
            Type2Token::Number(403),
            Type2Token::Operator("rlineto"),
            Type2Token::Number(-31),
            Type2Token::Number(-846),
            Type2Token::Operator("rmoveto"),
        ]
    );
    assert_eq!(
        &rebuilt_tokens[28..36],
        &[
            Type2Token::Number(66),
            Type2Token::Number(-100),
            Type2Token::Number(369),
            Type2Token::Number(147),
            Type2Token::Operator("rlineto"),
            Type2Token::Number(-100),
            Type2Token::Operator("rlineto"),
            Type2Token::Number(-31),
        ]
    );
}

#[test]
fn local_regular_current_rebuilt_short_callsubr_charstrings_show_operand_rewrite_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    assert_eq!(
        cff_charstring_bytes(&source_cff, 42),
        [0xfb, 0xd8, 0xed, 0x0a]
    );
    assert_eq!(
        cff_charstring_bytes(&rebuilt_cff, 42),
        [0x75, 0xd8, 0xed, 0x0a]
    );
    assert_eq!(cff_charstring_bytes(&source_cff, 194), [0xe8, 0xac, 0x0a]);
    assert_eq!(cff_charstring_bytes(&rebuilt_cff, 194), [0xac, 0xf5, 0x0a]);
}

#[test]
fn local_bold_current_rebuilt_short_subr_charstrings_show_operand_rewrite_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Bold.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("bold fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local bold source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    assert_eq!(
        cff_charstring_bytes(&source_cff, 42),
        [0xfb, 0xe7, 0xe4, 0x0a]
    );
    assert_eq!(
        cff_charstring_bytes(&rebuilt_cff, 42),
        [0x77, 0xe7, 0xe4, 0x0a]
    );
    assert_eq!(
        cff_charstring_bytes(&source_cff, 89),
        [0x20, 0xfb, 0x6b, 0x1d]
    );
    assert_eq!(
        cff_charstring_bytes(&rebuilt_cff, 89),
        [0x20, 0xfb, 0x15, 0x1d]
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_only_moves_first_outline_failure_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0]);
    let failure = first_outline_visit_error(&patched)
        .expect("backfilling only .notdef should still leave a later outline failure");

    assert_eq!(failure.0, 3);
    assert_eq!(failure.1.as_deref(), Some("quotedbl"));
}

#[test]
fn local_regular_source_backfill_of_notdef_and_quotedbl_moves_first_outline_failure_again_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3]);
    let failure = first_outline_visit_error(&patched)
        .expect("backfilling .notdef and quotedbl should still leave a later outline failure");

    assert_eq!(failure.0, 4);
    assert_eq!(failure.1.as_deref(), Some("numbersign"));
}

#[test]
fn local_regular_source_backfill_of_notdef_and_quotedbl_suffix_only_moves_first_outline_failure_to_numbersign_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph3_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 3));
    let glyph3_len = cff_charstring_bytes(&source_cff, 3).len();
    let patched = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, glyph3_prefix_len, glyph3_len),
        ],
    );
    let failure = first_outline_visit_error(&patched).expect(
        "backfilling .notdef and only the glyph 3 suffix should still leave a later failure",
    );

    assert_eq!(failure.0, 4);
    assert_eq!(failure.1.as_deref(), Some("numbersign"));
}

#[test]
fn local_regular_source_backfill_of_notdef_quotedbl_and_numbersign_moves_first_outline_failure_to_dollar_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4]);
    let failure = first_outline_visit_error(&patched)
        .expect("backfilling .notdef, quotedbl, and numbersign should still leave a later failure");

    assert_eq!(failure.0, 5);
    assert_eq!(failure.1.as_deref(), Some("dollar"));
    assert!(
        failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_quotedbl_and_numbersign_suffix_only_moves_first_outline_failure_to_dollar_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph4_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 4));
    let glyph4_len = cff_charstring_bytes(&source_cff, 4).len();
    let patched = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, glyph4_prefix_len, glyph4_len),
        ],
    );
    let failure = first_outline_visit_error(&patched).expect(
        "backfilling .notdef, quotedbl, and only the glyph 4 suffix should still leave a later failure",
    );

    assert_eq!(failure.0, 5);
    assert_eq!(failure.1.as_deref(), Some("dollar"));
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_moves_first_outline_failure_to_percent_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let failure = first_outline_visit_error(&patched)
        .expect("backfilling .notdef through dollar should still leave a later failure at percent");

    assert_eq!(failure.0, 6);
    assert_eq!(failure.1.as_deref(), Some("percent"));
    assert!(
        failure.2.contains("an invalid operator occurred"),
        "{}",
        failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_requires_more_than_one_region_of_dollar_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));
    let glyph5_len = cff_charstring_bytes(&source_cff, 5).len();
    let prefix_only = patch_rebuilt_cff_from_source_glyph_prefixes(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, cff_charstring_bytes(&source_cff, 3).len()),
            (4, cff_charstring_bytes(&source_cff, 4).len()),
            (5, glyph5_prefix_len),
        ],
    );
    let suffix_only = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, glyph5_prefix_len, glyph5_len),
        ],
    );
    let prefix_failure = first_outline_visit_error(&prefix_only)
        .expect("glyph 5 prefix-only patch should still leave a failure at glyph 5");
    let suffix_failure = first_outline_visit_error(&suffix_only)
        .expect("glyph 5 suffix-only patch should still leave a failure at glyph 5");

    assert_eq!(prefix_failure.0, 5);
    assert_eq!(prefix_failure.1.as_deref(), Some("dollar"));
    assert_eq!(suffix_failure.0, 5);
    assert_eq!(suffix_failure.1.as_deref(), Some("dollar"));
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_and_glyph5_prefix_plus_late_window_moves_first_outline_failure_to_percent_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));
    let glyph5_late_window = (70usize, 91usize);
    let patched = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_window.0, glyph5_late_window.1),
        ],
    );
    let failure = first_outline_visit_error(&patched)
        .expect("glyph 5 prefix plus late-window patch should still leave a later failure");

    assert_eq!(failure.0, 6);
    assert_eq!(failure.1.as_deref(), Some("percent"));
    assert!(failure.2.contains("an invalid operator occurred"), "{}", failure.2);
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_shows_glyph5_late_window_is_causal_but_mid_window_is_not_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_len = cff_charstring_bytes(&source_cff, 5).len();
    let glyph5_mid_window = (31usize, 48usize);
    let glyph5_late_window = (70usize, 91usize);
    let full_except_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_mid_window.0),
            (5, glyph5_mid_window.1, glyph5_len),
        ],
    );
    let full_except_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_window.0),
            (5, glyph5_late_window.1, glyph5_len),
        ],
    );
    let mid_failure = first_outline_visit_error(&full_except_mid)
        .expect("glyph 5 full-except-mid patch should still leave a later failure");
    let late_failure = first_outline_visit_error(&full_except_late)
        .expect("glyph 5 full-except-late patch should still fail");

    assert_eq!(mid_failure.0, 6);
    assert_eq!(mid_failure.1.as_deref(), Some("percent"));
    assert!(
        mid_failure.2.contains("an invalid operator occurred"),
        "{}",
        mid_failure.2
    );
    assert_eq!(late_failure.0, 5);
    assert_eq!(late_failure.1.as_deref(), Some("dollar"));
    assert!(
        late_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        late_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_shows_glyph5_bytes_72_74_are_the_causal_late_subwindow_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_len = cff_charstring_bytes(&source_cff, 5).len();
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));
    let glyph5_late_head_a = (70usize, 72usize);
    let glyph5_late_head_b = (72usize, 74usize);
    let prefix_and_late_head_a = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_head_a.0, glyph5_late_head_a.1),
        ],
    );
    let prefix_and_late_head_b = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_head_b.0, glyph5_late_head_b.1),
        ],
    );
    let full_except_late_head_a = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_head_a.0),
            (5, glyph5_late_head_a.1, glyph5_len),
        ],
    );
    let full_except_late_head_b = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_head_b.0),
            (5, glyph5_late_head_b.1, glyph5_len),
        ],
    );
    let a_prefix_failure = first_outline_visit_error(&prefix_and_late_head_a)
        .expect("glyph 5 prefix plus [70,72) should still fail");
    let b_prefix_failure = first_outline_visit_error(&prefix_and_late_head_b)
        .expect("glyph 5 prefix plus [72,74) should still leave a later failure");
    let a_except_failure = first_outline_visit_error(&full_except_late_head_a)
        .expect("glyph 5 full-except-[70,72) should still leave a later failure");
    let b_except_failure = first_outline_visit_error(&full_except_late_head_b)
        .expect("glyph 5 full-except-[72,74) should still fail");

    assert_eq!(a_prefix_failure.0, 5);
    assert_eq!(a_prefix_failure.1.as_deref(), Some("dollar"));
    assert!(
        a_prefix_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        a_prefix_failure.2
    );
    assert_eq!(b_prefix_failure.0, 6);
    assert_eq!(b_prefix_failure.1.as_deref(), Some("percent"));
    assert!(
        b_prefix_failure.2.contains("an invalid operator occurred"),
        "{}",
        b_prefix_failure.2
    );
    assert_eq!(a_except_failure.0, 6);
    assert_eq!(a_except_failure.1.as_deref(), Some("percent"));
    assert!(
        a_except_failure.2.contains("an invalid operator occurred"),
        "{}",
        a_except_failure.2
    );
    assert_eq!(b_except_failure.0, 5);
    assert_eq!(b_except_failure.1.as_deref(), Some("dollar"));
    assert!(
        b_except_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        b_except_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_shows_glyph5_byte_72_is_the_single_causal_late_byte_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_source = cff_charstring_bytes(&source_cff, 5);
    let glyph5_rebuilt = cff_charstring_bytes(&rebuilt_cff, 5);
    let glyph5_len = glyph5_source.len();
    let glyph5_prefix_len = first_type2_draw_byte_offset(&glyph5_source);

    assert_eq!(glyph5_source.len(), glyph5_rebuilt.len());
    assert_eq!(glyph5_source[73], glyph5_rebuilt[73]);
    assert_ne!(glyph5_source[72], glyph5_rebuilt[72]);

    let prefix_and_byte72 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, 72, 73),
        ],
    );
    let full_except_byte72 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, 72),
            (5, 73, glyph5_len),
        ],
    );
    let prefix_failure = first_outline_visit_error(&prefix_and_byte72)
        .expect("glyph 5 prefix plus byte 72 patch should still leave a later failure");
    let except_failure = first_outline_visit_error(&full_except_byte72)
        .expect("glyph 5 full-except-byte-72 patch should still fail");

    assert_eq!(prefix_failure.0, 6);
    assert_eq!(prefix_failure.1.as_deref(), Some("percent"));
    assert!(
        prefix_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        prefix_failure.2
    );
    assert_eq!(except_failure.0, 5);
    assert_eq!(except_failure.1.as_deref(), Some("dollar"));
    assert!(
        except_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        except_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_shows_glyph6_needs_more_than_prefix_and_has_a_late_invalid_operator_region_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_len = cff_charstring_bytes(&source_cff, 6).len();
    let glyph6_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 6));
    let glyph6_mid_window = (30usize, 90usize);
    let glyph6_late_window = (90usize, glyph6_len);
    let prefix_only = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_prefix_len),
        ],
    );
    let full_except_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_mid_window.0),
            (6, glyph6_mid_window.1, glyph6_len),
        ],
    );
    let full_except_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_late_window.0),
            (6, glyph6_late_window.1, glyph6_len),
        ],
    );
    let prefix_failure = first_outline_visit_error(&prefix_only)
        .expect("glyph 6 prefix-only patch should still leave a failure at glyph 6");
    let mid_failure = first_outline_visit_error(&full_except_mid)
        .expect("glyph 6 full-except-mid patch should still leave a failure at glyph 6");
    let late_failure = first_outline_visit_error(&full_except_late)
        .expect("glyph 6 full-except-late patch should still leave a failure at glyph 6");

    assert_eq!(prefix_failure.0, 6);
    assert_eq!(prefix_failure.1.as_deref(), Some("percent"));
    assert!(
        prefix_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        prefix_failure.2
    );
    assert_eq!(mid_failure.0, 6);
    assert_eq!(mid_failure.1.as_deref(), Some("percent"));
    assert!(
        mid_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        mid_failure.2
    );
    assert_eq!(late_failure.0, 6);
    assert_eq!(late_failure.1.as_deref(), Some("percent"));
    assert!(
        late_failure.2.contains("an invalid operator occurred"),
        "{}",
        late_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_shows_glyph6_bytes_25_and_27_explain_the_prefix_invalid_operator_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let byte25_only = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 25, 26),
        ],
    );
    let byte27_only = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 27, 28),
        ],
    );
    let bytes25_27 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 25, 26),
            (6, 27, 28),
        ],
    );
    let byte25_failure = first_outline_visit_error(&byte25_only)
        .expect("glyph 6 byte25-only patch should still fail");
    let byte27_failure = first_outline_visit_error(&byte27_only)
        .expect("glyph 6 byte27-only patch should still fail");
    let both_failure = first_outline_visit_error(&bytes25_27)
        .expect("glyph 6 byte25+27 patch should still fail later");

    assert_eq!(byte25_failure.0, 6);
    assert_eq!(byte25_failure.1.as_deref(), Some("percent"));
    assert!(
        byte25_failure.2.contains("an invalid operator occurred"),
        "{}",
        byte25_failure.2
    );
    assert_eq!(byte27_failure.0, 6);
    assert_eq!(byte27_failure.1.as_deref(), Some("percent"));
    assert!(
        byte27_failure.2.contains("an invalid operator occurred"),
        "{}",
        byte27_failure.2
    );
    assert_eq!(both_failure.0, 6);
    assert_eq!(both_failure.1.as_deref(), Some("percent"));
    assert!(
        both_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        both_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_shows_glyph6_late_bytes_114_and_119_are_each_sufficient_to_retain_invalid_operator_but_118_121_is_only_contextual_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_len = cff_charstring_bytes(&source_cff, 6).len();
    let full_except_114 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, 114),
            (6, 115, glyph6_len),
        ],
    );
    let full_except_119 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, 119),
            (6, 120, glyph6_len),
        ],
    );
    let full_except_118_121 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, 118),
            (6, 121, glyph6_len),
        ],
    );
    let byte114_failure = first_outline_visit_error(&full_except_114)
        .expect("glyph 6 full-except-byte114 patch should still fail");
    let byte119_failure = first_outline_visit_error(&full_except_119)
        .expect("glyph 6 full-except-byte119 patch should still fail");
    let window118_121_failure = first_outline_visit_error(&full_except_118_121)
        .expect("glyph 6 full-except-118..121 patch should still leave a later failure");

    assert_eq!(byte114_failure.0, 6);
    assert_eq!(byte114_failure.1.as_deref(), Some("percent"));
    assert!(
        byte114_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        byte114_failure.2
    );
    assert_eq!(byte119_failure.0, 6);
    assert_eq!(byte119_failure.1.as_deref(), Some("percent"));
    assert!(
        byte119_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        byte119_failure.2
    );
    assert_eq!(window118_121_failure.0, 7);
    assert_eq!(window118_121_failure.1.as_deref(), Some("ampersand"));
    assert!(
        window118_121_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        window118_121_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_numbersign_and_glyph5_prefix_plus_kind_swapped_leads_moves_first_outline_failure_to_percent_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));
    let patched = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
        ],
    );
    let patched = patch_rebuilt_cff_from_source_kind_swapped_leads(&patched, &source_cff, &[5]);
    let failure = first_outline_visit_error(&patched)
        .expect("glyph 5 prefix plus kind-swapped leads should still leave a later failure");

    assert_eq!(failure.0, 6);
    assert_eq!(failure.1.as_deref(), Some("percent"));
    assert!(failure.2.contains("an invalid operator occurred"), "{}", failure.2);
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_kind_swapped_leads_changes_invalid_operator_to_stack_error_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched = patch_rebuilt_cff_from_source_kind_swapped_leads(&patched, &source_cff, &[6]);
    let failure = first_outline_visit_error(&patched)
        .expect("glyph 6 kind-swapped leads should still leave a later failure");

    assert_eq!(failure.0, 6);
    assert_eq!(failure.1.as_deref(), Some("percent"));
    assert!(
        failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        failure.2
    );
}

#[test]
fn local_regular_glyph6_late_region_token_starts_show_normal_draw_tokens_not_hintmask_boundary_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6 = cff_charstring_bytes(&source_cff, 6);
    let spans = scan_type2_token_spans(&glyph6);

    let span111 = spans
        .iter()
        .find(|span| span.offset == 111)
        .expect("glyph 6 should have a token start at byte 111");
    let span113 = spans
        .iter()
        .find(|span| span.offset == 113)
        .expect("glyph 6 should have a token start at byte 113");
    let span114 = spans
        .iter()
        .find(|span| span.offset == 114)
        .expect("glyph 6 should have a token start at byte 114");
    let span117 = spans
        .iter()
        .find(|span| span.offset == 117)
        .expect("glyph 6 should have a token start at byte 117");
    let span119 = spans
        .iter()
        .find(|span| span.offset == 119)
        .expect("glyph 6 should have a token start at byte 119");

    assert_eq!(span111.role, Type2TokenRole::Operator);
    assert_eq!(span111.operator, Some("hvcurveto"));
    assert_eq!(span113.role, Type2TokenRole::Operator);
    assert_eq!(span113.operator, Some("vmoveto"));
    assert_eq!(span114.role, Type2TokenRole::Number1);
    assert_eq!(span117.role, Type2TokenRole::Number2Pos);
    assert_eq!(span119.role, Type2TokenRole::Number2Pos);

    assert!(
        !spans.iter().any(|span| {
            matches!(
                span.role,
                Type2TokenRole::Hintmask | Type2TokenRole::Cntrmask | Type2TokenRole::MaskBytes
            ) && span.offset < 121
                && span.offset + span.len > 111
        }),
        "glyph 6 late region unexpectedly intersects a hintmask boundary: {spans:?}"
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_token_start_heads_do_at_least_as_well_as_kind_swapped_leads_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let kind_swapped =
        patch_rebuilt_cff_from_source_kind_swapped_leads(&patched_through_dollar, &source_cff, &[6]);
    let token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);

    let kind_swapped_failure = first_outline_visit_error(&kind_swapped)
        .expect("glyph 6 kind-swapped leads should still leave a later failure");
    let token_heads_failure = first_outline_visit_error(&token_heads)
        .expect("glyph 6 token-start heads should still leave a later failure");

    assert_eq!(kind_swapped_failure.0, 6);
    assert_eq!(kind_swapped_failure.1.as_deref(), Some("percent"));
    assert!(
        kind_swapped_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        kind_swapped_failure.2
    );
    assert_eq!(token_heads_failure.0, 7);
    assert_eq!(token_heads_failure.1.as_deref(), Some("ampersand"));
    assert!(
        token_heads_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        token_heads_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_glyph7_token_start_heads_moves_first_outline_failure_to_quotesingle_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads = patch_rebuilt_cff_from_source_token_start_heads(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[7],
    );
    let patched_through_glyph7_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph6_token_heads, &source_cff, &[7]);

    let token_heads_failure = first_outline_visit_error(&patched_through_glyph7_token_heads)
        .expect("glyph 7 token-start heads should still leave a later failure");
    let full_source_failure = first_outline_visit_error(&patched_through_glyph7_full)
        .expect("glyph 7 full source should still leave a later failure");

    assert_eq!(token_heads_failure.0, 8);
    assert_eq!(token_heads_failure.1.as_deref(), Some("quotesingle"));
    assert!(
        token_heads_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        token_heads_failure.2
    );
    assert_eq!(full_source_failure.0, 8);
    assert_eq!(full_source_failure.1.as_deref(), Some("quotesingle"));
    assert!(
        full_source_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        full_source_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_token_heads_shows_glyph7_window_98_101_is_the_causal_subwindow_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_window57_60 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 57, 60)],
    );
    let patched_through_glyph7_window98_101 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 98, 101)],
    );
    let patched_through_glyph7_window114_115 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 114, 115)],
    );
    let patched_through_glyph7_window132_133 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 132, 133)],
    );
    let patched_through_glyph7_window153_154 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 153, 154)],
    );

    let window57_60_failure = first_outline_visit_error(&patched_through_glyph7_window57_60)
        .expect("glyph 7 [57,60) should still leave a failure");
    let window98_101_failure = first_outline_visit_error(&patched_through_glyph7_window98_101)
        .expect("glyph 7 [98,101) should still leave a later failure");
    let window114_115_failure = first_outline_visit_error(&patched_through_glyph7_window114_115)
        .expect("glyph 7 [114,115) should still leave a failure");
    let window132_133_failure = first_outline_visit_error(&patched_through_glyph7_window132_133)
        .expect("glyph 7 [132,133) should still leave a failure");
    let window153_154_failure = first_outline_visit_error(&patched_through_glyph7_window153_154)
        .expect("glyph 7 [153,154) should still leave a failure");

    assert_eq!(window57_60_failure.0, 7);
    assert_eq!(window57_60_failure.1.as_deref(), Some("ampersand"));
    assert!(
        window57_60_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        window57_60_failure.2
    );
    assert_eq!(window98_101_failure.0, 8);
    assert_eq!(window98_101_failure.1.as_deref(), Some("quotesingle"));
    assert!(
        window98_101_failure
            .2
            .contains("an invalid operator occurred"),
        "{}",
        window98_101_failure.2
    );
    assert_eq!(window114_115_failure.0, 7);
    assert_eq!(window114_115_failure.1.as_deref(), Some("ampersand"));
    assert!(
        window114_115_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        window114_115_failure.2
    );
    assert_eq!(window132_133_failure.0, 7);
    assert_eq!(window132_133_failure.1.as_deref(), Some("ampersand"));
    assert!(
        window132_133_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        window132_133_failure.2
    );
    assert_eq!(window153_154_failure.0, 7);
    assert_eq!(window153_154_failure.1.as_deref(), Some("ampersand"));
    assert!(
        window153_154_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        window153_154_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_token_heads_shows_glyph7_bytes_98_and_100_are_jointly_required_heads_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_byte98 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 98, 99)],
    );
    let patched_through_glyph7_byte100 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 100, 101)],
    );
    let patched_through_glyph7_bytes98_100 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 98, 99), (7, 100, 101)],
    );

    let byte98_failure = first_outline_visit_error(&patched_through_glyph7_byte98)
        .expect("glyph 7 byte98-only patch should still leave a failure");
    let byte100_failure = first_outline_visit_error(&patched_through_glyph7_byte100)
        .expect("glyph 7 byte100-only patch should still leave a failure");
    let both_failure = first_outline_visit_error(&patched_through_glyph7_bytes98_100)
        .expect("glyph 7 bytes98+100 patch should still leave a later failure");

    assert_eq!(byte98_failure.0, 7);
    assert_eq!(byte98_failure.1.as_deref(), Some("ampersand"));
    assert!(
        byte98_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        byte98_failure.2
    );
    assert_eq!(byte100_failure.0, 7);
    assert_eq!(byte100_failure.1.as_deref(), Some("ampersand"));
    assert!(
        byte100_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        byte100_failure.2
    );
    assert_eq!(both_failure.0, 8);
    assert_eq!(both_failure.1.as_deref(), Some("quotesingle"));
    assert!(
        both_failure.2.contains("an invalid operator occurred"),
        "{}",
        both_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_glyph7_token_heads_leave_only_residual_bytes_99_and_132_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph6_token_heads, &source_cff, &[7]);
    let patched_cff = sfnt_table_bytes(&patched_through_glyph7_token_heads, TAG_CFF);
    let source_glyph7 = cff_charstring_bytes(&source_cff, 7);
    let patched_glyph7 = cff_charstring_bytes(&patched_cff, 7);

    let diff_offsets = source_glyph7
        .iter()
        .zip(&patched_glyph7)
        .enumerate()
        .filter_map(|(index, (source, patched))| if source != patched { Some(index) } else { None })
        .collect::<Vec<_>>();
    let spans = scan_type2_token_spans(&source_glyph7);

    assert_eq!(diff_offsets, vec![99, 132]);
    assert_eq!(
        spans
            .iter()
            .find(|span| span.offset == 98)
            .expect("glyph 7 should have a token at 98")
            .role,
        Type2TokenRole::Number2Pos
    );
    assert_eq!(
        spans
            .iter()
            .find(|span| span.offset == 100)
            .expect("glyph 7 should have a token at 100")
            .role,
        Type2TokenRole::Number2Pos
    );
    assert_eq!(
        spans
            .iter()
            .find(|span| span.offset == 102)
            .expect("glyph 7 should have a token at 102")
            .operator,
        Some("rmoveto")
    );
    assert_eq!(
        spans
            .iter()
            .find(|span| span.offset == 131)
            .expect("glyph 7 should have a token at 131")
            .role,
        Type2TokenRole::Hintmask
    );
    assert_eq!(
        spans
            .iter()
            .find(|span| span.offset == 132)
            .expect("glyph 7 should have a mask byte at 132")
            .role,
        Type2TokenRole::MaskBytes
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_glyph7_glyph8_token_start_heads_moves_first_outline_failure_to_parenleft_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph6_token_heads, &source_cff, &[7]);
    let patched_through_glyph8_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph7_token_heads, &source_cff, &[8]);
    let patched_through_glyph8_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph7_token_heads, &source_cff, &[8]);

    let token_heads_failure = first_outline_visit_error(&patched_through_glyph8_token_heads)
        .expect("glyph 8 token-start heads should still leave a later failure");
    let full_source_failure = first_outline_visit_error(&patched_through_glyph8_full)
        .expect("glyph 8 full source should still leave a later failure");

    assert_eq!(token_heads_failure.0, 9);
    assert_eq!(token_heads_failure.1.as_deref(), Some("parenleft"));
    assert!(
        token_heads_failure.2.contains("missing moveto operator"),
        "{}",
        token_heads_failure.2
    );
    assert_eq!(full_source_failure.0, 9);
    assert_eq!(full_source_failure.1.as_deref(), Some("parenleft"));
    assert!(
        full_source_failure.2.contains("missing moveto operator"),
        "{}",
        full_source_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_notdef_through_dollar_and_glyph6_glyph7_glyph8_glyph9_token_start_heads_moves_first_outline_failure_to_parenright_if_present(
) {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph6_token_heads, &source_cff, &[7]);
    let patched_through_glyph8_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph7_token_heads, &source_cff, &[8]);
    let patched_through_glyph9_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph8_token_heads, &source_cff, &[9]);
    let patched_through_glyph9_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph8_token_heads, &source_cff, &[9]);

    let token_heads_failure = first_outline_visit_error(&patched_through_glyph9_token_heads)
        .expect("glyph 9 token-start heads should still leave a later failure");
    let full_source_failure = first_outline_visit_error(&patched_through_glyph9_full)
        .expect("glyph 9 full source should still leave a later failure");

    assert_eq!(token_heads_failure.0, 10);
    assert_eq!(token_heads_failure.1.as_deref(), Some("parenright"));
    assert!(
        token_heads_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        token_heads_failure.2
    );
    assert_eq!(full_source_failure.0, 10);
    assert_eq!(full_source_failure.1.as_deref(), Some("parenright"));
    assert!(
        full_source_failure
            .2
            .contains("an invalid amount of items are in an arguments stack"),
        "{}",
        full_source_failure.2
    );
}

#[test]
fn local_regular_source_backfill_of_charstrings_data_unblocks_convert_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let charstrings_offset = top_dict_operand_u32(&source_cff, &[17]) as usize;
    let charstrings_data_start = read_index_header(&source_cff, charstrings_offset).data_start;
    let fdarray_offset = top_dict_operand_u32(&source_cff, &[12, 36]) as usize;
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[(charstrings_data_start, fdarray_offset)],
    );
    convert_otf_to_ttf(&patched, &[])
        .expect("source-backed CharStrings payload should now convert");
}

#[test]
fn local_regular_current_rebuilt_dieresis_fdselect_matches_source_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let rebuilt_cff = sfnt_table_bytes(&rebuilt_sfnt, TAG_CFF);
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    assert_eq!(fdselect_format3_fd_for_glyph(&rebuilt_cff, 0), 5);
    assert_eq!(fdselect_format3_fd_for_glyph(&source_cff, 0), 5);
    assert_eq!(fdselect_format3_fd_for_glyph(&rebuilt_cff, 103), 14);
    assert_eq!(fdselect_format3_fd_for_glyph(&source_cff, 103), 14);
}

#[test]
fn local_regular_charstrings_backfill_no_longer_has_outline_failure_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let charstrings_offset = top_dict_operand_u32(&source_cff, &[17]) as usize;
    let charstrings_data_start = read_index_header(&source_cff, charstrings_offset).data_start;
    let fdarray_offset = top_dict_operand_u32(&source_cff, &[12, 36]) as usize;
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[(charstrings_data_start, fdarray_offset)],
    );
    assert!(
        first_outline_visit_error(&patched).is_none(),
        "charstrings-backfilled font should no longer expose an outline failure"
    );
}

#[test]
fn local_regular_source_backfill_of_charstrings_data_and_fdselect_unblocks_convert_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let charstrings_offset = top_dict_operand_u32(&source_cff, &[17]) as usize;
    let charstrings_data_start = read_index_header(&source_cff, charstrings_offset).data_start;
    let fdarray_offset = top_dict_operand_u32(&source_cff, &[12, 36]) as usize;
    let fdselect_range = fdselect_format3_range(&source_cff);
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[(charstrings_data_start, fdarray_offset), fdselect_range],
    );

    convert_otf_to_ttf(&patched, &[])
        .expect("source-backed CharStrings data plus FDSelect should convert");
}

#[test]
fn local_bold_source_backfill_of_charstrings_data_unblocks_convert_if_present() {
    let Some(source_path) =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Bold.otf")
    else {
        return;
    };

    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("bold fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local bold source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let charstrings_offset = top_dict_operand_u32(&source_cff, &[17]) as usize;
    let charstrings_data_start = read_index_header(&source_cff, charstrings_offset).data_start;
    let fdarray_offset = top_dict_operand_u32(&source_cff, &[12, 36]) as usize;
    let patched = patch_rebuilt_cff_from_source_ranges(
        &rebuilt_sfnt,
        &source_cff,
        &[(charstrings_data_start, fdarray_offset)],
    );

    convert_otf_to_ttf(&patched, &[])
        .expect("source-backed bold CharStrings payload should convert");
}

#[test]
#[ignore = "diagnostic helper for glyph-level first-outline-failure probes"]
fn debug_print_first_outline_failure_after_glyph_backfills() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph3_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 3));
    let glyph3_len = cff_charstring_bytes(&source_cff, 3).len();
    let glyph4_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 4));
    let glyph4_len = cff_charstring_bytes(&source_cff, 4).len();
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));
    let glyph5_len = cff_charstring_bytes(&source_cff, 5).len();
    let patched_notdef = patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0]);
    let patched_notdef_and_glyph3_prefix = patch_rebuilt_cff_from_source_glyph_prefixes(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, glyph3_prefix_len),
        ],
    );
    let patched_notdef_and_glyph3_suffix = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, glyph3_prefix_len, glyph3_len),
        ],
    );
    let patched_notdef_and_quotedbl =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3]);
    let patched_notdef_quotedbl_and_glyph4_prefix = patch_rebuilt_cff_from_source_glyph_prefixes(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, cff_charstring_bytes(&source_cff, 3).len()),
            (4, glyph4_prefix_len),
        ],
    );
    let patched_notdef_quotedbl_and_glyph4_suffix = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, glyph4_prefix_len, glyph4_len),
        ],
    );
    let patched_notdef_quotedbl_and_numbersign =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4]);
    let patched_notdef_quotedbl_numbersign_and_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_notdef_quotedbl_numbersign_and_glyph5_prefix =
        patch_rebuilt_cff_from_source_glyph_prefixes(
            &rebuilt_sfnt,
            &source_cff,
            &[
                (0, cff_charstring_bytes(&source_cff, 0).len()),
                (3, cff_charstring_bytes(&source_cff, 3).len()),
                (4, cff_charstring_bytes(&source_cff, 4).len()),
                (5, glyph5_prefix_len),
            ],
        );
    let patched_notdef_quotedbl_numbersign_and_glyph5_suffix =
        patch_rebuilt_cff_from_source_glyph_slices(
            &rebuilt_sfnt,
            &source_cff,
            &[
                (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
                (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
                (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
                (5, glyph5_prefix_len, glyph5_len),
            ],
        );
    let patched_notdef_and_29 =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 29]);

    println!(
        "after glyph 0 backfill: {:?}",
        first_outline_visit_error(&patched_notdef)
    );
    println!(
        "glyph 3 prefix len through first draw: {:?}",
        glyph3_prefix_len
    );
    println!(
        "after glyph 0 + glyph 3 prefix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_and_glyph3_prefix)
    );
    println!(
        "after glyph 0 + glyph 3 suffix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_and_glyph3_suffix)
    );
    println!(
        "after glyph 0+3 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_and_quotedbl)
    );
    println!(
        "glyph 4 prefix len through first draw: {:?}",
        glyph4_prefix_len
    );
    println!(
        "after glyph 0+3 + glyph 4 prefix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_and_glyph4_prefix)
    );
    println!(
        "after glyph 0+3 + glyph 4 suffix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_and_glyph4_suffix)
    );
    println!(
        "after glyph 0+3+4 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_and_numbersign)
    );
    println!(
        "glyph 5 prefix len through first draw: {:?}",
        glyph5_prefix_len
    );
    println!(
        "after glyph 0+3+4 + glyph 5 prefix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_numbersign_and_glyph5_prefix)
    );
    println!(
        "after glyph 0+3+4 + glyph 5 suffix backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_numbersign_and_glyph5_suffix)
    );
    println!(
        "after glyph 0+3+4+5 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_quotedbl_numbersign_and_dollar)
    );
    println!(
        "after glyph 0+29 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_and_29)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 5 slice-matrix probes"]
fn debug_print_glyph5_slice_matrix() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph5_len = cff_charstring_bytes(&source_cff, 5).len();
    let glyph5_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 5));

    // These windows come from the current source-vs-rebuilt byte diffs for glyph 5:
    // one mid-outline cluster around the first vvcurveto arguments, and one later cluster
    // around the second hintmask/vlineto region.
    let glyph5_mid_window = (31usize, 48usize);
    let glyph5_late_window = (70usize, 91usize);
    let glyph5_late_head_window = (70usize, 74usize);
    let glyph5_late_tail_window = (74usize, 91usize);
    let glyph5_late_head_a_window = (70usize, 72usize);
    let glyph5_late_head_b_window = (72usize, 74usize);
    let glyph5_sign_flip_window = (88usize, 90usize);

    let patched_notdef_through_numbersign =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4]);
    let patched_prefix_and_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_mid_window.0, glyph5_mid_window.1),
        ],
    );
    let patched_prefix_and_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_window.0, glyph5_late_window.1),
        ],
    );
    let patched_prefix_mid_and_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_mid_window.0, glyph5_mid_window.1),
            (5, glyph5_late_window.0, glyph5_late_window.1),
        ],
    );
    let patched_full_except_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_mid_window.0),
            (5, glyph5_mid_window.1, glyph5_len),
        ],
    );
    let patched_full_except_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_window.0),
            (5, glyph5_late_window.1, glyph5_len),
        ],
    );
    let patched_full_except_mid_and_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_mid_window.0),
            (5, glyph5_mid_window.1, glyph5_late_window.0),
            (5, glyph5_late_window.1, glyph5_len),
        ],
    );
    let patched_prefix_and_late_head = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_head_window.0, glyph5_late_head_window.1),
        ],
    );
    let patched_prefix_and_late_tail = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_tail_window.0, glyph5_late_tail_window.1),
        ],
    );
    let patched_prefix_and_sign_flip = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_sign_flip_window.0, glyph5_sign_flip_window.1),
        ],
    );
    let patched_full_except_late_head = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_head_window.0),
            (5, glyph5_late_head_window.1, glyph5_len),
        ],
    );
    let patched_full_except_late_tail = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_tail_window.0),
            (5, glyph5_late_tail_window.1, glyph5_len),
        ],
    );
    let patched_prefix_and_late_head_a = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_head_a_window.0, glyph5_late_head_a_window.1),
        ],
    );
    let patched_prefix_and_late_head_b = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_prefix_len),
            (5, glyph5_late_head_b_window.0, glyph5_late_head_b_window.1),
        ],
    );
    let patched_full_except_late_head_a = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_head_a_window.0),
            (5, glyph5_late_head_a_window.1, glyph5_len),
        ],
    );
    let patched_full_except_late_head_b = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, glyph5_late_head_b_window.0),
            (5, glyph5_late_head_b_window.1, glyph5_len),
        ],
    );

    println!(
        "baseline after glyph 0+3+4 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_through_numbersign)
    );
    println!(
        "glyph 5 prefix+mid windows: {:?}",
        first_outline_visit_error(&patched_prefix_and_mid)
    );
    println!(
        "glyph 5 prefix+late windows: {:?}",
        first_outline_visit_error(&patched_prefix_and_late)
    );
    println!(
        "glyph 5 prefix+mid+late windows: {:?}",
        first_outline_visit_error(&patched_prefix_mid_and_late)
    );
    println!(
        "glyph 5 full except mid window: {:?}",
        first_outline_visit_error(&patched_full_except_mid)
    );
    println!(
        "glyph 5 full except late window: {:?}",
        first_outline_visit_error(&patched_full_except_late)
    );
    println!(
        "glyph 5 full except mid+late windows: {:?}",
        first_outline_visit_error(&patched_full_except_mid_and_late)
    );
    println!(
        "glyph 5 prefix+late-head window: {:?}",
        first_outline_visit_error(&patched_prefix_and_late_head)
    );
    println!(
        "glyph 5 prefix+late-tail window: {:?}",
        first_outline_visit_error(&patched_prefix_and_late_tail)
    );
    println!(
        "glyph 5 prefix+sign-flip window: {:?}",
        first_outline_visit_error(&patched_prefix_and_sign_flip)
    );
    println!(
        "glyph 5 full except late-head window: {:?}",
        first_outline_visit_error(&patched_full_except_late_head)
    );
    println!(
        "glyph 5 full except late-tail window: {:?}",
        first_outline_visit_error(&patched_full_except_late_tail)
    );
    println!(
        "glyph 5 prefix+late-head-a window: {:?}",
        first_outline_visit_error(&patched_prefix_and_late_head_a)
    );
    println!(
        "glyph 5 prefix+late-head-b window: {:?}",
        first_outline_visit_error(&patched_prefix_and_late_head_b)
    );
    println!(
        "glyph 5 full except late-head-a window: {:?}",
        first_outline_visit_error(&patched_full_except_late_head_a)
    );
    println!(
        "glyph 5 full except late-head-b window: {:?}",
        first_outline_visit_error(&patched_full_except_late_head_b)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 6 prefix-vs-suffix probes"]
fn debug_print_glyph6_prefix_suffix_probe() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_len = cff_charstring_bytes(&source_cff, 6).len();
    let glyph6_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 6));
    let patched_notdef_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_dollar_plus_glyph6_prefix = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_prefix_len),
        ],
    );
    let patched_through_dollar_plus_glyph6_suffix = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, glyph6_prefix_len, glyph6_len),
        ],
    );

    println!(
        "glyph 6 prefix len through first draw: {:?}",
        glyph6_prefix_len
    );
    println!(
        "after glyph 0+3+4+5 backfill: {:?}",
        first_outline_visit_error(&patched_notdef_through_dollar)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 prefix backfill: {:?}",
        first_outline_visit_error(&patched_through_dollar_plus_glyph6_prefix)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 suffix backfill: {:?}",
        first_outline_visit_error(&patched_through_dollar_plus_glyph6_suffix)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 6 coarse slice-matrix probes"]
fn debug_print_glyph6_slice_matrix() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_len = cff_charstring_bytes(&source_cff, 6).len();
    let glyph6_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 6));
    let glyph6_mid_window = (30usize, 90usize);
    let glyph6_late_window = (90usize, glyph6_len);
    let patched_prefix_and_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_prefix_len),
            (6, glyph6_mid_window.0, glyph6_mid_window.1),
        ],
    );
    let patched_prefix_and_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_prefix_len),
            (6, glyph6_late_window.0, glyph6_late_window.1),
        ],
    );
    let patched_full_except_mid = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_mid_window.0),
            (6, glyph6_mid_window.1, glyph6_len),
        ],
    );
    let patched_full_except_late = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &[
            (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
            (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
            (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
            (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
            (6, 0, glyph6_late_window.0),
            (6, glyph6_late_window.1, glyph6_len),
        ],
    );

    println!(
        "glyph 6 prefix len through first draw: {:?}",
        glyph6_prefix_len
    );
    println!(
        "glyph 6 prefix+mid window: {:?}",
        first_outline_visit_error(&patched_prefix_and_mid)
    );
    println!(
        "glyph 6 prefix+late window: {:?}",
        first_outline_visit_error(&patched_prefix_and_late)
    );
    println!(
        "glyph 6 full except mid window: {:?}",
        first_outline_visit_error(&patched_full_except_mid)
    );
    println!(
        "glyph 6 full except late window: {:?}",
        first_outline_visit_error(&patched_full_except_late)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 6 prefix hot-byte probes"]
fn debug_print_glyph6_prefix_hot_bytes() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_prefix_len = first_type2_draw_byte_offset(&cff_charstring_bytes(&source_cff, 6));
    let base = vec![
        (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
        (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
        (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
        (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
    ];
    let mut prefix_only = base.clone();
    prefix_only.push((6, 0, glyph6_prefix_len));
    let mut byte25_only = base.clone();
    byte25_only.push((6, 25, 26));
    let mut byte27_only = base.clone();
    byte27_only.push((6, 27, 28));
    let mut byte25_27 = base.clone();
    byte25_27.push((6, 25, 26));
    byte25_27.push((6, 27, 28));

    let patched_prefix_only =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &prefix_only);
    let patched_byte25_only =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &byte25_only);
    let patched_byte27_only =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &byte27_only);
    let patched_byte25_27 =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &byte25_27);

    println!(
        "after glyph 0+3+4+5 backfill: {:?}",
        first_outline_visit_error(&patch_rebuilt_cff_from_source_glyphs(
            &rebuilt_sfnt,
            &source_cff,
            &[0, 3, 4, 5],
        ))
    );
    println!(
        "after glyph 6 prefix-only patch: {:?}",
        first_outline_visit_error(&patched_prefix_only)
    );
    println!(
        "after glyph 6 byte25-only patch: {:?}",
        first_outline_visit_error(&patched_byte25_only)
    );
    println!(
        "after glyph 6 byte27-only patch: {:?}",
        first_outline_visit_error(&patched_byte27_only)
    );
    println!(
        "after glyph 6 byte25+27 patch: {:?}",
        first_outline_visit_error(&patched_byte25_27)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 6 late-byte probes"]
fn debug_print_glyph6_late_hot_bytes() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);
    let glyph6_len = cff_charstring_bytes(&source_cff, 6).len();

    let base = vec![
        (0, 0, cff_charstring_bytes(&source_cff, 0).len()),
        (3, 0, cff_charstring_bytes(&source_cff, 3).len()),
        (4, 0, cff_charstring_bytes(&source_cff, 4).len()),
        (5, 0, cff_charstring_bytes(&source_cff, 5).len()),
    ];

    let mut glyph6_25_27 = base.clone();
    glyph6_25_27.push((6, 25, 26));
    glyph6_25_27.push((6, 27, 28));

    let mut glyph6_25_27_114 = glyph6_25_27.clone();
    glyph6_25_27_114.push((6, 114, 115));
    let mut glyph6_25_27_119 = glyph6_25_27.clone();
    glyph6_25_27_119.push((6, 119, 120));
    let mut glyph6_25_27_114_119 = glyph6_25_27.clone();
    glyph6_25_27_114_119.push((6, 114, 115));
    glyph6_25_27_114_119.push((6, 119, 120));

    let mut glyph6_full_except_114 = base.clone();
    glyph6_full_except_114.push((6, 0, 114));
    glyph6_full_except_114.push((6, 115, glyph6_len));
    let mut glyph6_full_except_119 = base.clone();
    glyph6_full_except_119.push((6, 0, 119));
    glyph6_full_except_119.push((6, 120, glyph6_len));
    let mut glyph6_full_except_118_121 = base.clone();
    glyph6_full_except_118_121.push((6, 0, 118));
    glyph6_full_except_118_121.push((6, 121, glyph6_len));

    let patched_base = patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &base);
    let patched_25_27 =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &glyph6_25_27);
    let patched_25_27_114 =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &glyph6_25_27_114);
    let patched_25_27_119 =
        patch_rebuilt_cff_from_source_glyph_slices(&rebuilt_sfnt, &source_cff, &glyph6_25_27_119);
    let patched_25_27_114_119 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &glyph6_25_27_114_119,
    );
    let patched_full_except_114 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &glyph6_full_except_114,
    );
    let patched_full_except_119 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &glyph6_full_except_119,
    );
    let patched_full_except_118_121 = patch_rebuilt_cff_from_source_glyph_slices(
        &rebuilt_sfnt,
        &source_cff,
        &glyph6_full_except_118_121,
    );

    println!(
        "after glyph 0+3+4+5 backfill: {:?}",
        first_outline_visit_error(&patched_base)
    );
    println!(
        "after glyph 6 bytes25+27 patch: {:?}",
        first_outline_visit_error(&patched_25_27)
    );
    println!(
        "after glyph 6 bytes25+27+114 patch: {:?}",
        first_outline_visit_error(&patched_25_27_114)
    );
    println!(
        "after glyph 6 bytes25+27+119 patch: {:?}",
        first_outline_visit_error(&patched_25_27_119)
    );
    println!(
        "after glyph 6 bytes25+27+114+119 patch: {:?}",
        first_outline_visit_error(&patched_25_27_114_119)
    );
    println!(
        "after glyph 6 full except byte114: {:?}",
        first_outline_visit_error(&patched_full_except_114)
    );
    println!(
        "after glyph 6 full except byte119: {:?}",
        first_outline_visit_error(&patched_full_except_119)
    );
    println!(
        "after glyph 6 full except bytes118..121: {:?}",
        first_outline_visit_error(&patched_full_except_118_121)
    );
}

#[test]
#[ignore = "diagnostic helper for kind-swapped lead-byte patch progression"]
fn debug_print_kind_swapped_lead_patch_progression() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    let patched_through_numbersign =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4]);
    let patched_through_numbersign_plus_glyph5_leads =
        patch_rebuilt_cff_from_source_kind_swapped_leads(&patched_through_numbersign, &source_cff, &[5]);
    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_dollar_plus_glyph6_leads =
        patch_rebuilt_cff_from_source_kind_swapped_leads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_dollar_plus_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_numbersign_plus_glyph5_glyph6_leads =
        patch_rebuilt_cff_from_source_kind_swapped_leads(
            &patched_through_numbersign_plus_glyph5_leads,
            &source_cff,
            &[6],
        );

    println!(
        "after glyph 0+3+4 backfill: {:?}",
        first_outline_visit_error(&patched_through_numbersign)
    );
    println!(
        "after glyph 0+3+4 + glyph 5 kind-swapped leads: {:?}",
        first_outline_visit_error(&patched_through_numbersign_plus_glyph5_leads)
    );
    println!(
        "after glyph 0+3+4+5 backfill: {:?}",
        first_outline_visit_error(&patched_through_dollar)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 kind-swapped leads: {:?}",
        first_outline_visit_error(&patched_through_dollar_plus_glyph6_leads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_dollar_plus_glyph6_token_heads)
    );
    println!(
        "after glyph 0+3+4 + glyph 5 kind-swapped leads + glyph 6 kind-swapped leads: {:?}",
        first_outline_visit_error(&patched_through_numbersign_plus_glyph5_glyph6_leads)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 7 token-head and slice progression"]
fn debug_print_glyph7_token_head_progression() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(
            &patched_through_glyph6_token_heads,
            &source_cff,
            &[7],
        );
    let patched_through_glyph7_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph6_token_heads, &source_cff, &[7]);
    let patched_through_glyph7_all_diff_windows = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 57, 60), (7, 98, 101), (7, 114, 115), (7, 132, 133), (7, 153, 154)],
    );
    let patched_through_glyph7_window57_60 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 57, 60)],
    );
    let patched_through_glyph7_window98_101 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 98, 101)],
    );
    let patched_through_glyph7_window114_115 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 114, 115)],
    );
    let patched_through_glyph7_window132_133 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 132, 133)],
    );
    let patched_through_glyph7_window153_154 = patch_rebuilt_cff_from_source_glyph_slices(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[(7, 153, 154)],
    );

    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph6_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph7_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7 full source: {:?}",
        first_outline_visit_error(&patched_through_glyph7_full)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7 all diff windows: {:?}",
        first_outline_visit_error(&patched_through_glyph7_all_diff_windows)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7[57..60): {:?}",
        first_outline_visit_error(&patched_through_glyph7_window57_60)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7[98..101): {:?}",
        first_outline_visit_error(&patched_through_glyph7_window98_101)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7[114..115): {:?}",
        first_outline_visit_error(&patched_through_glyph7_window114_115)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7[132..133): {:?}",
        first_outline_visit_error(&patched_through_glyph7_window132_133)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6 token-start heads + glyph 7[153..154): {:?}",
        first_outline_visit_error(&patched_through_glyph7_window153_154)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 8 token-head progression"]
fn debug_print_glyph8_token_head_progression() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads = patch_rebuilt_cff_from_source_token_start_heads(
        &patched_through_glyph6_token_heads,
        &source_cff,
        &[7],
    );
    let patched_through_glyph8_token_heads = patch_rebuilt_cff_from_source_token_start_heads(
        &patched_through_glyph7_token_heads,
        &source_cff,
        &[8],
    );
    let patched_through_glyph8_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph7_token_heads, &source_cff, &[8]);

    println!(
        "after glyph 0+3+4+5 + glyph 6/7 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph7_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6/7/8 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph8_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6/7 token-start heads + glyph 8 full source: {:?}",
        first_outline_visit_error(&patched_through_glyph8_full)
    );
}

#[test]
#[ignore = "diagnostic helper for glyph 9 token-head progression"]
fn debug_print_glyph9_token_head_progression() {
    let source_path =
        optional_fixture("testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf")
            .expect("local regular source should exist for this diagnostic");
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let rebuilt_sfnt =
        rebuild_office_static_cff_sfnt(&block1).expect("regular fixture should rebuild sfnt");
    let source_bytes = fs::read(source_path).expect("local regular source should be readable");
    let source_cff = sfnt_table_bytes(&source_bytes, TAG_CFF);

    let patched_through_dollar =
        patch_rebuilt_cff_from_source_glyphs(&rebuilt_sfnt, &source_cff, &[0, 3, 4, 5]);
    let patched_through_glyph6_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_dollar, &source_cff, &[6]);
    let patched_through_glyph7_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph6_token_heads, &source_cff, &[7]);
    let patched_through_glyph8_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph7_token_heads, &source_cff, &[8]);
    let patched_through_glyph9_token_heads =
        patch_rebuilt_cff_from_source_token_start_heads(&patched_through_glyph8_token_heads, &source_cff, &[9]);
    let patched_through_glyph9_full =
        patch_rebuilt_cff_from_source_glyphs(&patched_through_glyph8_token_heads, &source_cff, &[9]);

    println!(
        "after glyph 0+3+4+5 + glyph 6/7/8 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph8_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6/7/8/9 token-start heads: {:?}",
        first_outline_visit_error(&patched_through_glyph9_token_heads)
    );
    println!(
        "after glyph 0+3+4+5 + glyph 6/7/8 token-start heads + glyph 9 full source: {:?}",
        first_outline_visit_error(&patched_through_glyph9_full)
    );
}

#[test]
#[ignore = "diagnostic helper for offline charstring analysis"]
fn debug_dump_rebuilt_cff_tables() {
    let out_dir = PathBuf::from("/tmp/eot-tool-charstrings");
    fs::create_dir_all(&out_dir).expect("diagnostic output dir should be creatable");

    let regular_block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let regular_sfnt =
        rebuild_office_static_cff_sfnt(&regular_block1).expect("regular fixture should rebuild");
    fs::write(
        out_dir.join("regular-rebuilt-cff.bin"),
        sfnt_table_bytes(&regular_sfnt, TAG_CFF),
    )
    .expect("regular diagnostic cff should be writable");

    let bold_block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let bold_sfnt =
        rebuild_office_static_cff_sfnt(&bold_block1).expect("bold fixture should rebuild");
    fs::write(
        out_dir.join("bold-rebuilt-cff.bin"),
        sfnt_table_bytes(&bold_sfnt, TAG_CFF),
    )
    .expect("bold diagnostic cff should be writable");
}
