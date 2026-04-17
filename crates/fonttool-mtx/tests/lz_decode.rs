use fonttool_mtx::{compress_lz, compress_lz_literals, decompress_lz, LzDecompressError};

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
fn rejects_additional_truncated_stream_shapes() {
    for bytes in [&[0x01][..], &[0x01, 0x00][..], &[0x01, 0x00, 0x05][..]] {
        let err = decompress_lz(bytes).unwrap_err();
        assert_eq!(err, LzDecompressError::Truncated);
    }
}

#[test]
fn returns_empty_output_for_empty_stream_payload() {
    let decompressed = decompress_lz(&[0x00, 0x00, 0x00, 0x00]).unwrap();

    assert!(decompressed.is_empty());
}

#[test]
fn decodes_java_reference_word_copy_stream() {
    let compressed = [
        0x00, 0x00, 0x0D, 0xB5, 0x3E, 0x40, 0xBD, 0x3B, 0x8A, 0x18, 0x60, 0xC3, 0x26, 0x20, 0x80,
    ];
    let expected = *b"WingdingsWingdingsWingdings";

    let decompressed = decompress_lz(&compressed).unwrap();

    assert_eq!(decompressed, expected);
}

#[test]
fn literal_encoder_roundtrips_literal_data() {
    let input = [
        0x00, 0x01, 0x02, 0x03, 0x10, 0x11, 0x12, 0x13, 0x20, 0x21, 0x22, 0x23, 0x30, 0x31, 0x32,
        0x33,
    ];

    let compressed = compress_lz_literals(&input).unwrap();
    let decompressed = decompress_lz(&compressed).unwrap();

    assert_eq!(decompressed, input);
}

#[test]
fn literal_encoder_roundtrips_repeated_data() {
    let input = *b"WingdingsWingdingsWingdingsWingdings";

    let compressed = compress_lz_literals(&input).unwrap();
    let decompressed = decompress_lz(&compressed).unwrap();

    assert_eq!(decompressed, input);
}

#[test]
fn literal_encoder_roundtrips_empty_data() {
    let compressed = compress_lz_literals(&[]).unwrap();
    let decompressed = decompress_lz(&compressed).unwrap();

    assert!(decompressed.is_empty());
}

#[test]
fn backreference_encoder_roundtrips_repeated_data() {
    let input = *b"WingdingsWingdingsWingdingsWingdings";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
}

#[test]
fn backreference_encoder_beats_literal_only_on_repeated_data() {
    let input = *b"WingdingsWingdingsWingdingsWingdings";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let literal_only = compress_lz_literals(&input).expect("literal-only encoder should succeed");

    assert!(
        compressed.len() < literal_only.len(),
        "expected backreference encoder to beat literal-only on repeated input"
    );
}

#[test]
fn backreference_encoder_roundtrips_dup2_friendly_data() {
    let input = *b"ABABABABABABABAB";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
}

#[test]
fn backreference_encoder_roundtrips_dup4_friendly_data() {
    let input = *b"ABCDABCDABCDABCDABCDABCD";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
}

#[test]
fn backreference_encoder_roundtrips_dup6_friendly_data() {
    let input = *b"ABCDEFABCDEFABCDEFABCDEF";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
}

#[test]
fn backreference_encoder_never_returns_larger_output_than_literal_only() {
    let input = [
        0x00, 0x91, 0xA2, 0x13, 0x24, 0x35, 0x46, 0x57, 0x68, 0x79, 0x8A, 0x9B, 0xAC, 0xBD, 0xCE,
        0xDF,
    ];

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let literal_only = compress_lz_literals(&input).expect("literal-only encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
    assert!(
        compressed.len() <= literal_only.len(),
        "expected fallback to prevent regression on incompressible input"
    );
}

#[test]
fn backreference_encoder_can_copy_from_preload_window() {
    let input = [
        0xFC, 0xFC, 0xFC, 0xFC, 0xFD, 0xFD, 0xFD, 0xFD, 0xFE, 0xFE, 0xFE, 0xFE, 0xFF, 0xFF, 0xFF,
        0xFF,
    ];

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let literal_only = compress_lz_literals(&input).expect("literal-only encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
    assert!(
        compressed.len() < literal_only.len(),
        "expected preload-sourced backreference to beat literal-only output"
    );
}

#[test]
fn backreference_encoder_falls_back_to_literal_output_on_speculative_failure() {
    for input in [
        &[0xFF, 0x94][..],
        &[0xFF, 0xE2, 0x57][..],
        &[0xCE, 0x43, 0xCE, 0x24][..],
    ] {
        let compressed = compress_lz(input).expect("compress_lz should fall back to literals");
        let literal_only = compress_lz_literals(input).expect("literal-only encoder should work");
        let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

        assert_eq!(compressed, literal_only);
        assert_eq!(decompressed, input);
    }
}

#[test]
fn backreference_encoder_matches_java_reference_length_on_banana_repeats() {
    // Measured from sfntly_web LzcompCompress.compress.
    const JAVA_REFERENCE_LEN: usize = 12;
    let input = *b"BANANABANANABANANA";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
    assert!(
        compressed.len() <= JAVA_REFERENCE_LEN,
        "expected BANANA repeats to encode within the Java reference length target of {JAVA_REFERENCE_LEN} bytes, got {}",
        compressed.len()
    );
}

#[test]
fn backreference_encoder_matches_java_reference_length_on_preload_four_byte_runs() {
    // Measured from sfntly_web LzcompCompress.compress.
    const JAVA_REFERENCE_LEN: usize = 10;
    let input = [0xF8, 0xF8, 0xF8, 0xF8, 0xF9, 0xF9, 0xF9, 0xF9];

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
    assert!(
        compressed.len() <= JAVA_REFERENCE_LEN,
        "expected preload four-byte runs to encode within the Java reference length target of {JAVA_REFERENCE_LEN} bytes, got {}",
        compressed.len()
    );
}

#[test]
fn backreference_encoder_matches_java_reference_length_on_overlapping_repeats() {
    // Measured from sfntly_web LzcompCompress.compress; current Rust encoder already matches this case.
    const JAVA_REFERENCE_LEN: usize = 12;
    let input = *b"abcabcabcdabcabcabcd";

    let compressed = compress_lz(&input).expect("backreference encoder should succeed");
    let decompressed = decompress_lz(&compressed).expect("compressed data should decode");

    assert_eq!(decompressed, input);
    assert_eq!(compressed.len(), JAVA_REFERENCE_LEN);
}
