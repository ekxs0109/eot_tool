use std::{fs, path::PathBuf};

use fonttool_cff::{
    inspect_otf_font, parse_variation_axes, subset_static_cff, subset_variable_cff2, CffError,
};
use fonttool_sfnt::load_sfnt;

const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

#[test]
fn inspect_static_cff_otf_reports_cff_and_not_variable() {
    let bytes = fs::read(fixture_path("cff-static.otf")).expect("static CFF fixture should load");

    let kind = inspect_otf_font(&bytes).expect("static fixture should inspect");

    assert!(kind.is_cff_flavor);
    assert!(!kind.is_variable);
}

#[test]
fn inspect_variable_cff2_otf_reports_cff_and_variable() {
    let bytes =
        fs::read(fixture_path("cff2-variable.otf")).expect("variable CFF2 fixture should load");

    let kind = inspect_otf_font(&bytes).expect("variable fixture should inspect");

    assert!(kind.is_cff_flavor);
    assert!(kind.is_variable);
}

#[test]
fn parse_variation_axis_list() {
    let axes = parse_variation_axes("wght=700,wdth=85").expect("axis list should parse");

    assert_eq!(axes.len(), 2);
    assert_eq!(axes[0].tag, *b"wght");
    assert_eq!(axes[0].value, 700.0);
    assert_eq!(axes[1].tag, *b"wdth");
    assert_eq!(axes[1].value, 85.0);
}

#[test]
fn rejects_variation_axis_segment_without_separator() {
    let error = parse_variation_axes("wght700").expect_err("segment without `=` should fail");

    assert_eq!(
        error,
        CffError::InvalidVariationAxis("invalid variation axis segment `wght700`".to_string())
    );
}

#[test]
fn rejects_variation_axis_tag_with_wrong_length() {
    let error = parse_variation_axes("weight=700").expect_err("tag must be exactly four bytes");

    assert_eq!(
        error,
        CffError::InvalidVariationAxis(
            "variation axis tag `weight` must be exactly 4 bytes".to_string()
        )
    );
}

#[test]
fn rejects_variation_axis_value_with_invalid_float() {
    let error = parse_variation_axes("wght=heavy").expect_err("invalid float value should fail");

    assert_eq!(
        error,
        CffError::InvalidVariationAxis(
            "variation axis value `heavy` is not a valid float".to_string()
        )
    );
}

#[test]
fn subset_static_cff_returns_legal_otf_subset() {
    let bytes = fs::read(fixture_path("cff-static.otf")).expect("static CFF fixture should load");

    let subset = subset_static_cff(&bytes, ".").expect("static CFF subset should succeed");
    let font = load_sfnt(&subset.sfnt_bytes).expect("subset output should parse as sfnt");

    assert!(font.table(TAG_CFF).is_some());
    assert!(font.table(TAG_CMAP).is_some());
    assert!(font.table(TAG_CFF2).is_none());
    assert!(font.table(TAG_FVAR).is_none());
}

#[test]
fn subset_variable_cff2_materializes_and_returns_legal_otf_subset() {
    let bytes =
        fs::read(fixture_path("cff2-variable.otf")).expect("variable CFF2 fixture should load");
    let axes = parse_variation_axes("wght=700").expect("axis list should parse");

    let subset =
        subset_variable_cff2(&bytes, "ABC", &axes).expect("variable CFF2 subset should succeed");
    let font = load_sfnt(&subset.sfnt_bytes).expect("subset output should parse as sfnt");

    assert!(font.table(TAG_CFF).is_some());
    assert!(font.table(TAG_CMAP).is_some());
    assert!(font.table(TAG_CFF2).is_none());
    assert!(font.table(TAG_FVAR).is_none());
}
