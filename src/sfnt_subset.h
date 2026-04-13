#ifndef EOT_TOOL_SFNT_SUBSET_H
#define EOT_TOOL_SFNT_SUBSET_H

#include <stddef.h>
#include <stdint.h>

#include "file_io.h"
#include "sfnt_font.h"

typedef enum {
  SUBSET_SELECTION_NONE = 0,
  SUBSET_SELECTION_TEXT,
  SUBSET_SELECTION_UNICODES,
  SUBSET_SELECTION_GLYPH_IDS
} subset_selection_mode_t;

typedef struct {
  char *input_path;
  char *output_path;
  char *selection_data;
  char *variation_axis_map;
  subset_selection_mode_t selection_mode;
  int keep_gids;
} subset_request_t;

#define SUBSET_GID_NOT_INCLUDED UINT16_MAX

typedef struct {
  uint16_t *included_glyph_ids;
  uint16_t *old_to_new_gid;
  size_t num_glyphs;
  size_t total_input_glyphs;
  size_t added_composite_dependencies;
  int keep_gids;
} subset_plan_t;

typedef struct {
  int dropped_hdmx;
  int dropped_vdmx;
} subset_warnings_t;

void subset_request_init(subset_request_t *request);
void subset_request_destroy(subset_request_t *request);
eot_status_t subset_request_init_for_text(subset_request_t *request, const char *text);
eot_status_t subset_request_init_for_glyph_ids(subset_request_t *request, const char *csv);
eot_status_t subset_request_init_for_glyph_ids_keep(subset_request_t *request, const char *csv);
void subset_plan_init(subset_plan_t *plan);
void subset_plan_destroy(subset_plan_t *plan);
void subset_warnings_init(subset_warnings_t *warnings);
eot_status_t sfnt_subset_plan(const sfnt_font_t *font,
                              const subset_request_t *request,
                              subset_plan_t *out_plan);
eot_status_t sfnt_subset_apply_output_table_policy(sfnt_font_t *output,
                                                   subset_warnings_t *warnings);
eot_status_t sfnt_subset_font_with_warnings(const sfnt_font_t *input,
                                            const subset_request_t *request,
                                            sfnt_font_t *output,
                                            subset_warnings_t *warnings);
eot_status_t sfnt_subset_font(const sfnt_font_t *input,
                              const subset_request_t *request,
                              sfnt_font_t *output);

#endif
