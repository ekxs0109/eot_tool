#ifndef EOT_TOOL_TT_REBUILDER_H_
#define EOT_TOOL_TT_REBUILDER_H_

#include <stddef.h>

#include "cff_types.h"
#include "file_io.h"
#include "sfnt_font.h"

#ifdef __cplusplus
extern "C" {
#endif

eot_status_t tt_rebuilder_build_font(const tt_glyph_outline_t *outlines,
                                     size_t num_outlines,
                                     sfnt_font_t *out_font);

#ifdef __cplusplus
}
#endif

#endif
