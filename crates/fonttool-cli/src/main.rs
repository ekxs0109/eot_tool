mod embedded_output;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use embedded_output::{
    build_embedded_output, embedded_output_allowed, EmbeddedEotVersion, EmbeddedMtxExtraBlocks,
    EmbeddedOutputOptions, EmbeddedPayloadFormat, EmbeddedXorMode,
};
use fonttool_cff::{
    inspect_otf_font, instantiate_variable_cff2, parse_variation_axes, serialize_subset_otf,
    subset_static_cff, subset_variable_cff2, CffError,
};
use fonttool_eot::parse_eot_header;
use fonttool_glyf::{decode_glyf, encode_glyf};
use fonttool_harfbuzz::subset_font_bytes;
use fonttool_mtx::{decompress_lz, parse_mtx_container};
use fonttool_sfnt::{load_sfnt, parse_sfnt, serialize_sfnt, OwnedSfntFont, SFNT_VERSION_TRUETYPE};
use fonttool_subset::{
    apply_output_table_policy, plan_glyph_subset, should_copy_encode_block1_table, GlyphIdRequest,
    SubsetWarnings,
};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
fn main() -> ExitCode {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        None | Some("-h") | Some("--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("decode") => match (args.next(), args.next(), args.next()) {
            (Some(input), Some(output), None) => match decode_file(&input, &output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("fonttool: {error}");
                    ExitCode::from(1)
                }
            },
            _ => {
                eprintln!("fonttool: decode expects INPUT and OUTPUT paths");
                ExitCode::from(2)
            }
        },
        Some("encode") => match handle_encode_args(args.collect()) {
            Ok(request) => match encode_file(
                &request.input_path,
                &request.output_path,
                request.embedded_output,
                request.variation_axes.as_deref(),
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("fonttool: {error}");
                    ExitCode::from(1)
                }
            },
            Err(error) => {
                eprintln!("fonttool: {error}");
                ExitCode::from(2)
            }
        },
        Some("subset") => match handle_subset_args(args.collect()) {
            Ok(request) => match subset_file(request) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("fonttool: {error}");
                    ExitCode::from(1)
                }
            },
            Err(error) => {
                eprintln!("fonttool: {error}");
                ExitCode::from(2)
            }
        },
        Some(command) => {
            eprintln!("fonttool: unknown command `{command}`");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EncodeCliRequest {
    input_path: PathBuf,
    output_path: PathBuf,
    variation_axes: Option<String>,
    embedded_output: EmbeddedOutputOptions,
}

fn parse_payload_format(value: &str) -> Result<EmbeddedPayloadFormat, String> {
    match value {
        "mtx" => Ok(EmbeddedPayloadFormat::Mtx),
        "sfnt" => Ok(EmbeddedPayloadFormat::Sfnt),
        _ => Err(format!("invalid value `{value}` for `--payload-format`")),
    }
}

fn parse_xor_mode(value: &str) -> Result<EmbeddedXorMode, String> {
    match value {
        "off" => Ok(EmbeddedXorMode::Off),
        "on" => Ok(EmbeddedXorMode::On),
        _ => Err(format!("invalid value `{value}` for `--xor`")),
    }
}

fn parse_eot_version(value: &str) -> Result<EmbeddedEotVersion, String> {
    match value {
        "v1" => Ok(EmbeddedEotVersion::V1),
        "v2" => Ok(EmbeddedEotVersion::V2),
        _ => Err(format!("invalid value `{value}` for `--eot-version`")),
    }
}

fn parse_embedded_output_args(
    args: &[String],
    start_index: usize,
    output_path: &Path,
) -> Result<(EmbeddedOutputOptions, bool), String> {
    let mut options = EmbeddedOutputOptions::default();
    let mut saw_embedded_output_flag = false;
    let mut index = start_index;

    while index < args.len() {
        let flag = &args[index];
        if index + 1 >= args.len() {
            return Err("embedded output flag is missing a value".to_string());
        }
        saw_embedded_output_flag = true;

        match flag.as_str() {
            "--payload-format" => {
                options.payload_format = parse_payload_format(&args[index + 1])?;
            }
            "--xor" => {
                options.xor_mode = parse_xor_mode(&args[index + 1])?;
            }
            "--eot-version" => {
                options.eot_version = parse_eot_version(&args[index + 1])?;
            }
            _ => return Err(format!("unsupported embedded output flag `{flag}`")),
        }

        index += 2;
    }

    if saw_embedded_output_flag && !embedded_output_allowed(output_path) {
        return Err("embedded output options only apply to .eot or .fntdata output".to_string());
    }

    Ok((options, saw_embedded_output_flag))
}

fn handle_encode_args(args: Vec<String>) -> Result<EncodeCliRequest, String> {
    if args.len() < 2 {
        return Err("encode expects INPUT and OUTPUT paths".to_string());
    }

    let mut request = EncodeCliRequest {
        input_path: args[0].clone().into(),
        output_path: args[1].clone().into(),
        variation_axes: None,
        embedded_output: EmbeddedOutputOptions::default(),
    };
    let mut embedded_output_args = Vec::new();
    let mut index = 2usize;

    while index < args.len() {
        let flag = &args[index];
        if index + 1 >= args.len() {
            return Err("encode flag is missing a value".to_string());
        }

        match flag.as_str() {
            "--variation" => {
                request.variation_axes = Some(args[index + 1].clone());
            }
            "--payload-format" | "--xor" | "--eot-version" => {
                embedded_output_args.push(flag.clone());
                embedded_output_args.push(args[index + 1].clone());
            }
            _ => return Err(format!("unsupported encode flag `{flag}`")),
        }

        index += 2;
    }

    if !embedded_output_args.is_empty() {
        let (embedded_output, _) =
            parse_embedded_output_args(&embedded_output_args, 0, &request.output_path)?;
        request.embedded_output = embedded_output;
    }

    Ok(request)
}

fn print_help() {
    println!("fonttool");
    println!();
    println!("Usage: fonttool <COMMAND>");
    println!();
    println!("Commands:");
    println!("  encode <INPUT> <OUTPUT>  Encode a font into EOT/MTX");
    println!("  decode <INPUT> <OUTPUT>  Decode an EOT/MTX font payload into SFNT");
    println!("  subset <INPUT> <OUTPUT> ...  Subset supported non-OTF glyph-id inputs in Rust");
    println!();
    println!("Options:");
    println!("  -h, --help  Print help");
}

fn decode_file(input_path: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Result<(), String> {
    let input_bytes = fs::read(input_path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", input_path.as_ref().display()))?;
    let sfnt_bytes = decode_embedded_font_bytes(&input_bytes)?;

    parse_sfnt(&sfnt_bytes).map_err(|error| format!("decoded SFNT is invalid: {error}"))?;
    fs::write(output_path.as_ref(), &sfnt_bytes).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            output_path.as_ref().display()
        )
    })?;

    Ok(())
}

struct PreparedEmbeddedPayload {
    payload_bytes: Vec<u8>,
}

const TAG_BASE: u32 = u32::from_be_bytes(*b"BASE");
const TAG_DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const TAG_GSUB: u32 = u32::from_be_bytes(*b"GSUB");
const TAG_HVAR: u32 = u32::from_be_bytes(*b"HVAR");
const TAG_STAT: u32 = u32::from_be_bytes(*b"STAT");
const TAG_VORG: u32 = u32::from_be_bytes(*b"VORG");
const TAG_AVAR: u32 = u32::from_be_bytes(*b"avar");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");
const TAG_POST: u32 = u32::from_be_bytes(*b"post");
const TAG_VHEA: u32 = u32::from_be_bytes(*b"vhea");
const TAG_VMTX: u32 = u32::from_be_bytes(*b"vmtx");
const OFFICE_CFF2_VALID_PREFIX_TAGS: [u32; 13] = [
    TAG_BASE, TAG_CFF2, TAG_DSIG, TAG_GSUB, TAG_HVAR, TAG_OS_2, TAG_STAT, TAG_VORG, TAG_AVAR,
    TAG_CMAP, TAG_FVAR, TAG_HEAD, TAG_HHEA,
];
const OFFICE_CFF2_TRUNCATED_SUFFIX_TAGS: [u32; 6] =
    [TAG_HMTX, TAG_MAXP, TAG_NAME, TAG_POST, TAG_VHEA, TAG_VMTX];

#[derive(Clone, Copy)]
struct SfntRecord {
    tag: u32,
    offset: usize,
    length: usize,
}

fn read_u16_be(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn parse_sfnt_directory_record(bytes: &[u8], index: usize) -> Option<SfntRecord> {
    let record_offset = 12 + index * 16;
    Some(SfntRecord {
        tag: read_u32_be(bytes, record_offset)?,
        offset: usize::try_from(read_u32_be(bytes, record_offset + 8)?).ok()?,
        length: usize::try_from(read_u32_be(bytes, record_offset + 12)?).ok()?,
    })
}

fn salvage_office_prefixed_cff2_sfnt(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.starts_with(b"OTTO") {
        return None;
    }

    let num_tables = usize::from(read_u16_be(bytes, 4)?);
    if num_tables != OFFICE_CFF2_VALID_PREFIX_TAGS.len() + OFFICE_CFF2_TRUNCATED_SUFFIX_TAGS.len() {
        return None;
    }
    let directory_len = 12usize.checked_add(num_tables.checked_mul(16)?)?;
    if bytes.len() < directory_len {
        return None;
    }

    let mut seen_tags = Vec::with_capacity(num_tables);
    let mut kept_records = Vec::new();
    let mut dropped_tags = Vec::new();
    let mut seen_invalid = false;
    let mut last_valid_end = 0usize;
    let mut first_invalid_offset = None;

    for index in 0..num_tables {
        let record = parse_sfnt_directory_record(bytes, index)?;
        if record.length == 0 || seen_tags.contains(&record.tag) {
            return None;
        }
        seen_tags.push(record.tag);

        let end = record.offset.checked_add(record.length)?;
        let is_valid = record.offset >= directory_len && end <= bytes.len();

        if is_valid {
            if seen_invalid || record.offset < last_valid_end {
                return None;
            }
            last_valid_end = end;
            kept_records.push(record);
        } else {
            if record.offset < directory_len {
                return None;
            }
            if first_invalid_offset.is_none() {
                first_invalid_offset = Some(record.offset);
                if record.offset != last_valid_end {
                    return None;
                }
            }
            if record.offset < first_invalid_offset? || end <= bytes.len() {
                return None;
            }
            seen_invalid = true;
            dropped_tags.push(record.tag);
        }
    }

    if kept_records.len() != OFFICE_CFF2_VALID_PREFIX_TAGS.len()
        || dropped_tags.len() != OFFICE_CFF2_TRUNCATED_SUFFIX_TAGS.len()
    {
        return None;
    }

    let kept_tags: Vec<u32> = kept_records.iter().map(|record| record.tag).collect();
    if kept_tags != OFFICE_CFF2_VALID_PREFIX_TAGS
        || dropped_tags != OFFICE_CFF2_TRUNCATED_SUFFIX_TAGS
    {
        return None;
    }

    let mut font = OwnedSfntFont::new(u32::from_be_bytes(*b"OTTO"));
    for record in kept_records {
        let table_end = record.offset.checked_add(record.length)?;
        font.add_table(record.tag, bytes[record.offset..table_end].to_vec());
    }

    let serialized = serialize_sfnt(&font).ok()?;
    let kind = inspect_otf_font(&serialized).ok()?;
    if !kind.is_cff_flavor || !kind.is_variable {
        return None;
    }

    Some(serialized)
}

fn extract_office_prefixed_static_cff_sfnt(bytes: &[u8]) -> Option<Vec<u8>> {
    let prefixed_bytes = bytes.get(1..)?;
    if !prefixed_bytes.starts_with(b"OTTO") {
        return None;
    }

    parse_sfnt(prefixed_bytes).ok()?;
    let kind = inspect_otf_font(prefixed_bytes).ok()?;
    if !kind.is_cff_flavor || kind.is_variable {
        return None;
    }

    let font = load_sfnt(prefixed_bytes).ok()?;
    if font.table(TAG_CFF).is_none()
        || font.table(TAG_CFF2).is_some()
        || font.table(TAG_GLYF).is_some()
    {
        return None;
    }

    Some(prefixed_bytes.to_vec())
}

fn extract_single_byte_prefixed_cff_sfnt(bytes: &[u8]) -> Option<Vec<u8>> {
    let prefixed_bytes = bytes.get(1..)?;
    if !prefixed_bytes.starts_with(b"OTTO") {
        return None;
    }

    parse_sfnt(prefixed_bytes).ok()?;
    let kind = inspect_otf_font(prefixed_bytes).ok()?;
    if !kind.is_cff_flavor {
        return None;
    }

    Some(prefixed_bytes.to_vec())
}

fn extract_cff_sfnt_payload(block1: &[u8], block2: &[u8], block3: &[u8]) -> Option<Vec<u8>> {
    if !block2.is_empty() || !block3.is_empty() {
        return None;
    }

    if let Ok(kind) = inspect_otf_font(block1) {
        if kind.is_cff_flavor {
            return Some(block1.to_vec());
        }
    }

    // The real Office static CFF fixture carries a single-byte prefix in
    // block1 before the embedded OTTO font.
    if let Some(static_cff_payload) = extract_office_prefixed_static_cff_sfnt(block1) {
        return Some(static_cff_payload);
    }

    if let Some(prefixed_cff_payload) = extract_single_byte_prefixed_cff_sfnt(block1) {
        return Some(prefixed_cff_payload);
    }

    salvage_office_prefixed_cff2_sfnt(block1.get(1..)?)
}

fn prepare_embedded_payload(input_bytes: &[u8]) -> Result<PreparedEmbeddedPayload, String> {
    let header =
        parse_eot_header(input_bytes).map_err(|error| format!("invalid EOT header: {error}"))?;
    let payload_start = header.header_length as usize;
    let payload_len = header.font_data_size as usize;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or_else(|| "invalid EOT payload range".to_string())?;
    let payload = input_bytes
        .get(payload_start..payload_end)
        .ok_or_else(|| "invalid EOT payload range".to_string())?;

    let mut payload_bytes = payload.to_vec();
    if header.flags & EOT_FLAG_PPT_XOR != 0 {
        for byte in &mut payload_bytes {
            *byte ^= 0x50;
        }
    }

    Ok(PreparedEmbeddedPayload { payload_bytes })
}

fn decode_embedded_font_bytes(input_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let prepared = prepare_embedded_payload(input_bytes)?;

    if parse_sfnt(&prepared.payload_bytes).is_ok() {
        return Ok(prepared.payload_bytes);
    }

    let container = parse_mtx_container(&prepared.payload_bytes)
        .map_err(|error| format!("invalid MTX container: {error}"))?;
    let block1 = decompress_lz(container.block1)
        .map_err(|error| format!("failed to decode MTX block1: {error}"))?;
    let block2 = match container.block2 {
        Some(block) => {
            decompress_lz(block).map_err(|error| format!("failed to decode MTX block2: {error}"))?
        }
        None => Vec::new(),
    };
    let block3 = match container.block3 {
        Some(block) => {
            decompress_lz(block).map_err(|error| format!("failed to decode MTX block3: {error}"))?
        }
        None => Vec::new(),
    };

    if let Some(cff_payload) = extract_cff_sfnt_payload(&block1, &block2, &block3) {
        return Ok(cff_payload);
    }

    decode_embedded_font_bytes_full(block1, block2, block3)
}

fn decode_embedded_font_bytes_full(
    block1: Vec<u8>,
    block2: Vec<u8>,
    block3: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut font = load_sfnt(&block1).map_err(|error| format!("invalid block1 SFNT: {error}"))?;
    let needs_glyf_reconstruction = font.version_tag() == SFNT_VERSION_TRUETYPE
        && font.table(TAG_GLYF).is_some()
        && font
            .table(TAG_LOCA)
            .is_some_and(|table| table.data.is_empty());

    if !needs_glyf_reconstruction && block2.is_empty() && block3.is_empty() {
        return Ok(block1);
    }
    if !needs_glyf_reconstruction && (block2.is_empty() || block3.is_empty()) {
        return Err(
            "current Rust MTX decode requires both block2 and block3 when extra blocks are present"
                .to_string(),
        );
    }

    let head = table_bytes(&font, TAG_HEAD, "head")?;
    let maxp = table_bytes(&font, TAG_MAXP, "maxp")?;
    if head.len() < 52 {
        return Err("head table is truncated".to_string());
    }
    if maxp.len() < 6 {
        return Err("maxp table is truncated".to_string());
    }

    let index_to_loca_format = i16::from_be_bytes([head[50], head[51]]);
    let num_glyphs = u16::from_be_bytes([maxp[4], maxp[5]]);
    let glyf_stream = table_bytes(&font, TAG_GLYF, "glyf")?;
    let decoded_glyf = decode_glyf(
        glyf_stream,
        &block2,
        &block3,
        index_to_loca_format,
        num_glyphs,
    )
    .map_err(|error| format!("failed to reconstruct glyf/loca tables from MTX blocks: {error}"))?;
    font.remove_table(TAG_GLYF);
    font.remove_table(TAG_LOCA);
    font.add_table(TAG_GLYF, decoded_glyf.glyf_data);
    font.add_table(TAG_LOCA, decoded_glyf.loca_data);
    serialize_sfnt(&font)
        .map_err(|error| format!("failed to serialize reconstructed SFNT: {error}"))
}

fn encode_file(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    output_options: EmbeddedOutputOptions,
    variation_axes: Option<&str>,
) -> Result<(), String> {
    let output_path = output_path.as_ref();
    if output_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("fntdata"))
    {
        return Err(
            "PowerPoint-compatible .fntdata encode remains Phase 2-owned; use the archived native binary for compatibility flows".to_string(),
        );
    }

    let input_bytes = fs::read(input_path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", input_path.as_ref().display()))?;
    let kind = inspect_otf_font(&input_bytes).map_err(|error| error.to_string())?;
    if kind.is_cff_flavor {
        return encode_otf_file(
            &input_bytes,
            output_path,
            output_options,
            variation_axes,
            &kind,
        );
    }
    if variation_axes.is_some() {
        return Err("encode does not support --variation for non-OTF input".to_string());
    }

    let source_font = load_sfnt(&input_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;

    if source_font.version_tag() != SFNT_VERSION_TRUETYPE {
        return Err(
            "encode currently only supports TrueType glyf fonts in the Rust-owned Phase 1 boundary"
                .to_string(),
        );
    }

    let head = table_bytes(&source_font, TAG_HEAD, "head")?;
    let maxp = table_bytes(&source_font, TAG_MAXP, "maxp")?;
    let glyf = table_bytes(&source_font, TAG_GLYF, "glyf")?;
    let loca = table_bytes(&source_font, TAG_LOCA, "loca")?;
    let _hhea = table_bytes(&source_font, TAG_HHEA, "hhea")?;
    let _hmtx = table_bytes(&source_font, TAG_HMTX, "hmtx")?;
    let os2 = table_bytes(&source_font, TAG_OS_2, "OS/2")?;
    let name = source_font
        .table(TAG_NAME)
        .map(|table| table.data.as_slice())
        .unwrap_or(&[]);

    if head.len() < 52 {
        return Err("head table is truncated".to_string());
    }
    if maxp.len() < 6 {
        return Err("maxp table is truncated".to_string());
    }

    let index_to_loca_format = i16::from_be_bytes([head[50], head[51]]);
    let num_glyphs = u16::from_be_bytes([maxp[4], maxp[5]]);
    let encoded_glyf = encode_glyf(glyf, loca, index_to_loca_format, num_glyphs)
        .map_err(|error| format!("failed to encode glyf/loca tables: {error}"))?;
    let block1_font = build_block1_font(&source_font, head, encoded_glyf.glyf_stream)?;
    let block1_sfnt = serialize_sfnt(&block1_font)
        .map_err(|error| format!("failed to serialize encoded SFNT: {error}"))?;
    let sfnt_payload = match output_options.payload_format {
        EmbeddedPayloadFormat::Mtx => block1_sfnt.as_slice(),
        EmbeddedPayloadFormat::Sfnt => input_bytes.as_slice(),
    };
    let encoded_eot = build_embedded_output(
        head,
        os2,
        name,
        sfnt_payload,
        Some(EmbeddedMtxExtraBlocks {
            block2: Some(&encoded_glyf.push_stream),
            block3: Some(&encoded_glyf.code_stream),
        }),
        output_options,
    )?;

    fs::write(output_path, encoded_eot)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;

    Ok(())
}

fn encode_otf_file(
    input_bytes: &[u8],
    output_path: &Path,
    output_options: EmbeddedOutputOptions,
    variation_axes: Option<&str>,
    kind: &fonttool_cff::CffFontKind,
) -> Result<(), String> {
    let otf_bytes = if kind.is_variable {
        let axes = parse_variation_axes(variation_axes.unwrap_or_default())
            .map_err(|error| error.to_string())?;
        instantiate_variable_cff2(input_bytes, &axes).map_err(|error| error.to_string())?
    } else {
        if variation_axes.is_some() {
            return Err(CffError::VariationRejectedForStaticInput.to_string());
        }
        input_bytes.to_vec()
    };

    let otf_font = load_sfnt(&otf_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;
    let head = table_bytes(&otf_font, TAG_HEAD, "head")?;
    let os2 = table_bytes(&otf_font, TAG_OS_2, "OS/2")?;
    let name = otf_font
        .table(TAG_NAME)
        .map(|table| table.data.as_slice())
        .unwrap_or(&[]);
    let encoded_eot = build_embedded_output(head, os2, name, &otf_bytes, None, output_options)?;

    fs::write(output_path, encoded_eot)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;

    Ok(())
}

fn subset_file(request: SubsetCliRequest) -> Result<(), String> {
    let input_bytes = load_subset_input_sfnt_bytes(&request.input_path)?;
    let kind = inspect_otf_font(&input_bytes).map_err(|error| error.to_string())?;
    if kind.is_cff_flavor {
        return subset_otf_file(&request, &input_bytes, &kind);
    }
    if request.variation_axes.is_some() {
        return Err("subset does not support --variation for non-OTF input".to_string());
    }

    let glyph_ids_csv = request.glyph_ids.as_deref().ok_or_else(|| {
        "subset currently only supports --glyph-ids for non-OTF input".to_string()
    })?;
    let glyph_ids = GlyphIdRequest::parse_csv(glyph_ids_csv)
        .map_err(|error| format!("invalid subset arguments: {error}"))?;
    let input_font = load_sfnt(&input_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;
    let plan =
        plan_glyph_subset(&input_font, &glyph_ids, false).map_err(|error| error.to_string())?;
    let mut harfbuzz_input = input_font.clone();
    let mut subset_warnings = SubsetWarnings::default();
    apply_output_table_policy(&mut harfbuzz_input, &mut subset_warnings);
    let harfbuzz_input_bytes = serialize_sfnt(&harfbuzz_input)
        .map_err(|error| format!("failed to serialize subset input: {error}"))?;
    let subset_bytes = subset_font_bytes(&harfbuzz_input_bytes, &plan)
        .map_err(|error| format!("failed to subset font with HarfBuzz: {error}"))?;
    let mut subset_font = load_sfnt(&subset_bytes)
        .map_err(|error| format!("invalid HarfBuzz subset SFNT: {error}"))?;
    apply_output_table_policy(&mut subset_font, &mut subset_warnings);
    let subset_bytes = serialize_sfnt(&subset_font)
        .map_err(|error| format!("failed to serialize subset: {error}"))?;
    let head = table_bytes(&subset_font, TAG_HEAD, "head")?;
    let os2 = table_bytes(&subset_font, TAG_OS_2, "OS/2")?;
    let name = subset_font
        .table(TAG_NAME)
        .map(|table| table.data.as_slice())
        .unwrap_or(&[]);
    let encoded_output = build_embedded_output(
        head,
        os2,
        name,
        &subset_bytes,
        None,
        request.embedded_output,
    )?;

    fs::write(&request.output_path, encoded_output)
        .map_err(|error| format!("failed to write {}: {error}", request.output_path.display()))?;

    emit_subset_warnings(&subset_warnings);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubsetCliRequest {
    input_path: PathBuf,
    output_path: PathBuf,
    glyph_ids: Option<String>,
    text: Option<String>,
    variation_axes: Option<String>,
    embedded_output: EmbeddedOutputOptions,
    explicit_xor_mode: bool,
}

fn handle_subset_args(args: Vec<String>) -> Result<SubsetCliRequest, String> {
    if args.len() < 4 {
        return Err("subset expects INPUT OUTPUT plus a selection flag".to_string());
    }

    let mut request = SubsetCliRequest {
        input_path: args[0].clone().into(),
        output_path: args[1].clone().into(),
        glyph_ids: None,
        text: None,
        variation_axes: None,
        embedded_output: EmbeddedOutputOptions::default(),
        explicit_xor_mode: false,
    };
    let mut embedded_output_args = Vec::new();

    let mut index = 2usize;
    while index < args.len() {
        let flag = &args[index];
        if index + 1 >= args.len() {
            return Err("subset flag is missing a value".to_string());
        }

        match flag.as_str() {
            "--glyph-ids" => {
                if request.glyph_ids.is_some() || request.text.is_some() {
                    return Err("subset accepts only one selection mode".to_string());
                }
                request.glyph_ids = Some(args[index + 1].clone());
            }
            "--text" => {
                if request.glyph_ids.is_some() || request.text.is_some() {
                    return Err("subset accepts only one selection mode".to_string());
                }
                request.text = Some(args[index + 1].clone());
            }
            "--variation" => {
                request.variation_axes = Some(args[index + 1].clone());
            }
            "--payload-format" => {
                embedded_output_args.push(flag.clone());
                embedded_output_args.push(args[index + 1].clone());
            }
            "--xor" => {
                request.explicit_xor_mode = true;
                embedded_output_args.push(flag.clone());
                embedded_output_args.push(args[index + 1].clone());
            }
            "--eot-version" => {
                embedded_output_args.push(flag.clone());
                embedded_output_args.push(args[index + 1].clone());
            }
            _ => return Err(format!("unsupported subset flag `{flag}`")),
        }

        index += 2;
    }

    if request.glyph_ids.is_none() && request.text.is_none() {
        return Err("subset requires either --glyph-ids or --text".to_string());
    }
    if !embedded_output_args.is_empty() {
        let (embedded_output, _) =
            parse_embedded_output_args(&embedded_output_args, 0, &request.output_path)?;
        request.embedded_output = embedded_output;
    }
    if is_fntdata_output(&request.output_path) && !request.explicit_xor_mode {
        request.embedded_output.xor_mode = EmbeddedXorMode::On;
    }

    Ok(request)
}

fn load_subset_input_sfnt_bytes(input_path: &Path) -> Result<Vec<u8>, String> {
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("eot") | Some("fntdata") => {
            let input_bytes = fs::read(input_path)
                .map_err(|error| format!("failed to read {}: {error}", input_path.display()))?;
            load_subset_input_sfnt_from_embedded_bytes(&input_bytes).map_err(|error| {
                format!(
                    "failed to load subset input {}: {error}",
                    input_path.display()
                )
            })
        }
        _ => fs::read(input_path)
            .map_err(|error| format!("failed to read {}: {error}", input_path.display())),
    }
}

fn load_subset_input_sfnt_from_embedded_bytes(input_bytes: &[u8]) -> Result<Vec<u8>, String> {
    decode_embedded_font_bytes(input_bytes)
}

fn table_bytes<'a>(font: &'a OwnedSfntFont, tag: u32, name: &str) -> Result<&'a [u8], String> {
    font.table(tag)
        .map(|table| table.data.as_slice())
        .ok_or_else(|| format!("required table `{name}` is missing"))
}

fn build_block1_font(
    source_font: &OwnedSfntFont,
    head_table: &[u8],
    encoded_glyf: Vec<u8>,
) -> Result<OwnedSfntFont, String> {
    let mut subset = OwnedSfntFont::new(source_font.version_tag());
    for table in source_font.tables() {
        if should_copy_block1_table(table.tag) {
            subset.add_table(table.tag, table.data.clone());
        }
    }

    subset.add_table(TAG_HEAD, head_table.to_vec());
    subset.add_table(TAG_GLYF, encoded_glyf);
    subset.add_table(TAG_LOCA, Vec::new());
    Ok(subset)
}

fn should_copy_block1_table(tag: u32) -> bool {
    if matches!(tag, TAG_HEAD | TAG_GLYF | TAG_LOCA) {
        return false;
    }

    should_copy_encode_block1_table(tag)
}

fn is_fntdata_output(output_path: &Path) -> bool {
    output_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("fntdata"))
}

fn emit_subset_warnings(subset_warnings: &fonttool_subset::SubsetWarnings) {
    if subset_warnings.dropped_hdmx {
        eprintln!("warning: unsupported HDMX in subset path; dropping table");
    }
    if subset_warnings.dropped_vdmx {
        eprintln!("warning: unsupported VDMX in MTX encode/subset path; dropping table");
    }
}

fn subset_otf_file(
    request: &SubsetCliRequest,
    input_bytes: &[u8],
    kind: &fonttool_cff::CffFontKind,
) -> Result<(), String> {
    let text = request
        .text
        .as_deref()
        .ok_or_else(|| "subset currently requires --text for OTF input".to_string())?;
    if request.variation_axes.is_some() && !kind.is_variable {
        return Err(CffError::VariationRejectedForStaticInput.to_string());
    }

    let subset = if kind.is_variable {
        let axes = parse_variation_axes(request.variation_axes.as_deref().unwrap_or_default())
            .map_err(|error| error.to_string())?;
        subset_variable_cff2(input_bytes, text, &axes).map_err(|error| error.to_string())?
    } else {
        subset_static_cff(input_bytes, text).map_err(|error| error.to_string())?
    };
    let subset_bytes = serialize_subset_otf(subset).map_err(|error| error.to_string())?;
    let subset_font = load_sfnt(&subset_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;
    let head = table_bytes(&subset_font, TAG_HEAD, "head")?;
    let os2 = table_bytes(&subset_font, TAG_OS_2, "OS/2")?;
    let name = subset_font
        .table(TAG_NAME)
        .map(|table| table.data.as_slice())
        .unwrap_or(&[]);
    let encoded_output = build_embedded_output(
        head,
        os2,
        name,
        &subset_bytes,
        None,
        request.embedded_output,
    )?;

    fs::write(&request.output_path, encoded_output)
        .map_err(|error| format!("failed to write {}: {error}", request.output_path.display()))?;

    Ok(())
}
