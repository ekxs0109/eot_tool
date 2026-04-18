mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::{build_eot_file, parse_eot_header};
use fonttool_mtx::parse_mtx_container;
use fonttool_sfnt::{load_sfnt, serialize_sfnt};

const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");

fn workspace_root() -> PathBuf {
    support::workspace_root()
}

fn temp_file(extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-subset-{}-{unique}.{extension}",
        std::process::id()
    ))
}

fn temp_derived_file(stem: &str, extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-subset-{stem}-{}-{unique}.{extension}",
        std::process::id()
    ))
}

fn decode_subset_output(output_path: &Path) -> PathBuf {
    let decoded_path = temp_file("ttf");
    let output = support::run_fonttool_in_dir(
        [
            "decode",
            output_path
                .to_str()
                .expect("subset output path should be valid utf-8"),
            decoded_path
                .to_str()
                .expect("decoded path should be valid utf-8"),
        ],
        &workspace_root(),
    );

    assert!(
        output.status.success(),
        "expected subset output to decode, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    decoded_path
}

fn read_embedded_payload_bytes(encoded_bytes: &[u8]) -> Vec<u8> {
    let header = parse_eot_header(encoded_bytes).expect("encoded subset header should parse");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let mut payload = encoded_bytes[payload_start..payload_end].to_vec();

    if header.flags & 0x1000_0000 != 0 {
        for byte in &mut payload {
            *byte ^= 0x50;
        }
    }

    payload
}

fn read_font(path: &Path) -> fonttool_sfnt::OwnedSfntFont {
    let bytes = fs::read(path).expect("font should be readable");
    load_sfnt(&bytes).expect("font should parse")
}

fn read_u16_be(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("table should be long enough"),
    )
}

fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("table should be long enough"),
    )
}

fn read_maxp_num_glyphs(font: &fonttool_sfnt::OwnedSfntFont) -> u16 {
    let maxp = font.table(TAG_MAXP).expect("font should contain maxp");
    read_u16_be(&maxp.data, 4)
}

fn read_index_to_loca_format(font: &fonttool_sfnt::OwnedSfntFont) -> i16 {
    let head = font.table(TAG_HEAD).expect("font should contain head");
    assert!(head.data.len() >= 52, "head table should be long enough");
    i16::from_be_bytes([head.data[50], head.data[51]])
}

fn read_num_hmetrics(font: &fonttool_sfnt::OwnedSfntFont) -> u16 {
    let hhea = font.table(TAG_HHEA).expect("font should contain hhea");
    assert!(hhea.data.len() >= 36, "hhea table should be long enough");
    read_u16_be(&hhea.data, 34)
}

fn glyph_length(font: &fonttool_sfnt::OwnedSfntFont, glyph_id: u16) -> usize {
    let loca = font.table(TAG_LOCA).expect("font should contain loca");
    let glyf = font.table(TAG_GLYF).expect("font should contain glyf");
    match read_index_to_loca_format(font) {
        0 => {
            let offset = usize::from(glyph_id) * 2;
            assert!(offset + 4 <= loca.data.len(), "loca should be long enough");
            let start = usize::from(read_u16_be(&loca.data, offset)) * 2;
            let end = usize::from(read_u16_be(&loca.data, offset + 2)) * 2;
            assert!(end >= start, "glyph range should be ordered");
            assert!(end <= glyf.data.len(), "glyph range should fit glyf table");
            end - start
        }
        1 => {
            let offset = usize::from(glyph_id) * 4;
            assert!(offset + 8 <= loca.data.len(), "loca should be long enough");
            let start = read_u32_be(&loca.data, offset) as usize;
            let end = read_u32_be(&loca.data, offset + 4) as usize;
            assert!(end >= start, "glyph range should be ordered");
            assert!(end <= glyf.data.len(), "glyph range should fit glyf table");
            end - start
        }
        other => panic!("unsupported loca format {other}"),
    }
}

fn cmap_lookup_gid(font: &fonttool_sfnt::OwnedSfntFont, codepoint: u32) -> u16 {
    let cmap = font.table(TAG_CMAP).expect("font should contain cmap");
    assert!(cmap.data.len() >= 4, "cmap table should be long enough");
    let num_tables = read_u16_be(&cmap.data, 2) as usize;
    let mut best_score = -1i32;
    let mut best_format = 0u16;
    let mut best_offset = 0usize;

    for index in 0..num_tables {
        let record_offset = 4 + index * 8;
        let platform_id = read_u16_be(&cmap.data, record_offset);
        let encoding_id = read_u16_be(&cmap.data, record_offset + 2);
        let subtable_offset = read_u32_be(&cmap.data, record_offset + 4) as usize;
        if subtable_offset + 2 > cmap.data.len() {
            continue;
        }

        let format = read_u16_be(&cmap.data, subtable_offset);
        let score = match format {
            12 if platform_id == 3 && encoding_id == 10 => 4,
            12 if platform_id == 0 => 3,
            4 if platform_id == 3 && encoding_id == 1 => 2,
            4 if platform_id == 0 => 1,
            _ => -1,
        };
        if score > best_score {
            best_score = score;
            best_format = format;
            best_offset = subtable_offset;
        }
    }

    match best_format {
        4 => {
            let subtable = &cmap.data[best_offset..];
            let length = read_u16_be(subtable, 2) as usize;
            assert!(length <= subtable.len(), "format 4 cmap should fit");
            let seg_count = usize::from(read_u16_be(subtable, 6) / 2);
            let end_codes_offset = 14;
            let start_codes_offset = end_codes_offset + seg_count * 2 + 2;
            let id_deltas_offset = start_codes_offset + seg_count * 2;
            let id_range_offsets_offset = id_deltas_offset + seg_count * 2;

            for seg_index in 0..seg_count {
                let end_code = read_u16_be(subtable, end_codes_offset + seg_index * 2);
                if codepoint > u32::from(end_code) {
                    continue;
                }
                let start_code = read_u16_be(subtable, start_codes_offset + seg_index * 2);
                if codepoint < u32::from(start_code) {
                    return 0;
                }

                let id_delta = i16::from_be_bytes(
                    subtable
                        [id_deltas_offset + seg_index * 2..id_deltas_offset + seg_index * 2 + 2]
                        .try_into()
                        .expect("delta should exist"),
                );
                let id_range_offset =
                    read_u16_be(subtable, id_range_offsets_offset + seg_index * 2);
                if id_range_offset == 0 {
                    return ((codepoint as i32 + id_delta as i32) & 0xFFFF) as u16;
                }

                let id_range_pos = id_range_offsets_offset + seg_index * 2;
                let glyph_index_pos = id_range_pos
                    + usize::from(id_range_offset)
                    + usize::try_from(codepoint - u32::from(start_code))
                        .expect("range width should fit")
                        * 2;
                if glyph_index_pos + 2 > length {
                    return 0;
                }
                let glyph = read_u16_be(subtable, glyph_index_pos);
                if glyph == 0 {
                    return 0;
                }
                return ((u32::from(glyph) + id_delta as i32 as u32) & 0xFFFF) as u16;
            }
            0
        }
        12 => {
            let subtable = &cmap.data[best_offset..];
            let length = read_u32_be(subtable, 4) as usize;
            assert!(length <= subtable.len(), "format 12 cmap should fit");
            let num_groups = read_u32_be(subtable, 12) as usize;

            for group_index in 0..num_groups {
                let group_offset = 16 + group_index * 12;
                let start_char = read_u32_be(subtable, group_offset);
                let end_char = read_u32_be(subtable, group_offset + 4);
                if codepoint < start_char {
                    break;
                }
                if codepoint <= end_char {
                    return (read_u32_be(subtable, group_offset + 8) + (codepoint - start_char))
                        as u16;
                }
            }
            0
        }
        _ => 0,
    }
}

fn assert_supported_subset_core_tables(font: &fonttool_sfnt::OwnedSfntFont) {
    for tag in [
        TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_GLYF, TAG_LOCA, TAG_CMAP,
    ] {
        assert!(
            font.table(tag).is_some(),
            "expected table {:08X} in subset output",
            tag
        );
    }
}

fn assert_subset_rebuilds_core_tables(input_path: &Path, output_path: &Path) {
    let input_font = read_font(input_path);
    let output_font = read_font(output_path);

    assert_supported_subset_core_tables(&output_font);
    assert_eq!(read_maxp_num_glyphs(&output_font), 4);
    assert_eq!(read_num_hmetrics(&output_font), 4);
    assert_eq!(
        output_font
            .table(TAG_HMTX)
            .expect("hmtx should exist")
            .data
            .len(),
        16
    );
    assert_eq!(
        output_font
            .table(TAG_LOCA)
            .expect("loca should exist")
            .data
            .len(),
        match read_index_to_loca_format(&output_font) {
            0 => 10,
            1 => 20,
            other => panic!("unsupported loca format {other}"),
        }
    );

    assert!(
        output_font
            .table(TAG_GLYF)
            .expect("glyf should exist")
            .data
            .len()
            < input_font
                .table(TAG_GLYF)
                .expect("input glyf should exist")
                .data
                .len(),
        "subset glyf table should be rebuilt smaller than the source"
    );

    assert!(
        glyph_length(&output_font, 0) > 0,
        "subset glyph 0 should retain a .notdef outline"
    );
    assert!(
        glyph_length(&output_font, 1) > 0,
        "subset glyph 1 should retain outline data"
    );
    assert!(
        glyph_length(&output_font, 2) > 0,
        "subset glyph 2 should retain outline data"
    );
    assert!(
        glyph_length(&output_font, 3) > 0,
        "subset glyph 3 should retain outline data"
    );

    assert_eq!(cmap_lookup_gid(&input_font, u32::from('A')), 36);
    assert_eq!(cmap_lookup_gid(&input_font, u32::from('B')), 37);
    assert_eq!(cmap_lookup_gid(&input_font, u32::from('C')), 38);
    assert_eq!(cmap_lookup_gid(&output_font, u32::from('A')), 1);
    assert_eq!(cmap_lookup_gid(&output_font, u32::from('B')), 2);
    assert_eq!(cmap_lookup_gid(&output_font, u32::from('C')), 3);
    assert_eq!(cmap_lookup_gid(&output_font, u32::from('Z')), 0);
}

fn run_subset_command(args: &[&str], cwd: &Path) -> std::process::Output {
    support::run_fonttool_in_dir(args.iter().copied(), cwd)
}

fn build_subset_fixture_font(include_extra_tables: bool) -> fonttool_sfnt::OwnedSfntFont {
    let bytes = fs::read(workspace_root().join("testdata/OpenSans-Regular.ttf"))
        .expect("OpenSans fixture should be readable");
    let mut font = load_sfnt(&bytes).expect("OpenSans fixture should load");

    if include_extra_tables {
        font.add_table(
            TAG_HDMX,
            vec![
                0x00, 0x00, 0x00, 0x01, 0x00, 0x04, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00,
            ],
        );
        font.add_table(TAG_VDMX, vec![0x00, 0x01, 0x00, 0x00]);
    }

    font
}

fn prepare_plain_subset_ttf_fixture() -> (PathBuf, TempCleanup) {
    let ttf_path = temp_derived_file("fixture", "ttf");
    let font = build_subset_fixture_font(true);
    fs::write(
        &ttf_path,
        serialize_sfnt(&font).expect("fixture font should serialize"),
    )
    .expect("fixture ttf should be writable");

    let cleanup = TempCleanup::new(vec![ttf_path.clone()]);
    (ttf_path, cleanup)
}

fn prepare_real_subset_wrapper_fixture() -> (PathBuf, PathBuf, PathBuf, TempCleanup) {
    let ttf_path = temp_derived_file("fixture", "ttf");
    let eot_path = temp_derived_file("fixture", "eot");
    let fntdata_path = temp_derived_file("fixture", "fntdata");
    let font = build_subset_fixture_font(true);
    let subset_bytes = serialize_sfnt(&font).expect("fixture font should serialize");
    fs::write(&ttf_path, &subset_bytes).expect("fixture ttf should be writable");

    let encode = support::run_fonttool([
        "encode",
        ttf_path
            .to_str()
            .expect("fixture path should be valid utf-8"),
        eot_path
            .to_str()
            .expect("fixture path should be valid utf-8"),
    ]);
    assert!(
        encode.status.success(),
        "expected Rust encode fixture to succeed, stderr: {}",
        String::from_utf8_lossy(&encode.stderr)
    );

    let eot_bytes = fs::read(&eot_path).expect("fixture eot should be readable");
    let header = parse_eot_header(&eot_bytes).expect("fixture header should parse");
    let payload_start = header.header_length as usize;
    let payload_end = payload_start + header.font_data_size as usize;
    let payload = &eot_bytes[payload_start..payload_end];
    let container = parse_mtx_container(payload).expect("fixture MTX should parse");
    assert_eq!(
        container.num_blocks, 3,
        "fixture should use real multi-block MTX"
    );

    let head = font
        .table(TAG_HEAD)
        .expect("fixture font should contain head")
        .data
        .clone();
    let os2 = font
        .table(TAG_OS_2)
        .expect("fixture font should contain OS/2")
        .data
        .clone();
    let name = font
        .table(TAG_NAME)
        .map(|table| table.data.clone())
        .unwrap_or_default();
    let fntdata_bytes =
        build_eot_file(&head, &os2, &name, payload, true).expect("fixture fntdata should build");
    fs::write(&fntdata_path, fntdata_bytes).expect("fixture fntdata should be writable");

    let cleanup = TempCleanup::new(vec![
        ttf_path.clone(),
        eot_path.clone(),
        fntdata_path.clone(),
    ]);
    (ttf_path, eot_path, fntdata_path, cleanup)
}

struct TempCleanup {
    paths: Vec<PathBuf>,
}

impl TempCleanup {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

#[test]
fn subset_wingdings3_eot_glyph_ids_rebuilds_core_tables_and_drops_warning_tables() {
    let output_path = temp_file("eot");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");
    let (ttf_input_path, eot_input_path, _fntdata_path, _cleanup) =
        prepare_real_subset_wrapper_fixture();

    let output = run_subset_command(
        &[
            "subset",
            eot_input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,36,37,38",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected subset to succeed for supported EOT input, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: unsupported HDMX in subset path; dropping table"),
        "expected HDMX warning, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "real Rust encode already drops VDMX before subset, stderr: {stderr}"
    );
    assert!(
        output_path.is_file(),
        "expected subset to create an output file"
    );

    let decoded_path = decode_subset_output(&output_path);
    assert_subset_rebuilds_core_tables(&ttf_input_path, &decoded_path);

    let font = read_font(&decoded_path);
    assert!(font.table(TAG_HDMX).is_none(), "hdmx should be dropped");
    assert!(font.table(TAG_VDMX).is_none(), "VDMX should be dropped");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}

#[test]
fn subset_wingdings3_fntdata_glyph_ids_rebuilds_core_tables_and_drops_warning_tables() {
    let (ttf_input_path, _eot_path, fntdata_path, _cleanup) = prepare_real_subset_wrapper_fixture();
    let output_path = temp_file("fntdata");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");

    let output = run_subset_command(
        &[
            "subset",
            fntdata_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,36,37,38",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected subset to succeed for supported fntdata input, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: unsupported HDMX in subset path; dropping table"),
        "expected HDMX warning, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "real Rust encode already drops VDMX before subset, stderr: {stderr}"
    );
    assert!(
        output_path.is_file(),
        "expected subset to create an output file"
    );

    let decoded_path = decode_subset_output(&output_path);
    assert_subset_rebuilds_core_tables(&ttf_input_path, &decoded_path);

    let font = read_font(&decoded_path);
    assert!(font.table(TAG_HDMX).is_none(), "hdmx should be dropped");
    assert!(font.table(TAG_VDMX).is_none(), "VDMX should be dropped");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}

#[test]
fn subset_opensans_ttf_glyph_ids_rebuilds_core_tables() {
    let output_path = temp_file("eot");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");
    let (ttf_path, _cleanup) = prepare_plain_subset_ttf_fixture();

    let output = run_subset_command(
        &[
            "subset",
            ttf_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,36,37,38",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected subset to succeed for supported TTF input, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: unsupported HDMX in subset path; dropping table"),
        "expected HDMX warning, stderr: {stderr}"
    );
    assert!(
        stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "expected VDMX warning, stderr: {stderr}"
    );
    assert!(
        output_path.is_file(),
        "expected subset to create an output file"
    );

    let decoded_path = decode_subset_output(&output_path);
    assert_subset_rebuilds_core_tables(&ttf_path, &decoded_path);
    let font = read_font(&decoded_path);
    assert!(
        font.table(TAG_HDMX).is_none(),
        "OpenSans output should not add hdmx"
    );
    assert!(
        font.table(TAG_VDMX).is_none(),
        "OpenSans output should not add VDMX"
    );

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
    let _ = fs::remove_file(ttf_path);
}

#[test]
fn subset_static_cff_text_input_emits_a_legal_static_cff_font() {
    let input_path = workspace_root().join("testdata/cff-static.otf");
    let output_path = temp_file("fntdata");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");

    let output = run_subset_command(
        &[
            "subset",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--text",
            ".",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected static CFF subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.is_file(),
        "expected subset to create an output file"
    );

    let decoded_path = decode_subset_output(&output_path);
    support::assert_decoded_otto_static_cff_output(&decoded_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}

#[test]
fn subset_variable_cff2_text_input_materializes_static_cff_output() {
    let input_path = workspace_root().join("testdata/cff2-variable.otf");
    let output_path = temp_file("fntdata");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");

    let output = run_subset_command(
        &[
            "subset",
            input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            "--text",
            "ABC",
            "--variation",
            "wght=700",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected variable CFF2 subset to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_path.is_file(),
        "expected subset to create an output file"
    );

    let decoded_path = decode_subset_output(&output_path);
    support::assert_decoded_otto_static_cff_output(&decoded_path);

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}

#[test]
fn subset_can_emit_raw_sfnt_v2_output() {
    let source_path = workspace_root().join("testdata/OpenSans-Regular.ttf");
    let output_path = temp_file("eot");
    let _cleanup = TempCleanup::new(vec![output_path.clone()]);

    let output = run_subset_command(
        &[
            "subset",
            source_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--glyph-ids",
            "0,36,37,38",
            "--payload-format",
            "sfnt",
            "--eot-version",
            "v2",
        ],
        &workspace_root(),
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&output_path).unwrap();
    let header = parse_eot_header(&bytes).unwrap();
    assert_eq!(header.version, 0x0002_0002);
    let payload = read_embedded_payload_bytes(&bytes);
    load_sfnt(&payload).expect("raw subset payload should be a valid SFNT");
}

#[test]
fn subset_fntdata_output_defaults_to_xor_on() {
    let source_path = workspace_root().join("testdata/OpenSans-Regular.ttf");
    let output_path = temp_file("fntdata");
    let _cleanup = TempCleanup::new(vec![output_path.clone()]);

    let output = run_subset_command(
        &[
            "subset",
            source_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "--glyph-ids",
            "0,36,37,38",
        ],
        &workspace_root(),
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&output_path).unwrap();
    let header = parse_eot_header(&bytes).unwrap();
    assert_ne!(header.flags & 0x1000_0000, 0);
    let payload = read_embedded_payload_bytes(&bytes);
    parse_mtx_container(&payload)
        .expect("default fntdata payload should remain an XOR-decoded MTX container");
}

#[test]
fn subset_supports_all_embedded_output_mode_combinations() {
    let source_path = workspace_root().join("testdata/OpenSans-Regular.ttf");
    let cases = [
        ("sfnt", "off", "v1", 0x0002_0001, false),
        ("sfnt", "off", "v2", 0x0002_0002, false),
        ("sfnt", "on", "v1", 0x0002_0001, true),
        ("sfnt", "on", "v2", 0x0002_0002, true),
        ("mtx", "off", "v1", 0x0002_0001, false),
        ("mtx", "off", "v2", 0x0002_0002, false),
        ("mtx", "on", "v1", 0x0002_0001, true),
        ("mtx", "on", "v2", 0x0002_0002, true),
    ];

    for (index, (payload_format, xor, eot_version, expected_version, expect_xor)) in
        cases.into_iter().enumerate()
    {
        let output_path = temp_derived_file(&format!("mode-{index}"), "eot");
        let _cleanup = TempCleanup::new(vec![output_path.clone()]);
        let output = run_subset_command(
            &[
                "subset",
                source_path.to_str().unwrap(),
                output_path.to_str().unwrap(),
                "--glyph-ids",
                "0,36,37,38",
                "--payload-format",
                payload_format,
                "--xor",
                xor,
                "--eot-version",
                eot_version,
            ],
            &workspace_root(),
        );

        assert!(
            output.status.success(),
            "case {payload_format}/{xor}/{eot_version} failed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let bytes = fs::read(&output_path).unwrap();
        let header = parse_eot_header(&bytes).unwrap();
        assert_eq!(header.version, expected_version);
        if expect_xor {
            assert_ne!(header.flags & 0x1000_0000, 0);
        } else {
            assert_eq!(header.flags & 0x1000_0000, 0);
        }

        let payload = read_embedded_payload_bytes(&bytes);
        match payload_format {
            "sfnt" => {
                load_sfnt(&payload).expect("raw subset payload should be a valid SFNT");
            }
            "mtx" => {
                parse_mtx_container(&payload)
                    .expect("subset payload should be a valid MTX container");
            }
            other => panic!("unexpected payload format {other}"),
        }
    }
}
