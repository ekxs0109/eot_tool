#ifndef EOT_TOOL_SFNT_WRITER_H
#define EOT_TOOL_SFNT_WRITER_H

#include <stddef.h>
#include <stdint.h>

#include "file_io.h"
#include "sfnt_font.h"

#ifdef __cplusplus
extern "C" {
#endif

eot_status_t sfnt_writer_serialize(sfnt_font_t *font, uint8_t **out_data, size_t *out_size);

#ifdef __cplusplus
}
#endif

#endif
