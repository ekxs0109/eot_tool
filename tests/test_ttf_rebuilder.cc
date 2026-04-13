#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern "C" {
#include "../src/byte_io.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_reader.h"
#include "../src/sfnt_writer.h"
#include "../src/tt_rebuilder.h"
void test_register(const char *name, void (*fn)(void));
void test_fail_with_message(const char *message);
}

#define ASSERT_OK(expr) do { \
  eot_status_t status__ = (expr); \
  if (status__ != EOT_OK) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s returned %d", #expr, status__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s", #expr); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define TAG_glyf 0x676c7966u
#define TAG_head 0x68656164u
#define TAG_hhea 0x68686561u
#define TAG_hmtx 0x686d7478u
#define TAG_loca 0x6c6f6361u
#define TAG_maxp 0x6d617870u
#define TAG_post 0x706f7374u

static int16_t read_s16be_local(const uint8_t *data) {
  return (int16_t)read_u16be(data);
}

static char *test_strdup(const char *value) {
  size_t length = strlen(value) + 1u;
  char *copy = (char *)malloc(length);
  if (copy != NULL) {
    memcpy(copy, value, length);
  }
  return copy;
}

static eot_status_t test_init_empty_outline(tt_glyph_outline_t *outline,
                                            int16_t advance_width,
                                            const char *glyph_name) {
  if (outline == NULL || glyph_name == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  outline->contours = NULL;
  outline->num_contours = 0;
  outline->advance_width = (uint16_t)advance_width;
  outline->glyph_name = test_strdup(glyph_name);
  if (outline->glyph_name == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  return EOT_OK;
}

static eot_status_t test_init_outline(tt_glyph_outline_t *outline,
                                      const tt_outline_point_t *points,
                                      size_t point_count,
                                      int16_t advance_width,
                                      const char *glyph_name) {
  tt_contour_t *contours;
  tt_outline_point_t *point_copy;

  if (outline == NULL || points == NULL || point_count == 0 || glyph_name == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  outline->contours = NULL;
  outline->num_contours = 0;
  outline->advance_width = 0;
  outline->glyph_name = NULL;

  contours = (tt_contour_t *)calloc(1u, sizeof(tt_contour_t));
  point_copy = (tt_outline_point_t *)malloc(point_count * sizeof(tt_outline_point_t));
  outline->glyph_name = test_strdup(glyph_name);
  if (contours == NULL || point_copy == NULL || outline->glyph_name == NULL) {
    free(contours);
    free(point_copy);
    free(outline->glyph_name);
    outline->glyph_name = NULL;
    return EOT_ERR_ALLOCATION;
  }

  memcpy(point_copy, points, point_count * sizeof(tt_outline_point_t));
  contours[0].points = point_copy;
  contours[0].num_points = point_count;
  outline->contours = contours;
  outline->num_contours = 1;
  outline->advance_width = (uint16_t)advance_width;
  return EOT_OK;
}

static eot_status_t test_make_simple_quadratic_outline(tt_glyph_outline_t *outline) {
  const tt_outline_point_t points[] = {
      {0, 0, 1},
      {40, 80, 0},
      {80, 0, 1},
      {80, 60, 1},
      {0, 60, 1},
  };
  return test_init_outline(outline, points, sizeof(points) / sizeof(points[0]),
                           160, ".notdef");
}

static eot_status_t test_make_negative_xmin_outline(tt_glyph_outline_t *outline) {
  const tt_outline_point_t points[] = {
      {-40, 0, 1},
      {0, 80, 0},
      {40, 0, 1},
      {0, -20, 1},
  };
  return test_init_outline(outline, points, sizeof(points) / sizeof(points[0]),
                           180, ".notdef");
}

static eot_status_t test_make_positive_bbox_outline(tt_glyph_outline_t *outline,
                                                    const char *glyph_name) {
  const tt_outline_point_t points[] = {
      {20, 10, 1},
      {50, 90, 0},
      {70, 30, 1},
      {30, 20, 1},
  };
  return test_init_outline(outline, points, sizeof(points) / sizeof(points[0]),
                           120, glyph_name);
}

static eot_status_t test_build_rebuilt_font_with_negative_xmin(sfnt_font_t *rebuilt) {
  tt_glyph_outline_t outline = {};
  eot_status_t status = test_make_negative_xmin_outline(&outline);
  if (status == EOT_OK) {
    status = tt_rebuilder_build_font(&outline, 1, rebuilt);
  }
  tt_glyph_outline_destroy(&outline);
  return status;
}

static int test_hmtx_lsb_matches_xmin(sfnt_font_t *rebuilt) {
  sfnt_table_t *head = sfnt_font_get_table(rebuilt, TAG_head);
  sfnt_table_t *loca = sfnt_font_get_table(rebuilt, TAG_loca);
  sfnt_table_t *glyf = sfnt_font_get_table(rebuilt, TAG_glyf);
  sfnt_table_t *hmtx = sfnt_font_get_table(rebuilt, TAG_hmtx);
  int16_t index_to_loca_format;
  size_t glyph_offset;
  size_t next_offset;
  int16_t xmin;
  int16_t lsb;

  if (head == NULL || loca == NULL || glyf == NULL || hmtx == NULL ||
      head->length < 52 || hmtx->length < 4) {
    return 0;
  }

  index_to_loca_format = read_s16be_local(head->data + 50);
  if (index_to_loca_format == 0) {
    if (loca->length < 4) {
      return 0;
    }
    glyph_offset = (size_t)read_u16be(loca->data) * 2u;
    next_offset = (size_t)read_u16be(loca->data + 2) * 2u;
  } else if (index_to_loca_format == 1) {
    if (loca->length < 8) {
      return 0;
    }
    glyph_offset = (size_t)read_u32be(loca->data);
    next_offset = (size_t)read_u32be(loca->data + 4);
  } else {
    return 0;
  }

  if (next_offset < glyph_offset || next_offset > glyf->length ||
      next_offset - glyph_offset < 10) {
    return 0;
  }

  xmin = read_s16be_local(glyf->data + glyph_offset + 2u);
  lsb = read_s16be_local(hmtx->data + 2u);
  return xmin == lsb;
}

static int test_head_bbox_matches(sfnt_font_t *rebuilt,
                                  int16_t x_min,
                                  int16_t y_min,
                                  int16_t x_max,
                                  int16_t y_max) {
  sfnt_table_t *head = sfnt_font_get_table(rebuilt, TAG_head);
  if (head == NULL || head->length < 54) {
    return 0;
  }

  return read_s16be_local(head->data + 36u) == x_min &&
         read_s16be_local(head->data + 38u) == y_min &&
         read_s16be_local(head->data + 40u) == x_max &&
         read_s16be_local(head->data + 42u) == y_max;
}

static int test_hhea_aggregates_match(sfnt_font_t *rebuilt,
                                      uint16_t advance_width_max,
                                      int16_t descender,
                                      int16_t min_left_side_bearing,
                                      int16_t x_max_extent) {
  sfnt_table_t *hhea = sfnt_font_get_table(rebuilt, TAG_hhea);
  if (hhea == NULL || hhea->length < 18) {
    return 0;
  }

  return read_u16be(hhea->data + 10u) == advance_width_max &&
         read_s16be_local(hhea->data + 6u) == descender &&
         read_s16be_local(hhea->data + 12u) == min_left_side_bearing &&
         read_s16be_local(hhea->data + 16u) == x_max_extent;
}

static void test_ttf_rebuilder_creates_parseable_glyf_and_loca_tables(void) {
  tt_glyph_outline_t outline = {};
  sfnt_font_t rebuilt;
  sfnt_font_t reparsed;
  uint8_t *font_data = NULL;
  size_t font_size = 0;
  ASSERT_OK(test_make_simple_quadratic_outline(&outline));
  ASSERT_OK(tt_rebuilder_build_font(&outline, 1, &rebuilt));
  ASSERT_TRUE(sfnt_font_has_table(&rebuilt, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&rebuilt, TAG_loca));
  ASSERT_TRUE(sfnt_font_has_table(&rebuilt, TAG_maxp));
  ASSERT_OK(sfnt_writer_serialize(&rebuilt, &font_data, &font_size));
  ASSERT_OK(sfnt_reader_parse(font_data, font_size, &reparsed));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_head));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_hhea));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_hmtx));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_loca));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_maxp));
  ASSERT_TRUE(sfnt_font_has_table(&reparsed, TAG_post));
  sfnt_font_destroy(&reparsed);
  free(font_data);
  sfnt_font_destroy(&rebuilt);
  tt_glyph_outline_destroy(&outline);
}

static void test_ttf_rebuilder_updates_hmtx_lsb_from_bbox(void) {
  sfnt_font_t rebuilt;
  ASSERT_OK(test_build_rebuilt_font_with_negative_xmin(&rebuilt));
  ASSERT_TRUE(test_hmtx_lsb_matches_xmin(&rebuilt));
  sfnt_font_destroy(&rebuilt);
}

static void test_ttf_rebuilder_skips_empty_notdef_for_head_and_hhea_aggregates(void) {
  tt_glyph_outline_t outlines[2] = {};
  sfnt_font_t rebuilt;
  ASSERT_OK(test_init_empty_outline(&outlines[0], 80, ".notdef"));
  ASSERT_OK(test_make_positive_bbox_outline(&outlines[1], "visible"));
  ASSERT_OK(tt_rebuilder_build_font(outlines, 2, &rebuilt));
  ASSERT_TRUE(test_head_bbox_matches(&rebuilt, 20, 10, 70, 90));
  ASSERT_TRUE(test_hhea_aggregates_match(&rebuilt, 120, 10, 20, 70));
  sfnt_font_destroy(&rebuilt);
  tt_glyph_outline_destroy(&outlines[0]);
  tt_glyph_outline_destroy(&outlines[1]);
}

extern "C" void register_ttf_rebuilder_tests(void) {
  test_register("test_ttf_rebuilder_creates_parseable_glyf_and_loca_tables",
                test_ttf_rebuilder_creates_parseable_glyf_and_loca_tables);
  test_register("test_ttf_rebuilder_updates_hmtx_lsb_from_bbox",
                test_ttf_rebuilder_updates_hmtx_lsb_from_bbox);
  test_register("test_ttf_rebuilder_skips_empty_notdef_for_head_and_hhea_aggregates",
                test_ttf_rebuilder_skips_empty_notdef_for_head_and_hhea_aggregates);
}
