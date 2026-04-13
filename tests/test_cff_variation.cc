#include <math.h>
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

#define ASSERT_FLOAT_NEAR(actual, expected, tolerance) do { \
  if (fabs((actual) - (expected)) > (tolerance)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), \
             "assertion failed: %s ~= %.6f (actual %.6f, tolerance %.6f)", \
             #actual, (double)(expected), (double)(actual), (double)(tolerance)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static void test_cff_variation_accepts_axis_tag_map_and_normalizes_values(void) {
  cff_font_t font;
  variation_location_t location;

  cff_font_init(&font);
  variation_location_init(&location);

  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_OK(variation_location_init_from_axis_map(&location, "wght=700"));
  ASSERT_OK(cff_variation_resolve_location(&font, &location));
  ASSERT_TRUE(location.num_axes == 1);
  ASSERT_TRUE(strcmp(location.axes[0].tag, "wght") == 0);
  ASSERT_TRUE(location.axes[0].user_value == 700.0f);
  ASSERT_FLOAT_NEAR(location.axes[0].normalized_value, 0.7215f, 0.0002f);

  cff_font_destroy(&font);
  variation_location_destroy(&location);
}

static void test_cff_variation_resolves_axis_tag_map_to_full_axis_order(void) {
  cff_axis_t axes[3] = {
      {"opsz", 8.0f, 12.0f, 72.0f, NULL, 0},
      {"wdth", 50.0f, 100.0f, 200.0f, NULL, 0},
      {"wght", 100.0f, 400.0f, 900.0f, NULL, 0},
  };
  cff_font_t font = {};
  variation_location_t location;

  font.axes = axes;
  font.num_axes = sizeof(axes) / sizeof(axes[0]);

  variation_location_init(&location);
  ASSERT_OK(variation_location_init_from_axis_map(&location, "wght=900, wdth=75"));
  ASSERT_OK(cff_variation_resolve_location(&font, &location));

  ASSERT_TRUE(location.num_axes == 3);
  ASSERT_TRUE(strcmp(location.axes[0].tag, "opsz") == 0);
  ASSERT_TRUE(strcmp(location.axes[1].tag, "wdth") == 0);
  ASSERT_TRUE(strcmp(location.axes[2].tag, "wght") == 0);
  ASSERT_TRUE(location.axes[0].user_value == 12.0f);
  ASSERT_TRUE(location.axes[1].user_value == 75.0f);
  ASSERT_TRUE(location.axes[2].user_value == 900.0f);
  ASSERT_FLOAT_NEAR(location.axes[0].normalized_value, 0.0f, 0.0001f);
  ASSERT_FLOAT_NEAR(location.axes[1].normalized_value, -0.5f, 0.0001f);
  ASSERT_FLOAT_NEAR(location.axes[2].normalized_value, 1.0f, 0.0001f);

  variation_location_destroy(&location);
}

static void test_cff_variation_rejects_unknown_axis_tags(void) {
  cff_font_t font;
  variation_location_t location;

  cff_font_init(&font);
  variation_location_init(&location);

  ASSERT_OK(cff_reader_load_file("testdata/cff2-variable.otf", &font));
  ASSERT_OK(variation_location_init_from_axis_map(&location, "wdth=90"));
  ASSERT_TRUE(cff_variation_resolve_location(&font, &location) ==
              EOT_ERR_INVALID_ARGUMENT);

  cff_font_destroy(&font);
  variation_location_destroy(&location);
}

extern "C" void register_cff_variation_tests(void) {
  test_register("test_cff_variation_accepts_axis_tag_map_and_normalizes_values",
                test_cff_variation_accepts_axis_tag_map_and_normalizes_values);
  test_register("test_cff_variation_resolves_axis_tag_map_to_full_axis_order",
                test_cff_variation_resolves_axis_tag_map_to_full_axis_order);
  test_register("test_cff_variation_rejects_unknown_axis_tags",
                test_cff_variation_rejects_unknown_axis_tags);
}
