#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

extern "C" {
#include "../src/byte_io.h"
#include "../src/cff_reader.h"
#include "../src/cff_types.h"
#include "../src/cff_variation.h"
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
eot_status_t otf_convert_find_first_failed_status_for_testing(
    const eot_status_t *statuses, size_t status_count, size_t *out_glyph_index);
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

#define TAG_glyf 0x676c7966u
#define TAG_avar 0x61766172u
#define TAG_CFF  0x43464620u
#define TAG_CFF2 0x43464632u
#define TAG_fvar 0x66766172u
#define TAG_maxp 0x6d617870u

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

/* Fixture source: fonttools/Tests/ttx/data/TestOTF.otf */
static void test_otf_static_fixture_is_detected_as_otto(void) {
  sfnt_font_t font;
  ASSERT_OK(sfnt_reader_load_file("testdata/cff-static.otf", &font));
  ASSERT_TRUE(!sfnt_font_has_table(&font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_CFF));
  sfnt_font_destroy(&font);
}

/* Fixture source: fonttools/Tests/varLib/data/master_ttx_varfont_otf/TestCFF2VF.ttx */
static void test_otf_cff2_fixture_is_detected_as_variable_source(void) {
  sfnt_font_t font;
  ASSERT_OK(sfnt_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_TRUE(!sfnt_font_has_table(&font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_CFF2));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_fvar));
  sfnt_font_destroy(&font);
}

static void test_otf_convert_shared_types_are_usable(void) {
  cff_point_t point = {1.0, 2.0};
  cubic_curve_t curve = {point, point, point, point};
  variation_axis_value_t axis = {"wght", 400.0f, 400.0f, 0.0f};
  variation_location_t location = {&axis, 1};
  otf_convert_options_t options = {1.0, 1, 2};
  ASSERT_TRUE(curve.p3.y == 2.0);
  ASSERT_TRUE(location.num_axes == 1);
  ASSERT_TRUE(options.post_format == 2);
}

static void test_otf_static_cff_encode_to_eot(void) {
  byte_buffer_t eot;
  sfnt_font_t decoded;

  if (!ensure_build_output_dir()) {
    return;
  }

  ASSERT_OK(mtx_encode_ttf_file("testdata/cff-static.otf", &eot));
  ASSERT_OK(file_io_write_all("build/out/cff-static.eot", eot.data, eot.length));
  ASSERT_OK(mtx_decode_eot_file("build/out/cff-static.eot", &decoded));
  ASSERT_TRUE(sfnt_font_has_table(&decoded, TAG_glyf));
  sfnt_font_destroy(&decoded);
  byte_buffer_destroy(&eot);
}

static void test_otf_convert_static_cff_matches_across_single_and_threaded_runtime_modes(void) {
  file_buffer_t otf = {};
  sfnt_font_t source_font = {};
  sfnt_font_t single_thread_font = {};
  sfnt_font_t threaded_font = {};
  sfnt_table_t *maxp = NULL;
  uint8_t *single_thread_bytes = NULL;
  uint8_t *threaded_bytes = NULL;
  size_t single_thread_size = 0u;
  size_t threaded_size = 0u;
  size_t glyph_count = 0u;
  size_t threaded_request = 4u;
  eot_status_t status = EOT_OK;

  sfnt_font_init(&source_font);
  sfnt_font_init(&single_thread_font);
  sfnt_font_init(&threaded_font);

  parallel_runtime_clear_test_env();
  ASSERT_OK(file_io_read_all("testdata/cff-static.otf", &otf));
  ASSERT_OK(sfnt_reader_parse(otf.data, otf.length, &source_font));
  maxp = sfnt_font_get_table(&source_font, TAG_maxp);
  ASSERT_TRUE(maxp != NULL);
  ASSERT_TRUE(maxp->length >= 6u);
  glyph_count = (size_t)read_u16be(maxp->data + 4u);
  ASSERT_TRUE(glyph_count > 1u);

  status = parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8");
  if (status != EOT_OK) {
    char msg[256];
    snprintf(msg, sizeof(msg),
             "assertion failed: parallel_runtime_set_test_env(\"EOT_TOOL_THREADS\", \"8\") returned %d",
             status);
    test_fail_with_message(msg);
    goto cleanup;
  }
  ASSERT_OK(parallel_runtime_set_requested_mode("single"));

  status = otf_convert_to_truetype_sfnt(otf.data, otf.length, NULL, &single_thread_font);
  parallel_runtime_clear_test_env();
  if (status != EOT_OK) {
    char msg[256];
    snprintf(msg, sizeof(msg),
             "assertion failed: otf_convert_to_truetype_sfnt(otf.data, otf.length, NULL, &single_thread_font) returned %d",
             status);
    test_fail_with_message(msg);
    goto cleanup;
  }

  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), glyph_count);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 1u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "single");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "requested-single");

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "4"));
  ASSERT_OK(otf_convert_to_truetype_sfnt(otf.data, otf.length, NULL, &threaded_font));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), glyph_count);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), threaded_request);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(),
                 glyph_count < threaded_request ? glyph_count : threaded_request);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "threaded");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(),
               glyph_count < threaded_request ? "task-count-clamped" : "");
  ASSERT_OK(sfnt_writer_serialize(&single_thread_font, &single_thread_bytes,
                                  &single_thread_size));
  ASSERT_OK(sfnt_writer_serialize(&threaded_font, &threaded_bytes, &threaded_size));
  ASSERT_EQ_SIZE(single_thread_size, threaded_size);
  ASSERT_TRUE(memcmp(single_thread_bytes, threaded_bytes, single_thread_size) == 0);

cleanup:
  parallel_runtime_clear_test_env();
  free(threaded_bytes);
  free(single_thread_bytes);
  sfnt_font_destroy(&threaded_font);
  sfnt_font_destroy(&single_thread_font);
  sfnt_font_destroy(&source_font);
  file_io_free(&otf);
}

static void test_otf_cff2_instance_encode_to_fntdata(void) {
  file_buffer_t otf = {};
  sfnt_font_t converted = {};
  cff_font_t cff_font = {};
  variation_location_t location = {};
  byte_buffer_t eot = {};

  sfnt_font_init(&converted);
  cff_font_init(&cff_font);
  variation_location_init(&location);

  ASSERT_OK(file_io_read_all("testdata/cff2-variable.otf", &otf));
  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &cff_font));
  ASSERT_OK(variation_location_init_from_axis_map(&location, "wght=700"));
  ASSERT_OK(cff_variation_resolve_location(&cff_font, &location));
  ASSERT_OK(otf_convert_to_truetype_sfnt(otf.data, otf.length, &location,
                                         &converted));
  ASSERT_TRUE(sfnt_font_has_table(&converted, TAG_glyf));
  ASSERT_TRUE(!sfnt_font_has_table(&converted, TAG_CFF2));
  ASSERT_TRUE(!sfnt_font_has_table(&converted, TAG_avar));
  ASSERT_TRUE(!sfnt_font_has_table(&converted, TAG_fvar));
  ASSERT_OK(mtx_encode_font(&converted, &eot));
  ASSERT_TRUE(eot.length > 0);

  sfnt_font_destroy(&converted);
  cff_font_destroy(&cff_font);
  variation_location_destroy(&location);
  file_io_free(&otf);
  byte_buffer_destroy(&eot);
}

static void test_parallel_glyph_aggregation_returns_lowest_failing_glyph_index(void) {
  const eot_status_t statuses[] = {
      EOT_OK, EOT_ERR_CORRUPT_DATA, EOT_OK, EOT_ERR_ALLOCATION};
  size_t failing_glyph_index = 0u;
  eot_status_t status = otf_convert_find_first_failed_status_for_testing(
      statuses, sizeof(statuses) / sizeof(statuses[0]), &failing_glyph_index);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
  ASSERT_EQ_SIZE(failing_glyph_index, 1u);
}

extern "C" void register_otf_convert_tests(void) {
  test_register("test_otf_static_fixture_is_detected_as_otto",
                test_otf_static_fixture_is_detected_as_otto);
  test_register("test_otf_cff2_fixture_is_detected_as_variable_source",
                test_otf_cff2_fixture_is_detected_as_variable_source);
  test_register("test_otf_convert_shared_types_are_usable",
                test_otf_convert_shared_types_are_usable);
  test_register("test_otf_static_cff_encode_to_eot",
                test_otf_static_cff_encode_to_eot);
  test_register("test_otf_convert_static_cff_matches_across_single_and_threaded_runtime_modes",
                test_otf_convert_static_cff_matches_across_single_and_threaded_runtime_modes);
  test_register("test_otf_cff2_instance_encode_to_fntdata",
                test_otf_cff2_instance_encode_to_fntdata);
  test_register("test_parallel_glyph_aggregation_returns_lowest_failing_glyph_index",
                test_parallel_glyph_aggregation_returns_lowest_failing_glyph_index);
}
