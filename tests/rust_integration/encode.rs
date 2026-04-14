use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::parse_eot_header;
use fonttool_mtx::{decompress_lz, parse_mtx_container};
use fonttool_sfnt::parse_sfnt;

const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn temp_eot() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-encode-{}-{unique}.eot",
        std::process::id()
    ))
}

fn run_fonttool<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("fonttool binary should launch")
}

fn find_table_length(sfnt_bytes: &[u8], tag: u32) -> Option<u32> {
    let font = parse_sfnt(sfnt_bytes).expect("sfnt should parse");
    font.table_directory()
        .entries()
        .iter()
        .find(|record| record.tag == tag)
        .map(|record| record.length)
}

#[test]
fn encode_ttf_to_eot_roundtrips_required_tables() {
    let output_path = temp_eot();
    let source_path = workspace_root().join("testdata/OpenSans-Regular.ttf");
    let source_bytes = fs::read(&source_path).expect("source font should be readable");

    let output = run_fonttool([
        "encode",
        source_path
            .to_str()
            .expect("fixture path should be valid utf-8"),
        output_path
            .to_str()
            .expect("temp path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let encoded_bytes = fs::read(&output_path).expect("encoded eot should be readable");
    let header = parse_eot_header(&encoded_bytes).expect("encoded eot header should parse");
    assert_eq!(header.version, 0x0002_0002);
    assert_ne!(header.flags & 0x4, 0, "compressed flag should be set");

    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let container =
        parse_mtx_container(&encoded_bytes[payload_start..payload_end]).expect("mtx should parse");

    assert_eq!(container.num_blocks, 3);

    let block1 = decompress_lz(container.block1).expect("block1 should decompress");
    let block2 = decompress_lz(container.block2.expect("block2 should exist"))
        .expect("block2 should decompress");
    let block3 = decompress_lz(container.block3.expect("block3 should exist"))
        .expect("block3 should decompress");

    assert!(!block2.is_empty(), "block2 should contain push data");
    assert!(!block3.is_empty(), "block3 should contain code data");

    parse_sfnt(&block1).expect("block1 should be a standard sfnt");
    for tag in [
        TAG_HEAD, TAG_NAME, TAG_OS_2, TAG_CMAP, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_GLYF, TAG_LOCA,
    ] {
        assert!(
            find_table_length(&block1, tag).is_some(),
            "block1 should include tag 0x{tag:08x}"
        );
    }

    let source_glyf_length = find_table_length(&source_bytes, TAG_GLYF).expect("source glyf");
    let block1_glyf_length = find_table_length(&block1, TAG_GLYF).expect("block1 glyf");
    let block1_loca_length = find_table_length(&block1, TAG_LOCA).expect("block1 loca");

    assert_eq!(block1_loca_length, 0, "block1 loca should be zero-length");
    assert_ne!(
        block1_glyf_length, source_glyf_length,
        "block1 glyf should differ from source glyf"
    );

    let _ = fs::remove_file(output_path);
}
