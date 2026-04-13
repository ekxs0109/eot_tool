#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "../src/byte_io.h"
#include "../src/eot_header.h"
#include "../src/file_io.h"
#include "../src/lzcomp.h"
#include "../src/mtx_decode.h"
#include "../src/mtx_encode.h"
#include "../src/mtx_container.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_reader.h"
#include "../src/sfnt_writer.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

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

#define TAG_head 0x68656164
#define TAG_name 0x6e616d65
#define TAG_glyf 0x676c7966
#define TAG_hdmx 0x68646d78
#define TAG_loca 0x6c6f6361
#define TAG_maxp 0x6d617870
#define TAG_cvt  0x63767420
#define EOT_FLAG_PPT_XOR 0x10000000u

static int ensure_build_output_dir(void) {
  if (mkdir("build", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build directory");
    return 0;
  }
  if (mkdir("build/out", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build/out directory");
    return 0;
  }
  return 1;
}

static void write_obfuscated_fixture_copy(const char *source_path,
                                          const char *dest_path) {
  file_buffer_t input = {0};
  eot_header_t header;
  buffer_view_t view;
  uint32_t flags;

  ASSERT_OK(file_io_read_all(source_path, &input));
  view = buffer_view_make(input.data, input.length);
  ASSERT_OK(eot_header_parse(view, &header));
  ASSERT_TRUE(header.header_length < input.length);

  flags = read_u32le(input.data + 12);
  write_u32le(input.data + 12, flags | EOT_FLAG_PPT_XOR);

  for (size_t i = header.header_length; i < input.length; i++) {
    input.data[i] ^= 0x50u;
  }

  ASSERT_OK(file_io_write_all(dest_path, input.data, input.length));

  eot_header_destroy(&header);
  file_io_free(&input);
}

static uint16_t read_u16be_local(const uint8_t *data) {
  return ((uint16_t)data[0] << 8) | (uint16_t)data[1];
}

static uint32_t read_u32be_local(const uint8_t *data) {
  return ((uint32_t)data[0] << 24) | ((uint32_t)data[1] << 16) |
         ((uint32_t)data[2] << 8) | (uint32_t)data[3];
}

static void write_u16be_local(uint8_t *dest, uint16_t value) {
  dest[0] = (uint8_t)(value >> 8);
  dest[1] = (uint8_t)(value & 0xFF);
}

static void init_minimal_font_with_empty_glyf(sfnt_font_t *font) {
  uint8_t head[54] = {0};
  uint8_t os2[86] = {0};
  uint8_t maxp[6] = {0};
  uint8_t loca[4] = {0};

  sfnt_font_init(font);

  write_u16be_local(head + 18, 1000);
  write_u16be_local(head + 50, 0);
  write_u16be_local(os2 + 4, 400);
  write_u16be_local(maxp + 4, 1);

  ASSERT_OK(sfnt_font_add_table(font, 0x68656164u, head, sizeof(head)));
  ASSERT_OK(sfnt_font_add_table(font, 0x4f532f32u, os2, sizeof(os2)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_maxp, maxp, sizeof(maxp)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_glyf, NULL, 0));
  ASSERT_OK(sfnt_font_add_table(font, TAG_loca, loca, sizeof(loca)));
}

static void force_empty_split_blocks(byte_buffer_t *eot) {
  eot_header_t header;
  buffer_view_t view = buffer_view_make(eot->data, eot->length);
  uint8_t zeros[8] = {0};
  uint32_t original_font_data_size;

  ASSERT_OK(eot_header_parse(view, &header));
  original_font_data_size = (uint32_t)(eot->length - header.header_length);

  ASSERT_OK(byte_buffer_append(eot, zeros, sizeof(zeros)));
  write_u32le(eot->data + 0, (uint32_t)eot->length);
  write_u32le(eot->data + 4, original_font_data_size + (uint32_t)sizeof(zeros));

  eot->data[header.header_length] = 3;
  write_u24be(eot->data + header.header_length + 4, original_font_data_size);
  write_u24be(eot->data + header.header_length + 7, original_font_data_size + 4u);

  eot_header_destroy(&header);
}

static void rewrite_fixture_with_mutated_block1_table(const char *source_path,
                                                      const char *dest_path,
                                                      uint32_t tag,
                                                      const uint8_t *replacement,
                                                      size_t replacement_length) {
  file_buffer_t input = {0};
  eot_header_t header;
  buffer_view_t mtx_view;
  mtx_container_t container;
  sfnt_font_t block1_font;
  sfnt_table_t *table;
  uint8_t *block1_data = NULL;
  size_t block1_size = 0;
  uint8_t *serialized_block1 = NULL;
  size_t serialized_block1_size = 0;
  uint8_t *compressed1 = NULL;
  size_t compressed1_size = 0;
  uint8_t *mtx_data = NULL;
  size_t mtx_size = 0;
  uint8_t *output = NULL;
  size_t output_size = 0;
  encoded_blocks_t blocks = {0};
  eot_status_t status;
  int ppt_xor_enabled;

  ASSERT_OK(file_io_read_all(source_path, &input));
  ASSERT_OK(eot_header_parse(buffer_view_make(input.data, input.length), &header));

  ppt_xor_enabled = (header.flags & EOT_FLAG_PPT_XOR) != 0u;
  if (ppt_xor_enabled) {
    for (size_t i = 0; i < header.font_data_size; i++) {
      input.data[header.header_length + i] ^= 0x50u;
    }
  }

  mtx_view = buffer_view_make(input.data + header.header_length, header.font_data_size);
  ASSERT_OK(mtx_container_parse(mtx_view, &container));
  ASSERT_OK(lzcomp_decompress(
      buffer_view_make(container.payload_data + 10, container.offset_block2 - 10),
      &block1_data, &block1_size));
  ASSERT_OK(sfnt_reader_parse(block1_data, block1_size, &block1_font));

  table = sfnt_font_get_table(&block1_font, tag);
  ASSERT_TRUE(table != NULL);

  free(table->data);
  table->data = NULL;
  table->length = replacement_length;
  if (replacement_length > 0) {
    table->data = malloc(replacement_length);
    ASSERT_TRUE(table->data != NULL);
    memcpy(table->data, replacement, replacement_length);
  }

  ASSERT_OK(sfnt_writer_serialize(&block1_font, &serialized_block1, &serialized_block1_size));
  ASSERT_OK(lzcomp_compress(serialized_block1, serialized_block1_size,
                            &compressed1, &compressed1_size));

  blocks.block1 = compressed1;
  blocks.block1_size = compressed1_size;
  if (container.num_blocks >= 2) {
    blocks.block2 = (uint8_t *)(container.payload_data + container.offset_block2);
    blocks.block2_size = container.offset_block3 - container.offset_block2;
  }
  if (container.num_blocks >= 3) {
    blocks.block3 = (uint8_t *)(container.payload_data + container.offset_block3);
    blocks.block3_size = container.payload_size - container.offset_block3;
  }

  ASSERT_OK(mtx_container_pack(&blocks, &mtx_data, &mtx_size));

  output_size = header.header_length + mtx_size;
  output = malloc(output_size);
  ASSERT_TRUE(output != NULL);
  memcpy(output, input.data, header.header_length);
  memcpy(output + header.header_length, mtx_data, mtx_size);
  write_u32le(output + 0, (uint32_t)output_size);
  write_u32le(output + 4, (uint32_t)mtx_size);

  if (ppt_xor_enabled) {
    for (size_t i = 0; i < mtx_size; i++) {
      output[header.header_length + i] ^= 0x50u;
    }
  }

  status = file_io_write_all(dest_path, output, output_size);
  ASSERT_TRUE(status == EOT_OK);

  free(output);
  free(mtx_data);
  free(compressed1);
  free(serialized_block1);
  sfnt_font_destroy(&block1_font);
  free(block1_data);
  eot_header_destroy(&header);
  file_io_free(&input);
}

static void test_decode_fixture_builds_required_sfnt_tables(void) {
  sfnt_font_t font;

  ASSERT_OK(mtx_decode_eot_file("testdata/wingdings3.eot", &font));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_head));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_name));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_loca));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_maxp));
  sfnt_font_destroy(&font);
}

static void test_decode_fixture_serializes_valid_table_directory(void) {
  sfnt_font_t font;
  uint8_t *serialized = NULL;
  size_t serialized_size = 0;
  uint16_t num_tables;
  uint32_t previous_tag = 0;
  int saw_head = 0;
  int saw_name = 0;
  int saw_glyf = 0;
  int saw_loca = 0;
  int saw_maxp = 0;

  ASSERT_OK(mtx_decode_eot_file("testdata/wingdings3.eot", &font));
  ASSERT_OK(sfnt_writer_serialize(&font, &serialized, &serialized_size));
  sfnt_font_destroy(&font);

  ASSERT_TRUE(serialized != NULL);
  ASSERT_TRUE(serialized_size > 12);
  ASSERT_TRUE(read_u32be_local(serialized) == 0x00010000u);

  num_tables = read_u16be_local(serialized + 4);
  ASSERT_TRUE(num_tables >= 5);
  ASSERT_TRUE(serialized_size >= 12u + (size_t)num_tables * 16u);

  for (uint16_t i = 0; i < num_tables; i++) {
    size_t entry_offset = 12u + (size_t)i * 16u;
    uint32_t tag = read_u32be_local(serialized + entry_offset);
    uint32_t table_offset = read_u32be_local(serialized + entry_offset + 8);
    uint32_t table_length = read_u32be_local(serialized + entry_offset + 12);

    ASSERT_TRUE(i == 0 || tag > previous_tag);
    ASSERT_TRUE(table_offset % 4u == 0u);
    ASSERT_TRUE(table_offset + table_length <= serialized_size);

    if (tag == TAG_head) saw_head = 1;
    if (tag == TAG_name) saw_name = 1;
    if (tag == TAG_glyf) saw_glyf = 1;
    if (tag == TAG_loca) saw_loca = 1;
    if (tag == TAG_maxp) saw_maxp = 1;

    previous_tag = tag;
  }

  ASSERT_TRUE(saw_head);
  ASSERT_TRUE(saw_name);
  ASSERT_TRUE(saw_glyf);
  ASSERT_TRUE(saw_loca);
  ASSERT_TRUE(saw_maxp);

  free(serialized);
}

static void test_decode_obfuscated_fixture_builds_required_sfnt_tables(void) {
  const char *obfuscated_path = "build/out/wingdings3-obfuscated.fntdata";
  sfnt_font_t font;

  if (!ensure_build_output_dir()) {
    return;
  }

  unlink(obfuscated_path);
  write_obfuscated_fixture_copy("testdata/wingdings3.eot", obfuscated_path);

  ASSERT_OK(mtx_decode_eot_file(obfuscated_path, &font));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_head));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_name));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_loca));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_maxp));
  sfnt_font_destroy(&font);
}

static void test_decode_three_block_empty_aux_streams_reconstructs_loca(void) {
  const char *fixture_path = "build/out/minimal-empty-split.eot";
  sfnt_font_t source_font;
  sfnt_font_t decoded_font;
  byte_buffer_t eot;

  if (!ensure_build_output_dir()) {
    return;
  }

  init_minimal_font_with_empty_glyf(&source_font);
  byte_buffer_init(&eot);

  ASSERT_OK(mtx_encode_font(&source_font, &eot));
  force_empty_split_blocks(&eot);
  ASSERT_OK(file_io_write_all(fixture_path, eot.data, eot.length));

  ASSERT_OK(mtx_decode_eot_file(fixture_path, &decoded_font));
  ASSERT_TRUE(sfnt_font_has_table(&decoded_font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&decoded_font, TAG_loca));

  sfnt_font_destroy(&decoded_font);
  byte_buffer_destroy(&eot);
  sfnt_font_destroy(&source_font);
}

static void test_decode_rejects_corrupt_encoded_cvt_table(void) {
  static const uint8_t corrupt_cvt[] = {0x00, 0x02, 0x64};
  const char *fixture_path = "build/out/wingdings3-corrupt-cvt.eot";
  sfnt_font_t font;
  eot_status_t status;

  if (!ensure_build_output_dir()) {
    return;
  }

  rewrite_fixture_with_mutated_block1_table("testdata/wingdings3.eot", fixture_path,
                                            TAG_cvt, corrupt_cvt,
                                            sizeof(corrupt_cvt));
  status = mtx_decode_eot_file(fixture_path, &font);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_decode_rejects_corrupt_encoded_hdmx_table(void) {
  static const uint8_t corrupt_hdmx[] = {0x00, 0x00, 0x00};
  const char *fixture_path = "build/out/wingdings3-corrupt-hdmx.eot";
  sfnt_font_t font;
  eot_status_t status;

  if (!ensure_build_output_dir()) {
    return;
  }

  rewrite_fixture_with_mutated_block1_table("testdata/wingdings3.eot", fixture_path,
                                            TAG_hdmx, corrupt_hdmx,
                                            sizeof(corrupt_hdmx));
  status = mtx_decode_eot_file(fixture_path, &font);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

void register_decode_pipeline_tests(void) {
  test_register("test_decode_fixture_builds_required_sfnt_tables",
                test_decode_fixture_builds_required_sfnt_tables);
  test_register("test_decode_fixture_serializes_valid_table_directory",
                test_decode_fixture_serializes_valid_table_directory);
  test_register("test_decode_obfuscated_fixture_builds_required_sfnt_tables",
                test_decode_obfuscated_fixture_builds_required_sfnt_tables);
  test_register("test_decode_three_block_empty_aux_streams_reconstructs_loca",
                test_decode_three_block_empty_aux_streams_reconstructs_loca);
  test_register("test_decode_rejects_corrupt_encoded_cvt_table",
                test_decode_rejects_corrupt_encoded_cvt_table);
  test_register("test_decode_rejects_corrupt_encoded_hdmx_table",
                test_decode_rejects_corrupt_encoded_hdmx_table);
}
