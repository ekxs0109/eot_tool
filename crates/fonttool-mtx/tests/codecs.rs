use fonttool_mtx::{cvt_decode, cvt_encode, hdmx_decode, hdmx_encode};

fn read_u16_be(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("slice should fit"),
    )
}

fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("slice should fit"),
    )
}

fn init_hhea(num_h_metrics: u16) -> [u8; 36] {
    let mut hhea = [0u8; 36];
    hhea[34..36].copy_from_slice(&num_h_metrics.to_be_bytes());
    hhea
}

#[test]
fn cvt_decode_simple_deltas() {
    let encoded = [
        0x00, 0x03, // num_entries = 3
        100, 5, 3,
    ];

    let decoded = cvt_decode(&encoded).expect("cvt decode should succeed");

    assert_eq!(decoded.len(), 6);
    assert_eq!(read_u16_be(&decoded, 0), 100);
    assert_eq!(read_u16_be(&decoded, 2), 105);
    assert_eq!(read_u16_be(&decoded, 4), 108);
}

#[test]
fn cvt_decode_word_and_signed_ranges() {
    let word_encoded = [
        0x00, 0x02, // num_entries = 2
        238, 0x10, 0x00, // +4096
        238, 0xFF, 0xFF, // -1 delta
    ];
    let negative_range = [
        0x00, 0x02, // num_entries = 2
        239, 10, // -10
        240, 20, // -258
    ];
    let positive_range = [
        0x00, 0x02, // num_entries = 2
        248, 10, // +248
        249, 20, // +496
    ];

    let decoded_word = cvt_decode(&word_encoded).expect("word decode should succeed");
    let decoded_negative =
        cvt_decode(&negative_range).expect("negative-range decode should succeed");
    let decoded_positive =
        cvt_decode(&positive_range).expect("positive-range decode should succeed");

    assert_eq!(read_u16_be(&decoded_word, 0), 4096);
    assert_eq!(read_u16_be(&decoded_word, 2), 4095);
    assert_eq!(read_u16_be(&decoded_negative, 0) as i16, -10);
    assert_eq!(read_u16_be(&decoded_negative, 2) as i16, -268);
    assert_eq!(read_u16_be(&decoded_positive, 0), 248);
    assert_eq!(read_u16_be(&decoded_positive, 2), 744);
}

#[test]
fn cvt_encode_roundtrips_decoded_values() {
    let decoded = [0x00, 0x64, 0x00, 0x69, 0x00, 0x6C];

    let encoded = cvt_encode(&decoded).expect("cvt encode should succeed");
    let roundtrip = cvt_decode(&encoded).expect("cvt roundtrip decode should succeed");

    assert_eq!(roundtrip, decoded);
}

#[test]
fn cvt_decode_rejects_truncated_inputs() {
    for encoded in [
        &[0x00][..],
        &[0x00, 0x03, 100][..],
        &[0x00, 0x01, 238, 0x10][..],
    ] {
        assert!(
            cvt_decode(encoded).is_err(),
            "expected decode to reject {encoded:?}"
        );
    }
}

#[test]
fn hdmx_decode_basic_and_shared_widths() {
    let encoded = [
        0x00, 0x00, // version
        0x00, 0x01, // num_records = 1
        0x00, 0x00, 0x00, 0x04, // record_size = 4
        12, 7, // ppem, maxWidth
        140, 140, // surprise values for two glyphs
    ];
    let hmtx = [
        0x01, 0xF4, 0x00, 0x00, // advanceWidth = 500
        0x00, 0x00,
    ];
    let hhea = init_hhea(1);
    let mut head = [0u8; 54];
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let mut maxp = [0u8; 32];
    maxp[4..6].copy_from_slice(&2u16.to_be_bytes());

    let decoded =
        hdmx_decode(&encoded, &hmtx, &hhea, &head, &maxp).expect("hdmx decode should succeed");

    assert_eq!(decoded.len(), 12);
    assert_eq!(read_u16_be(&decoded, 0), 0);
    assert_eq!(read_u16_be(&decoded, 2), 1);
    assert_eq!(read_u32_be(&decoded, 4), 4);
    assert_eq!(decoded[8], 12);
    assert_eq!(decoded[9], 7);
    assert_eq!(decoded[10], 7);
    assert_eq!(decoded[11], 7);
}

#[test]
fn hdmx_encode_roundtrips_decoded_values() {
    let decoded = [
        0x00, 0x00, // version
        0x00, 0x01, // num_records = 1
        0x00, 0x00, 0x00, 0x03, // record_size = 3
        12, 100, 2,
    ];
    let hmtx = [
        0x00, 0x64, 0x00, 0x00, // advanceWidth = 100
    ];
    let hhea = init_hhea(1);
    let mut head = [0u8; 54];
    head[18..20].copy_from_slice(&1024u16.to_be_bytes());
    let mut maxp = [0u8; 32];
    maxp[4..6].copy_from_slice(&1u16.to_be_bytes());

    let encoded =
        hdmx_encode(&decoded, &hmtx, &hhea, &head, &maxp).expect("hdmx encode should succeed");
    let roundtrip =
        hdmx_decode(&encoded, &hmtx, &hhea, &head, &maxp).expect("hdmx roundtrip should decode");

    assert_eq!(roundtrip, decoded);
}

#[test]
fn hdmx_decode_rejects_corrupt_inputs() {
    let encoded_too_short = [0x00, 0x00, 0x00];
    let encoded_truncated_records = [
        0x00, 0x00, // version
        0x00, 0x02, // numRecords = 2
        0x00, 0x00, 0x00, 0x03, // recordSize = 3
        12, 100,
    ];
    let hmtx = [0u8; 4];
    let hhea = init_hhea(1);
    let mut head = [0u8; 54];
    head[18..20].copy_from_slice(&1024u16.to_be_bytes());
    let mut maxp = [0u8; 32];
    maxp[4..6].copy_from_slice(&1u16.to_be_bytes());

    assert!(hdmx_decode(&encoded_too_short, &hmtx, &hhea, &head, &maxp).is_err());
    assert!(hdmx_decode(&encoded_truncated_records, &hmtx, &hhea, &head, &maxp).is_err());
}
