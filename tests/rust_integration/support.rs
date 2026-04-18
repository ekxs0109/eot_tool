use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fonttool_eot::{parse_eot_header, EotHeaderError};
use fonttool_glyf::{decode_glyf, GlyfDecodeError};
use fonttool_mtx::{decompress_lz, parse_mtx_container, LzDecompressError, MtxContainerError};
use fonttool_sfnt::{
    load_sfnt, serialize_sfnt, OwnedSfntFont, ParseError, SerializeError, SFNT_VERSION_OTTO,
};

const EOT_FLAG_PPT_XOR: u32 = 0x1000_0000;
const TAG_BASE: u32 = u32::from_be_bytes(*b"BASE");
const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_CFF2: u32 = u32::from_be_bytes(*b"CFF2");
const TAG_CMAP: u32 = u32::from_be_bytes(*b"cmap");
const TAG_DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const TAG_FVAR: u32 = u32::from_be_bytes(*b"fvar");
const TAG_GDEF: u32 = u32::from_be_bytes(*b"GDEF");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_GPOS: u32 = u32::from_be_bytes(*b"GPOS");
const TAG_GSUB: u32 = u32::from_be_bytes(*b"GSUB");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_HHEA: u32 = u32::from_be_bytes(*b"hhea");
const TAG_HMTX: u32 = u32::from_be_bytes(*b"hmtx");
const TAG_LOCA: u32 = u32::from_be_bytes(*b"loca");
const TAG_MAXP: u32 = u32::from_be_bytes(*b"maxp");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_OS_2: u32 = u32::from_be_bytes(*b"OS/2");
const TAG_POST: u32 = u32::from_be_bytes(*b"post");
const TAG_VHEA: u32 = u32::from_be_bytes(*b"vhea");
const TAG_VMTX: u32 = u32::from_be_bytes(*b"vmtx");
const TAG_VORG: u32 = u32::from_be_bytes(*b"VORG");
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const GENERIC_STATIC_CFF_TAGS: [u32; 7] = [
    TAG_CFF, TAG_CMAP, TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_OS_2,
];
const OFFICE_STATIC_CFF_TAGS: [u32; 17] = [
    TAG_BASE, TAG_CFF, TAG_DSIG, TAG_GDEF, TAG_GPOS, TAG_GSUB, TAG_OS_2, TAG_VORG, TAG_CMAP,
    TAG_HEAD, TAG_HHEA, TAG_HMTX, TAG_MAXP, TAG_NAME, TAG_POST, TAG_VHEA, TAG_VMTX,
];

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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

pub fn fixture_path(relative: &str) -> PathBuf {
    let workspace_path = workspace_root().join(relative);
    if workspace_path.exists() {
        return workspace_path;
    }

    if let Some(shared_root) = shared_repo_root() {
        let shared_path = shared_root.join(relative);
        if shared_path.exists() {
            return shared_path;
        }
    }

    workspace_path
}

pub fn tracked_testdata_path(relative: &str) -> PathBuf {
    assert!(
        relative.starts_with("testdata/"),
        "tracked_testdata_path only accepts testdata/* paths"
    );

    let workspace_path = workspace_root().join(relative);
    assert!(
        workspace_path.exists(),
        "expected tracked fixture to exist in this worktree: {}",
        workspace_path.display()
    );

    workspace_path
}

#[allow(dead_code)]
pub fn otf_parity_fixture() -> PathBuf {
    for relative in [
        "testdata/aipptfonts/香蕉Plus__20220301185701917366.otf",
        "testdata/20220301185701917366.otf",
    ] {
        let path = fixture_path(relative);
        if path.exists() {
            return path;
        }
    }

    panic!("expected OTF parity fixture to exist in a known testdata location");
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "fonttool-{label}-{}-{unique}-{counter}.{extension}",
        std::process::id(),
    ))
}

#[allow(dead_code)]
pub fn temp_fntdata() -> PathBuf {
    temp_path("otf-convert", "fntdata")
}

#[allow(dead_code)]
pub fn temp_ttf() -> PathBuf {
    temp_path("otf-convert", "ttf")
}

#[allow(dead_code)]
pub fn temp_eot() -> PathBuf {
    temp_path("otf-convert", "eot")
}

#[allow(dead_code)]
pub fn run_fonttool<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_fonttool_in_dir(args, &workspace_root())
}

pub fn run_fonttool_in_dir<I, S>(args: I, current_dir: &Path) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fonttool"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("fonttool binary should launch")
}

pub fn assert_decoded_otto_cff2_variable_output(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert!(
        bytes.len() >= 4,
        "decoded font should contain an sfnt version header"
    );
    assert_eq!(&bytes[..4], b"OTTO");

    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");
    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);
    assert!(
        font.table(TAG_CFF2).is_some(),
        "decoded font should contain CFF2"
    );
    assert!(
        font.table(TAG_FVAR).is_some(),
        "decoded font should contain fvar"
    );
}

pub fn assert_decoded_otto_office_static_cff_output(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert!(
        bytes.len() >= 4,
        "decoded font should contain an sfnt version header"
    );
    assert_eq!(&bytes[..4], b"OTTO");

    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");
    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);

    for tag in OFFICE_STATIC_CFF_TAGS {
        assert!(
            font.table(tag).is_some(),
            "decoded Office static CFF font should contain required table `{}`",
            String::from_utf8_lossy(&tag.to_be_bytes())
        );
    }

    assert!(
        font.table(TAG_CFF2).is_none(),
        "decoded Office static CFF font should not contain CFF2"
    );
    assert!(
        font.table(TAG_FVAR).is_none(),
        "decoded Office static CFF font should not contain fvar"
    );
    assert!(
        font.table(TAG_GLYF).is_none(),
        "decoded Office static CFF font should not contain glyf"
    );
}

pub fn assert_decoded_otto_static_cff_output(path: &Path) {
    let bytes = fs::read(path).expect("decoded font should be readable");
    assert!(
        bytes.len() >= 4,
        "decoded font should contain an sfnt version header"
    );
    assert_eq!(&bytes[..4], b"OTTO");

    let font = load_sfnt(&bytes).expect("decoded font should load as sfnt");
    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);

    for tag in GENERIC_STATIC_CFF_TAGS {
        assert!(
            font.table(tag).is_some(),
            "decoded static CFF font should contain required table `{}`",
            String::from_utf8_lossy(&tag.to_be_bytes())
        );
    }

    assert!(
        font.table(TAG_CFF2).is_none(),
        "decoded static CFF font should not contain CFF2"
    );
    assert!(
        font.table(TAG_FVAR).is_none(),
        "decoded static CFF font should not contain fvar"
    );
    assert!(
        font.table(TAG_GLYF).is_none(),
        "decoded static CFF font should not contain glyf"
    );
}

pub fn assert_decoded_otto_preserves_office_style_static_cff_tables(
    decoded_path: &Path,
    source_bytes: &[u8],
) {
    let decoded_bytes = fs::read(decoded_path).expect("decoded font should be readable");
    let decoded_font = load_sfnt(&decoded_bytes).expect("decoded font should load as sfnt");
    let source_font = load_sfnt(source_bytes).expect("source font should load as sfnt");

    assert_decoded_otto_static_cff_output(decoded_path);

    let preserved_tags = OFFICE_STATIC_CFF_TAGS
        .into_iter()
        .filter(|tag| source_font.table(*tag).is_some())
        .collect::<Vec<_>>();

    assert!(
        !preserved_tags.is_empty(),
        "source font should contain at least one Office-style static CFF table"
    );

    for tag in preserved_tags {
        assert!(
            decoded_font.table(tag).is_some(),
            "decoded static CFF font should preserve Office-style table `{}` when present in the source",
            String::from_utf8_lossy(&tag.to_be_bytes())
        );
    }
}

#[allow(dead_code)]
pub fn run_python<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let workspace = workspace_root();
    let candidate_roots = std::iter::once(workspace.clone()).chain(
        fs::read_dir(workspace.join(".worktrees"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path())),
    );
    let python = candidate_roots
        .map(|root| root.join("build/venv/bin/python"))
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("python3"));

    Command::new(python)
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("python binary should launch")
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RustDecodeError {
    InvalidHeader(EotHeaderError),
    InvalidContainer(MtxContainerError),
    InvalidBlock(String),
    InvalidSfnt(ParseError),
    InvalidGlyf(GlyfDecodeError),
    Serialize(SerializeError),
}

impl std::fmt::Display for RustDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RustDecodeError::InvalidHeader(error) => write!(f, "invalid EOT header: {error}"),
            RustDecodeError::InvalidContainer(error) => {
                write!(f, "invalid MTX container: {error}")
            }
            RustDecodeError::InvalidBlock(message) => f.write_str(message),
            RustDecodeError::InvalidSfnt(error) => write!(f, "invalid SFNT: {error}"),
            RustDecodeError::InvalidGlyf(error) => {
                write!(f, "failed to reconstruct glyf/loca: {error}")
            }
            RustDecodeError::Serialize(error) => {
                write!(f, "failed to serialize reconstructed SFNT: {error}")
            }
        }
    }
}

impl std::error::Error for RustDecodeError {}

struct PreparedEmbeddedFontPayload {
    header_length: usize,
    font_data_size: usize,
    flags: u32,
    payload_bytes: Vec<u8>,
}

fn prepare_embedded_font_payload(
    input_bytes: &[u8],
) -> Result<PreparedEmbeddedFontPayload, RustDecodeError> {
    let header = parse_eot_header(input_bytes).map_err(RustDecodeError::InvalidHeader)?;
    let payload_start = header.header_length as usize;
    let payload_end = payload_start
        .checked_add(header.font_data_size as usize)
        .ok_or_else(|| RustDecodeError::InvalidBlock("invalid EOT payload range".to_string()))?;
    let payload = input_bytes
        .get(payload_start..payload_end)
        .ok_or_else(|| RustDecodeError::InvalidBlock("invalid EOT payload range".to_string()))?;

    let mut payload_bytes = payload.to_vec();
    if header.flags & EOT_FLAG_PPT_XOR != 0 {
        for byte in &mut payload_bytes {
            *byte ^= 0x50;
        }
    }

    Ok(PreparedEmbeddedFontPayload {
        header_length: header.header_length as usize,
        font_data_size: header.font_data_size as usize,
        flags: header.flags,
        payload_bytes,
    })
}

#[allow(dead_code)]
pub fn decode_current_rust_encoded_bytes(input_bytes: &[u8]) -> Result<Vec<u8>, RustDecodeError> {
    let prepared = prepare_embedded_font_payload(input_bytes)?;
    let container =
        parse_mtx_container(&prepared.payload_bytes).map_err(RustDecodeError::InvalidContainer)?;
    let block1 = decode_lz_block(container.block1, "block1")?;
    let block2 = match container.block2 {
        Some(block) => decode_lz_block(block, "block2")?,
        None => Vec::new(),
    };
    let block3 = match container.block3 {
        Some(block) => decode_lz_block(block, "block3")?,
        None => Vec::new(),
    };

    if block2.is_empty() && block3.is_empty() {
        return Ok(block1);
    }
    if block2.is_empty() || block3.is_empty() {
        return Err(RustDecodeError::InvalidBlock(
            "current Rust MTX decode requires both block2 and block3 when extra blocks are present"
                .to_string(),
        ));
    }

    let mut font = load_sfnt(&block1).map_err(RustDecodeError::InvalidSfnt)?;
    let head = table_bytes(&font, TAG_HEAD, "head")?;
    let maxp = table_bytes(&font, TAG_MAXP, "maxp")?;
    if head.len() < 52 {
        return Err(RustDecodeError::InvalidBlock(
            "head table is truncated".to_string(),
        ));
    }
    if maxp.len() < 6 {
        return Err(RustDecodeError::InvalidBlock(
            "maxp table is truncated".to_string(),
        ));
    }

    let index_to_loca_format = i16::from_be_bytes([head[50], head[51]]);
    let num_glyphs = u16::from_be_bytes([maxp[4], maxp[5]]);
    let glyf_stream = table_bytes(&font, TAG_GLYF, "glyf")?;
    let decoded_glyf = decode_glyf(
        glyf_stream,
        &block2,
        &block3,
        index_to_loca_format,
        num_glyphs,
    )
    .map_err(RustDecodeError::InvalidGlyf)?;
    font.remove_table(TAG_GLYF);
    font.remove_table(TAG_LOCA);
    font.add_table(TAG_GLYF, decoded_glyf.glyf_data);
    font.add_table(TAG_LOCA, decoded_glyf.loca_data);

    serialize_sfnt(&font).map_err(RustDecodeError::Serialize)
}

#[allow(dead_code)]
pub fn decode_current_rust_encoded_file(input: &Path, output: &Path) {
    let input_bytes = fs::read(input).expect("encoded font should be readable");
    let decoded = decode_current_rust_encoded_bytes(&input_bytes)
        .expect("current Rust-encoded EOT/MTX should reconstruct to SFNT");
    fs::write(output, decoded).expect("decoded SFNT should be writable");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtxBlockReport {
    pub compressed_len: usize,
    pub decompressed_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedFontReport {
    pub file_size: usize,
    pub header_length: usize,
    pub font_data_size: usize,
    pub flags: u32,
    pub block1: MtxBlockReport,
    pub block2: MtxBlockReport,
    pub block3: MtxBlockReport,
}

#[allow(dead_code)]
pub fn inspect_embedded_font_bytes(
    input_bytes: &[u8],
) -> Result<EmbeddedFontReport, RustDecodeError> {
    let prepared = prepare_embedded_font_payload(input_bytes)?;
    let container =
        parse_mtx_container(&prepared.payload_bytes).map_err(RustDecodeError::InvalidContainer)?;

    let block1 = decode_lz_block(container.block1, "block1")?;
    let block2 = match container.block2 {
        Some(block) => decode_lz_block(block, "block2")?,
        None => Vec::new(),
    };
    let block3 = match container.block3 {
        Some(block) => decode_lz_block(block, "block3")?,
        None => Vec::new(),
    };

    Ok(EmbeddedFontReport {
        file_size: input_bytes.len(),
        header_length: prepared.header_length,
        font_data_size: prepared.font_data_size,
        flags: prepared.flags,
        block1: MtxBlockReport {
            compressed_len: container.block1.len(),
            decompressed_len: block1.len(),
        },
        block2: MtxBlockReport {
            compressed_len: container.block2.map_or(0, |block| block.len()),
            decompressed_len: block2.len(),
        },
        block3: MtxBlockReport {
            compressed_len: container.block3.map_or(0, |block| block.len()),
            decompressed_len: block3.len(),
        },
    })
}

#[allow(dead_code)]
pub fn inspect_embedded_font_file(path: &Path) -> EmbeddedFontReport {
    let bytes = fs::read(path).expect("embedded font should be readable");
    inspect_embedded_font_bytes(&bytes).expect("embedded font should parse")
}

fn decode_lz_block(block: &[u8], label: &str) -> Result<Vec<u8>, RustDecodeError> {
    decompress_lz(block).map_err(|error| {
        RustDecodeError::InvalidBlock(format!(
            "failed to decode MTX {label}: {}",
            display_lz_error(error)
        ))
    })
}

fn display_lz_error(error: LzDecompressError) -> String {
    error.to_string()
}

fn table_bytes<'a>(
    font: &'a OwnedSfntFont,
    tag: u32,
    name: &str,
) -> Result<&'a [u8], RustDecodeError> {
    font.table(tag)
        .map(|table| table.data.as_slice())
        .ok_or_else(|| RustDecodeError::InvalidBlock(format!("required table `{name}` is missing")))
}

#[allow(dead_code)]
pub fn save_ttf_with_fonttools(input: &Path, output: &Path) {
    let save = run_python([
        OsStr::new("-c"),
        OsStr::new(
            "from fontTools.ttLib import TTFont; import sys; \
             font = TTFont(sys.argv[1]); font.save(sys.argv[2]); font.close()",
        ),
        input.as_os_str(),
        output.as_os_str(),
    ]);

    assert!(
        save.status.success(),
        "expected fonttools save to succeed, stderr: {}",
        String::from_utf8_lossy(&save.stderr)
    );
}

#[allow(dead_code)]
pub fn run_fonttools_parity(left: &Path, right: &Path) -> Output {
    run_python([
        OsStr::new("tests/test_fonttools_parity.py"),
        left.as_os_str(),
        right.as_os_str(),
    ])
}
