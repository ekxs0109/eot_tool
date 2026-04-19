#include "sfnt_reader.h"

#include <stdlib.h>
#include <string.h>

#include "byte_io.h"

#define SFNT_VERSION_TRUETYPE 0x00010000
#define SFNT_VERSION_OTTO 0x4f54544f

eot_status_t sfnt_reader_parse(const uint8_t *data, size_t length, sfnt_font_t *font) {
  if (data == NULL || font == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (length < 12) {
    return EOT_ERR_TRUNCATED;
  }

  uint32_t version = read_u32be(data);
  if (version != SFNT_VERSION_TRUETYPE && version != SFNT_VERSION_OTTO) {
    return EOT_ERR_INVALID_MAGIC;
  }

  uint16_t num_tables = read_u16be(data + 4);
  size_t table_dir_size = 12 + (size_t)num_tables * 16;

  if (length < table_dir_size) {
    return EOT_ERR_TRUNCATED;
  }

  sfnt_font_init(font);

  for (uint16_t i = 0; i < num_tables; i++) {
    const uint8_t *entry = data + 12 + i * 16;
    uint32_t tag = read_u32be(entry);
    uint32_t offset = read_u32be(entry + 8);
    uint32_t table_length = read_u32be(entry + 12);

    if (offset > length || table_length > length - offset) {
      sfnt_font_destroy(font);
      return EOT_ERR_TRUNCATED;
    }

    eot_status_t status = sfnt_font_add_table(font, tag, data + offset, table_length);
    if (status != EOT_OK) {
      sfnt_font_destroy(font);
      return status;
    }
  }

  return EOT_OK;
}

eot_status_t sfnt_reader_load_file(const char *path, sfnt_font_t *font) {
  file_buffer_t buffer;
  eot_status_t status = file_io_read_all(path, &buffer);
  if (status != EOT_OK) {
    return status;
  }

  status = sfnt_reader_parse(buffer.data, buffer.length, font);
  file_io_free(&buffer);
  return status;
}
