use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use fonttool_eot::parse_eot_header;
use fonttool_mtx::{decompress_lz, parse_mtx_container};
use fonttool_sfnt::parse_sfnt;

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;

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
    println!("  decode <INPUT> <OUTPUT>  Decode an EOT/MTX font payload into SFNT");
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
