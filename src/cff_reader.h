#ifndef EOT_TOOL_CFF_READER_H_
#define EOT_TOOL_CFF_READER_H_

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#include "cff_types.h"
#include "file_io.h"

/*
 * Lifecycle:
 * - Call cff_font_init() before first use, or provide an equivalent zero-initialized object.
 * - cff_reader_load_file() may be called repeatedly on an initialized object.
 * - cff_font_destroy() safely clears an initialized object for reuse or disposal.
 */
eot_status_t cff_reader_load_file(const char *path, cff_font_t *font);
eot_status_t cff_reader_load_memory(const uint8_t *data, size_t size,
                                    cff_font_t *font);
eot_status_t cff_reader_extract_glyph_outline(
    const cff_font_t *font, const char *glyph_name,
    const variation_location_t *location,
    cff_glyph_outline_t *outline);
eot_status_t cff_reader_extract_glyph_outline_by_id(
    const cff_font_t *font, size_t glyph_id,
    cff_glyph_outline_t *outline);
size_t cff_font_axis_count(const cff_font_t *font);
size_t cff_font_glyph_count(const cff_font_t *font);

#ifdef __cplusplus
}
#endif

#endif
