#include <stdio.h>
#include <stdlib.h>

#include <limits>

extern "C" {
#include "../src/file_io.h"
#include "../src/cu2qu.h"
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

#define ASSERT_EQ(actual, expected) do { \
  int actual__ = (int)(actual); \
  int expected__ = (int)(expected); \
  if (actual__ != expected__) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual=%d expected=%d)", \
             #actual, #expected, actual__, expected__); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static cubic_curve_t make_cubic(double x0, double y0,
                                double x1, double y1,
                                double x2, double y2,
                                double x3, double y3) {
  cubic_curve_t cubic;
  cubic.p0.x = x0;
  cubic.p0.y = y0;
  cubic.p1.x = x1;
  cubic.p1.y = y1;
  cubic.p2.x = x2;
  cubic.p2.y = y2;
  cubic.p3.x = x3;
  cubic.p3.y = y3;
  return cubic;
}

static quadratic_spline_t make_empty_spline(void) {
  quadratic_spline_t spline;
  spline.points = NULL;
  spline.num_points = 0;
  spline.closed = 0;
  return spline;
}

static quadratic_spline_t make_owned_dummy_spline(void) {
  quadratic_spline_t spline = make_empty_spline();
  spline.points = (cff_point_t *)malloc(sizeof(cff_point_t));
  if (spline.points != NULL) {
    spline.points[0].x = -1.0;
    spline.points[0].y = -1.0;
    spline.num_points = 1;
  }
  return spline;
}

static void test_curve_to_quadratic_converts_simple_cubic(void) {
  cubic_curve_t cubic = make_cubic(0, 0, 50, 100, 100, 100, 150, 0);
  quadratic_spline_t spline = make_empty_spline();
  ASSERT_OK(curve_to_quadratic(&cubic, 1.0, &spline));
  ASSERT_EQ(spline.num_points, 3);
  ASSERT_TRUE(spline.points != NULL);
  ASSERT_TRUE(spline.points[0].x == 0.0);
  ASSERT_TRUE(spline.points[0].y == 0.0);
  ASSERT_TRUE(spline.points[1].x == 75.0);
  ASSERT_TRUE(spline.points[1].y == 150.0);
  ASSERT_TRUE(spline.points[2].x == 150.0);
  ASSERT_TRUE(spline.points[2].y == 0.0);
  quadratic_spline_destroy(&spline);
}

static void test_curve_to_quadratic_emits_multi_segment_spline_when_needed(void) {
  cubic_curve_t cubic = make_cubic(0, 0, 0, 1, 2, 1, 2, 0);
  quadratic_spline_t spline = make_empty_spline();
  ASSERT_OK(curve_to_quadratic(&cubic, 0.1, &spline));
  ASSERT_EQ(spline.num_points, 4);
  ASSERT_TRUE(spline.points != NULL);
  ASSERT_TRUE(spline.points[0].x == 0.0);
  ASSERT_TRUE(spline.points[1].y == 0.75);
  ASSERT_TRUE(spline.points[2].x == 2.0);
  ASSERT_TRUE(spline.points[2].y == 0.75);
  ASSERT_TRUE(spline.points[3].x == 2.0);
  ASSERT_TRUE(spline.points[3].y == 0.0);
  quadratic_spline_destroy(&spline);
}

static void test_curve_to_quadratic_rejects_non_positive_tolerance(void) {
  cubic_curve_t cubic = make_cubic(0, 0, 50, 100, 100, 100, 150, 0);
  quadratic_spline_t spline = make_empty_spline();
  ASSERT_EQ(curve_to_quadratic(&cubic, 0.0, &spline), EOT_ERR_INVALID_ARGUMENT);
  ASSERT_TRUE(spline.points == NULL);
  ASSERT_EQ(spline.num_points, 0);
}

static void test_curve_to_quadratic_rejects_non_finite_input(void) {
  cubic_curve_t cubic = make_cubic(0, 0,
                                   std::numeric_limits<double>::infinity(), 1,
                                   2, 1,
                                   2, 0);
  quadratic_spline_t spline = make_owned_dummy_spline();
  ASSERT_TRUE(spline.points != NULL);
  ASSERT_EQ(curve_to_quadratic(&cubic, 0.1, &spline), EOT_ERR_INVALID_ARGUMENT);
  ASSERT_TRUE(spline.points == NULL);
  ASSERT_EQ(spline.num_points, 0);
}

static void test_curve_to_quadratic_reuses_existing_output_safely(void) {
  cubic_curve_t cubic1 = make_cubic(0, 0, 50, 100, 100, 100, 150, 0);
  cubic_curve_t cubic2 = make_cubic(0, 0, 0, 1, 2, 1, 2, 0);
  quadratic_spline_t spline = make_empty_spline();

  ASSERT_OK(curve_to_quadratic(&cubic1, 1.0, &spline));
  ASSERT_EQ(spline.num_points, 3);
  ASSERT_TRUE(spline.points != NULL);

  ASSERT_OK(curve_to_quadratic(&cubic2, 0.1, &spline));
  ASSERT_EQ(spline.num_points, 4);
  ASSERT_TRUE(spline.points != NULL);
  ASSERT_TRUE(spline.points[0].x == 0.0);
  ASSERT_TRUE(spline.points[1].y == 0.75);
  ASSERT_TRUE(spline.points[3].x == 2.0);
  ASSERT_TRUE(spline.points[3].y == 0.0);
  quadratic_spline_destroy(&spline);
}

static void test_curve_to_quadratic_handles_collinear_degenerate_curve(void) {
  cubic_curve_t cubic = make_cubic(64.94, 550.998,
                                   65.199, 550.032,
                                   65.199, 550.032,
                                   65.458, 549.066);
  quadratic_spline_t spline = make_empty_spline();
  ASSERT_OK(curve_to_quadratic(&cubic, 1.0, &spline));
  ASSERT_EQ(spline.num_points, 4);
  ASSERT_TRUE(spline.points != NULL);
  ASSERT_TRUE(spline.points[0].x == 64.94);
  ASSERT_TRUE(spline.points[0].y == 550.998);
  ASSERT_TRUE(spline.points[3].x == 65.458);
  ASSERT_TRUE(spline.points[3].y == 549.066);
  quadratic_spline_destroy(&spline);
}

static void test_curve_to_quadratic_returns_corrupt_data_for_non_approximable_loop(void) {
  cubic_curve_t cubic = make_cubic(0, 0, 1000, 0, -1000, 0, 0, 0);
  quadratic_spline_t spline = make_empty_spline();

  ASSERT_EQ(curve_to_quadratic(&cubic, 0.001, &spline), EOT_ERR_CORRUPT_DATA);
  ASSERT_TRUE(spline.points == NULL);
  ASSERT_EQ(spline.num_points, 0);
}

extern "C" void register_cu2qu_tests(void) {
  test_register("test_curve_to_quadratic_converts_simple_cubic",
                test_curve_to_quadratic_converts_simple_cubic);
  test_register("test_curve_to_quadratic_emits_multi_segment_spline_when_needed",
                test_curve_to_quadratic_emits_multi_segment_spline_when_needed);
  test_register("test_curve_to_quadratic_rejects_non_positive_tolerance",
                test_curve_to_quadratic_rejects_non_positive_tolerance);
  test_register("test_curve_to_quadratic_rejects_non_finite_input",
                test_curve_to_quadratic_rejects_non_finite_input);
  test_register("test_curve_to_quadratic_reuses_existing_output_safely",
                test_curve_to_quadratic_reuses_existing_output_safely);
  test_register("test_curve_to_quadratic_handles_collinear_degenerate_curve",
                test_curve_to_quadratic_handles_collinear_degenerate_curve);
  test_register("test_curve_to_quadratic_returns_corrupt_data_for_non_approximable_loop",
                test_curve_to_quadratic_returns_corrupt_data_for_non_approximable_loop);
}
