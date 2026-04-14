//! MTX container parsing and block modeling.

use core::fmt;

mod lz;

pub const MTX_HEADER_SIZE: usize = 10;

pub use lz::{decompress_lz, LzDecompressError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtxContainerError {
    Truncated,
    InvalidMetadata,
}

impl fmt::Display for MtxContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtxContainerError::Truncated => f.write_str("MTX container is truncated"),
            MtxContainerError::InvalidMetadata => f.write_str("invalid MTX metadata"),
        }
    }
}

impl std::error::Error for MtxContainerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtxContainer<'a> {
    pub num_blocks: u8,
    pub copy_dist: u32,
    pub block1: &'a [u8],
    pub block2: Option<&'a [u8]>,
    pub block3: Option<&'a [u8]>,
    offset_block2: usize,
    offset_block3: usize,
}

#[must_use]
pub fn parse_mtx_container<'a>(bytes: &'a [u8]) -> Result<MtxContainer<'a>, MtxContainerError> {
    if bytes.len() < MTX_HEADER_SIZE {
        return Err(MtxContainerError::Truncated);
    }

    let num_blocks = bytes[0];
    let copy_dist = read_u24_be(&bytes[1..4]);
    let offset_block2 = read_u24_be(&bytes[4..7]) as usize;
    let offset_block3 = read_u24_be(&bytes[7..10]) as usize;

    if !(1..=3).contains(&num_blocks) || copy_dist == 0 {
        return Err(MtxContainerError::InvalidMetadata);
    }

    if offset_block2 < MTX_HEADER_SIZE || offset_block2 > bytes.len() {
        return Err(MtxContainerError::InvalidMetadata);
    }

    if num_blocks >= 2 && offset_block2 >= bytes.len() {
        return Err(MtxContainerError::InvalidMetadata);
    }

    if num_blocks >= 3 && (offset_block3 < offset_block2 || offset_block3 >= bytes.len()) {
        return Err(MtxContainerError::InvalidMetadata);
    }

    let block1 = &bytes[MTX_HEADER_SIZE..offset_block2];
    let block2 = if num_blocks >= 2 {
        let end = if num_blocks >= 3 {
            offset_block3
        } else {
            bytes.len()
        };
        Some(&bytes[offset_block2..end])
    } else {
        None
    };
    let block3 = if num_blocks >= 3 {
        Some(&bytes[offset_block3..bytes.len()])
    } else {
        None
    };

    Ok(MtxContainer {
        num_blocks,
        copy_dist,
        offset_block2,
        offset_block3,
        block1,
        block2,
        block3,
    })
}

fn read_u24_be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}
