#ifndef EOT_TOOL_CFF_TYPES_H_
#define EOT_TOOL_CFF_TYPES_H_

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  double x;
  double y;
} cff_point_t;

typedef struct {
  cff_point_t p0;
  cff_point_t p1;
  cff_point_t p2;
  cff_point_t p3;
} cubic_curve_t;

typedef struct {
  cff_point_t *points;
  size_t num_points;
  int closed;
} quadratic_spline_t;

typedef struct {
  char tag[5];
  float user_value;
  float resolved_value;
  float normalized_value;
} variation_axis_value_t;

typedef struct {
  variation_axis_value_t *axes;
  size_t num_axes;
} variation_location_t;

typedef struct {
  int16_t x;
  int16_t y;
  int on_curve;
} tt_outline_point_t;

typedef struct {
  tt_outline_point_t *points;
  size_t num_points;
} tt_contour_t;

typedef struct {
  tt_contour_t *contours;
  size_t num_contours;
  uint16_t advance_width;
  char *glyph_name;
} tt_glyph_outline_t;

void quadratic_spline_destroy(quadratic_spline_t *spline);
static inline void tt_glyph_outline_destroy(tt_glyph_outline_t *outline) {
  size_t i;

  if (outline == NULL) {
    return;
  }

  for (i = 0; i < outline->num_contours; i++) {
    free(outline->contours[i].points);
  }
  free(outline->contours);
  free(outline->glyph_name);

  outline->contours = NULL;
  outline->num_contours = 0;
  outline->advance_width = 0;
  outline->glyph_name = NULL;
}

#ifdef __cplusplus
}
#endif

#endif
