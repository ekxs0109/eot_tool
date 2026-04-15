mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::build_eot_file;
use fonttool_mtx::{compress_lz_literals, pack_mtx_container};
use fonttool_sfnt::{load_sfnt, serialize_sfnt};

const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_HDMX: u32 = u32::from_be_bytes(*b"hdmx");
const TAG_VDMX: u32 = u32::from_be_bytes(*b"VDMX");

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

fn read_loaded_font(path: &Path) -> fonttool_sfnt::OwnedSfntFont {
    let bytes = fs::read(path).expect("decoded subset output should be readable");
    load_sfnt(&bytes).expect("decoded subset output should parse")
}

fn read_maxp_num_glyphs(path: &Path) -> u16 {
    let font = read_loaded_font(path);
    let maxp = font.table(TAG_MAXP).expect("subset output should keep maxp");
    u16::from_be_bytes([maxp.data[4], maxp.data[5]])
}

fn assert_supported_subset_core_tables(path: &Path) {
    let font = read_loaded_font(path);
    for tag in [TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_GLYF, TAG_LOCA, TAG_CMAP] {
        assert!(
            font.table(tag).is_some(),
            "expected table {:08X} in subset output",
            tag
        );
    }
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
    let font = build_subset_fixture_font(false);
    fs::write(&ttf_path, serialize_sfnt(&font).expect("fixture font should serialize"))
        .expect("fixture ttf should be writable");

    let cleanup = TempCleanup::new(vec![ttf_path.clone()]);
    (ttf_path, cleanup)
}

fn prepare_supported_subset_wrapper_fixture() -> (PathBuf, PathBuf, TempCleanup) {
    let ttf_path = temp_derived_file("fixture", "ttf");
    let eot_path = temp_derived_file("fixture", "eot");
    let fntdata_path = temp_derived_file("fixture", "fntdata");
    let font = build_subset_fixture_font(true);
    let subset_bytes = serialize_sfnt(&font).expect("fixture font should serialize");
    fs::write(&ttf_path, &subset_bytes).expect("fixture ttf should be writable");

    let head = font
        .table(TAG_HEAD)
        .expect("fixture font should contain head")
        .data
        .clone();
    let os2 = font
        .table(u32::from_be_bytes(*b"OS/2"))
        .expect("fixture font should contain OS/2")
        .data
        .clone();
    let block1 = compress_lz_literals(&subset_bytes).expect("fixture block1 should compress");
    let mtx_payload = pack_mtx_container(&block1, None, None).expect("fixture MTX should pack");
    let eot_bytes = build_eot_file(&head, &os2, &mtx_payload, false)
        .expect("fixture EOT should build");
    fs::write(&eot_path, eot_bytes).expect("fixture eot should be writable");
    fs::copy(&eot_path, &fntdata_path).expect("fixture fntdata copy should be writable");

    let cleanup = TempCleanup::new(vec![ttf_path, eot_path.clone(), fntdata_path.clone()]);
    (eot_path, fntdata_path, cleanup)
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
fn subset_wingdings3_eot_glyph_ids_updates_maxp_and_drops_warning_tables() {
    let output_path = temp_file("eot");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");
    let (eot_input_path, _fntdata_path, _cleanup) = prepare_supported_subset_wrapper_fixture();

    let output = run_subset_command(
        &[
            "subset",
            eot_input_path
                .to_str()
                .expect("fixture path should be valid utf-8"),
            output_path.to_str().expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,1,2",
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
        stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "expected VDMX warning, stderr: {stderr}"
    );
    assert!(output_path.is_file(), "expected subset to create an output file");

    let decoded_path = decode_subset_output(&output_path);
    assert_eq!(read_maxp_num_glyphs(&decoded_path), 3);
    assert_supported_subset_core_tables(&decoded_path);

    let font = read_loaded_font(&decoded_path);
    assert!(font.table(TAG_HDMX).is_none(), "hdmx should be dropped");
    assert!(font.table(TAG_VDMX).is_none(), "VDMX should be dropped");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
}

#[test]
fn subset_wingdings3_fntdata_glyph_ids_updates_maxp_and_drops_warning_tables() {
    let (ttf_path, fntdata_path, _cleanup) = prepare_supported_subset_wrapper_fixture();
    let output_path = temp_file("fntdata");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");

    let output = run_subset_command(
        &[
            "subset",
            fntdata_path
                .to_str()
                .expect("temp path should be valid utf-8"),
            output_path.to_str().expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,1,2",
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
        stderr.contains("warning: unsupported VDMX in MTX encode/subset path; dropping table"),
        "expected VDMX warning, stderr: {stderr}"
    );
    assert!(output_path.is_file(), "expected subset to create an output file");

    let decoded_path = decode_subset_output(&output_path);
    assert_eq!(read_maxp_num_glyphs(&decoded_path), 3);
    assert_supported_subset_core_tables(&decoded_path);

    let font = read_loaded_font(&decoded_path);
    assert!(font.table(TAG_HDMX).is_none(), "hdmx should be dropped");
    assert!(font.table(TAG_VDMX).is_none(), "VDMX should be dropped");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
    let _ = fs::remove_file(ttf_path);
}

#[test]
fn subset_opensans_ttf_glyph_ids_updates_maxp_and_keeps_core_tables() {
    let output_path = temp_file("eot");
    let isolated_cwd = temp_file("cwd");
    fs::create_dir_all(&isolated_cwd).expect("isolated cwd should be creatable");
    let (ttf_path, _cleanup) = prepare_plain_subset_ttf_fixture();

    let output = run_subset_command(
        &[
            "subset",
            ttf_path.to_str().expect("fixture path should be valid utf-8"),
            output_path.to_str().expect("temp path should be valid utf-8"),
            "--glyph-ids",
            "0,1,2",
        ],
        &isolated_cwd,
    );

    assert!(
        output.status.success(),
        "expected subset to succeed for supported TTF input, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning: unsupported HDMX"), "stderr: {stderr}");
    assert!(!stderr.contains("warning: unsupported VDMX"), "stderr: {stderr}");
    assert!(output_path.is_file(), "expected subset to create an output file");

    let decoded_path = decode_subset_output(&output_path);
    assert_eq!(read_maxp_num_glyphs(&decoded_path), 3);
    assert_supported_subset_core_tables(&decoded_path);
    let font = read_loaded_font(&decoded_path);
    assert!(font.table(TAG_HDMX).is_none(), "OpenSans output should not add hdmx");
    assert!(font.table(TAG_VDMX).is_none(), "OpenSans output should not add VDMX");

    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file(decoded_path);
    let _ = fs::remove_dir_all(isolated_cwd);
    let _ = fs::remove_file(ttf_path);
}
