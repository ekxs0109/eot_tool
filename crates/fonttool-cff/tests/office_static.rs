use std::fs;
use std::path::PathBuf;

use fonttool_cff::extract_office_static_cff;
use fonttool_eot::parse_eot_header;
use fonttool_mtx::{decompress_lz_with_limit, parse_mtx_container};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn decode_block1_from_fntdata(relative: &str) -> Vec<u8> {
    let input = fs::read(fixture(relative)).expect("fixture should be readable");
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

#[test]
fn extract_office_static_cff_finds_regular_fixture_payload() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let office = extract_office_static_cff(&block1).expect("regular fixture should be recognized");

    assert!(office.sfnt_bytes.starts_with(b"OTTO"));
    assert_eq!(office.cff_offset, 0x20e);
    assert!(office.cff_bytes.starts_with(&[0x04, 0x03, 0x00, 0x01]));
}

#[test]
fn extract_office_static_cff_finds_bold_fixture_payload() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let office = extract_office_static_cff(&block1).expect("bold fixture should be recognized");

    assert!(office.sfnt_bytes.starts_with(b"OTTO"));
    assert_eq!(office.cff_offset, 0x20e);
    assert!(office.cff_bytes.starts_with(&[0x04, 0x03, 0x00, 0x01]));
}
