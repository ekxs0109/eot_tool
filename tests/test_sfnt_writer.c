#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/sfnt_writer.h"
#include "../src/sfnt_font.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define TAG_head 0x68656164u
#define SFNT_CHECKSUM_MAGIC 0xB1B0AFBAu

#define ASSERT_OK(expr) do { \
  eot_status_t status = (expr); \
  if (status != EOT_OK) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s returned %d", #expr, status); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s", #expr); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static uint16_t read_u16be_local(const uint8_t *data) {
  return ((uint16_t)data[0] << 8) | (uint16_t)data[1];
}

static uint32_t read_u32be_local(const uint8_t *data) {
  return ((uint32_t)data[0] << 24) | ((uint32_t)data[1] << 16) |
         ((uint32_t)data[2] << 8) | (uint32_t)data[3];
}

static uint32_t calc_checksum(const uint8_t *data, size_t length) {
  uint32_t sum = 0;
  size_t nlongs = (length + 3) / 4;

  for (size_t i = 0; i < nlongs; i++) {
    uint32_t value = 0;
    for (int j = 0; j < 4; j++) {
      size_t offset = i * 4 + j;
      if (offset < length) {
        value = (value << 8) | data[offset];
      }
    }
    sum += value;
  }

  return sum;
}

static size_t find_table_entry_offset(const uint8_t *data, uint16_t num_tables,
                                      uint32_t tag) {
  for (uint16_t i = 0; i < num_tables; i++) {
    size_t entry_offset = 12u + (size_t)i * 16u;
    if (read_u32be_local(data + entry_offset) == tag) {
      return entry_offset;
    }
  }
  return 0;
}

static size_t count_tag_instances(const sfnt_font_t *font, uint32_t tag) {
  size_t count = 0;
  for (size_t i = 0; i < font->num_tables; i++) {
    if (font->tables[i].tag == tag) {
      count++;
    }
  }
  return count;
}

static void test_sfnt_writer_basic_structure(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add a simple table
  uint8_t table_data[] = { 0x00, 0x01, 0x02, 0x03 };
  ASSERT_OK(sfnt_font_add_table(&font, 0x74657374, table_data, sizeof(table_data)));  // 'test'

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Check SFNT header
  ASSERT_EQ(read_u32be_local(output), 0x00010000);  // version
  ASSERT_EQ(read_u16be_local(output + 4), 1);       // numTables

  // Check table directory entry
  size_t entry_offset = 12;
  ASSERT_EQ(read_u32be_local(output + entry_offset), 0x74657374);  // tag

  // Verify checksum
  uint32_t expected_checksum = calc_checksum(table_data, sizeof(table_data));
  ASSERT_EQ(read_u32be_local(output + entry_offset + 4), expected_checksum);

  // Verify offset and length
  uint32_t table_offset = read_u32be_local(output + entry_offset + 8);
  uint32_t table_length = read_u32be_local(output + entry_offset + 12);
  ASSERT_EQ(table_length, 4);
  ASSERT_EQ(memcmp(output + table_offset, table_data, 4), 0);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_table_alignment(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add tables with non-aligned sizes
  uint8_t table1[] = { 0x01, 0x02, 0x03 };  // 3 bytes (not 4-byte aligned)
  uint8_t table2[] = { 0x04, 0x05 };        // 2 bytes (not 4-byte aligned)

  ASSERT_OK(sfnt_font_add_table(&font, 0x61616161, table1, sizeof(table1)));  // 'aaaa'
  ASSERT_OK(sfnt_font_add_table(&font, 0x62626262, table2, sizeof(table2)));  // 'bbbb'

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Get table offsets from directory
  uint32_t offset1 = read_u32be_local(output + 12 + 8);   // First table offset
  uint32_t offset2 = read_u32be_local(output + 28 + 8);   // Second table offset

  // Verify offsets are 4-byte aligned
  ASSERT_EQ(offset1 % 4, 0);
  ASSERT_EQ(offset2 % 4, 0);

  // Verify second table starts after first table + padding
  // First table: 3 bytes, aligned to 4 bytes
  ASSERT_EQ(offset2, offset1 + 4);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_table_sorting(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add tables in non-sorted order
  uint8_t data[] = { 0x00 };
  ASSERT_OK(sfnt_font_add_table(&font, 0x7A7A7A7A, data, 1));  // 'zzzz'
  ASSERT_OK(sfnt_font_add_table(&font, 0x61616161, data, 1));  // 'aaaa'
  ASSERT_OK(sfnt_font_add_table(&font, 0x6D6D6D6D, data, 1));  // 'mmmm'

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Verify tables are sorted in the directory
  uint32_t tag1 = read_u32be_local(output + 12);
  uint32_t tag2 = read_u32be_local(output + 28);
  uint32_t tag3 = read_u32be_local(output + 44);

  ASSERT_EQ(tag1, 0x61616161);  // 'aaaa'
  ASSERT_EQ(tag2, 0x6D6D6D6D);  // 'mmmm'
  ASSERT_EQ(tag3, 0x7A7A7A7A);  // 'zzzz'

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_search_range_calculation(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add 5 tables (search_range should be 4, entrySelector should be 2)
  uint8_t data[] = { 0x00 };
  for (int i = 0; i < 5; i++) {
    uint32_t tag = 0x61000000 + i;  // 'a\0\0\0', 'a\0\0\1', etc.
    ASSERT_OK(sfnt_font_add_table(&font, tag, data, 1));
  }

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Check search range fields
  uint16_t num_tables = read_u16be_local(output + 4);
  uint16_t search_range = read_u16be_local(output + 6);
  uint16_t entry_selector = read_u16be_local(output + 8);
  uint16_t range_shift = read_u16be_local(output + 10);

  ASSERT_EQ(num_tables, 5);
  ASSERT_EQ(search_range, 64);   // 4 * 16 (highest power of 2 <= 5 is 4)
  ASSERT_EQ(entry_selector, 2);  // log2(4) = 2
  ASSERT_EQ(range_shift, 16);    // (5 - 4) * 16

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_checksum_calculation(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add a table with known data
  uint8_t table_data[] = { 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC };
  ASSERT_OK(sfnt_font_add_table(&font, 0x74657374, table_data, sizeof(table_data)));

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Calculate expected checksum
  uint32_t expected_checksum = calc_checksum(table_data, sizeof(table_data));

  // Read checksum from table directory
  uint32_t stored_checksum = read_u32be_local(output + 12 + 4);

  ASSERT_EQ(stored_checksum, expected_checksum);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_padding_zeros(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add a table with size not multiple of 4
  uint8_t table_data[] = { 0xFF, 0xFF, 0xFF };  // 3 bytes
  ASSERT_OK(sfnt_font_add_table(&font, 0x74657374, table_data, sizeof(table_data)));

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Get table offset
  uint32_t table_offset = read_u32be_local(output + 12 + 8);

  // Verify table data
  ASSERT_EQ(output[table_offset], 0xFF);
  ASSERT_EQ(output[table_offset + 1], 0xFF);
  ASSERT_EQ(output[table_offset + 2], 0xFF);

  // Verify padding byte is zero
  ASSERT_EQ(output[table_offset + 3], 0x00);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_multiple_tables(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add multiple tables with different sizes
  uint8_t head_data[54] = {0};
  uint8_t name_data[100] = {0};
  uint8_t glyf_data[200] = {0};

  ASSERT_OK(sfnt_font_add_table(&font, 0x68656164, head_data, sizeof(head_data)));  // 'head'
  ASSERT_OK(sfnt_font_add_table(&font, 0x6E616D65, name_data, sizeof(name_data)));  // 'name'
  ASSERT_OK(sfnt_font_add_table(&font, 0x676C7966, glyf_data, sizeof(glyf_data)));  // 'glyf'

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Verify number of tables
  ASSERT_EQ(read_u16be_local(output + 4), 3);

  // Verify all tables are present and sorted
  uint32_t tag1 = read_u32be_local(output + 12);
  uint32_t tag2 = read_u32be_local(output + 28);
  uint32_t tag3 = read_u32be_local(output + 44);

  ASSERT_EQ(tag1, 0x676C7966);  // 'glyf'
  ASSERT_EQ(tag2, 0x68656164);  // 'head'
  ASSERT_EQ(tag3, 0x6E616D65);  // 'name'

  // Verify all offsets are 4-byte aligned
  uint32_t offset1 = read_u32be_local(output + 12 + 8);
  uint32_t offset2 = read_u32be_local(output + 28 + 8);
  uint32_t offset3 = read_u32be_local(output + 44 + 8);

  ASSERT_EQ(offset1 % 4, 0);
  ASSERT_EQ(offset2 % 4, 0);
  ASSERT_EQ(offset3 % 4, 0);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_empty_font_fails(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  uint8_t *output = NULL;
  size_t output_size = 0;

  // Should fail with no tables
  eot_status_t status = sfnt_writer_serialize(&font, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
  ASSERT_TRUE(output == NULL);

  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_total_size_calculation(void) {
  sfnt_font_t font;
  sfnt_font_init(&font);

  // Add 2 tables
  uint8_t table1[10] = {0};
  uint8_t table2[7] = {0};

  ASSERT_OK(sfnt_font_add_table(&font, 0x61616161, table1, sizeof(table1)));
  ASSERT_OK(sfnt_font_add_table(&font, 0x62626262, table2, sizeof(table2)));

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Expected size:
  // Header: 12 bytes
  // Table directory: 2 * 16 = 32 bytes
  // Table 1: 10 bytes, aligned to 12 bytes
  // Table 2: 7 bytes, aligned to 8 bytes
  // Total: 12 + 32 + 12 + 8 = 64 bytes

  ASSERT_EQ(output_size, 64);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_writer_recomputes_head_checksum_adjustment(void) {
  sfnt_font_t font;
  uint8_t head[54] = {0};
  uint8_t name[] = {0x00, 0x01, 0x00, 0x00};
  uint8_t *output = NULL;
  size_t output_size = 0;
  uint16_t num_tables;
  size_t head_entry_offset;
  uint32_t head_offset;
  uint32_t head_adjustment;

  sfnt_font_init(&font);
  ASSERT_OK(sfnt_font_add_table(&font, TAG_head, head, sizeof(head)));
  ASSERT_OK(sfnt_font_add_table(&font, 0x6e616d65u, name, sizeof(name)));

  ASSERT_OK(sfnt_writer_serialize(&font, &output, &output_size));
  ASSERT_TRUE(output != NULL);

  num_tables = read_u16be_local(output + 4);
  head_entry_offset = find_table_entry_offset(output, num_tables, TAG_head);
  ASSERT_TRUE(head_entry_offset != 0);

  head_offset = read_u32be_local(output + head_entry_offset + 8);
  ASSERT_TRUE(head_offset != 0);

  head_adjustment = read_u32be_local(output + head_offset + 8);
  ASSERT_TRUE(head_adjustment != 0u);
  ASSERT_EQ(calc_checksum(output, output_size), SFNT_CHECKSUM_MAGIC);

  free(output);
  sfnt_font_destroy(&font);
}

static void test_sfnt_font_add_table_replaces_and_collapses_duplicate_tags(void) {
  sfnt_font_t font;
  const uint32_t kTag = 0x61616161u;   /* 'aaaa' */
  const uint32_t kOther = 0x62626262u; /* 'bbbb' */
  uint8_t first[] = {0x01, 0x02};
  uint8_t other[] = {0x10};
  uint8_t duplicate[] = {0xAA};
  uint8_t replacement[] = {0xF0, 0x0D, 0xBE, 0xEF};
  uint8_t *dup_data = NULL;

  sfnt_font_init(&font);
  ASSERT_OK(sfnt_font_add_table(&font, kTag, first, sizeof(first)));
  ASSERT_OK(sfnt_font_add_table(&font, kOther, other, sizeof(other)));

  dup_data = (uint8_t *)malloc(sizeof(duplicate));
  ASSERT_TRUE(dup_data != NULL);
  memcpy(dup_data, duplicate, sizeof(duplicate));

  font.tables[2].tag = kTag;
  font.tables[2].data = dup_data;
  font.tables[2].length = sizeof(duplicate);
  font.num_tables = 3;

  ASSERT_EQ(count_tag_instances(&font, kTag), 2);
  ASSERT_OK(sfnt_font_add_table(&font, kTag, replacement, sizeof(replacement)));

  ASSERT_EQ(font.num_tables, 2);
  ASSERT_EQ(count_tag_instances(&font, kTag), 1);
  ASSERT_TRUE(sfnt_font_get_table(&font, kOther) != NULL);

  sfnt_table_t *updated = sfnt_font_get_table(&font, kTag);
  ASSERT_TRUE(updated != NULL);
  ASSERT_EQ(updated->length, sizeof(replacement));
  ASSERT_EQ(memcmp(updated->data, replacement, sizeof(replacement)), 0);

  sfnt_font_destroy(&font);
}

void register_sfnt_writer_tests(void) {
  test_register("test_sfnt_writer_basic_structure", test_sfnt_writer_basic_structure);
  test_register("test_sfnt_writer_table_alignment", test_sfnt_writer_table_alignment);
  test_register("test_sfnt_writer_table_sorting", test_sfnt_writer_table_sorting);
  test_register("test_sfnt_writer_search_range_calculation", test_sfnt_writer_search_range_calculation);
  test_register("test_sfnt_writer_checksum_calculation", test_sfnt_writer_checksum_calculation);
  test_register("test_sfnt_writer_padding_zeros", test_sfnt_writer_padding_zeros);
  test_register("test_sfnt_writer_multiple_tables", test_sfnt_writer_multiple_tables);
  test_register("test_sfnt_writer_empty_font_fails", test_sfnt_writer_empty_font_fails);
  test_register("test_sfnt_writer_total_size_calculation", test_sfnt_writer_total_size_calculation);
  test_register("test_sfnt_writer_recomputes_head_checksum_adjustment",
                test_sfnt_writer_recomputes_head_checksum_adjustment);
  test_register("test_sfnt_font_add_table_replaces_and_collapses_duplicate_tags",
                test_sfnt_font_add_table_replaces_and_collapses_duplicate_tags);
}
