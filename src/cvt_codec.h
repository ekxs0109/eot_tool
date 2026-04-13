#ifndef EOT_TOOL_CVT_CODEC_H
#define EOT_TOOL_CVT_CODEC_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

eot_status_t cvt_decode(buffer_view_t encoded, uint8_t **out_data, size_t *out_size);
eot_status_t cvt_encode(buffer_view_t decoded, uint8_t **out_data, size_t *out_size);

#endif
