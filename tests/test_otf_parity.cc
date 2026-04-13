#include <errno.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <time.h>

extern "C" {
#include "../src/byte_io.h"
#include "../src/file_io.h"
#include "../src/mtx_decode.h"
#include "../src/mtx_encode.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_writer.h"
void test_register(const char *name, void (*fn)(void));
void test_fail_with_message(const char *message);
}

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

#define TAG_hhea 0x68686561u
#define TAG_head 0x68656164u
#define TAG_post 0x706f7374u
#define MAC_EPOCH_OFFSET 2082844800u
#define MAC_EPOCH_OFFSET_LOW32 0x7C25B080u
#define SFNT_CHECKSUM_MAGIC 0xB1B0AFBAu

static uint32_t calc_checksum(const uint8_t *data, size_t length) {
  uint32_t sum = 0;
  size_t nlongs = (length + 3) / 4;

  for (size_t i = 0; i < nlongs; i++) {
    uint32_t value = 0;
    for (int j = 0; j < 4; j++) {
      size_t offset = i * 4 + (size_t)j;
      if (offset < length) {
        value = (value << 8) | data[offset];
      }
    }
    sum += value;
  }

  return sum;
}

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

static void test_otf_cff_roundtrip_preserves_expected_post_and_hhea_fields(void) {
  const char *source_path =
      "testdata/aipptfonts/\351\246\231\350\225\211Plus__20220301185701917366.otf";
  const char *eot_path = "build/out/0213-parity.eot";
  byte_buffer_t eot = {};
  sfnt_font_t decoded = {};
  sfnt_table_t *post = NULL;
  sfnt_table_t *hhea = NULL;

  if (!ensure_build_output_dir()) {
    return;
  }

  sfnt_font_init(&decoded);

  ASSERT_OK(mtx_encode_ttf_file(source_path, &eot));
  ASSERT_OK(file_io_write_all(eot_path, eot.data, eot.length));
  ASSERT_OK(mtx_decode_eot_file(eot_path, &decoded));

  post = sfnt_font_get_table(&decoded, TAG_post);
  hhea = sfnt_font_get_table(&decoded, TAG_hhea);

  ASSERT_TRUE(post != NULL);
  ASSERT_TRUE(hhea != NULL);
  ASSERT_TRUE(read_u32be(post->data) == 0x00030000u);
  ASSERT_TRUE((int16_t)read_u16be(post->data + 8) == -75);
  ASSERT_TRUE((int16_t)read_u16be(post->data + 10) == 50);
  ASSERT_TRUE(read_u16be(hhea->data + 34) == 4518);

  sfnt_font_destroy(&decoded);
  byte_buffer_destroy(&eot);
}

static void test_otf_cff_roundtrip_head_fields_look_serialized(void) {
  const char *source_path =
      "testdata/aipptfonts/\351\246\231\350\225\211Plus__20220301185701917366.otf";
  const char *eot_path = "build/out/0213-head-parity.eot";
  time_t before_unix_time;
  time_t after_unix_time;
  byte_buffer_t eot = {};
  sfnt_font_t decoded = {};
  sfnt_table_t *head = NULL;
  uint8_t *serialized_sfnt = NULL;
  size_t serialized_sfnt_size = 0u;
  uint64_t created = 0u;
  uint64_t modified = 0u;
  uint32_t check_sum_adjustment = 0u;
  uint64_t min_expected = 0u;
  uint64_t max_expected = 0u;

  if (!ensure_build_output_dir()) {
    return;
  }

  sfnt_font_init(&decoded);
  before_unix_time = time(NULL);

  ASSERT_OK(mtx_encode_ttf_file(source_path, &eot));
  ASSERT_OK(file_io_write_all(eot_path, eot.data, eot.length));
  ASSERT_OK(mtx_decode_eot_file(eot_path, &decoded));
  after_unix_time = time(NULL);

  head = sfnt_font_get_table(&decoded, TAG_head);
  ASSERT_TRUE(head != NULL);
  ASSERT_TRUE(head->length >= 36u);

  created = ((uint64_t)read_u32be(head->data + 20u) << 32) |
            (uint64_t)read_u32be(head->data + 24u);
  check_sum_adjustment = read_u32be(head->data + 8);
  modified = ((uint64_t)read_u32be(head->data + 28u) << 32) |
             (uint64_t)read_u32be(head->data + 32u);

  min_expected = before_unix_time > 0 ? (uint64_t)before_unix_time + (uint64_t)MAC_EPOCH_OFFSET : 0u;
  max_expected = after_unix_time > 0 ? (uint64_t)after_unix_time + (uint64_t)MAC_EPOCH_OFFSET + 1u
                                     : UINT64_MAX;

  ASSERT_TRUE(created != 0u);
  ASSERT_TRUE(modified != 0u);
  ASSERT_TRUE(created == modified);
  ASSERT_TRUE(created >= min_expected);
  ASSERT_TRUE(created <= max_expected);
  ASSERT_TRUE(read_u32be(head->data + 24u) != MAC_EPOCH_OFFSET_LOW32);
  ASSERT_TRUE(read_u32be(head->data + 32u) != MAC_EPOCH_OFFSET_LOW32);
  ASSERT_OK(sfnt_writer_serialize(&decoded, &serialized_sfnt, &serialized_sfnt_size));
  ASSERT_TRUE(serialized_sfnt != NULL);
  ASSERT_TRUE(calc_checksum(serialized_sfnt, serialized_sfnt_size) ==
              SFNT_CHECKSUM_MAGIC);
  ASSERT_TRUE(check_sum_adjustment != 0u);

  free(serialized_sfnt);
  sfnt_font_destroy(&decoded);
  byte_buffer_destroy(&eot);
}

extern "C" void register_otf_parity_tests(void) {
  test_register("test_otf_cff_roundtrip_preserves_expected_post_and_hhea_fields",
                test_otf_cff_roundtrip_preserves_expected_post_and_hhea_fields);
  test_register("test_otf_cff_roundtrip_head_fields_look_serialized",
                test_otf_cff_roundtrip_head_fields_look_serialized);
}
