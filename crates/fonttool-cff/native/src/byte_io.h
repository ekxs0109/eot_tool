#ifndef EOT_TOOL_BYTE_IO_H
#define EOT_TOOL_BYTE_IO_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  const uint8_t *data;
  size_t length;
} buffer_view_t;

buffer_view_t buffer_view_make(const uint8_t *data, size_t length);
int buffer_view_has_range(buffer_view_t view, size_t offset, size_t length);

uint16_t read_u16le(const uint8_t *data);
uint32_t read_u32le(const uint8_t *data);
uint32_t read_u24be(const uint8_t *data);
uint16_t read_u16be(const uint8_t *data);
uint32_t read_u32be(const uint8_t *data);

void write_u16le(uint8_t *data, uint16_t value);
void write_u32le(uint8_t *data, uint32_t value);
void write_u16be(uint8_t *data, uint16_t value);
void write_u32be(uint8_t *data, uint32_t value);
void write_u24be(uint8_t *data, uint32_t value);

#ifdef __cplusplus
}
#endif

#endif
