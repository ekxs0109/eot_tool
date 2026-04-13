#ifndef EOT_TOOL_EOT_HEADER_H
#define EOT_TOOL_EOT_HEADER_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

typedef struct {
  uint8_t *data;
  uint16_t size;
} eot_string_t;

typedef struct {
  uint32_t eot_size;
  uint32_t font_data_size;
  uint32_t version;
  uint32_t flags;
  uint8_t panose[10];
  uint8_t charset;
  uint8_t italic;
  uint32_t weight;
  uint16_t fs_type;
  uint16_t magic_number;
  uint32_t unicode_range[4];
  uint32_t code_page_range[2];
  uint32_t check_sum_adjustment;
  uint32_t reserved[4];
  uint16_t padding1;
  eot_string_t family_name;
  uint16_t padding2;
  eot_string_t style_name;
  uint16_t padding3;
  eot_string_t version_name;
  uint16_t padding4;
  eot_string_t full_name;
  uint16_t padding5;
  eot_string_t root_string;
  uint32_t root_string_checksum;
  uint32_t eudc_code_page;
  uint16_t padding6;
  uint16_t signature_size;
  uint8_t *signature;
  uint32_t eudc_flags;
  uint32_t eudc_font_size;
  uint32_t header_length;
} eot_header_t;

eot_status_t eot_header_parse(buffer_view_t view, eot_header_t *out);
eot_status_t eot_header_read_file(const char *path, eot_header_t *out);
void eot_header_destroy(eot_header_t *header);

#endif
