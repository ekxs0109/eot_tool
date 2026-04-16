use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use fonttool_cff::{inspect_otf_font, CffError};
use fonttool_eot::{build_eot_file, parse_eot_header};
use fonttool_glyf::{decode_glyf, encode_glyf};
use fonttool_harfbuzz::subset_font_bytes;
use fonttool_mtx::{compress_lz, decompress_lz, pack_mtx_container, parse_mtx_container};
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
        Some("encode") => match (args.next(), args.next(), args.next()) {
            (Some(input), Some(output), None) => match encode_file(&input, &output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("fonttool: {error}");
                    ExitCode::from(1)
                }
            },
            _ => {
                eprintln!("fonttool: encode expects INPUT and OUTPUT paths");
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

fn print_help() {
    println!("fonttool");
    println!();
    println!("Usage: fonttool <COMMAND>");
    println!();
    println!("Commands:");
    println!("  encode <INPUT> <OUTPUT>  Encode a TrueType font into EOT/MTX");
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

fn decode_embedded_font_bytes(input_bytes: &[u8]) -> Result<Vec<u8>, String> {
    decode_embedded_font_bytes_full(input_bytes)
}

fn decode_embedded_font_bytes_full(input_bytes: &[u8]) -> Result<Vec<u8>, String> {
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

    let container = parse_mtx_container(&payload_bytes)
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

fn encode_file(input_path: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Result<(), String> {
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
    let source_font = load_sfnt(&input_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;

    if source_font.table(TAG_CFF).is_some() || source_font.table(TAG_CFF2).is_some() {
        return Err(CffError::EncodeDeferredToPhase3.to_string());
    }

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

    let block1_font = build_block1_font(&source_font, &head, encoded_glyf.glyf_stream)?;
    let block1 = serialize_sfnt(&block1_font)
        .map_err(|error| format!("failed to serialize encoded SFNT: {error}"))?;
    let block2 = compress_lz(&encoded_glyf.push_stream)
        .map_err(|error| format!("failed to compress MTX block2: {error}"))?;
    let block3 = compress_lz(&encoded_glyf.code_stream)
        .map_err(|error| format!("failed to compress MTX block3: {error}"))?;
    let block1 =
        compress_lz(&block1).map_err(|error| format!("failed to compress MTX block1: {error}"))?;
    let mtx_payload = pack_mtx_container(&block1, Some(&block2), Some(&block3))
        .map_err(|error| format!("failed to pack MTX container: {error}"))?;
    let encoded_eot = build_eot_file(head, os2, &mtx_payload, false)
        .map_err(|error| format!("failed to build EOT header: {error}"))?;

    fs::write(output_path, encoded_eot)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;

    Ok(())
}

fn subset_file(request: SubsetCliRequest) -> Result<(), String> {
    let input_bytes = load_subset_input_sfnt_bytes(&request.input_path)?;
    let kind = inspect_otf_font(&input_bytes).map_err(|error| error.to_string())?;
    if kind.is_cff_flavor {
        return subset_otf_file(&request, &kind);
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
    let block1 = compress_lz(&subset_bytes)
        .map_err(|error| format!("failed to compress subset block1: {error}"))?;
    let mtx_payload = pack_mtx_container(&block1, None, None)
        .map_err(|error| format!("failed to pack subset MTX container: {error}"))?;
    let head = table_bytes(&subset_font, TAG_HEAD, "head")?;
    let os2 = table_bytes(&subset_font, TAG_OS_2, "OS/2")?;
    let encoded_output = build_eot_file(
        head,
        os2,
        &mtx_payload,
        is_fntdata_output(&request.output_path),
    )
    .map_err(|error| format!("failed to build subset EOT header: {error}"))?;

    fs::write(&request.output_path, encoded_output)
        .map_err(|error| format!("failed to write {}: {error}", request.output_path.display()))?;

    emit_subset_warnings(&subset_warnings);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubsetCliRequest {
    input_path: std::path::PathBuf,
    output_path: std::path::PathBuf,
    glyph_ids: Option<String>,
    text: Option<String>,
    variation_axes: Option<String>,
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
    };

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
            _ => return Err(format!("unsupported subset flag `{flag}`")),
        }

        index += 2;
    }

    if request.glyph_ids.is_none() && request.text.is_none() {
        return Err("subset requires either --glyph-ids or --text".to_string());
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
    decode_embedded_font_bytes_full(input_bytes)
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
    kind: &fonttool_cff::CffFontKind,
) -> Result<(), String> {
    let text = request
        .text
        .as_deref()
        .ok_or_else(|| "subset currently requires --text for OTF input".to_string())?;
    if request.variation_axes.is_some() && !kind.is_variable {
        return Err(CffError::VariationRejectedForStaticInput.to_string());
    }

    let _ = text;
    Err(CffError::SubsetDeferredToPhase3.to_string())
}
