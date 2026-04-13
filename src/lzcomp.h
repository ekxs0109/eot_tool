#ifndef EOT_TOOL_LZCOMP_H
#define EOT_TOOL_LZCOMP_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

eot_status_t lzcomp_decompress(buffer_view_t compressed, uint8_t **out_data, size_t *out_size);
eot_status_t lzcomp_compress(const uint8_t *data, size_t length, uint8_t **out_data, size_t *out_size);

#endif
