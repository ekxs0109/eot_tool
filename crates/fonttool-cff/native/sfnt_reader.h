#ifndef EOT_TOOL_SFNT_READER_H
#define EOT_TOOL_SFNT_READER_H

#include <stddef.h>
#include <stdint.h>

#include "file_io.h"
#include "sfnt_font.h"

eot_status_t sfnt_reader_load_file(const char *path, sfnt_font_t *font);
eot_status_t sfnt_reader_parse(const uint8_t *data, size_t length, sfnt_font_t *font);

#endif
