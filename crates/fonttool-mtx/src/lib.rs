//! MTX container parsing and block modeling.

use core::fmt;

mod cvt;
mod hdmx;
mod lz;

pub const MTX_HEADER_SIZE: usize = 10;
pub const MTX_PRELOAD_SIZE: usize = 7168;

pub use cvt::{cvt_decode, cvt_encode, CvtCodecError};
pub use hdmx::{hdmx_decode, hdmx_encode, HdmxCodecError};
pub use lz::{
    analyze_lz, compress_lz, compress_lz_literals, decompress_lz, decompress_lz_with_limit,
    LzAnalysis, LzDecompressError,
};

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
pub enum MtxPackError {
    MissingBlock1,
    PayloadTooLarge,
}

impl fmt::Display for MtxPackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtxPackError::MissingBlock1 => f.write_str("mtx block1 is required"),
            MtxPackError::PayloadTooLarge => f.write_str("mtx payload is too large"),
        }
    }
}

impl std::error::Error for MtxPackError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtxContainer<'a> {
    pub num_blocks: u8,
    // This is the MTX header's Copy Limit field. For compatibility we treat it
    // as a Java/sfntly-style safe upper bound rather than trying to encode the
    // smallest workable back-reference span for a specific payload.
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

#[must_use]
pub fn pack_mtx_container(
    block1: &[u8],
    block2: Option<&[u8]>,
    block3: Option<&[u8]>,
) -> Result<Vec<u8>, MtxPackError> {
    pack_mtx_container_with_copy_dist(block1, block2, block3, None)
}

#[must_use]
pub fn pack_mtx_container_with_copy_dist(
    block1: &[u8],
    block2: Option<&[u8]>,
    block3: Option<&[u8]>,
    copy_dist: Option<usize>,
) -> Result<Vec<u8>, MtxPackError> {
    if block1.is_empty() {
        return Err(MtxPackError::MissingBlock1);
    }

    let block2 = block2.unwrap_or(&[]);
    let block3 = block3.unwrap_or(&[]);
    let num_blocks = 1 + u8::from(!block2.is_empty()) + u8::from(!block3.is_empty());

    let total_size = MTX_HEADER_SIZE
        .checked_add(block1.len())
        .and_then(|size| size.checked_add(block2.len()))
        .and_then(|size| size.checked_add(block3.len()))
        .ok_or(MtxPackError::PayloadTooLarge)?;

    let computed_copy_dist = MTX_PRELOAD_SIZE
        .checked_add(block1.len().max(block2.len()).max(block3.len()))
        .ok_or(MtxPackError::PayloadTooLarge)?;
    let copy_dist = copy_dist.unwrap_or(computed_copy_dist);
    let offset_block2 = MTX_HEADER_SIZE
        .checked_add(block1.len())
        .ok_or(MtxPackError::PayloadTooLarge)?;
    let offset_block3 = offset_block2
        .checked_add(block2.len())
        .ok_or(MtxPackError::PayloadTooLarge)?;

    if copy_dist > 0x00FF_FFFF || offset_block2 > 0x00FF_FFFF || offset_block3 > 0x00FF_FFFF {
        return Err(MtxPackError::PayloadTooLarge);
    }

    let mut data = Vec::with_capacity(total_size);
    data.push(num_blocks);
    write_u24_be(
        u32::try_from(copy_dist).map_err(|_| MtxPackError::PayloadTooLarge)?,
        &mut data,
    );
    write_u24_be(
        u32::try_from(offset_block2).map_err(|_| MtxPackError::PayloadTooLarge)?,
        &mut data,
    );
    write_u24_be(
        u32::try_from(offset_block3).map_err(|_| MtxPackError::PayloadTooLarge)?,
        &mut data,
    );
    data.extend_from_slice(block1);
    data.extend_from_slice(block2);
    data.extend_from_slice(block3);
    Ok(data)
}

fn read_u24_be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]])
}

fn write_u24_be(value: u32, out: &mut Vec<u8>) {
    let bytes = value.to_be_bytes();
    out.extend_from_slice(&bytes[1..4]);
}
