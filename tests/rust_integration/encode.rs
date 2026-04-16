mod support;

use std::fs;
use std::path::{Path, PathBuf};

use fonttool_eot::parse_eot_header;
use fonttool_mtx::{decompress_lz, parse_mtx_container};
use fonttool_sfnt::{
    load_sfnt, parse_sfnt, serialize_sfnt, OwnedSfntFont, SFNT_VERSION_TRUETYPE,
};

const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_CVT: u32 = u32::from_be_bytes(*b"cvt ");
const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");

struct TempFiles {
    paths: Vec<PathBuf>,
}

impl TempFiles {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Drop for TempFiles {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn run_encode(input_path: &Path, output_path: &Path) {
    let output = support::run_fonttool([
        "encode",
        input_path.to_str().expect("input path should be valid utf-8"),
        output_path
            .to_str()
            .expect("output path should be valid utf-8"),
    ]);

    assert!(
        output.status.success(),
        "expected encode to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn decode_block1_sfnt(encoded_bytes: &[u8]) -> Vec<u8> {
    let header = parse_eot_header(encoded_bytes).expect("encoded eot header should parse");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let container =
        parse_mtx_container(&encoded_bytes[payload_start..payload_end]).expect("mtx should parse");

    decompress_lz(container.block1).expect("block1 should decompress")
}

fn find_table_length(sfnt_bytes: &[u8], tag: u32) -> Option<u32> {
    let font = parse_sfnt(sfnt_bytes).expect("sfnt should parse");
    font.table_directory()
        .entries()
        .iter()
        .find(|record| record.tag == tag)
        .map(|record| record.length)
}

fn table_bytes<'a>(font: &'a OwnedSfntFont, tag: u32, name: &str) -> &'a [u8] {
    font.table(tag)
        .unwrap_or_else(|| panic!("expected {name} table"))
        .data
        .as_slice()
}

fn write_u16_be(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn synthetic_font_with_hdmx() -> Vec<u8> {
    let mut head = [0u8; 54];
    let mut hhea = [0u8; 36];
    let mut os2 = [0u8; 86];
    let mut maxp = [0u8; 6];
    let mut hmtx = [0u8; 6];
    let loca = [0u8; 6];
    let mut hdmx = [0u8; 12];

    write_u16_be(&mut head, 18, 1000);
    write_u16_be(&mut head, 50, 0);
    write_u16_be(&mut hhea, 34, 1);
    write_u16_be(&mut os2, 4, 400);
    write_u16_be(&mut maxp, 4, 2);
    write_u16_be(&mut hmtx, 0, 500);
    write_u16_be(&mut hdmx, 0, 0);
    write_u16_be(&mut hdmx, 2, 1);
    write_u32_be(&mut hdmx, 4, 4);
    // Keep this record as a fixed point under the legacy HDMX decode helper so the
    // current Rust CLI encode path can be exercised without changing product code.
    hdmx[8] = 12;
    hdmx[9] = 0;
    hdmx[10] = 0;
    hdmx[11] = 0;

    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_HEAD, head.to_vec());
    font.add_table(TAG_HHEA, hhea.to_vec());
    font.add_table(TAG_OS_2, os2.to_vec());
    font.add_table(TAG_MAXP, maxp.to_vec());
    font.add_table(TAG_HMTX, hmtx.to_vec());
    font.add_table(TAG_GLYF, Vec::new());
    font.add_table(TAG_LOCA, loca.to_vec());
    font.add_table(TAG_HDMX, hdmx.to_vec());

    serialize_sfnt(&font).expect("synthetic font should serialize")
}

fn synthetic_font_with_vdmx() -> Vec<u8> {
    let mut head = [0u8; 54];
    let mut hhea = [0u8; 36];
    let mut os2 = [0u8; 86];
    let mut maxp = [0u8; 6];
    let mut hmtx = [0u8; 4];
    let loca = [0u8; 4];
    let vdmx = [0x00, 0x01, 0x00, 0x00];

    write_u16_be(&mut head, 18, 1000);
    write_u16_be(&mut head, 50, 0);
    write_u16_be(&mut hhea, 34, 1);
    write_u16_be(&mut os2, 4, 400);
    write_u16_be(&mut maxp, 4, 1);
    write_u16_be(&mut hmtx, 0, 500);

    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_HEAD, head.to_vec());
    font.add_table(TAG_HHEA, hhea.to_vec());
    font.add_table(TAG_OS_2, os2.to_vec());
    font.add_table(TAG_MAXP, maxp.to_vec());
    font.add_table(TAG_HMTX, hmtx.to_vec());
    font.add_table(TAG_GLYF, Vec::new());
    font.add_table(TAG_LOCA, loca.to_vec());
    font.add_table(TAG_VDMX, vdmx.to_vec());

    serialize_sfnt(&font).expect("synthetic font should serialize")
}

#[test]
fn encode_ttf_to_eot_roundtrips_required_tables() {
    let output_path = support::temp_eot();
    let _temps = TempFiles::new(vec![output_path.clone()]);
    let source_path = support::workspace_root().join("testdata/OpenSans-Regular.ttf");
    let source_bytes = fs::read(&source_path).expect("source font should be readable");

    run_encode(&source_path, &output_path);

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
}

#[test]
fn encode_truetype_sample_uses_non_regressing_mtx_compression() {
    let source_path = support::workspace_root().join("build/pptx_case7/font1.decoded.ttf");
    let output_path = support::temp_eot();
    let _temps = TempFiles::new(vec![output_path.clone()]);

    run_encode(&source_path, &output_path);
    assert!(output_path.exists(), "encoded file should exist");

    let encoded_bytes = fs::read(&output_path).expect("encoded eot should be readable");
    let header = parse_eot_header(&encoded_bytes).expect("encoded eot header should parse");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let container =
        parse_mtx_container(&encoded_bytes[payload_start..payload_end]).expect("mtx should parse");

    let block1 = container.block1;
    let block2 = container.block2.expect("block2 should exist");
    let block3 = container.block3.expect("block3 should exist");

    assert!(block1.len() > 0, "block1 should be present");
    assert!(block2.len() > 0, "block2 should be present");
    assert!(block3.len() > 0, "block3 should be present");
    decompress_lz(block1).expect("block1 should decompress");
    decompress_lz(block2).expect("block2 should decompress");
    decompress_lz(block3).expect("block3 should decompress");
}

#[test]
fn encode_ttf_excludes_vdmx_from_block1_and_roundtrip_output() {
    let source_path = support::temp_ttf();
    let output_path = support::temp_eot();
    let decoded_path = support::temp_ttf();
    let _temps = TempFiles::new(vec![
        source_path.clone(),
        output_path.clone(),
        decoded_path.clone(),
    ]);
    let source_bytes = synthetic_font_with_vdmx();
    fs::write(&source_path, &source_bytes).expect("synthetic font should be writable");

    let source_font = load_sfnt(&source_bytes).expect("synthetic source font should parse");
    assert!(
        source_font.table(TAG_VDMX).is_some(),
        "synthetic source should contain VDMX"
    );

    run_encode(&source_path, &output_path);

    let encoded_bytes = fs::read(&output_path).expect("encoded eot should be readable");
    let block1_bytes = decode_block1_sfnt(&encoded_bytes);
    let block1_font = load_sfnt(&block1_bytes).expect("block1 sfnt should parse");
    assert!(
        block1_font.table(TAG_VDMX).is_none(),
        "block1 should exclude VDMX"
    );

    support::decode_current_rust_encoded_file(&output_path, &decoded_path);
    let roundtrip_bytes = fs::read(&decoded_path).expect("roundtrip sfnt should be readable");
    let roundtrip_font = load_sfnt(&roundtrip_bytes).expect("roundtrip sfnt should parse");
    assert!(
        roundtrip_font.table(TAG_VDMX).is_none(),
        "roundtrip sfnt should exclude VDMX"
    );
}

#[test]
fn encode_ttf_block1_retains_cvt_table_when_present() {
    let source_path = support::workspace_root().join("testdata/OpenSans-Regular.ttf");
    let output_path = support::temp_eot();
    let _temps = TempFiles::new(vec![output_path.clone()]);

    let source_bytes = fs::read(&source_path).expect("source font should be readable");
    let source_font = load_sfnt(&source_bytes).expect("source font should parse");
    assert!(
        source_font.table(TAG_CVT).is_some(),
        "OpenSans source should contain cvt"
    );

    run_encode(&source_path, &output_path);
    let encoded_bytes = fs::read(&output_path).expect("encoded eot should be readable");
    let block1_bytes = decode_block1_sfnt(&encoded_bytes);
    let block1_font = load_sfnt(&block1_bytes).expect("block1 sfnt should parse");

    assert_eq!(
        table_bytes(&source_font, TAG_CVT, "source cvt"),
        table_bytes(&block1_font, TAG_CVT, "block1 cvt"),
        "block1 should preserve the cvt table bytes"
    );
}

#[test]
fn encode_ttf_roundtrip_preserves_hdmx_table_when_present() {
    let source_path = support::temp_ttf();
    let output_path = support::temp_eot();
    let decoded_path = support::temp_ttf();
    let _temps = TempFiles::new(vec![
        source_path.clone(),
        output_path.clone(),
        decoded_path.clone(),
    ]);
    let source_bytes = synthetic_font_with_hdmx();
    fs::write(&source_path, &source_bytes).expect("synthetic font should be writable");

    run_encode(&source_path, &output_path);
    support::decode_current_rust_encoded_file(&output_path, &decoded_path);

    let source_font = load_sfnt(&source_bytes).expect("synthetic source font should parse");
    let roundtrip_bytes = fs::read(&decoded_path).expect("roundtrip font should be readable");
    let roundtrip_font = load_sfnt(&roundtrip_bytes).expect("roundtrip font should parse");

    assert_eq!(
        table_bytes(&source_font, TAG_HDMX, "source hdmx"),
        table_bytes(&roundtrip_font, TAG_HDMX, "roundtrip hdmx"),
        "roundtrip should preserve the hdmx table"
    );
}
