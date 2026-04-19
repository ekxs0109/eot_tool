#ifndef EOT_TOOL_GLYF_CODEC_H
#define EOT_TOOL_GLYF_CODEC_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

#ifdef __cplusplus
extern "C" {
#endif

eot_status_t glyf_encode_255_ushort(uint16_t value, uint8_t *out_data,
                                    size_t out_capacity, size_t *out_size);
eot_status_t glyf_encode_255_short(int16_t value, uint8_t *out_data,
                                   size_t out_capacity, size_t *out_size);
eot_status_t glyf_encode_triplet(int on_curve, int dx, int dy, uint8_t *out_flag,
                                 uint8_t *out_data, size_t out_capacity,
                                 size_t *out_size);
eot_status_t glyf_split_push_code(buffer_view_t instructions, int *out_push_count,
                                  uint8_t **out_push_stream,
                                  size_t *out_push_stream_size,
                                  uint8_t **out_code_stream,
                                  size_t *out_code_stream_size);
eot_status_t glyf_encode(buffer_view_t glyf_table, buffer_view_t loca_table,
                         int index_to_loca_format, int num_glyphs,
                         uint8_t **out_glyf_stream,
                         size_t *out_glyf_stream_size,
                         uint8_t **out_push_stream,
                         size_t *out_push_stream_size,
                         uint8_t **out_code_stream,
                         size_t *out_code_stream_size);

eot_status_t glyf_decode(buffer_view_t glyf_stream, buffer_view_t push_stream,
                         buffer_view_t code_stream, int num_glyphs,
                         uint8_t **out_glyf, size_t *out_glyf_size,
                         uint8_t **out_loca, size_t *out_loca_size);
eot_status_t glyf_decode_with_loca_format(buffer_view_t glyf_stream,
                                          buffer_view_t push_stream,
                                          buffer_view_t code_stream,
                                          int num_glyphs,
                                          int index_to_loca_format,
                                          uint8_t **out_glyf,
                                          size_t *out_glyf_size,
                                          uint8_t **out_loca,
                                          size_t *out_loca_size);

#ifdef __cplusplus
}
#endif

#endif
