#ifndef EOT_TOOL_OTF_CONVERT_H_
#define EOT_TOOL_OTF_CONVERT_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#include "cff_types.h"
#include "file_io.h"
#include "sfnt_font.h"

eot_status_t otf_convert_to_truetype_sfnt(
    const uint8_t *font_bytes, size_t font_size,
    const variation_location_t *location, sfnt_font_t *out_font);

#ifdef __cplusplus
}
#endif

#endif
