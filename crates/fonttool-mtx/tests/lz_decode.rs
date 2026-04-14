use fonttool_mtx::{decompress_lz, LzDecompressError};

#[test]
fn decodes_java_reference_literal_stream() {
    let compressed = [
        0x00, 0x00, 0x05, 0x04, 0xC2, 0x82, 0x31, 0x20, 0x4C, 0x28, 0x23, 0x12, 0x04, 0xC2, 0x80,
    ];
    let expected = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];

    let decompressed = decompress_lz(&compressed).unwrap();

    assert_eq!(decompressed, expected);
}

#[test]
fn decodes_java_reference_copy_stream() {
    let compressed = [0x00, 0x00, 0x08, 0x2A, 0x2A, 0x89, 0x80, 0xA8, 0x0C, 0x20];
    let expected = *b"ABABABABABABABAB";

    let decompressed = decompress_lz(&compressed).unwrap();

    assert_eq!(decompressed, expected);
}

#[test]
fn rejects_truncated_stream() {
    let err = decompress_lz(&[]).unwrap_err();

    assert_eq!(err, LzDecompressError::Truncated);
}

#[test]
fn returns_empty_output_for_empty_stream_payload() {
    let decompressed = decompress_lz(&[0x00, 0x00, 0x00, 0x00]).unwrap();

    assert!(decompressed.is_empty());
}
