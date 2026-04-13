#include "eot_header.h"

#include <stdlib.h>
#include <string.h>

#define EOT_FIXED_HEADER_SIZE 82u

#define REQUIRE_BYTES(view, offset, count)             \
  do {                                                 \
    if (!buffer_view_has_range((view), (offset), (count))) { \
      return EOT_ERR_TRUNCATED;                        \
    }                                                  \
  } while (0)

static void eot_string_destroy(eot_string_t *value) {
  if (value == NULL) {
    return;
  }

  free(value->data);
  value->data = NULL;
  value->size = 0;
}

static void consume_signature(uint8_t **signature) {
  free(*signature);
  *signature = NULL;
}

static eot_status_t read_zero_padding(buffer_view_t view, size_t *offset,
                                      uint16_t *out) {
  REQUIRE_BYTES(view, *offset, 2u);
  *out = read_u16le(view.data + *offset);
  *offset += 2u;
  if (*out != 0u) {
    return EOT_ERR_INVALID_PADDING;
  }
  return EOT_OK;
}

static eot_status_t read_length_prefixed_string(buffer_view_t view,
                                                size_t *offset,
                                                eot_string_t *out) {
  uint16_t size;

  REQUIRE_BYTES(view, *offset, 2u);
  size = read_u16le(view.data + *offset);
  *offset += 2u;

  if ((size & 1u) != 0u) {
    return EOT_ERR_INVALID_STRING_LENGTH;
  }

  REQUIRE_BYTES(view, *offset, size);

  out->data = NULL;
  out->size = size;
  if (size > 0u) {
    out->data = (uint8_t *)malloc((size_t)size);
    if (out->data == NULL) {
      return EOT_ERR_ALLOCATION;
    }
    memcpy(out->data, view.data + *offset, size);
  }

  *offset += size;
  return EOT_OK;
}

static eot_status_t read_variable_strings(buffer_view_t view,
                                          size_t *offset,
                                          eot_header_t *header) {
  eot_status_t status;

  status = read_length_prefixed_string(view, offset, &header->family_name);
  if (status != EOT_OK) {
    return status;
  }

  status = read_zero_padding(view, offset, &header->padding2);
  if (status != EOT_OK) {
    return status;
  }

  status = read_length_prefixed_string(view, offset, &header->style_name);
  if (status != EOT_OK) {
    return status;
  }

  status = read_zero_padding(view, offset, &header->padding3);
  if (status != EOT_OK) {
    return status;
  }

  status = read_length_prefixed_string(view, offset, &header->version_name);
  if (status != EOT_OK) {
    return status;
  }

  status = read_zero_padding(view, offset, &header->padding4);
  if (status != EOT_OK) {
    return status;
  }

  status = read_length_prefixed_string(view, offset, &header->full_name);
  if (status != EOT_OK) {
    return status;
  }

  status = read_zero_padding(view, offset, &header->padding5);
  if (status != EOT_OK) {
    return status;
  }

  return read_length_prefixed_string(view, offset, &header->root_string);
}

static eot_status_t read_version_20002_trailer(buffer_view_t view,
                                               size_t *offset,
                                               eot_header_t *header) {
  eot_status_t status;

  REQUIRE_BYTES(view, *offset, 8u);
  header->root_string_checksum = read_u32le(view.data + *offset);
  *offset += 4u;
  header->eudc_code_page = read_u32le(view.data + *offset);
  *offset += 4u;

  status = read_zero_padding(view, offset, &header->padding6);
  if (status != EOT_OK) {
    return status;
  }

  REQUIRE_BYTES(view, *offset, 2u);
  header->signature_size = read_u16le(view.data + *offset);
  *offset += 2u;

  REQUIRE_BYTES(view, *offset, header->signature_size);
  if (header->signature_size > 0u) {
    header->signature = (uint8_t *)malloc(header->signature_size);
    if (header->signature == NULL) {
      return EOT_ERR_ALLOCATION;
    }
    memcpy(header->signature, view.data + *offset, header->signature_size);
  }
  *offset += header->signature_size;

  REQUIRE_BYTES(view, *offset, 8u);
  header->eudc_flags = read_u32le(view.data + *offset);
  *offset += 4u;
  header->eudc_font_size = read_u32le(view.data + *offset);
  *offset += 4u;

  REQUIRE_BYTES(view, *offset, header->eudc_font_size);
  *offset += header->eudc_font_size;
  return EOT_OK;
}

eot_status_t eot_header_parse(buffer_view_t view, eot_header_t *out) {
  eot_header_t header;
  size_t offset = EOT_FIXED_HEADER_SIZE;

  if (out == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  memset(&header, 0, sizeof(header));

  REQUIRE_BYTES(view, 0u, EOT_FIXED_HEADER_SIZE);

  header.eot_size = read_u32le(view.data + 0u);
  header.font_data_size = read_u32le(view.data + 4u);
  header.version = read_u32le(view.data + 8u);
  header.flags = read_u32le(view.data + 12u);
  memcpy(header.panose, view.data + 16u, sizeof(header.panose));
  header.charset = view.data[26u];
  header.italic = view.data[27u];
  header.weight = read_u32le(view.data + 28u);
  header.fs_type = read_u16le(view.data + 32u);
  header.magic_number = read_u16le(view.data + 34u);
  header.unicode_range[0] = read_u32le(view.data + 36u);
  header.unicode_range[1] = read_u32le(view.data + 40u);
  header.unicode_range[2] = read_u32le(view.data + 44u);
  header.unicode_range[3] = read_u32le(view.data + 48u);
  header.code_page_range[0] = read_u32le(view.data + 52u);
  header.code_page_range[1] = read_u32le(view.data + 56u);
  header.check_sum_adjustment = read_u32le(view.data + 60u);
  header.reserved[0] = read_u32le(view.data + 64u);
  header.reserved[1] = read_u32le(view.data + 68u);
  header.reserved[2] = read_u32le(view.data + 72u);
  header.reserved[3] = read_u32le(view.data + 76u);
  header.padding1 = read_u16le(view.data + 80u);

  if (header.magic_number != 0x504cu) {
    return EOT_ERR_INVALID_MAGIC;
  }

  {
    eot_status_t status = read_variable_strings(view, &offset, &header);
    if (status != EOT_OK) {
      eot_header_destroy(&header);
      return status;
    }
  }

  if (header.version == 0x00020002u) {
    eot_status_t status = read_version_20002_trailer(view, &offset, &header);
    if (status != EOT_OK) {
      eot_header_destroy(&header);
      return status;
    }
  }

  header.header_length = (uint32_t)offset;

  if (header.eot_size < header.font_data_size) {
    eot_header_destroy(&header);
    return EOT_ERR_INVALID_SIZE_METADATA;
  }

  if (header.header_length != header.eot_size - header.font_data_size) {
    eot_header_destroy(&header);
    return EOT_ERR_INVALID_SIZE_METADATA;
  }

  *out = header;
  return EOT_OK;
}

eot_status_t eot_header_read_file(const char *path, eot_header_t *out) {
  file_buffer_t file_buffer;
  buffer_view_t view;
  eot_status_t status;

  if (out == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  file_buffer.data = NULL;
  file_buffer.length = 0;

  status = file_io_read_all(path, &file_buffer);
  if (status != EOT_OK) {
    return status;
  }

  view = buffer_view_make(file_buffer.data, file_buffer.length);
  status = eot_header_parse(view, out);
  file_io_free(&file_buffer);
  return status;
}

void eot_header_destroy(eot_header_t *header) {
  if (header == NULL) {
    return;
  }

  eot_string_destroy(&header->family_name);
  eot_string_destroy(&header->style_name);
  eot_string_destroy(&header->version_name);
  eot_string_destroy(&header->full_name);
  eot_string_destroy(&header->root_string);
  consume_signature(&header->signature);
  memset(header, 0, sizeof(*header));
}
