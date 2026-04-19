#include "byte_io.h"

buffer_view_t buffer_view_make(const uint8_t *data, size_t length) {
  buffer_view_t view;

  view.data = data;
  view.length = length;
  return view;
}

int buffer_view_has_range(buffer_view_t view, size_t offset, size_t length) {
  return offset <= view.length && length <= view.length - offset;
}

uint16_t read_u16le(const uint8_t *data) {
  return (uint16_t)(data[0] | ((uint16_t)data[1] << 8));
}

uint32_t read_u32le(const uint8_t *data) {
  return (uint32_t)data[0] | ((uint32_t)data[1] << 8) |
         ((uint32_t)data[2] << 16) | ((uint32_t)data[3] << 24);
}

uint32_t read_u24be(const uint8_t *data) {
  return ((uint32_t)data[0] << 16) | ((uint32_t)data[1] << 8) | data[2];
}

uint16_t read_u16be(const uint8_t *data) {
  return (uint16_t)((data[0] << 8) | data[1]);
}

uint32_t read_u32be(const uint8_t *data) {
  return ((uint32_t)data[0] << 24) | ((uint32_t)data[1] << 16) |
         ((uint32_t)data[2] << 8) | data[3];
}

void write_u16le(uint8_t *data, uint16_t value) {
  data[0] = (uint8_t)(value & 0xffu);
  data[1] = (uint8_t)((value >> 8) & 0xffu);
}

void write_u32le(uint8_t *data, uint32_t value) {
  data[0] = (uint8_t)(value & 0xffu);
  data[1] = (uint8_t)((value >> 8) & 0xffu);
  data[2] = (uint8_t)((value >> 16) & 0xffu);
  data[3] = (uint8_t)((value >> 24) & 0xffu);
}

void write_u16be(uint8_t *data, uint16_t value) {
  data[0] = (uint8_t)((value >> 8) & 0xffu);
  data[1] = (uint8_t)(value & 0xffu);
}

void write_u32be(uint8_t *data, uint32_t value) {
  data[0] = (uint8_t)((value >> 24) & 0xffu);
  data[1] = (uint8_t)((value >> 16) & 0xffu);
  data[2] = (uint8_t)((value >> 8) & 0xffu);
  data[3] = (uint8_t)(value & 0xffu);
}

void write_u24be(uint8_t *data, uint32_t value) {
  data[0] = (uint8_t)((value >> 16) & 0xffu);
  data[1] = (uint8_t)((value >> 8) & 0xffu);
  data[2] = (uint8_t)(value & 0xffu);
}
