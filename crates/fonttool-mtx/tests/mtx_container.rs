use fonttool_mtx::{parse_mtx_container, MtxContainerError};

fn fixture_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 18];
    bytes[0] = 3;
    bytes[1..4].copy_from_slice(&0x000100u32.to_be_bytes()[1..4]);
    bytes[4..7].copy_from_slice(&12u32.to_be_bytes()[1..4]);
    bytes[7..10].copy_from_slice(&15u32.to_be_bytes()[1..4]);
    bytes[10..12].copy_from_slice(&[0xaa, 0xbb]);
    bytes[12..15].copy_from_slice(&[0xcc, 0xdd, 0xee]);
    bytes[15..18].copy_from_slice(&[0xff, 0x11, 0x22]);
    bytes
}

#[test]
fn parses_mtx_container_block_slices() {
    let bytes = fixture_bytes();
    let container = parse_mtx_container(&bytes).unwrap();

    assert_eq!(container.num_blocks, 3);
    assert_eq!(container.copy_dist, 0x000100);
    assert_eq!(container.block1, &[0xaa, 0xbb]);
    assert_eq!(container.block2.unwrap(), &[0xcc, 0xdd, 0xee]);
    assert_eq!(container.block3.unwrap(), &[0xff, 0x11, 0x22]);
}

#[test]
fn rejects_truncated_header() {
    let bytes = [0u8; 9];

    let err = parse_mtx_container(&bytes).unwrap_err();
    assert_eq!(err, MtxContainerError::Truncated);
}

#[test]
fn rejects_invalid_copy_distance() {
    let mut bytes = fixture_bytes();
    bytes[1..4].copy_from_slice(&[0, 0, 0]);

    let err = parse_mtx_container(&bytes).unwrap_err();
    assert_eq!(err, MtxContainerError::InvalidMetadata);
}

#[test]
fn rejects_invalid_block_order() {
    let mut bytes = fixture_bytes();
    bytes[7..10].copy_from_slice(&[0x00, 0x00, 0x0b]);

    let err = parse_mtx_container(&bytes).unwrap_err();
    assert_eq!(err, MtxContainerError::InvalidMetadata);
}
