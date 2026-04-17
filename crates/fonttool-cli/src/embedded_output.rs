use std::path::Path;

use fonttool_eot::{build_eot_file, EotBuildOptions, EotVersion};
use fonttool_mtx::{compress_lz, pack_mtx_container_with_copy_dist, MTX_PRELOAD_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedPayloadFormat {
    Mtx,
    Sfnt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedXorMode {
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedEotVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedOutputOptions {
    pub payload_format: EmbeddedPayloadFormat,
    pub xor_mode: EmbeddedXorMode,
    pub eot_version: EmbeddedEotVersion,
}

impl Default for EmbeddedOutputOptions {
    fn default() -> Self {
        Self {
            payload_format: EmbeddedPayloadFormat::Mtx,
            xor_mode: EmbeddedXorMode::Off,
            eot_version: EmbeddedEotVersion::V2,
        }
    }
}

pub fn embedded_output_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "eot" | "fntdata"))
}

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedMtxExtraBlocks<'a> {
    pub block2: Option<&'a [u8]>,
    pub block3: Option<&'a [u8]>,
}

pub fn build_embedded_output(
    head_table: &[u8],
    os2_table: &[u8],
    name_table: &[u8],
    sfnt_payload: &[u8],
    mtx_extra_blocks: Option<EmbeddedMtxExtraBlocks<'_>>,
    options: EmbeddedOutputOptions,
) -> Result<Vec<u8>, String> {
    let payload = match options.payload_format {
        EmbeddedPayloadFormat::Mtx => build_mtx_payload(sfnt_payload, mtx_extra_blocks)?,
        EmbeddedPayloadFormat::Sfnt => sfnt_payload.to_vec(),
    };

    build_eot_file(
        head_table,
        os2_table,
        name_table,
        &payload,
        EotBuildOptions {
            version: options.eot_version.into(),
            apply_ppt_xor: options.xor_mode.into(),
        },
    )
    .map_err(|error| format!("failed to build EOT header: {error}"))
}

fn build_mtx_payload(
    block1_sfnt: &[u8],
    extra_blocks: Option<EmbeddedMtxExtraBlocks<'_>>,
) -> Result<Vec<u8>, String> {
    let extra_blocks = extra_blocks.unwrap_or(EmbeddedMtxExtraBlocks {
        block2: None,
        block3: None,
    });
    let copy_dist = block1_sfnt
        .len()
        .max(extra_blocks.block2.map_or(0, |block| block.len()))
        .max(extra_blocks.block3.map_or(0, |block| block.len()))
        + MTX_PRELOAD_SIZE;
    let block1 = compress_lz(block1_sfnt)
        .map_err(|error| format!("failed to compress MTX block1: {error}"))?;
    let block2 = extra_blocks
        .block2
        .map(|block| {
            compress_lz(block).map_err(|error| format!("failed to compress MTX block2: {error}"))
        })
        .transpose()?;
    let block3 = extra_blocks
        .block3
        .map(|block| {
            compress_lz(block).map_err(|error| format!("failed to compress MTX block3: {error}"))
        })
        .transpose()?;

    pack_mtx_container_with_copy_dist(
        &block1,
        block2.as_deref(),
        block3.as_deref(),
        Some(copy_dist),
    )
    .map_err(|error| format!("failed to pack MTX container: {error}"))
}

impl From<EmbeddedXorMode> for bool {
    fn from(value: EmbeddedXorMode) -> Self {
        matches!(value, EmbeddedXorMode::On)
    }
}

impl From<EmbeddedEotVersion> for EotVersion {
    fn from(value: EmbeddedEotVersion) -> Self {
        match value {
            EmbeddedEotVersion::V1 => EotVersion::V1,
            EmbeddedEotVersion::V2 => EotVersion::V2,
        }
    }
}
