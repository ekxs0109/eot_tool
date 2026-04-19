use std::{ffi::OsStr, fs, path::PathBuf};

use fonttool_cff::{inspect_otf_font, load_font_source};
use fonttool_sfnt::{load_sfnt, SFNT_VERSION_OTTO, SFNT_VERSION_TRUETYPE};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn shared_repo_root() -> Option<PathBuf> {
    let workspace = workspace_root();
    let worktrees_dir = workspace.parent()?;
    if worktrees_dir.file_name() != Some(OsStr::new(".worktrees")) {
        return None;
    }

    Some(worktrees_dir.parent()?.to_path_buf())
}

fn fixture_path(name: &str) -> PathBuf {
    let workspace_path = workspace_root().join("testdata").join(name);
    if workspace_path.exists() {
        return workspace_path;
    }

    if let Some(shared_root) = shared_repo_root() {
        let shared_path = shared_root.join("testdata").join(name);
        if shared_path.exists() {
            return shared_path;
        }
    }

    workspace_path
}

#[test]
fn load_font_source_leaves_raw_otf_bytes_unchanged() {
    let bytes = fs::read(fixture_path("cff-static.otf")).expect("raw OTF fixture should load");

    let materialized = load_font_source(&bytes).expect("raw OTF source should load");
    let font = load_sfnt(&materialized).expect("materialized OTF should parse");

    assert_eq!(materialized, bytes);
    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);
}

#[test]
fn load_font_source_materializes_woff_to_truetype_sfnt() {
    let fixture = fixture_path("OpenSans-Regular.woff");
    if !fixture.exists() {
        eprintln!("skipping: {} is not present yet", fixture.display());
        return;
    }

    let bytes = fs::read(&fixture).expect("WOFF fixture should load");
    let materialized = load_font_source(&bytes).expect("WOFF source should materialize");
    let font = load_sfnt(&materialized).expect("materialized WOFF should parse");

    assert_eq!(font.version_tag(), SFNT_VERSION_TRUETYPE);
}

#[test]
fn load_font_source_materializes_woff2_to_truetype_sfnt() {
    let fixture = fixture_path("OpenSans-Regular.woff2");
    if !fixture.exists() {
        eprintln!("skipping: {} is not present yet", fixture.display());
        return;
    }

    let bytes = fs::read(&fixture).expect("WOFF2 fixture should load");
    let materialized = load_font_source(&bytes).expect("WOFF2 source should materialize");
    let font = load_sfnt(&materialized).expect("materialized WOFF2 should parse");

    assert_eq!(font.version_tag(), SFNT_VERSION_TRUETYPE);
}

#[test]
fn materialized_otf_source_can_still_be_inspected_as_cff() {
    let bytes = fs::read(fixture_path("cff-static.otf")).expect("raw OTF fixture should load");

    let materialized = load_font_source(&bytes).expect("OTF source should load");
    let kind = inspect_otf_font(&materialized).expect("materialized OTF should inspect");

    assert!(kind.is_cff_flavor);
    assert!(!kind.is_variable);
}
