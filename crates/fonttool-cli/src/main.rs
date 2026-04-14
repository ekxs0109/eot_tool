use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::{build_eot_file, parse_eot_header};
use fonttool_glyf::encode_glyf;
use fonttool_harfbuzz::{run_subset_adapter, LegacySubsetRequest};
use fonttool_mtx::{compress_lz_literals, decompress_lz, pack_mtx_container, parse_mtx_container};
use fonttool_sfnt::{load_sfnt, parse_sfnt, serialize_sfnt, OwnedSfntFont, SFNT_VERSION_TRUETYPE};
use fonttool_subset::{plan_glyph_subset, GlyphIdRequest, SubsetWarnings};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const TAG_DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");

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
            Ok((input, output, glyph_ids)) => match subset_file(&input, &output, &glyph_ids) {
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
    println!("  subset <INPUT> <OUTPUT> --glyph-ids <LIST>  Subset a font through the Rust adapter boundary");
    println!();
    println!("Options:");
    println!("  -h, --help  Print help");
}

fn decode_file(input_path: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Result<(), String> {
    let input_bytes = fs::read(input_path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", input_path.as_ref().display()))?;

    let header =
        parse_eot_header(&input_bytes).map_err(|error| format!("invalid EOT header: {error}"))?;
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
    reject_unsupported_extra_blocks(&container)?;
    let sfnt_bytes = decompress_lz(container.block1)
        .map_err(|error| format!("failed to decode MTX block1: {error}"))?;

    parse_sfnt(&sfnt_bytes).map_err(|error| format!("decoded SFNT is invalid: {error}"))?;
    fs::write(output_path.as_ref(), &sfnt_bytes).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            output_path.as_ref().display()
        )
    })?;

    Ok(())
}

fn encode_file(input_path: impl AsRef<Path>, output_path: impl AsRef<Path>) -> Result<(), String> {
    let input_bytes = fs::read(input_path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", input_path.as_ref().display()))?;
    let source_font = load_sfnt(&input_bytes).map_err(|error| format!("invalid SFNT: {error}"))?;

    if source_font.version_tag() != SFNT_VERSION_TRUETYPE {
        return Err("encode currently only supports TrueType glyf fonts".to_string());
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
    let block2 = compress_lz_literals(&encoded_glyf.push_stream)
        .map_err(|error| format!("failed to compress MTX block2: {error}"))?;
    let block3 = compress_lz_literals(&encoded_glyf.code_stream)
        .map_err(|error| format!("failed to compress MTX block3: {error}"))?;
    let block1 = compress_lz_literals(&block1)
        .map_err(|error| format!("failed to compress MTX block1: {error}"))?;
    let mtx_payload = pack_mtx_container(&block1, Some(&block2), Some(&block3))
        .map_err(|error| format!("failed to pack MTX container: {error}"))?;
    let encoded_eot = build_eot_file(head, os2, &mtx_payload, false)
        .map_err(|error| format!("failed to build EOT header: {error}"))?;

    fs::write(output_path.as_ref(), encoded_eot).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            output_path.as_ref().display()
        )
    })?;

    Ok(())
}

fn subset_file(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    glyph_ids_csv: &str,
) -> Result<(), String> {
    let request =
        GlyphIdRequest::parse_csv(glyph_ids_csv).map_err(|error| format!("invalid subset arguments: {error}"))?;

    // Keep the Rust-owned planning boundary narrow for Task 6. The adapter executes
    // the currently trusted backend while we migrate the rest of the subset pipeline.
    let plan = plan_subset_for_input(input_path.as_ref(), &request)?;
    let warnings = run_subset_adapter(LegacySubsetRequest {
        input_path: input_path.as_ref(),
        output_path: output_path.as_ref(),
        plan: &plan,
    })
    .map_err(|error| error.to_string())?;
    emit_subset_warnings(warnings);

    Ok(())
}

fn handle_subset_args(args: Vec<String>) -> Result<(String, String, String), String> {
    if args.len() != 4 {
        return Err("subset expects INPUT OUTPUT --glyph-ids LIST".to_string());
    }

    if args[2] != "--glyph-ids" {
        return Err("subset currently only supports --glyph-ids".to_string());
    }

    Ok((args[0].clone(), args[1].clone(), args[3].clone()))
}

fn plan_subset_for_input(input_path: &Path, request: &GlyphIdRequest) -> Result<fonttool_subset::SubsetPlan, String> {
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let font = match extension.as_deref() {
        Some("ttf") | Some("otf") => {
            let input_bytes = fs::read(input_path)
                .map_err(|error| format!("failed to read {}: {error}", input_path.display()))?;
            load_sfnt(&input_bytes).map_err(|error| format!("invalid SFNT: {error}"))?
        }
        Some("eot") | Some("fntdata") => {
            let temp_output = temp_subset_decode_path();
            let decode_status = std::process::Command::new("build/fonttool")
                .arg("decode")
                .arg(input_path)
                .arg(&temp_output)
                .output()
                .map_err(|error| format!("failed to launch legacy decode backend: {error}"))?;
            if !decode_status.status.success() {
                return Err(format!(
                    "failed to load subset input {}: {}",
                    input_path.display(),
                    String::from_utf8_lossy(&decode_status.stderr).trim()
                ));
            }
            let sfnt_bytes = fs::read(&temp_output)
                .map_err(|error| format!("failed to read decoded subset input: {error}"))?;
            let _ = fs::remove_file(&temp_output);
            load_sfnt(&sfnt_bytes).map_err(|error| format!("invalid decoded SFNT: {error}"))?
        }
        _ => return Err(format!("unsupported subset input path: {}", input_path.display())),
    };

    plan_glyph_subset(&font, request, false).map_err(|error| error.to_string())
}

fn emit_subset_warnings(warnings: SubsetWarnings) {
    if warnings.dropped_hdmx {
        eprintln!("warning: unsupported HDMX in subset path; dropping table");
    }
    if warnings.dropped_vdmx {
        eprintln!("warning: unsupported VDMX in MTX encode/subset path; dropping table");
    }
}

fn temp_subset_decode_path() -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir()
        .join(format!(
            "fonttool-subset-plan-{}-{unique}.ttf",
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned()
}

fn reject_unsupported_extra_blocks(
    container: &fonttool_mtx::MtxContainer<'_>,
) -> Result<(), String> {
    for (index, block) in [container.block2, container.block3].into_iter().enumerate() {
        let Some(block) = block else {
            continue;
        };

        let decoded = decompress_lz(block)
            .map_err(|error| format!("failed to decode MTX block{}: {error}", index + 2))?;
        if !decoded.is_empty() {
            return Err(
                "non-empty extra MTX blocks are not supported in this decode slice".to_string(),
            );
        }
    }

    Ok(())
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
    !matches!(tag, TAG_HEAD | TAG_GLYF | TAG_LOCA | TAG_DSIG | TAG_VDMX)
}
