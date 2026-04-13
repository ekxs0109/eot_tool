#include <errno.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

extern "C" {
#include "../src/byte_io.h"
#include "../src/file_io.h"
#include "../src/mtx_decode.h"
#include "../src/mtx_encode.h"
#include "../src/otf_convert.h"
#include "../src/parallel_runtime.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_reader.h"
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

#define ASSERT_EQ_SIZE(actual, expected) do { \
  size_t actual__ = (size_t)(actual); \
  size_t expected__ = (size_t)(expected); \
  if (actual__ != expected__) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s == %s (actual=%zu expected=%zu)", \
             #actual, #expected, actual__, expected__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_STREQ(actual, expected) do { \
  const char *actual__ = (actual); \
  const char *expected__ = (expected); \
  if (((actual__ == NULL) != (expected__ == NULL)) || \
      (actual__ != NULL && strcmp(actual__, expected__) != 0)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s == %s (actual=%s expected=%s)", \
             #actual, #expected, actual__ != NULL ? actual__ : "(null)", \
             expected__ != NULL ? expected__ : "(null)"); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define TAG_hhea 0x68686561u
#define TAG_head 0x68656164u
#define TAG_maxp 0x6d617870u
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

static void test_otf_convert_variable_instance_is_deterministic_across_runtime_modes(void) {
  file_buffer_t otf = {};
  sfnt_font_t source_font = {};
  sfnt_font_t single_font = {};
  sfnt_font_t threaded_font = {};
  sfnt_table_t *maxp = NULL;
  variation_axis_value_t axis = {"wght", 700.0f, 700.0f, 0.5f};
  variation_location_t location = {&axis, 1u};
  uint8_t *single_bytes = NULL;
  uint8_t *threaded_bytes = NULL;
  size_t single_size = 0u;
  size_t threaded_size = 0u;
  size_t glyph_count = 0u;

  sfnt_font_init(&source_font);
  sfnt_font_init(&single_font);
  sfnt_font_init(&threaded_font);

  parallel_runtime_clear_test_env();
  ASSERT_OK(file_io_read_all("testdata/cff2-variable.otf", &otf));
  ASSERT_OK(sfnt_reader_parse(otf.data, otf.length, &source_font));
  maxp = sfnt_font_get_table(&source_font, TAG_maxp);
  ASSERT_TRUE(maxp != NULL);
  ASSERT_TRUE(maxp->length >= 6u);
  glyph_count = (size_t)read_u16be(maxp->data + 4u);
  ASSERT_TRUE(glyph_count > 1u);

  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_set_requested_mode("single"));
  ASSERT_OK(otf_convert_to_truetype_sfnt(otf.data, otf.length, &location, &single_font));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), glyph_count);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 1u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "single");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "requested-single");

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "4"));
  ASSERT_OK(otf_convert_to_truetype_sfnt(otf.data, otf.length, &location, &threaded_font));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), glyph_count);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 4u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(),
                 glyph_count < 4u ? glyph_count : 4u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "threaded");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(),
               glyph_count < 4u ? "task-count-clamped" : "");

  ASSERT_OK(sfnt_writer_serialize(&single_font, &single_bytes, &single_size));
  ASSERT_OK(sfnt_writer_serialize(&threaded_font, &threaded_bytes, &threaded_size));
  ASSERT_EQ_SIZE(single_size, threaded_size);
  ASSERT_TRUE(memcmp(single_bytes, threaded_bytes, single_size) == 0);

  free(threaded_bytes);
  free(single_bytes);
  sfnt_font_destroy(&threaded_font);
  sfnt_font_destroy(&single_font);
  sfnt_font_destroy(&source_font);
  file_io_free(&otf);
  parallel_runtime_clear_test_env();
}

extern "C" void register_otf_parity_tests(void) {
  test_register("test_otf_cff_roundtrip_preserves_expected_post_and_hhea_fields",
                test_otf_cff_roundtrip_preserves_expected_post_and_hhea_fields);
  test_register("test_otf_cff_roundtrip_head_fields_look_serialized",
                test_otf_cff_roundtrip_head_fields_look_serialized);
  test_register("test_otf_convert_variable_instance_is_deterministic_across_runtime_modes",
                test_otf_convert_variable_instance_is_deterministic_across_runtime_modes);
}
