#ifndef EOT_TOOL_MTX_DECODE_H
#define EOT_TOOL_MTX_DECODE_H

#include "file_io.h"
#include "sfnt_font.h"

eot_status_t mtx_decode_eot_file(const char *path, sfnt_font_t *out_font);

#endif
