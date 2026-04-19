use std::{fs, path::PathBuf};

use fonttool_cff::{inspect_otf_font, instantiate_variable_cff2, parse_variation_axes, CffError};
use fonttool_sfnt::{load_sfnt, SFNT_VERSION_OTTO};

const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");
const TAG_BASE: u32 = u32::from_be_bytes(*b"BASE");
const TAG_GDEF: u32 = u32::from_be_bytes(*b"GDEF");
const TAG_GPOS: u32 = u32::from_be_bytes(*b"GPOS");
const TAG_GSUB: u32 = u32::from_be_bytes(*b"GSUB");
const TAG_STAT: u32 = u32::from_be_bytes(*b"STAT");
const TAG_VORG: u32 = u32::from_be_bytes(*b"VORG");
const TAG_VHEA: u32 = u32::from_be_bytes(*b"vhea");
const TAG_VMTX: u32 = u32::from_be_bytes(*b"vmtx");

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

#[test]
fn parse_variation_axes_accepts_multiple_axes() {
    let axes = parse_variation_axes("wght=700,wdth=85").expect("axis list should parse");

    assert_eq!(axes.len(), 2);
    assert_eq!(axes[0].tag, *b"wght");
    assert_eq!(axes[0].value, 700.0);
    assert_eq!(axes[1].tag, *b"wdth");
    assert_eq!(axes[1].value, 85.0);
}

#[test]
fn instantiate_variable_cff2_materializes_a_static_font() {
    let bytes =
        fs::read(fixture_path("cff2-variable.otf")).expect("variable CFF2 fixture should load");
    let axes = parse_variation_axes("wght=700").expect("axis list should parse");

    let instantiated =
        instantiate_variable_cff2(&bytes, &axes).expect("variable CFF2 should instantiate");

    let source_font = load_sfnt(&bytes).expect("source font should parse");
    let font = load_sfnt(&instantiated).expect("instantiated font should parse");
    let kind = inspect_otf_font(&instantiated).expect("instantiated font should inspect");

    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);
    assert!(font.table(TAG_CFF).is_some());
    assert!(font.table(TAG_CFF2).is_none());
    assert!(font.table(TAG_FVAR).is_none());
    assert!(kind.is_cff_flavor);
    assert!(!kind.is_variable);

    let preserved_tags = [
        TAG_BASE, TAG_GDEF, TAG_GPOS, TAG_GSUB, TAG_STAT, TAG_VHEA, TAG_VMTX, TAG_VORG,
    ]
    .into_iter()
    .filter(|tag| source_font.table(*tag).is_some())
    .collect::<Vec<_>>();

    assert!(
        !preserved_tags.is_empty(),
        "fixture should contain at least one representative preserved table"
    );

    for tag in preserved_tags {
        assert!(
            font.table(tag).is_some(),
            "instantiated font should preserve table {tag:08x}"
        );
    }
}

#[test]
fn instantiate_variable_cff2_rejects_static_input() {
    let bytes = fs::read(fixture_path("cff-static.otf")).expect("static CFF fixture should load");
    let axes = parse_variation_axes("wght=700").expect("axis list should parse");

    let error =
        instantiate_variable_cff2(&bytes, &axes).expect_err("static CFF should be rejected");

    assert_eq!(error, CffError::VariationRejectedForStaticInput);
}

#[test]
fn instantiate_variable_cff2_rejects_unknown_axis_tags() {
    let bytes =
        fs::read(fixture_path("cff2-variable.otf")).expect("variable CFF2 fixture should load");
    let axes = parse_variation_axes("wght=700,opsz=12").expect("axis list should parse");

    let error = instantiate_variable_cff2(&bytes, &axes)
        .expect_err("unknown axis tags should not be ignored");

    assert_eq!(
        error,
        CffError::InvalidVariationAxis("unknown variation axis tag `opsz`".to_string())
    );
}
