#include "hdmx_codec.h"
#include "byte_io.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
  uint8_t *data;
  size_t length;
  size_t capacity;
} mag_writer_t;

static eot_status_t mag_writer_reserve(mag_writer_t *writer, size_t additional) {
  if (writer->length + additional <= writer->capacity) {
    return EOT_OK;
  }

  size_t needed = writer->length + additional;
  size_t capacity = writer->capacity == 0 ? 16u : writer->capacity;
  while (capacity < needed) {
    capacity *= 2u;
  }

  uint8_t *data = realloc(writer->data, capacity);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  writer->data = data;
  writer->capacity = capacity;
  return EOT_OK;
}

static eot_status_t mag_writer_append_byte(mag_writer_t *writer, uint8_t value) {
  eot_status_t status = mag_writer_reserve(writer, 1u);
  if (status != EOT_OK) {
    return status;
  }

  writer->data[writer->length++] = value;
  return EOT_OK;
}

static eot_status_t mag_writer_write_value(mag_writer_t *writer, int value) {
  if (value >= -139 && value <= 111) {
    return mag_writer_append_byte(writer, (uint8_t)(value + 139));
  }

  if (value >= 108 && value <= 875) {
    int biased = value - 108;
    eot_status_t status = mag_writer_append_byte(writer,
                                                 (uint8_t)(251 + biased / 256));
    if (status != EOT_OK) {
      return status;
    }
    return mag_writer_append_byte(writer, (uint8_t)(biased % 256));
  }

  if (value >= -363 && value <= -108) {
    eot_status_t status = mag_writer_append_byte(writer, 254);
    if (status != EOT_OK) {
      return status;
    }
    return mag_writer_append_byte(writer, (uint8_t)(-(value + 108)));
  }

  if (value <= -364) {
    int biased = -(value + 364);
    eot_status_t status = mag_writer_append_byte(writer, 255);
    if (status != EOT_OK) {
      return status;
    }
    status = mag_writer_append_byte(writer, (uint8_t)((biased >> 8) & 0xFF));
    if (status != EOT_OK) {
      return status;
    }
    return mag_writer_append_byte(writer, (uint8_t)(biased & 0xFF));
  }

  return EOT_ERR_CORRUPT_DATA;
}

static void mag_writer_destroy(mag_writer_t *writer) {
  free(writer->data);
  writer->data = NULL;
  writer->length = 0;
  writer->capacity = 0;
}

static uint16_t hdmx_read_advance_width(buffer_view_t hmtx, buffer_view_t hhea,
                                        uint16_t glyph) {
  if (hhea.length >= 36) {
    uint16_t num_h_metrics = read_u16be(hhea.data + 34);
    if (num_h_metrics > 0) {
      if (glyph < num_h_metrics) {
        size_t offset = (size_t)glyph * 4u;
        if (offset + 2u <= hmtx.length) {
          return read_u16be(hmtx.data + offset);
        }
        return 0;
      }

      size_t offset = ((size_t)num_h_metrics - 1u) * 4u;
      if (offset + 2u <= hmtx.length) {
        return read_u16be(hmtx.data + offset);
      }
      return 0;
    }
  }

  size_t offset = (size_t)glyph * 4u;
  if (offset + 2u > hmtx.length) {
    return 0;
  }

  return read_u16be(hmtx.data + offset);
}

eot_status_t hdmx_encode(buffer_view_t decoded, buffer_view_t hmtx, buffer_view_t hhea,
                         buffer_view_t head, buffer_view_t maxp,
                         uint8_t **out_data, size_t *out_size) {
  if (out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_data = NULL;
  *out_size = 0;

  if (decoded.length < 8 || head.length < 20 || maxp.length < 6) {
    return EOT_ERR_CORRUPT_DATA;
  }

  uint16_t num_records = read_u16be(decoded.data + 2);
  uint32_t record_size = read_u32be(decoded.data + 4);
  uint16_t units_per_em = read_u16be(head.data + 18);
  uint16_t num_glyphs = read_u16be(maxp.data + 4);
  size_t expected_size = 8u + (size_t)num_records * (size_t)record_size;

  if (units_per_em == 0 ||
      record_size < (uint32_t)num_glyphs + 2u ||
      expected_size > decoded.length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  mag_writer_t writer = {0};
  for (uint16_t rec = 0; rec < num_records; rec++) {
    size_t record_offset = 8u + (size_t)rec * (size_t)record_size;
    uint8_t ppem = decoded.data[record_offset];

    for (uint16_t glyph = 0; glyph < num_glyphs; glyph++) {
      uint16_t advance_width = hdmx_read_advance_width(hmtx, hhea, glyph);
      int rounded_tt_aw =
          ((64 * (int)ppem * (int)advance_width + units_per_em / 2) / units_per_em + 32) / 64;
      int surprise = (int)decoded.data[record_offset + 2u + glyph] - rounded_tt_aw;
      eot_status_t status = mag_writer_write_value(&writer, surprise);
      if (status != EOT_OK) {
        mag_writer_destroy(&writer);
        return status;
      }
    }
  }

  size_t result_size = 8u + (size_t)num_records * 2u + writer.length;
  if (result_size < 12u) {
    result_size = 12u;
  }
  uint8_t *output = malloc(result_size);
  if (output == NULL) {
    mag_writer_destroy(&writer);
    return EOT_ERR_ALLOCATION;
  }
  memset(output, 0, result_size);

  write_u16be(output, 0);
  write_u16be(output + 2, num_records);
  write_u32be(output + 4, record_size);
  for (uint16_t rec = 0; rec < num_records; rec++) {
    size_t record_offset = 8u + (size_t)rec * (size_t)record_size;
    output[8u + (size_t)rec * 2u] = decoded.data[record_offset];
    output[8u + (size_t)rec * 2u + 1u] = decoded.data[record_offset + 1u];
  }
  memcpy(output + 8u + (size_t)num_records * 2u, writer.data, writer.length);

  *out_data = output;
  *out_size = result_size;
  mag_writer_destroy(&writer);
  return EOT_OK;
}

static int read_magnitude_dependent_value(const uint8_t *data, size_t length, size_t *pos) {
  if (*pos >= length) {
    return 0;
  }

  uint8_t first_byte = data[(*pos)++];

  if (first_byte < 251) {
    return (int)first_byte - 139;
  } else if (first_byte < 254) {
    if (*pos >= length) {
      return 0;
    }
    uint8_t second_byte = data[(*pos)++];
    int value = (first_byte - 251) * 256 + second_byte + 108;
    return value;
  } else if (first_byte == 254) {
    if (*pos >= length) {
      return 0;
    }
    uint8_t second_byte = data[(*pos)++];
    int value = -(int)second_byte - 108;
    return value;
  } else {
    if (*pos + 1 >= length) {
      return 0;
    }
    uint8_t byte1 = data[(*pos)++];
    uint8_t byte2 = data[(*pos)++];
    int value = -((int)byte1 * 256 + (int)byte2) - 364;
    return value;
  }
}

eot_status_t hdmx_decode(buffer_view_t encoded, buffer_view_t hmtx, buffer_view_t hhea,
                         buffer_view_t head,
                         buffer_view_t maxp, uint8_t **out_data, size_t *out_size) {
  if (encoded.length < 12 || head.length < 18 || maxp.length < 6) {
    return EOT_ERR_CORRUPT_DATA;
  }

  uint16_t num_records = read_u16be(encoded.data + 2);
  uint32_t record_size = read_u32be(encoded.data + 4);
  uint16_t units_per_em = read_u16be(head.data + 18);
  uint16_t num_glyphs = read_u16be(maxp.data + 4);

  size_t mag_data_offset = 8 + num_records * 2;
  if (mag_data_offset > encoded.length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  size_t output_size = 8 + num_records * record_size;
  uint8_t *output = malloc(output_size);
  if (!output) {
    return EOT_ERR_ALLOCATION;
  }

  write_u16be(output, 0);
  write_u16be(output + 2, num_records);
  write_u32be(output + 4, record_size);

  size_t mag_pos = mag_data_offset;

  for (uint16_t rec = 0; rec < num_records; rec++) {
    uint8_t ppem = encoded.data[8 + rec * 2];
    uint8_t max_width = encoded.data[8 + rec * 2 + 1];

    size_t record_offset = 8 + rec * record_size;
    output[record_offset] = ppem;
    output[record_offset + 1] = max_width;

    for (uint16_t glyph = 0; glyph < num_glyphs; glyph++) {
      uint16_t advance_width = hdmx_read_advance_width(hmtx, hhea, glyph);

      int rounded_tt_aw = ((64 * ppem * advance_width + units_per_em / 2) / units_per_em + 32) / 64;
      int surprise = read_magnitude_dependent_value(encoded.data, encoded.length, &mag_pos);
      int width = rounded_tt_aw + surprise;

      if (width < 0) width = 0;
      if (width > 255) width = 255;

      output[record_offset + 2 + glyph] = (uint8_t)width;
    }

    for (uint32_t pad = num_glyphs + 2; pad < record_size; pad++) {
      output[record_offset + pad] = 0;
    }
  }

  *out_data = output;
  *out_size = output_size;
  return EOT_OK;
}
