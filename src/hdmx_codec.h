#ifndef EOT_TOOL_HDMX_CODEC_H
#define EOT_TOOL_HDMX_CODEC_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

eot_status_t hdmx_encode(buffer_view_t decoded, buffer_view_t hmtx, buffer_view_t hhea,
                         buffer_view_t head, buffer_view_t maxp,
                         uint8_t **out_data, size_t *out_size);
eot_status_t hdmx_decode(buffer_view_t encoded, buffer_view_t hmtx, buffer_view_t hhea,
                         buffer_view_t head,
                         buffer_view_t maxp, uint8_t **out_data, size_t *out_size);

#endif
