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
  double cu2qu_max_error;
  int reverse_direction;
  int post_format;
} otf_convert_options_t;

typedef struct {
  float from_coordinate;
  float to_coordinate;
} cff_avar_mapping_t;

typedef struct {
  char tag[5];
  float min_value;
  float default_value;
  float max_value;
  cff_avar_mapping_t *avar_mappings;
  size_t num_avar_mappings;
} cff_axis_t;

typedef struct {
  cubic_curve_t *cubics;
  size_t num_cubics;
  size_t *contour_end_indices;
  size_t num_contours;
} cff_glyph_outline_t;

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

typedef struct {
  void *impl;
  cff_axis_t *axes;
  size_t num_axes;
  int is_cff2;
} cff_font_t;

void quadratic_spline_destroy(quadratic_spline_t *spline);
void cff_glyph_outline_destroy(cff_glyph_outline_t *outline);
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
/* Initialize a font object before first use. Zero-initialization is equivalent. */
void cff_font_init(cff_font_t *font);
/* Destroy an initialized font object. Safe to call on an already-cleared object. */
void cff_font_destroy(cff_font_t *font);

#ifdef __cplusplus
}
#endif

#endif
