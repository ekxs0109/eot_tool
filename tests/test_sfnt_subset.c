#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/byte_io.h"
#include "../src/mtx_decode.h"
#include "../src/parallel_runtime.h"
#include "../src/sfnt_reader.h"
#include "../src/sfnt_subset.h"
#include "../src/sfnt_writer.h"
#include "../src/subset_backend_harfbuzz.h"

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

#define ASSERT_FALSE(expr) ASSERT_TRUE(!(expr))

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_EQ_SIZE(actual, expected) do { \
  if ((size_t)(actual) != (size_t)(expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), \
             "assertion failed: %s == %s (actual: %zu, expected: %zu)", \
             #actual, #expected, (size_t)(actual), (size_t)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_STREQ(actual, expected) do { \
  const char *actual__ = (actual); \
  const char *expected__ = (expected); \
  if (((actual__ == NULL) != (expected__ == NULL)) || \
      (actual__ != NULL && strcmp(actual__, expected__) != 0)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %s, expected: %s)", \
             #actual, #expected, actual__ != NULL ? actual__ : "(null)", \
             expected__ != NULL ? expected__ : "(null)"); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define TAG_cmap 0x636d6170u
#define TAG_cvt 0x63767420u
#define TAG_glyf 0x676c7966u
#define TAG_hdmx 0x68646d78u
#define TAG_head 0x68656164u
#define TAG_hhea 0x68686561u
#define TAG_hmtx 0x686d7478u
#define TAG_loca 0x6c6f6361u
#define TAG_maxp 0x6d617870u
#define TAG_VDMX 0x56444d58u

static sfnt_table_t *test_get_table(const sfnt_font_t *font, uint32_t tag) {
  return sfnt_font_get_table((sfnt_font_t *)font, tag);
}

static uint16_t test_read_num_glyphs(const sfnt_font_t *font) {
  sfnt_table_t *maxp = test_get_table(font, TAG_maxp);

  if (maxp == NULL || maxp->length < 6) {
    test_fail_with_message("font is missing a valid maxp table");
    return 0;
  }

  return read_u16be(maxp->data + 4);
}

static int16_t test_read_index_to_loca_format(const sfnt_font_t *font) {
  sfnt_table_t *head = test_get_table(font, TAG_head);

  if (head == NULL || head->length < 52) {
    test_fail_with_message("font is missing a valid head table");
    return -1;
  }

  return (int16_t)read_u16be(head->data + 50);
}

static size_t test_read_glyph_length(const sfnt_font_t *font, uint16_t gid) {
  sfnt_table_t *loca = test_get_table(font, TAG_loca);
  sfnt_table_t *glyf = test_get_table(font, TAG_glyf);
  int16_t index_to_loca_format;
  size_t start;
  size_t end;

  if (loca == NULL || glyf == NULL) {
    test_fail_with_message("font is missing loca/glyf tables");
    return 0;
  }

  index_to_loca_format = test_read_index_to_loca_format(font);
  if (index_to_loca_format == 0) {
    size_t offset = (size_t)gid * 2u;
    if (offset + 4u > loca->length) {
      test_fail_with_message("short loca table");
      return 0;
    }
    start = (size_t)read_u16be(loca->data + offset) * 2u;
    end = (size_t)read_u16be(loca->data + offset + 2u) * 2u;
  } else if (index_to_loca_format == 1) {
    size_t offset = (size_t)gid * 4u;
    if (offset + 8u > loca->length) {
      test_fail_with_message("short loca table");
      return 0;
    }
    start = (size_t)read_u32be(loca->data + offset);
    end = (size_t)read_u32be(loca->data + offset + 4u);
  } else {
    test_fail_with_message("invalid indexToLocFormat");
    return 0;
  }

  if (end < start || end > glyf->length) {
    test_fail_with_message("invalid glyf range");
    return 0;
  }

  return end - start;
}

static uint16_t test_count_expected_hmetrics(const sfnt_font_t *font) {
  sfnt_table_t *hmtx = test_get_table(font, TAG_hmtx);
  uint16_t num_glyphs;
  size_t expected_hmetrics;

  if (hmtx == NULL) {
    test_fail_with_message("font is missing hmtx");
    return 0;
  }

  num_glyphs = test_read_num_glyphs(font);
  if (num_glyphs == 0 || hmtx->length < (size_t)num_glyphs * 2u) {
    test_fail_with_message("invalid hmtx length");
    return 0;
  }
  if (((hmtx->length - (size_t)num_glyphs * 2u) & 1u) != 0u) {
    test_fail_with_message("misaligned hmtx length");
    return 0;
  }

  expected_hmetrics =
      (hmtx->length - (size_t)num_glyphs * 2u) / 2u;
  if (expected_hmetrics == 0 || expected_hmetrics > num_glyphs) {
    test_fail_with_message("derived invalid numberOfHMetrics");
    return 0;
  }

  return (uint16_t)expected_hmetrics;
}

static int test_subset_font_has_zero_length_glyph_holes(const sfnt_font_t *font) {
  uint16_t num_glyphs = test_read_num_glyphs(font);
  uint16_t gid;

  if (num_glyphs <= 35) {
    return 0;
  }
  if (test_read_glyph_length(font, 0) == 0) {
    return 0;
  }
  if (test_read_glyph_length(font, 35) == 0) {
    return 0;
  }

  for (gid = 1; gid < num_glyphs; gid++) {
    size_t glyph_length = test_read_glyph_length(font, gid);
    if (gid == 35) {
      if (glyph_length == 0) {
        return 0;
      }
      continue;
    }
    if (glyph_length != 0) {
      return 0;
    }
  }

  return 1;
}

static int test_subset_plan_contains_gid(const subset_plan_t *plan, uint16_t gid) {
  size_t i;

  if (plan == NULL || plan->included_glyph_ids == NULL) {
    return 0;
  }

  for (i = 0; i < plan->num_glyphs; i++) {
    if (plan->included_glyph_ids[i] == gid) {
      return 1;
    }
  }

  return 0;
}

static eot_status_t test_init_request_for_unicodes(subset_request_t *request,
                                                   const char *selection) {
  size_t length;

  if (request == NULL || selection == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  subset_request_init(request);
  length = strlen(selection);
  request->selection_data = (char *)malloc(length + 1);
  if (request->selection_data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  memcpy(request->selection_data, selection, length + 1);
  request->selection_mode = SUBSET_SELECTION_UNICODES;
  return EOT_OK;
}

static eot_status_t test_serialize_font(const sfnt_font_t *font,
                                        uint8_t **out_data,
                                        size_t *out_size) {
  if (font == NULL || out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_data = NULL;
  *out_size = 0;
  return sfnt_writer_serialize((sfnt_font_t *)font, out_data, out_size);
}

#define ASSERT_OK_CLEANUP(expr) do { \
  status = (expr); \
  if (status != EOT_OK) { \
    snprintf(failure, sizeof(failure), "assertion failed: %s returned %d", #expr, status); \
    goto cleanup; \
  } \
} while (0)

#define ASSERT_TRUE_CLEANUP(expr) do { \
  if (!(expr)) { \
    snprintf(failure, sizeof(failure), "assertion failed: %s", #expr); \
    goto cleanup; \
  } \
} while (0)

#define ASSERT_EQ_SIZE_CLEANUP(actual, expected) do { \
  size_t actual__ = (size_t)(actual); \
  size_t expected__ = (size_t)(expected); \
  if (actual__ != expected__) { \
    snprintf(failure, sizeof(failure), \
             "assertion failed: %s == %s (actual: %zu, expected: %zu)", \
             #actual, #expected, actual__, expected__); \
    goto cleanup; \
  } \
} while (0)

static void test_subset_always_keeps_notdef(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_text_mode_maps_opensans_abc_to_expected_gids(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_EQ(plan.num_glyphs, 4);
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 36));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 37));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 38));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_unicode_mode_maps_opensans_basic_latin_range(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(test_init_request_for_unicodes(&request, "U+0041-0043"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_EQ(plan.num_glyphs, 4);
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 36));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 37));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 38));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_unicode_mode_rejects_invalid_scalar_values(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;
  eot_status_t status;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(test_init_request_for_unicodes(&request, "U+D800,U+110000"));

  status = sfnt_subset_plan(&font, &request, &plan);
  ASSERT_EQ(status, EOT_ERR_INVALID_ARGUMENT);

  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_unicode_mode_rejects_hex_overflow(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;
  eot_status_t status;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(test_init_request_for_unicodes(&request, "U+100000000"));

  status = sfnt_subset_plan(&font, &request, &plan);
  ASSERT_EQ(status, EOT_ERR_INVALID_ARGUMENT);

  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_glyph_id_mode_rejects_decimal_overflow(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;
  eot_status_t status;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &font));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "4294967296"));

  status = sfnt_subset_plan(&font, &request, &plan);
  ASSERT_EQ(status, EOT_ERR_INVALID_ARGUMENT);

  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

/*
 * testdata/cmap-priority.ttf is a minimized OpenSans-derived fixture with a
 * leading Windows Symbol format 4 cmap for A/B/C and a later Unicode format 4
 * cmap preserving the real OpenSans mappings (A/B/C -> 36/37/38).
 */
static void test_subset_prefers_unicode_cmap_over_symbol_format4(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/cmap-priority.ttf", &font));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 36));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 37));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 38));
  ASSERT_TRUE(!test_subset_plan_contains_gid(&plan, 1));
  ASSERT_TRUE(!test_subset_plan_contains_gid(&plan, 2));
  ASSERT_TRUE(!test_subset_plan_contains_gid(&plan, 3));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

/*
 * testdata/cmap-symbol-only.ttf is derived from cmap-priority.ttf with only
 * the leading Windows Symbol format 4 cmap record exposed. It maps A/B/C to
 * glyph ids 1/2/3 and should still be usable for text selection fallback.
 */
static void test_subset_falls_back_to_symbol_cmap_when_unicode_missing(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/cmap-symbol-only.ttf", &font));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_EQ(plan.num_glyphs, 4);
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 1));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 2));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 3));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_unicode_mode_rejects_symbol_only_cmap(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;
  eot_status_t status;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/cmap-symbol-only.ttf", &font));
  ASSERT_OK(test_init_request_for_unicodes(&request, "U+0041-0043"));
  status = sfnt_subset_plan(&font, &request, &plan);
  ASSERT_EQ(status, EOT_ERR_CORRUPT_DATA);

  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_adds_composite_glyph_dependencies(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/composite-subset.ttf", &font));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "1"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 2));
  ASSERT_TRUE(plan.added_composite_dependencies > 0);

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_renumbers_requested_glyphs_by_default(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/composite-subset.ttf", &font));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "10,15"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_TRUE(!plan.keep_gids);
  ASSERT_EQ(plan.old_to_new_gid[0], 0);
  ASSERT_EQ(plan.old_to_new_gid[10], 1);
  ASSERT_EQ(plan.old_to_new_gid[15], 2);
  ASSERT_EQ(plan.old_to_new_gid[1], SUBSET_GID_NOT_INCLUDED);

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_keep_gids_preserves_original_mapping(void) {
  sfnt_font_t font;
  subset_request_t request;
  subset_plan_t plan;

  memset(&plan, 0, sizeof(plan));

  ASSERT_OK(sfnt_reader_load_file("testdata/composite-subset.ttf", &font));
  ASSERT_OK(subset_request_init_for_glyph_ids_keep(&request, "10,15"));
  ASSERT_OK(sfnt_subset_plan(&font, &request, &plan));
  ASSERT_TRUE(plan.keep_gids);
  ASSERT_EQ(plan.old_to_new_gid[0], 0);
  ASSERT_EQ(plan.old_to_new_gid[10], 10);
  ASSERT_EQ(plan.old_to_new_gid[15], 15);
  ASSERT_EQ(plan.old_to_new_gid[1], SUBSET_GID_NOT_INCLUDED);

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&font);
}

static void test_subset_rebuilds_hhea_and_hmtx_consistently(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_font(&input, &request, &output));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_hhea));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_hmtx));
  ASSERT_EQ(read_u16be(sfnt_font_get_table(&output, TAG_hhea)->data + 34),
            test_count_expected_hmetrics(&output));

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_font_text_produces_parseable_core_tables(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  sfnt_font_t reparsed;
  subset_request_t request;
  uint8_t *subset_bytes = NULL;
  size_t subset_size = 0;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_font(&input, &request, &output));
  ASSERT_OK(sfnt_writer_serialize(&output, &subset_bytes, &subset_size));
  ASSERT_OK(sfnt_reader_parse(subset_bytes, subset_size, &reparsed));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_head));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_cmap));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_hhea));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_hmtx));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_loca));

  free(subset_bytes);
  sfnt_font_destroy(&reparsed);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
  subset_request_destroy(&request);
}

static void test_subset_backend_uses_planned_symbol_text_fallback(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_plan_t plan;

  subset_plan_init(&plan);
  ASSERT_OK(sfnt_reader_load_file("testdata/cmap-symbol-only.ttf", &input));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_plan(&input, &request, &plan));
  ASSERT_EQ(plan.num_glyphs, 4);
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 0));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 1));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 2));
  ASSERT_TRUE(test_subset_plan_contains_gid(&plan, 3));
  ASSERT_OK(subset_backend_harfbuzz_run(&input, &plan, &output, NULL));
  ASSERT_EQ(test_read_num_glyphs(&output), 4);
  ASSERT_EQ(test_read_glyph_length(&output, 1), 0);
  ASSERT_EQ(test_read_glyph_length(&output, 2), 0);
  ASSERT_EQ(test_read_glyph_length(&output, 3), 0);

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_backend_keep_gids_follows_planned_mapping(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_plan_t plan;

  subset_plan_init(&plan);
  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK(subset_request_init_for_glyph_ids_keep(&request, "0,35"));
  ASSERT_OK(sfnt_subset_plan(&input, &request, &plan));
  ASSERT_OK(subset_backend_harfbuzz_run(&input, &plan, &output, NULL));
  ASSERT_TRUE(test_subset_font_has_zero_length_glyph_holes(&output));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_backend_preserves_wingdings_extra_tables_before_api_policy(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_plan_t plan;

  subset_plan_init(&plan);
  ASSERT_OK(mtx_decode_eot_file("testdata/wingdings3.eot", &input));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "0,1,2"));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_hdmx));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_VDMX));
  ASSERT_OK(sfnt_subset_plan(&input, &request, &plan));
  ASSERT_OK(subset_backend_harfbuzz_run(&input, &plan, &output, NULL));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_hdmx));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_VDMX));

  subset_plan_destroy(&plan);
  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_font_with_warnings_keeps_zero_warning_flags_when_unused(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_warnings_t warnings;

  warnings.dropped_hdmx = 7;
  warnings.dropped_vdmx = 9;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_font_with_warnings(&input, &request, &output, &warnings));
  ASSERT_EQ(warnings.dropped_hdmx, 0);
  ASSERT_EQ(warnings.dropped_vdmx, 0);

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_wingdings_sets_warning_flags_for_unsupported_extra_tables(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_warnings_t warnings;

  ASSERT_OK(mtx_decode_eot_file("testdata/wingdings3.eot", &input));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_hdmx));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_VDMX));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "0,1,2"));
  subset_warnings_init(&warnings);
  ASSERT_OK(sfnt_subset_font_with_warnings(&input, &request, &output, &warnings));
  ASSERT_TRUE(warnings.dropped_hdmx);
  ASSERT_TRUE(warnings.dropped_vdmx);

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_wingdings_output_omits_hdmx_and_vdmx_in_memory(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  subset_warnings_t warnings;

  ASSERT_OK(mtx_decode_eot_file("testdata/wingdings3.eot", &input));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_hdmx));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_VDMX));
  ASSERT_OK(subset_request_init_for_glyph_ids(&request, "0,1,2"));
  subset_warnings_init(&warnings);
  ASSERT_OK(sfnt_subset_font_with_warnings(&input, &request, &output, &warnings));
  ASSERT_FALSE(sfnt_font_has_table(&output, TAG_hdmx));
  ASSERT_FALSE(sfnt_font_has_table(&output, TAG_VDMX));

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_output_policy_drops_warning_tables_and_keeps_cvt(void) {
  sfnt_font_t output;
  subset_warnings_t warnings;
  static const uint8_t cvt_data[] = {0x00, 0x10};
  static const uint8_t hdmx_data[] = {0x00, 0x01};
  static const uint8_t vdmx_data[] = {0x00, 0x01};

  sfnt_font_init(&output);
  subset_warnings_init(&warnings);

  ASSERT_OK(sfnt_font_add_table(&output, TAG_cvt, cvt_data, sizeof(cvt_data)));
  ASSERT_OK(sfnt_font_add_table(&output, TAG_hdmx, hdmx_data, sizeof(hdmx_data)));
  ASSERT_OK(sfnt_font_add_table(&output, TAG_VDMX, vdmx_data, sizeof(vdmx_data)));
  ASSERT_OK(sfnt_subset_apply_output_table_policy(&output, &warnings));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_cvt));
  ASSERT_FALSE(sfnt_font_has_table(&output, TAG_hdmx));
  ASSERT_FALSE(sfnt_font_has_table(&output, TAG_VDMX));
  ASSERT_TRUE(warnings.dropped_hdmx);
  ASSERT_TRUE(warnings.dropped_vdmx);

  sfnt_font_destroy(&output);
}

static void test_subset_output_policy_rejects_null_output(void) {
  subset_warnings_t warnings;

  subset_warnings_init(&warnings);
  ASSERT_EQ(sfnt_subset_apply_output_table_policy(NULL, &warnings),
            EOT_ERR_INVALID_ARGUMENT);
}

static void test_subset_output_preserves_cvt_when_present_in_backend_result(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_TRUE(sfnt_font_has_table(&input, TAG_cvt));
  ASSERT_OK(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK(sfnt_subset_font(&input, &request, &output));
  ASSERT_TRUE(sfnt_font_has_table(&output, TAG_cvt));

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_keep_gids_preserves_zero_length_holes(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK(subset_request_init_for_glyph_ids_keep(&request, "0,35"));
  ASSERT_OK(sfnt_subset_font(&input, &request, &output));
  ASSERT_TRUE(test_subset_font_has_zero_length_glyph_holes(&output));

  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
}

static void test_subset_runtime_reports_single_shared_task_when_parallelism_requested(void) {
  sfnt_font_t input;
  sfnt_font_t output;
  subset_request_t request;
  eot_status_t status = EOT_OK;
  char failure[256] = {0};

  sfnt_font_init(&input);
  sfnt_font_init(&output);
  subset_request_init(&request);
  parallel_runtime_clear_test_env();
  ASSERT_OK_CLEANUP(parallel_runtime_run_indexed_tasks(0u, NULL, NULL));
  ASSERT_OK_CLEANUP(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK_CLEANUP(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK_CLEANUP(subset_request_init_for_text(&request, "ABC"));
  ASSERT_OK_CLEANUP(sfnt_subset_font(&input, &request, &output));
  ASSERT_TRUE_CLEANUP(parallel_runtime_last_run_task_count() > 0u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_requested_threads(), 8u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_effective_threads(), 1u);

cleanup:
  parallel_runtime_clear_test_env();
  subset_request_destroy(&request);
  sfnt_font_destroy(&output);
  sfnt_font_destroy(&input);
  if (failure[0] != '\0') {
    test_fail_with_message(failure);
  }
}

static void test_subset_matches_with_single_and_parallel_runtime_requests(void) {
  sfnt_font_t input;
  sfnt_font_t serial_output;
  sfnt_font_t parallel_output;
  subset_request_t request;
  uint8_t *serial_bytes = NULL;
  size_t serial_size = 0;
  uint8_t *parallel_bytes = NULL;
  size_t parallel_size = 0;
  eot_status_t status = EOT_OK;
  char failure[256] = {0};

  sfnt_font_init(&input);
  sfnt_font_init(&serial_output);
  sfnt_font_init(&parallel_output);
  subset_request_init(&request);
  parallel_runtime_clear_test_env();
  ASSERT_OK_CLEANUP(parallel_runtime_run_indexed_tasks(0u, NULL, NULL));
  ASSERT_OK_CLEANUP(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &input));
  ASSERT_OK_CLEANUP(subset_request_init_for_text(&request, "ABC"));

  ASSERT_OK_CLEANUP(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "1"));
  ASSERT_OK_CLEANUP(sfnt_subset_font(&input, &request, &serial_output));
  ASSERT_TRUE_CLEANUP(parallel_runtime_last_run_task_count() > 0u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_requested_threads(), 1u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_OK_CLEANUP(test_serialize_font(&serial_output, &serial_bytes, &serial_size));

  parallel_runtime_clear_test_env();
  ASSERT_OK_CLEANUP(parallel_runtime_run_indexed_tasks(0u, NULL, NULL));
  ASSERT_OK_CLEANUP(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK_CLEANUP(sfnt_subset_font(&input, &request, &parallel_output));
  ASSERT_TRUE_CLEANUP(parallel_runtime_last_run_task_count() > 0u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_requested_threads(), 8u);
  ASSERT_EQ_SIZE_CLEANUP(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_OK_CLEANUP(test_serialize_font(&parallel_output, &parallel_bytes, &parallel_size));

  ASSERT_EQ_SIZE_CLEANUP(serial_size, parallel_size);
  ASSERT_TRUE_CLEANUP(memcmp(serial_bytes, parallel_bytes, serial_size) == 0);

cleanup:
  parallel_runtime_clear_test_env();
  free(serial_bytes);
  free(parallel_bytes);
  subset_request_destroy(&request);
  sfnt_font_destroy(&parallel_output);
  sfnt_font_destroy(&serial_output);
  sfnt_font_destroy(&input);
  if (failure[0] != '\0') {
    test_fail_with_message(failure);
  }
}

void register_sfnt_subset_tests(void) {
  test_register("test_subset_always_keeps_notdef",
                test_subset_always_keeps_notdef);
  test_register("test_subset_text_mode_maps_opensans_abc_to_expected_gids",
                test_subset_text_mode_maps_opensans_abc_to_expected_gids);
  test_register("test_subset_unicode_mode_maps_opensans_basic_latin_range",
                test_subset_unicode_mode_maps_opensans_basic_latin_range);
  test_register("test_subset_unicode_mode_rejects_invalid_scalar_values",
                test_subset_unicode_mode_rejects_invalid_scalar_values);
  test_register("test_subset_unicode_mode_rejects_hex_overflow",
                test_subset_unicode_mode_rejects_hex_overflow);
  test_register("test_subset_glyph_id_mode_rejects_decimal_overflow",
                test_subset_glyph_id_mode_rejects_decimal_overflow);
  test_register("test_subset_prefers_unicode_cmap_over_symbol_format4",
                test_subset_prefers_unicode_cmap_over_symbol_format4);
  test_register("test_subset_falls_back_to_symbol_cmap_when_unicode_missing",
                test_subset_falls_back_to_symbol_cmap_when_unicode_missing);
  test_register("test_subset_unicode_mode_rejects_symbol_only_cmap",
                test_subset_unicode_mode_rejects_symbol_only_cmap);
  test_register("test_subset_adds_composite_glyph_dependencies",
                test_subset_adds_composite_glyph_dependencies);
  test_register("test_subset_renumbers_requested_glyphs_by_default",
                test_subset_renumbers_requested_glyphs_by_default);
  test_register("test_subset_keep_gids_preserves_original_mapping",
                test_subset_keep_gids_preserves_original_mapping);
  test_register("test_subset_rebuilds_hhea_and_hmtx_consistently",
                test_subset_rebuilds_hhea_and_hmtx_consistently);
  test_register("test_subset_font_text_produces_parseable_core_tables",
                test_subset_font_text_produces_parseable_core_tables);
  test_register("test_subset_backend_uses_planned_symbol_text_fallback",
                test_subset_backend_uses_planned_symbol_text_fallback);
  test_register("test_subset_backend_keep_gids_follows_planned_mapping",
                test_subset_backend_keep_gids_follows_planned_mapping);
  test_register("test_subset_backend_preserves_wingdings_extra_tables_before_api_policy",
                test_subset_backend_preserves_wingdings_extra_tables_before_api_policy);
  test_register("test_subset_font_with_warnings_keeps_zero_warning_flags_when_unused",
                test_subset_font_with_warnings_keeps_zero_warning_flags_when_unused);
  test_register("test_subset_wingdings_sets_warning_flags_for_unsupported_extra_tables",
                test_subset_wingdings_sets_warning_flags_for_unsupported_extra_tables);
  test_register("test_subset_wingdings_output_omits_hdmx_and_vdmx_in_memory",
                test_subset_wingdings_output_omits_hdmx_and_vdmx_in_memory);
  test_register("test_subset_output_policy_drops_warning_tables_and_keeps_cvt",
                test_subset_output_policy_drops_warning_tables_and_keeps_cvt);
  test_register("test_subset_output_policy_rejects_null_output",
                test_subset_output_policy_rejects_null_output);
  test_register("test_subset_output_preserves_cvt_when_present_in_backend_result",
                test_subset_output_preserves_cvt_when_present_in_backend_result);
  test_register("test_subset_keep_gids_preserves_zero_length_holes",
                test_subset_keep_gids_preserves_zero_length_holes);
  test_register("test_subset_runtime_reports_single_shared_task_when_parallelism_requested",
                test_subset_runtime_reports_single_shared_task_when_parallelism_requested);
  test_register("test_subset_matches_with_single_and_parallel_runtime_requests",
                test_subset_matches_with_single_and_parallel_runtime_requests);
}
