use std::{fs, path::PathBuf};

use fonttool_cff::{
    inspect_otf_font, parse_variation_axes, subset_static_cff, subset_variable_cff2, CffError,
};

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
    let error =
        parse_variation_axes("wght=heavy").expect_err("invalid float value should fail");

    assert_eq!(
        error,
        CffError::InvalidVariationAxis(
            "variation axis value `heavy` is not a valid float".to_string()
        )
    );
}

#[test]
fn subset_scaffolds_return_expected_placeholder_errors() {
    let static_error =
        subset_static_cff(&[], "abc").expect_err("static subset scaffold should not succeed");
    let variable_error = subset_variable_cff2(&[], "abc", &[])
        .expect_err("variable subset scaffold should not succeed");

    assert_eq!(
        static_error,
        CffError::SubsetFailed("static CFF subset not implemented yet".to_string())
    );
    assert_eq!(
        variable_error,
        CffError::SubsetFailed("variable CFF2 subset not implemented yet".to_string())
    );
}
