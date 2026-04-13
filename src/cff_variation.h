#ifndef EOT_TOOL_CFF_VARIATION_H_
#define EOT_TOOL_CFF_VARIATION_H_

#ifdef __cplusplus
extern "C" {
#endif

#include "cff_reader.h"

void variation_location_init(variation_location_t *location);
void variation_location_destroy(variation_location_t *location);
eot_status_t variation_location_init_from_axis_map(
    variation_location_t *location, const char *axis_map);
eot_status_t cff_variation_resolve_location(
    const cff_font_t *font, variation_location_t *location);

#ifdef __cplusplus
}
#endif

#endif
