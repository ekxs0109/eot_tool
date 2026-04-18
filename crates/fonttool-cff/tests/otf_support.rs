use std::{fs, path::PathBuf};

use fonttool_cff::{inspect_otf_font, parse_variation_axes};

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
    assert_eq!(axes[1].tag, *b"wdth");
}
