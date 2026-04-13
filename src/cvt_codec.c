#include "cvt_codec.h"
#include "byte_io.h"

#include <stdlib.h>
#include <string.h>

#define CVT_LOWESTCODE 238
#define CVT_WORDCODE 238
#define CVT_NEG0 239
#define CVT_NEG1 240
#define CVT_NEG8 247
#define CVT_POS1 248
#define CVT_POS8 255

static int16_t read_i16be(const uint8_t *data) {
  return (int16_t)(((uint16_t)data[0] << 8) | (uint16_t)data[1]);
}

eot_status_t cvt_encode(buffer_view_t decoded, uint8_t **out_data, size_t *out_size) {
  if (out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_data = NULL;
  *out_size = 0;

  if ((decoded.length % 2u) != 0u) {
    return EOT_ERR_CORRUPT_DATA;
  }

  size_t num_entries = decoded.length / 2u;
  size_t capacity = 2u + num_entries * 3u;
  uint8_t *output = malloc(capacity);
  if (output == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  output[0] = (uint8_t)((num_entries >> 8) & 0xFFu);
  output[1] = (uint8_t)(num_entries & 0xFFu);

  size_t out_pos = 2u;
  int16_t last_value = 0;

  for (size_t i = 0; i < num_entries; i++) {
    int16_t value = read_i16be(decoded.data + i * 2u);
    int16_t delta_value = (int16_t)(value - last_value);
    int abs_value = delta_value < 0 ? -(int)delta_value : (int)delta_value;
    int index = abs_value / CVT_LOWESTCODE;

    if (index <= 8) {
      if (delta_value < 0) {
        output[out_pos++] = (uint8_t)(CVT_NEG0 + index);
        output[out_pos++] = (uint8_t)(abs_value - index * CVT_LOWESTCODE);
      } else {
        if (index > 0) {
          output[out_pos++] = (uint8_t)(CVT_POS1 + index - 1);
        }
        output[out_pos++] = (uint8_t)(abs_value - index * CVT_LOWESTCODE);
      }
    } else {
      output[out_pos++] = CVT_WORDCODE;
      write_u16be(output + out_pos, (uint16_t)delta_value);
      out_pos += 2u;
    }

    last_value = value;
  }

  *out_data = output;
  *out_size = out_pos;
  return EOT_OK;
}

eot_status_t cvt_decode(buffer_view_t encoded, uint8_t **out_data, size_t *out_size) {
  if (encoded.length < 2) {
    return EOT_ERR_CORRUPT_DATA;
  }

  uint16_t num_entries = ((uint16_t)encoded.data[0] << 8) | (uint16_t)encoded.data[1];
  size_t output_size = num_entries * 2;
  uint8_t *output = malloc(output_size);
  if (!output) {
    return EOT_ERR_ALLOCATION;
  }

  size_t in_pos = 2;
  int16_t last_value = 0;

  for (uint16_t i = 0; i < num_entries; i++) {
    if (in_pos >= encoded.length) {
      free(output);
      return EOT_ERR_CORRUPT_DATA;
    }

    uint8_t code = encoded.data[in_pos++];
    int16_t delta_value;

    if (code == CVT_WORDCODE) {
      if (in_pos + 1 >= encoded.length) {
        free(output);
        return EOT_ERR_CORRUPT_DATA;
      }
      delta_value = read_i16be(encoded.data + in_pos);
      in_pos += 2;
    } else if (code >= CVT_NEG0 && code <= CVT_NEG8) {
      if (in_pos >= encoded.length) {
        free(output);
        return EOT_ERR_CORRUPT_DATA;
      }
      int index = code - CVT_NEG0;
      int abs_value = index * CVT_LOWESTCODE + encoded.data[in_pos++];
      delta_value = -abs_value;
    } else if (code >= CVT_POS1 && code <= CVT_POS8) {
      if (in_pos >= encoded.length) {
        free(output);
        return EOT_ERR_CORRUPT_DATA;
      }
      int index = code - CVT_POS1 + 1;
      int abs_value = index * CVT_LOWESTCODE + encoded.data[in_pos++];
      delta_value = abs_value;
    } else {
      delta_value = code;
    }

    int16_t value = last_value + delta_value;
    write_u16be(output + i * 2, (uint16_t)value);
    last_value = value;
  }

  *out_data = output;
  *out_size = output_size;
  return EOT_OK;
}
