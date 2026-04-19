use std::{fs, path::PathBuf};

use fonttool_cff::convert_otf_to_ttf;
use fonttool_sfnt::{load_sfnt, SFNT_VERSION_TRUETYPE};

const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

#[test]
fn convert_materialized_woff_cff_source_to_truetype() {
    let bytes = fs::read(fixture_path("cff-static.woff")).expect("WOFF CFF fixture should load");

    let converted = convert_otf_to_ttf(&bytes, &[]).expect("WOFF CFF convert should succeed");
    let font = load_sfnt(&converted).expect("converted bytes should parse as sfnt");

    assert_eq!(font.version_tag(), SFNT_VERSION_TRUETYPE);
    assert!(font.table(TAG_GLYF).is_some());
    assert!(font.table(TAG_LOCA).is_some());
}
