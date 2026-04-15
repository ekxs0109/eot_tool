use fonttool_sfnt::{
    load_sfnt, parse_sfnt, serialize_sfnt, OwnedSfntFont, SerializeError, SFNT_VERSION_OTTO,
    SFNT_VERSION_TRUETYPE,
};

const SFNT_CHECKSUM_MAGIC: u32 = 0xB1B0_AFBA;
const TAG_AAAA: u32 = u32::from_be_bytes(*b"aaaa");
const TAG_BBBB: u32 = u32::from_be_bytes(*b"bbbb");
const TAG_CFF: u32 = u32::from_be_bytes(*b"CFF ");
const TAG_GLYF: u32 = u32::from_be_bytes(*b"glyf");
const TAG_HEAD: u32 = u32::from_be_bytes(*b"head");
const TAG_MMMM: u32 = u32::from_be_bytes(*b"mmmm");
const TAG_NAME: u32 = u32::from_be_bytes(*b"name");
const TAG_TEST: u32 = u32::from_be_bytes(*b"test");
const TAG_ZZZZ: u32 = u32::from_be_bytes(*b"zzzz");

fn read_u16_be(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn align4(length: usize) -> usize {
    (length + 3) & !3
}

fn calc_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let padded_len = align4(data.len());

    for chunk_start in (0..padded_len).step_by(4) {
        let mut value = 0u32;
        for offset in 0..4 {
            value <<= 8;
            value |= u32::from(*data.get(chunk_start + offset).unwrap_or(&0));
        }
        sum = sum.wrapping_add(value);
    }

    sum
}

fn find_table_entry_offset(bytes: &[u8], tag: u32) -> Option<usize> {
    let num_tables = usize::from(read_u16_be(bytes, 4));

    (0..num_tables).find_map(|index| {
        let entry_offset = 12 + index * 16;
        (read_u32_be(bytes, entry_offset) == tag).then_some(entry_offset)
    })
}

#[test]
fn serializes_basic_sfnt_structure_and_directory_contents() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    let table_data = vec![0x00, 0x01, 0x02, 0x03];
    font.add_table(TAG_TEST, table_data.clone());

    let bytes = serialize_sfnt(&font).unwrap();
    let parsed = parse_sfnt(&bytes).unwrap();

    assert_eq!(read_u32_be(&bytes, 0), SFNT_VERSION_TRUETYPE);
    assert_eq!(read_u16_be(&bytes, 4), 1);
    assert_eq!(read_u16_be(&bytes, 6), 16);
    assert_eq!(read_u16_be(&bytes, 8), 0);
    assert_eq!(read_u16_be(&bytes, 10), 0);

    assert_eq!(parsed.version_tag(), SFNT_VERSION_TRUETYPE);
    assert_eq!(parsed.table_directory().len(), 1);

    let entry_offset = 12;
    let table_offset = read_u32_be(&bytes, entry_offset + 8) as usize;

    assert_eq!(read_u32_be(&bytes, entry_offset), TAG_TEST);
    assert_eq!(read_u32_be(&bytes, entry_offset + 4), calc_checksum(&table_data));
    assert_eq!(read_u32_be(&bytes, entry_offset + 12), table_data.len() as u32);
    assert_eq!(table_offset, 28);
    assert_eq!(&bytes[table_offset..table_offset + table_data.len()], table_data);
}

#[test]
fn aligns_table_offsets_to_four_bytes_and_updates_total_size() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_AAAA, vec![0x01, 0x02, 0x03]);
    font.add_table(TAG_BBBB, vec![0x04, 0x05]);

    let bytes = serialize_sfnt(&font).unwrap();
    let first_offset = read_u32_be(&bytes, 12 + 8);
    let second_offset = read_u32_be(&bytes, 28 + 8);

    assert_eq!(first_offset % 4, 0);
    assert_eq!(second_offset % 4, 0);
    assert_eq!(second_offset, first_offset + 4);
    assert_eq!(bytes.len(), 52);
}

#[test]
fn computes_total_size_from_aligned_table_lengths() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_AAAA, vec![0; 10]);
    font.add_table(TAG_BBBB, vec![0; 7]);

    let bytes = serialize_sfnt(&font).unwrap();

    assert_eq!(bytes.len(), 64);
}

#[test]
fn sorts_table_directory_entries_by_tag() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_ZZZZ, vec![0x00]);
    font.add_table(TAG_AAAA, vec![0x00]);
    font.add_table(TAG_MMMM, vec![0x00]);

    let bytes = serialize_sfnt(&font).unwrap();

    assert_eq!(read_u32_be(&bytes, 12), TAG_AAAA);
    assert_eq!(read_u32_be(&bytes, 28), TAG_MMMM);
    assert_eq!(read_u32_be(&bytes, 44), TAG_ZZZZ);
}

#[test]
fn calculates_search_range_fields_for_five_tables() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);

    for (tag, value) in [
        (u32::from_be_bytes(*b"aaa0"), 0),
        (u32::from_be_bytes(*b"aaa1"), 1),
        (u32::from_be_bytes(*b"aaa2"), 2),
        (u32::from_be_bytes(*b"aaa3"), 3),
        (u32::from_be_bytes(*b"aaa4"), 4),
    ] {
        font.add_table(tag, vec![value]);
    }

    let bytes = serialize_sfnt(&font).unwrap();

    assert_eq!(read_u16_be(&bytes, 4), 5);
    assert_eq!(read_u16_be(&bytes, 6), 64);
    assert_eq!(read_u16_be(&bytes, 8), 2);
    assert_eq!(read_u16_be(&bytes, 10), 16);
}

#[test]
fn preserves_otto_version_tag_through_parse_and_serialize() {
    let otto_font_bytes = [
        0x4f, 0x54, 0x54, 0x4f, 0x00, 0x01, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x43, 0x46,
        0x46, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x04,
        0xde, 0xad, 0xbe, 0xef,
    ];

    let font = load_sfnt(&otto_font_bytes).unwrap();
    let serialized = serialize_sfnt(&font).unwrap();
    let parsed = parse_sfnt(&serialized).unwrap();

    assert_eq!(font.version_tag(), SFNT_VERSION_OTTO);
    assert_eq!(read_u32_be(&serialized, 0), SFNT_VERSION_OTTO);
    assert_eq!(parsed.version_tag(), SFNT_VERSION_OTTO);
    assert!(font.table(TAG_CFF).is_some());
}

#[test]
fn stores_directory_checksums_for_table_entries() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    let table_data = vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc];
    font.add_table(TAG_TEST, table_data.clone());

    let bytes = serialize_sfnt(&font).unwrap();

    assert_eq!(read_u32_be(&bytes, 12 + 4), calc_checksum(&table_data));
}

#[test]
fn zero_fills_padding_for_non_aligned_tables() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_TEST, vec![0xff, 0xff, 0xff]);

    let bytes = serialize_sfnt(&font).unwrap();
    let table_offset = read_u32_be(&bytes, 12 + 8) as usize;

    assert_eq!(&bytes[table_offset..table_offset + 3], &[0xff, 0xff, 0xff]);
    assert_eq!(bytes[table_offset + 3], 0x00);
}

#[test]
fn serializes_multiple_tables_with_sorted_directory_and_aligned_offsets() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_HEAD, vec![0; 54]);
    font.add_table(TAG_NAME, vec![0; 100]);
    font.add_table(TAG_GLYF, vec![0; 200]);

    let bytes = serialize_sfnt(&font).unwrap();

    assert_eq!(read_u16_be(&bytes, 4), 3);
    assert_eq!(read_u32_be(&bytes, 12), TAG_GLYF);
    assert_eq!(read_u32_be(&bytes, 28), TAG_HEAD);
    assert_eq!(read_u32_be(&bytes, 44), TAG_NAME);

    assert_eq!(read_u32_be(&bytes, 12 + 8) % 4, 0);
    assert_eq!(read_u32_be(&bytes, 28 + 8) % 4, 0);
    assert_eq!(read_u32_be(&bytes, 44 + 8) % 4, 0);
}

#[test]
fn rejects_serializing_an_empty_font() {
    let font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);

    let err = serialize_sfnt(&font).unwrap_err();

    assert_eq!(err, SerializeError::EmptyFont);
}

#[test]
fn add_table_replaces_existing_tag_in_place() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_AAAA, vec![0x01, 0x02]);
    font.add_table(TAG_BBBB, vec![0x10]);
    font.add_table(TAG_AAAA, vec![0xf0, 0x0d, 0xbe, 0xef]);

    assert_eq!(font.tables().len(), 2);
    assert_eq!(font.table(TAG_BBBB).unwrap().data, vec![0x10]);
    assert_eq!(font.table(TAG_AAAA).unwrap().data, vec![0xf0, 0x0d, 0xbe, 0xef]);
}

#[test]
fn recomputes_head_checksum_adjustment_when_head_exists() {
    let mut font = OwnedSfntFont::new(SFNT_VERSION_TRUETYPE);
    font.add_table(TAG_HEAD, vec![0; 54]);
    font.add_table(TAG_NAME, vec![0x00, 0x01, 0x00, 0x00]);

    let bytes = serialize_sfnt(&font).unwrap();
    let head_entry_offset = find_table_entry_offset(&bytes, TAG_HEAD).unwrap();
    let head_offset = read_u32_be(&bytes, head_entry_offset + 8) as usize;
    let head_checksum_adjustment = read_u32_be(&bytes, head_offset + 8);

    assert_ne!(head_checksum_adjustment, 0);
    assert_eq!(calc_checksum(&bytes), SFNT_CHECKSUM_MAGIC);
}
