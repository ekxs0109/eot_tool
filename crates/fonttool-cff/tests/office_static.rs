use std::fs;
use std::path::PathBuf;

use fonttool_cff::office_static::{extract_office_static_cff, rebuild_office_static_cff_table};
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

fn tracked_bytes(relative: &str) -> Vec<u8> {
    fs::read(fixture(relative)).expect("tracked fixture should be readable")
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
fn rebuild_office_static_cff_table_restores_regular_standard_prefix_and_splices_office_tail() {
    let block1 = decode_block1_from_fntdata("testdata/otto-cff-office.fntdata");
    let office = extract_office_static_cff(&block1).expect("regular fixture should decode");
    let rebuilt =
        rebuild_office_static_cff_table(&block1).expect("regular fixture should rebuild");
    let prefix = tracked_bytes("testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin");

    assert!(rebuilt.starts_with(&prefix));
    assert_eq!(
        &rebuilt[prefix.len()..prefix.len() + 32],
        &office.office_cff_suffix[2804..2804 + 32]
    );

    let name = read_index_header(&rebuilt, 4);
    let top = read_index_header(&rebuilt, name.next);
    let string = read_index_header(&rebuilt, top.next);
    let global = read_index_header(&rebuilt, string.next);

    assert_eq!((name.count, name.off_size), (1, 1));
    assert_eq!((top.count, top.off_size), (1, 1));
    assert_eq!((string.count, string.off_size), (23, 2));
    assert_eq!((global.count, global.off_size, global.data_start), (932, 2, 2806));
}

#[test]
fn rebuild_office_static_cff_table_restores_bold_standard_prefix_and_splices_office_tail() {
    let block1 = decode_block1_from_fntdata("testdata/presentation1-font2-bold.fntdata");
    let office = extract_office_static_cff(&block1).expect("bold fixture should decode");
    let rebuilt = rebuild_office_static_cff_table(&block1).expect("bold fixture should rebuild");
    let prefix = tracked_bytes("testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin");

    assert!(rebuilt.starts_with(&prefix));
    assert_eq!(
        &rebuilt[prefix.len()..prefix.len() + 32],
        &office.office_cff_suffix[3360..3360 + 32]
    );

    let name = read_index_header(&rebuilt, 4);
    let top = read_index_header(&rebuilt, name.next);
    let string = read_index_header(&rebuilt, top.next);
    let global = read_index_header(&rebuilt, string.next);

    assert_eq!((name.count, name.off_size), (1, 1));
    assert_eq!((top.count, top.off_size), (1, 1));
    assert_eq!((string.count, string.off_size), (23, 2));
    assert_eq!((global.count, global.off_size, global.data_start), (1240, 2, 3362));
}
