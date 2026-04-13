#include <stdio.h>
#include <string.h>

extern "C" {
#include "../src/cff_reader.h"
#include "../src/cff_variation.h"
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

#define ASSERT_POINT_EQ(point, expected_x, expected_y) do { \
  if ((point).x != (expected_x) || (point).y != (expected_y)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), \
             "assertion failed: (%s).x == %.1f && (%s).y == %.1f; actual=(%.1f, %.1f)", \
             #point, (double)(expected_x), #point, (double)(expected_y), \
             (double)(point).x, (double)(point).y); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static int outlines_differ(const cff_glyph_outline_t *lhs,
                           const cff_glyph_outline_t *rhs) {
  if (lhs->num_cubics != rhs->num_cubics ||
      lhs->num_contours != rhs->num_contours) {
    return 1;
  }

  for (size_t i = 0; i < lhs->num_cubics; ++i) {
    if (memcmp(&lhs->cubics[i], &rhs->cubics[i], sizeof(lhs->cubics[i])) != 0) {
      return 1;
    }
  }

  for (size_t i = 0; i < lhs->num_contours; ++i) {
    if (lhs->contour_end_indices[i] != rhs->contour_end_indices[i]) {
      return 1;
    }
  }

  return 0;
}

static void test_cff_reader_extracts_static_glyph_cubic_outline(void) {
  cff_font_t font;
  cff_glyph_outline_t outline = {};

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff-static.otf", &font));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, "period", NULL, &outline));
  ASSERT_TRUE(outline.num_cubics == 4);
  ASSERT_TRUE(outline.num_contours == 1);
  ASSERT_TRUE(outline.contour_end_indices != NULL);
  ASSERT_TRUE(outline.contour_end_indices[outline.num_contours - 1] ==
              outline.num_cubics - 1);
  ASSERT_TRUE(outline.contour_end_indices[0] == 3);
  ASSERT_POINT_EQ(outline.cubics[0].p0, 55.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[0].p3, 186.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[outline.num_cubics - 1].p3, 55.0, 0.0);

  cff_glyph_outline_destroy(&outline);
  cff_font_destroy(&font);
}

static void test_cff_reader_extracts_notdef_with_multiple_closed_contours(void) {
  cff_font_t font;
  cff_glyph_outline_t outline = {};

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff-static.otf", &font));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, ".notdef", NULL, &outline));
  ASSERT_TRUE(outline.num_cubics == 8);
  ASSERT_TRUE(outline.num_contours == 2);
  ASSERT_TRUE(outline.contour_end_indices != NULL);
  ASSERT_TRUE(outline.contour_end_indices[0] == 3);
  ASSERT_TRUE(outline.contour_end_indices[1] == outline.num_cubics - 1);
  ASSERT_POINT_EQ(outline.cubics[0].p0, 450.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[3].p3, 450.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[4].p0, 500.0, 50.0);
  ASSERT_POINT_EQ(outline.cubics[7].p3, 500.0, 50.0);

  cff_glyph_outline_destroy(&outline);
  cff_font_destroy(&font);
}

static void test_cff_reader_extracts_standard_sid_ellipsis_outline(void) {
  cff_font_t font;
  cff_glyph_outline_t outline = {};

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff-static.otf", &font));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, "ellipsis", NULL, &outline));
  ASSERT_TRUE(outline.num_cubics == 12);
  ASSERT_TRUE(outline.num_contours == 3);
  ASSERT_TRUE(outline.contour_end_indices != NULL);
  ASSERT_TRUE(outline.contour_end_indices[0] == 3);
  ASSERT_TRUE(outline.contour_end_indices[1] == 7);
  ASSERT_TRUE(outline.contour_end_indices[2] == outline.num_cubics - 1);
  ASSERT_POINT_EQ(outline.cubics[0].p0, 55.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[3].p3, 55.0, 0.0);
  ASSERT_POINT_EQ(outline.cubics[4].p0, 296.0, -122.0);
  ASSERT_POINT_EQ(outline.cubics[8].p0, 537.0, -244.0);

  cff_glyph_outline_destroy(&outline);
  cff_font_destroy(&font);
}

static void test_cff_reader_lists_variable_axes_for_cff2_fixture(void) {
  cff_font_t font;

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_TRUE(cff_font_axis_count(&font) > 0);

  cff_font_destroy(&font);
}

static void test_cff_reader_reloads_font_object_in_place(void) {
  cff_font_t font;
  cff_glyph_outline_t outline = {};

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff-static.otf", &font));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, "period", NULL, &outline));
  ASSERT_TRUE(outline.num_cubics == 4);
  cff_glyph_outline_destroy(&outline);

  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_TRUE(cff_font_axis_count(&font) > 0);

  cff_font_destroy(&font);
}

static void test_cff_reader_extracts_cff2_outline_for_resolved_instance(void) {
  cff_font_t font;
  variation_location_t location;
  cff_glyph_outline_t regular_outline = {};
  cff_glyph_outline_t bold_outline = {};

  cff_font_init(&font);
  variation_location_init(&location);

  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, "gid1", NULL,
                                             &regular_outline));
  ASSERT_OK(variation_location_init_from_axis_map(&location, "wght=700"));
  ASSERT_OK(cff_variation_resolve_location(&font, &location));
  ASSERT_OK(cff_reader_extract_glyph_outline(&font, "gid1", &location,
                                             &bold_outline));
  ASSERT_TRUE(regular_outline.num_cubics > 0);
  ASSERT_TRUE(regular_outline.num_contours > 0);
  ASSERT_TRUE(bold_outline.num_cubics > 0);
  ASSERT_TRUE(bold_outline.num_contours > 0);
  ASSERT_TRUE(outlines_differ(&regular_outline, &bold_outline));

  cff_glyph_outline_destroy(&regular_outline);
  cff_glyph_outline_destroy(&bold_outline);
  variation_location_destroy(&location);
  cff_font_destroy(&font);
}

static void test_cff_reader_rejects_malformed_cff2_location(void) {
  cff_font_t font;
  variation_location_t location = {};
  cff_glyph_outline_t outline = {};

  cff_font_init(&font);
  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));

  location.axes = NULL;
  location.num_axes = 1;
  ASSERT_TRUE(cff_reader_extract_glyph_outline(&font, "gid1", &location, &outline) ==
              EOT_ERR_INVALID_ARGUMENT);

  cff_font_destroy(&font);
}

extern "C" void register_cff_reader_tests(void) {
  test_register("test_cff_reader_extracts_static_glyph_cubic_outline",
                test_cff_reader_extracts_static_glyph_cubic_outline);
  test_register("test_cff_reader_extracts_notdef_with_multiple_closed_contours",
                test_cff_reader_extracts_notdef_with_multiple_closed_contours);
  test_register("test_cff_reader_extracts_standard_sid_ellipsis_outline",
                test_cff_reader_extracts_standard_sid_ellipsis_outline);
  test_register("test_cff_reader_lists_variable_axes_for_cff2_fixture",
                test_cff_reader_lists_variable_axes_for_cff2_fixture);
  test_register("test_cff_reader_reloads_font_object_in_place",
                test_cff_reader_reloads_font_object_in_place);
  test_register("test_cff_reader_extracts_cff2_outline_for_resolved_instance",
                test_cff_reader_extracts_cff2_outline_for_resolved_instance);
  test_register("test_cff_reader_rejects_malformed_cff2_location",
                test_cff_reader_rejects_malformed_cff2_location);
}
