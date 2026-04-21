use std::fs;
use std::path::PathBuf;

use fonttool_cff::office_static::extract_office_static_cff;
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
    assert!(office.office_cff_suffix.starts_with(&[0x04, 0x03, 0x00, 0x01]));
}

#[test]
fn extract_office_static_cff_finds_bold_fixture_payload() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let office = extract_office_static_cff(&block1).expect("bold fixture should be recognized");

    assert!(office.sfnt_bytes.starts_with(b"OTTO"));
    assert_eq!(office.cff_offset, 0x20e);
    assert!(office.office_cff_suffix.starts_with(&[0x04, 0x03, 0x00, 0x01]));
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
