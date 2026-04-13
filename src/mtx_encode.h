#ifndef EOT_TOOL_MTX_ENCODE_H
#define EOT_TOOL_MTX_ENCODE_H

#include <stddef.h>
#include <stdint.h>

#include "file_io.h"
#include "sfnt_font.h"

typedef struct {
  uint8_t *data;
  size_t length;
  size_t capacity;
} byte_buffer_t;

typedef struct {
  int dropped_vdmx;
} mtx_encode_warnings_t;

void byte_buffer_init(byte_buffer_t *buf);
eot_status_t byte_buffer_append(byte_buffer_t *buf, const uint8_t *data, size_t length);
eot_status_t byte_buffer_reserve(byte_buffer_t *buf, size_t capacity);
void byte_buffer_destroy(byte_buffer_t *buf);

void mtx_encode_warnings_init(mtx_encode_warnings_t *warnings);
eot_status_t mtx_encode_ttf_file(const char *path, byte_buffer_t *out);
eot_status_t mtx_encode_font(const sfnt_font_t *font, byte_buffer_t *out);
eot_status_t mtx_encode_ttf_file_with_ppt_xor(const char *path, byte_buffer_t *out);
eot_status_t mtx_encode_font_with_ppt_xor(const sfnt_font_t *font, byte_buffer_t *out);
eot_status_t mtx_encode_ttf_file_with_warnings(const char *path, byte_buffer_t *out,
                                               mtx_encode_warnings_t *warnings);
eot_status_t mtx_encode_font_with_warnings(const sfnt_font_t *font, byte_buffer_t *out,
                                           mtx_encode_warnings_t *warnings);
eot_status_t mtx_encode_ttf_file_with_ppt_xor_and_warnings(const char *path,
                                                           byte_buffer_t *out,
                                                           mtx_encode_warnings_t *warnings);
eot_status_t mtx_encode_font_with_ppt_xor_and_warnings(const sfnt_font_t *font,
                                                       byte_buffer_t *out,
                                                       mtx_encode_warnings_t *warnings);

#endif
