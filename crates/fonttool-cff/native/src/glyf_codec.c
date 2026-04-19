#include "glyf_codec.h"
#include "byte_io.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#define SIMPLE_GLYPH_BBOX_MARKER 0x7FFF

#define TT_ON_CURVE 0x01
#define TT_X_SHORT 0x02
#define TT_Y_SHORT 0x04
#define TT_REPEAT_FLAG 0x08
#define TT_X_SAME 0x10
#define TT_Y_SAME 0x20

#define TT_NPUSHB 0x40
#define TT_NPUSHW 0x41
#define TT_PUSHB_BASE 0xB0
#define TT_PUSHW_BASE 0xB8

#define COMPOSITE_ARG_WORDS 0x0001
#define COMPOSITE_HAVE_SCALE 0x0008
#define COMPOSITE_MORE_COMPONENTS 0x0020
#define COMPOSITE_HAVE_XY_SCALE 0x0040
#define COMPOSITE_HAVE_TWO_BY_TWO 0x0080
#define COMPOSITE_HAVE_INSTRUCTIONS 0x0100

typedef struct {
  uint8_t *data;
  size_t size;
  size_t capacity;
} byte_writer_t;

typedef struct {
  int x;
  int y;
  int on_curve;
} decoded_point_t;

static void writer_init(byte_writer_t *writer) {
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
}

static void writer_destroy(byte_writer_t *writer) {
  free(writer->data);
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
}

static eot_status_t writer_reserve(byte_writer_t *writer, size_t additional) {
  if (additional > SIZE_MAX - writer->size) {
    return EOT_ERR_CORRUPT_DATA;
  }
  if (writer->size + additional <= writer->capacity) {
    return EOT_OK;
  }

  size_t new_capacity = writer->capacity == 0 ? 64 : writer->capacity;
  while (new_capacity < writer->size + additional) {
    if (new_capacity > SIZE_MAX / 2) {
      new_capacity = writer->size + additional;
      break;
    }
    new_capacity *= 2;
  }

  uint8_t *new_data = realloc(writer->data, new_capacity);
  if (!new_data) {
    return EOT_ERR_ALLOCATION;
  }

  writer->data = new_data;
  writer->capacity = new_capacity;
  return EOT_OK;
}

static eot_status_t writer_append_bytes(byte_writer_t *writer,
                                        const uint8_t *data, size_t length) {
  eot_status_t status = writer_reserve(writer, length);
  if (status != EOT_OK) {
    return status;
  }
  if (length > 0) {
    memcpy(writer->data + writer->size, data, length);
  }
  writer->size += length;
  return EOT_OK;
}

static eot_status_t writer_append_u8(byte_writer_t *writer, uint8_t value) {
  return writer_append_bytes(writer, &value, 1);
}

static eot_status_t writer_append_u16be(byte_writer_t *writer, uint16_t value) {
  uint8_t bytes[2];
  write_u16be(bytes, value);
  return writer_append_bytes(writer, bytes, sizeof(bytes));
}

static eot_status_t writer_append_i16be(byte_writer_t *writer, int16_t value) {
  return writer_append_u16be(writer, (uint16_t)value);
}

eot_status_t glyf_encode_255_ushort(uint16_t value, uint8_t *out_data,
                                    size_t out_capacity, size_t *out_size) {
  if (!out_data || !out_size) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (value < 253) {
    if (out_capacity < 1) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    out_data[0] = (uint8_t)value;
    *out_size = 1;
    return EOT_OK;
  }
  if (value < 506) {
    if (out_capacity < 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    out_data[0] = 255;
    out_data[1] = (uint8_t)(value - 253);
    *out_size = 2;
    return EOT_OK;
  }
  if (value < 762) {
    if (out_capacity < 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    out_data[0] = 254;
    out_data[1] = (uint8_t)(value - 506);
    *out_size = 2;
    return EOT_OK;
  }
  if (out_capacity < 3) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  out_data[0] = 253;
  write_u16be(out_data + 1, value);
  *out_size = 3;
  return EOT_OK;
}

eot_status_t glyf_encode_255_short(int16_t value, uint8_t *out_data,
                                   size_t out_capacity, size_t *out_size) {
  int short_value = value;
  size_t pos = 0;

  if (!out_data || !out_size) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (value >= 750 || value <= -750) {
    if (out_capacity < 3) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    out_data[0] = 253;
    write_u16be(out_data + 1, (uint16_t)value);
    *out_size = 3;
    return EOT_OK;
  }

  if (short_value < 0) {
    if (out_capacity < 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    out_data[pos++] = 250;
    short_value = -short_value;
  }

  if (short_value >= 250) {
    if (out_capacity < pos + 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    short_value -= 250;
    if (short_value >= 250) {
      short_value -= 250;
      out_data[pos++] = 254;
    } else {
      out_data[pos++] = 255;
    }
  } else if (out_capacity < pos + 1) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  out_data[pos++] = (uint8_t)short_value;
  *out_size = pos;
  return EOT_OK;
}

eot_status_t glyf_encode_triplet(int on_curve, int dx, int dy, uint8_t *out_flag,
                                 uint8_t *out_data, size_t out_capacity,
                                 size_t *out_size) {
  int abs_x;
  int abs_y;
  int on_curve_bit;
  int x_sign_bit;
  int y_sign_bit;
  int xy_sign_bits;

  if (!out_flag || !out_data || !out_size) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  abs_x = dx < 0 ? -dx : dx;
  abs_y = dy < 0 ? -dy : dy;
  on_curve_bit = on_curve ? 0 : 128;
  x_sign_bit = dx < 0 ? 0 : 1;
  y_sign_bit = dy < 0 ? 0 : 1;
  xy_sign_bits = x_sign_bit + 2 * y_sign_bit;

  if (dx == 0 && abs_y < 1280) {
    if (out_capacity < 1) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    *out_flag = (uint8_t)(on_curve_bit + ((abs_y & 0xF00) >> 7) + y_sign_bit);
    out_data[0] = (uint8_t)(abs_y & 0xFF);
    *out_size = 1;
    return EOT_OK;
  }
  if (dy == 0 && abs_x < 1280) {
    if (out_capacity < 1) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    *out_flag = (uint8_t)(on_curve_bit + 10 + ((abs_x & 0xF00) >> 7) + x_sign_bit);
    out_data[0] = (uint8_t)(abs_x & 0xFF);
    *out_size = 1;
    return EOT_OK;
  }
  if (abs_x < 65 && abs_y < 65) {
    if (out_capacity < 1) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    *out_flag = (uint8_t)(on_curve_bit + 20 + ((abs_x - 1) & 0x30) +
                          (((abs_y - 1) & 0x30) >> 2) + xy_sign_bits);
    out_data[0] = (uint8_t)((((abs_x - 1) & 0x0F) << 4) | ((abs_y - 1) & 0x0F));
    *out_size = 1;
    return EOT_OK;
  }
  if (abs_x < 769 && abs_y < 769) {
    if (out_capacity < 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    *out_flag = (uint8_t)(on_curve_bit + 84 +
                          12 * (((abs_x - 1) & 0x300) >> 8) +
                          (((abs_y - 1) & 0x300) >> 6) + xy_sign_bits);
    out_data[0] = (uint8_t)((abs_x - 1) & 0xFF);
    out_data[1] = (uint8_t)((abs_y - 1) & 0xFF);
    *out_size = 2;
    return EOT_OK;
  }
  if (abs_x < 4096 && abs_y < 4096) {
    if (out_capacity < 3) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    *out_flag = (uint8_t)(on_curve_bit + 120 + xy_sign_bits);
    out_data[0] = (uint8_t)(abs_x >> 4);
    out_data[1] = (uint8_t)(((abs_x & 0x0F) << 4) | (abs_y >> 8));
    out_data[2] = (uint8_t)(abs_y & 0xFF);
    *out_size = 3;
    return EOT_OK;
  }
  if (out_capacity < 4) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  *out_flag = (uint8_t)(on_curve_bit + 124 + xy_sign_bits);
  out_data[0] = (uint8_t)(abs_x >> 8);
  out_data[1] = (uint8_t)(abs_x & 0xFF);
  out_data[2] = (uint8_t)(abs_y >> 8);
  out_data[3] = (uint8_t)(abs_y & 0xFF);
  *out_size = 4;
  return EOT_OK;
}

static int16_t read_s16be_local(const uint8_t *data) {
  return (int16_t)read_u16be(data);
}

static int read_s16be_stream(buffer_view_t view, size_t *pos, int16_t *out_value) {
  if (!buffer_view_has_range(view, *pos, 2)) {
    return 0;
  }
  *out_value = read_s16be_local(view.data + *pos);
  *pos += 2;
  return 1;
}

static int read_u16be_stream(buffer_view_t view, size_t *pos, uint16_t *out_value) {
  if (!buffer_view_has_range(view, *pos, 2)) {
    return 0;
  }
  *out_value = read_u16be(view.data + *pos);
  *pos += 2;
  return 1;
}

static int read_u8_stream(buffer_view_t view, size_t *pos, uint8_t *out_value) {
  if (!buffer_view_has_range(view, *pos, 1)) {
    return 0;
  }
  *out_value = view.data[*pos];
  *pos += 1;
  return 1;
}

static int read_255_ushort(const uint8_t *data, size_t length, size_t *pos) {
  if (*pos >= length) {
    return -1;
  }

  uint8_t code = data[(*pos)++];
  if (code == 253) {
    if (*pos + 1 >= length) {
      return -1;
    }
    uint16_t value = read_u16be(data + *pos);
    *pos += 2;
    return value;
  }
  if (code == 254) {
    if (*pos >= length) {
      return -1;
    }
    return 506 + data[(*pos)++];
  }
  if (code == 255) {
    if (*pos >= length) {
      return -1;
    }
    return 253 + data[(*pos)++];
  }
  return code;
}

static int read_255_short(const uint8_t *data, size_t length, size_t *pos,
                          int16_t *out_value) {
  if (*pos >= length) {
    return 0;
  }

  uint8_t code = data[(*pos)++];
  if (code == 253) {
    if (*pos + 1 >= length) {
      return 0;
    }
    *out_value = read_s16be_local(data + *pos);
    *pos += 2;
    return 1;
  }

  int negative = 0;
  if (code == 250) {
    negative = 1;
    if (*pos >= length) {
      return 0;
    }
    code = data[(*pos)++];
  }

  int value = 0;
  if (code == 254) {
    if (*pos >= length) {
      return 0;
    }
    value = 500 + data[(*pos)++];
  } else if (code == 255) {
    if (*pos >= length) {
      return 0;
    }
    value = 250 + data[(*pos)++];
  } else if (code < 250) {
    value = code;
  } else {
    return 0;
  }

  *out_value = (int16_t)(negative ? -value : value);
  return 1;
}

static int with_sign(int sign_bits, int value) {
  return (sign_bits & 1) ? value : -value;
}

static int value_fits_in_int16(int value) {
  return value >= INT16_MIN && value <= INT16_MAX;
}

static eot_status_t append_encoded_255_short(byte_writer_t *writer, int16_t value) {
  uint8_t encoded[3];
  size_t encoded_size = 0;
  eot_status_t status = glyf_encode_255_short(value, encoded, sizeof(encoded),
                                              &encoded_size);

  if (status != EOT_OK) {
    return status;
  }
  return writer_append_bytes(writer, encoded, encoded_size);
}

static eot_status_t append_encoded_255_ushort(byte_writer_t *writer,
                                              uint16_t value) {
  uint8_t encoded[3];
  size_t encoded_size = 0;
  eot_status_t status = glyf_encode_255_ushort(value, encoded, sizeof(encoded),
                                               &encoded_size);

  if (status != EOT_OK) {
    return status;
  }
  return writer_append_bytes(writer, encoded, encoded_size);
}

static eot_status_t ensure_push_value_capacity(int16_t **values, size_t *capacity,
                                               size_t needed) {
  int16_t *new_values;
  size_t new_capacity = *capacity == 0 ? 16 : *capacity;

  if (needed <= *capacity) {
    return EOT_OK;
  }

  while (new_capacity < needed) {
    if (new_capacity > SIZE_MAX / 2) {
      new_capacity = needed;
      break;
    }
    new_capacity *= 2;
  }

  new_values = realloc(*values, new_capacity * sizeof(int16_t));
  if (!new_values) {
    return EOT_ERR_ALLOCATION;
  }

  *values = new_values;
  *capacity = new_capacity;
  return EOT_OK;
}

static eot_status_t encode_push_sequence(byte_writer_t *writer,
                                         const int16_t *values, size_t count) {
  int hop_skip = 0;

  for (size_t i = 0; i < count; i++) {
    if ((hop_skip & 1) == 0) {
      int16_t value = values[i];
      if (hop_skip == 0 && i >= 2 && i + 2 < count &&
          value == values[i - 2] && value == values[i + 2]) {
        if (i + 4 < count && value == values[i + 4]) {
          eot_status_t status = writer_append_u8(writer, 252);
          if (status != EOT_OK) {
            return status;
          }
          hop_skip = 0x14;
        } else {
          eot_status_t status = writer_append_u8(writer, 251);
          if (status != EOT_OK) {
            return status;
          }
          hop_skip = 4;
        }
      } else {
        eot_status_t status = append_encoded_255_short(writer, value);
        if (status != EOT_OK) {
          return status;
        }
      }
    }
    hop_skip >>= 1;
  }

  return EOT_OK;
}

eot_status_t glyf_split_push_code(buffer_view_t instructions, int *out_push_count,
                                  uint8_t **out_push_stream,
                                  size_t *out_push_stream_size,
                                  uint8_t **out_code_stream,
                                  size_t *out_code_stream_size) {
  size_t i = 0;
  int16_t *push_values = NULL;
  size_t push_value_count = 0;
  size_t push_value_capacity = 0;
  byte_writer_t push_writer;
  byte_writer_t code_writer;
  eot_status_t status = EOT_OK;

  if (!out_push_count || !out_push_stream || !out_push_stream_size ||
      !out_code_stream || !out_code_stream_size) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_push_count = 0;
  *out_push_stream = NULL;
  *out_push_stream_size = 0;
  *out_code_stream = NULL;
  *out_code_stream_size = 0;

  writer_init(&push_writer);
  writer_init(&code_writer);

  while (i + 1 < instructions.length) {
    size_t ix = i;
    uint8_t instr = instructions.data[ix++];
    size_t count = 0;
    size_t value_size = 0;
    size_t payload_size = 0;

    if (instr == TT_NPUSHB || instr == TT_NPUSHW) {
      count = instructions.data[ix++];
      value_size = (size_t)((instr & 1) + 1);
    } else if (instr >= TT_PUSHB_BASE && instr < 0xC0) {
      count = (size_t)(1 + (instr & 7));
      value_size = (size_t)(((instr & 8) >> 3) + 1);
    } else {
      break;
    }

    if (count > 0 && value_size > SIZE_MAX / count) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    payload_size = count * value_size;
    if (i > SIZE_MAX - payload_size) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    if (i + payload_size > instructions.length) {
      break;
    }

    status = ensure_push_value_capacity(&push_values, &push_value_capacity,
                                        push_value_count + count);
    if (status != EOT_OK) {
      goto cleanup;
    }

    for (size_t j = 0; j < count; j++) {
      if ((value_size == 1 && ix >= instructions.length) ||
          (value_size == 2 && !buffer_view_has_range(instructions, ix, 2))) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      push_values[push_value_count++] = value_size == 1
        ? (int16_t)instructions.data[ix]
        : read_s16be_local(instructions.data + ix);
      ix += value_size;
    }

    i = ix;
  }

  if (push_value_count > (size_t)INT_MAX) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  status = encode_push_sequence(&push_writer, push_values, push_value_count);
  if (status != EOT_OK) {
    goto cleanup;
  }

  status = writer_append_bytes(&code_writer, instructions.data + i,
                               instructions.length - i);
  if (status != EOT_OK) {
    goto cleanup;
  }

  *out_push_count = (int)push_value_count;
  *out_push_stream = push_writer.data;
  *out_push_stream_size = push_writer.size;
  *out_code_stream = code_writer.data;
  *out_code_stream_size = code_writer.size;

  free(push_values);
  return EOT_OK;

cleanup:
  free(push_values);
  writer_destroy(&push_writer);
  writer_destroy(&code_writer);
  return status;
}

static int read_loca_offset(buffer_view_t loca_table, int index_to_loca_format,
                            int glyph_id, uint32_t *out_offset) {
  size_t entry_offset = 0;

  if (!out_offset || glyph_id < 0) {
    return 0;
  }

  if (index_to_loca_format == 0) {
    entry_offset = (size_t)glyph_id * 2u;
    if (!buffer_view_has_range(loca_table, entry_offset, 2)) {
      return 0;
    }
    *out_offset = (uint32_t)read_u16be(loca_table.data + entry_offset) * 2u;
    return 1;
  }

  if (index_to_loca_format == 1) {
    entry_offset = (size_t)glyph_id * 4u;
    if (!buffer_view_has_range(loca_table, entry_offset, 4)) {
      return 0;
    }
    *out_offset = read_u32be(loca_table.data + entry_offset);
    return 1;
  }

  return 0;
}

static int trailing_bytes_are_zero(buffer_view_t view, size_t pos) {
  if (pos > view.length) {
    return 0;
  }
  for (size_t i = pos; i < view.length; i++) {
    if (view.data[i] != 0) {
      return 0;
    }
  }
  return 1;
}

static eot_status_t split_and_append_instructions(buffer_view_t instructions,
                                                  byte_writer_t *glyf_writer,
                                                  byte_writer_t *push_writer,
                                                  byte_writer_t *code_writer) {
  int push_count = 0;
  uint8_t *push_stream = NULL;
  size_t push_stream_size = 0;
  uint8_t *code_stream = NULL;
  size_t code_stream_size = 0;
  eot_status_t status = glyf_split_push_code(instructions, &push_count,
                                             &push_stream, &push_stream_size,
                                             &code_stream, &code_stream_size);
  if (status != EOT_OK) {
    return status;
  }

  if (push_count < 0 || push_count > 0xFFFF ||
      code_stream_size > 0xFFFFu) {
    free(push_stream);
    free(code_stream);
    return EOT_ERR_CORRUPT_DATA;
  }

  status = append_encoded_255_ushort(glyf_writer, (uint16_t)push_count);
  if (status == EOT_OK) {
    status = append_encoded_255_ushort(glyf_writer, (uint16_t)code_stream_size);
  }
  if (status == EOT_OK) {
    status = writer_append_bytes(push_writer, push_stream, push_stream_size);
  }
  if (status == EOT_OK) {
    status = writer_append_bytes(code_writer, code_stream, code_stream_size);
  }

  free(push_stream);
  free(code_stream);
  return status;
}

static eot_status_t encode_simple_glyph(buffer_view_t glyph_view,
                                        int16_t contour_count,
                                        byte_writer_t *glyf_writer,
                                        byte_writer_t *push_writer,
                                        byte_writer_t *code_writer) {
  size_t pos = 10;
  uint16_t *end_pts = NULL;
  uint8_t *flags = NULL;
  decoded_point_t *points = NULL;
  byte_writer_t payload_writer;
  size_t total_points = 0;
  eot_status_t status = EOT_OK;

  writer_init(&payload_writer);

  status = writer_append_i16be(glyf_writer, contour_count);
  if (status != EOT_OK) {
    goto cleanup;
  }

  if (glyph_view.length < 10) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  if (contour_count == 0) {
    goto cleanup;
  }

  end_pts = malloc((size_t)contour_count * sizeof(uint16_t));
  if (!end_pts) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  for (int i = 0; i < contour_count; i++) {
    uint16_t end_point = 0;
    uint16_t contour_points = 0;
    size_t previous_total_points = total_points;

    if (!read_u16be_stream(glyph_view, &pos, &end_point)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    end_pts[i] = end_point;
    if ((size_t)end_point + 1u < previous_total_points + 1u) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    contour_points = (uint16_t)((size_t)end_point + 1u - previous_total_points);
    if (contour_points == 0) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    total_points += contour_points;

    status = append_encoded_255_ushort(
        glyf_writer, (uint16_t)(contour_points - (i == 0 ? 1u : 0u)));
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (total_points == 0) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  uint16_t instruction_length = 0;
  if (!read_u16be_stream(glyph_view, &pos, &instruction_length) ||
      !buffer_view_has_range(glyph_view, pos, instruction_length)) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }
  buffer_view_t instructions = buffer_view_make(glyph_view.data + pos,
                                                instruction_length);
  pos += instruction_length;

  flags = malloc(total_points);
  points = malloc(total_points * sizeof(decoded_point_t));
  if (!flags || !points) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  for (size_t point_index = 0; point_index < total_points;) {
    uint8_t flag = 0;
    uint8_t repeat_count = 0;
    if (!read_u8_stream(glyph_view, &pos, &flag)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    flags[point_index++] = flag;

    if ((flag & TT_REPEAT_FLAG) != 0) {
      if (!read_u8_stream(glyph_view, &pos, &repeat_count) ||
          (size_t)repeat_count > total_points - point_index) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      for (uint8_t i = 0; i < repeat_count; i++) {
        flags[point_index++] = flag;
      }
    }
  }

  int last_x = 0;
  for (size_t i = 0; i < total_points; i++) {
    int dx = 0;
    if ((flags[i] & TT_X_SHORT) != 0) {
      uint8_t value = 0;
      if (!read_u8_stream(glyph_view, &pos, &value)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      dx = (flags[i] & TT_X_SAME) ? value : -(int)value;
    } else if ((flags[i] & TT_X_SAME) == 0) {
      int16_t value = 0;
      if (!read_s16be_stream(glyph_view, &pos, &value)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      dx = value;
    }

    if (!value_fits_in_int16(last_x + dx)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    last_x += dx;
    points[i].x = last_x;
    points[i].on_curve = (flags[i] & TT_ON_CURVE) != 0;
  }

  int last_y = 0;
  for (size_t i = 0; i < total_points; i++) {
    int dy = 0;
    if ((flags[i] & TT_Y_SHORT) != 0) {
      uint8_t value = 0;
      if (!read_u8_stream(glyph_view, &pos, &value)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      dy = (flags[i] & TT_Y_SAME) ? value : -(int)value;
    } else if ((flags[i] & TT_Y_SAME) == 0) {
      int16_t value = 0;
      if (!read_s16be_stream(glyph_view, &pos, &value)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      dy = value;
    }

    if (!value_fits_in_int16(last_y + dy)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    last_y += dy;
    points[i].y = last_y;
  }

  if (!trailing_bytes_are_zero(glyph_view, pos)) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  last_x = 0;
  last_y = 0;
  for (size_t i = 0; i < total_points; i++) {
    uint8_t flag = 0;
    uint8_t encoded_bytes[4];
    size_t encoded_size = 0;
    int dx = points[i].x - last_x;
    int dy = points[i].y - last_y;

    status = glyf_encode_triplet(points[i].on_curve, dx, dy, &flag,
                                 encoded_bytes, sizeof(encoded_bytes),
                                 &encoded_size);
    if (status != EOT_OK) {
      goto cleanup;
    }

    status = writer_append_u8(glyf_writer, flag);
    if (status != EOT_OK) {
      goto cleanup;
    }
    status = writer_append_bytes(&payload_writer, encoded_bytes, encoded_size);
    if (status != EOT_OK) {
      goto cleanup;
    }

    last_x = points[i].x;
    last_y = points[i].y;
  }

  status = writer_append_bytes(glyf_writer, payload_writer.data,
                               payload_writer.size);
  if (status != EOT_OK) {
    goto cleanup;
  }

  status = split_and_append_instructions(instructions, glyf_writer,
                                         push_writer, code_writer);

cleanup:
  free(end_pts);
  free(flags);
  free(points);
  writer_destroy(&payload_writer);
  return status;
}

static size_t composite_component_extra_bytes(uint16_t flags) {
  size_t bytes = (flags & COMPOSITE_ARG_WORDS) ? 4u : 2u;
  if (flags & COMPOSITE_HAVE_SCALE) {
    bytes += 2u;
  } else if (flags & COMPOSITE_HAVE_XY_SCALE) {
    bytes += 4u;
  } else if (flags & COMPOSITE_HAVE_TWO_BY_TWO) {
    bytes += 8u;
  }
  return bytes;
}

static eot_status_t encode_composite_glyph(buffer_view_t glyph_view,
                                           byte_writer_t *glyf_writer,
                                           byte_writer_t *push_writer,
                                           byte_writer_t *code_writer) {
  size_t pos = 10;
  uint16_t flags = 0;
  eot_status_t status = EOT_OK;

  if (glyph_view.length < 10) {
    return EOT_ERR_CORRUPT_DATA;
  }

  status = writer_append_i16be(glyf_writer, -1);
  if (status != EOT_OK) {
    return status;
  }
  status = writer_append_bytes(glyf_writer, glyph_view.data + 2, 8);
  if (status != EOT_OK) {
    return status;
  }

  do {
    size_t component_start = pos;
    size_t extra_bytes = 0;
    if (!buffer_view_has_range(glyph_view, pos, 4)) {
      return EOT_ERR_CORRUPT_DATA;
    }

    flags = read_u16be(glyph_view.data + pos);
    pos += 4;
    extra_bytes = composite_component_extra_bytes(flags);
    if (!buffer_view_has_range(glyph_view, pos, extra_bytes)) {
      return EOT_ERR_CORRUPT_DATA;
    }

    status = writer_append_bytes(glyf_writer, glyph_view.data + component_start,
                                 4u + extra_bytes);
    if (status != EOT_OK) {
      return status;
    }
    pos += extra_bytes;
  } while ((flags & COMPOSITE_MORE_COMPONENTS) != 0);

  if ((flags & COMPOSITE_HAVE_INSTRUCTIONS) != 0) {
    uint16_t instruction_length = 0;
    if (!read_u16be_stream(glyph_view, &pos, &instruction_length) ||
        !buffer_view_has_range(glyph_view, pos, instruction_length)) {
      return EOT_ERR_CORRUPT_DATA;
    }

    buffer_view_t instructions = buffer_view_make(glyph_view.data + pos,
                                                  instruction_length);
    pos += instruction_length;
    status = split_and_append_instructions(instructions, glyf_writer,
                                           push_writer, code_writer);
    if (status != EOT_OK) {
      return status;
    }
  }

  if (!trailing_bytes_are_zero(glyph_view, pos)) {
    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_OK;
}

eot_status_t glyf_encode(buffer_view_t glyf_table, buffer_view_t loca_table,
                         int index_to_loca_format, int num_glyphs,
                         uint8_t **out_glyf_stream,
                         size_t *out_glyf_stream_size,
                         uint8_t **out_push_stream,
                         size_t *out_push_stream_size,
                         uint8_t **out_code_stream,
                         size_t *out_code_stream_size) {
  byte_writer_t glyf_writer;
  byte_writer_t push_writer;
  byte_writer_t code_writer;
  eot_status_t status = EOT_OK;

  if (!out_glyf_stream || !out_glyf_stream_size ||
      !out_push_stream || !out_push_stream_size ||
      !out_code_stream || !out_code_stream_size ||
      num_glyphs < 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_glyf_stream = NULL;
  *out_glyf_stream_size = 0;
  *out_push_stream = NULL;
  *out_push_stream_size = 0;
  *out_code_stream = NULL;
  *out_code_stream_size = 0;

  writer_init(&glyf_writer);
  writer_init(&push_writer);
  writer_init(&code_writer);

  for (int glyph_id = 0; glyph_id < num_glyphs; glyph_id++) {
    uint32_t glyph_offset = 0;
    uint32_t next_offset = 0;
    int16_t contour_count = 0;

    if (!read_loca_offset(loca_table, index_to_loca_format, glyph_id,
                          &glyph_offset) ||
        !read_loca_offset(loca_table, index_to_loca_format, glyph_id + 1,
                          &next_offset) ||
        next_offset < glyph_offset ||
        next_offset > glyf_table.length) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    if (glyph_offset == next_offset) {
      status = writer_append_u16be(&glyf_writer, 0);
      if (status != EOT_OK) {
        goto cleanup;
      }
      continue;
    }

    buffer_view_t glyph_view =
        buffer_view_make(glyf_table.data + glyph_offset, next_offset - glyph_offset);
    if (glyph_view.length < 10) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    contour_count = read_s16be_local(glyph_view.data);
    if (contour_count >= 0) {
      status = encode_simple_glyph(glyph_view, contour_count, &glyf_writer,
                                   &push_writer, &code_writer);
    } else if (contour_count == -1) {
      status = encode_composite_glyph(glyph_view, &glyf_writer, &push_writer,
                                      &code_writer);
    } else {
      status = EOT_ERR_CORRUPT_DATA;
    }

    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  *out_glyf_stream = glyf_writer.data;
  *out_glyf_stream_size = glyf_writer.size;
  *out_push_stream = push_writer.data;
  *out_push_stream_size = push_writer.size;
  *out_code_stream = code_writer.data;
  *out_code_stream_size = code_writer.size;
  return EOT_OK;

cleanup:
  writer_destroy(&glyf_writer);
  writer_destroy(&push_writer);
  writer_destroy(&code_writer);
  return status;
}

static int decode_triplet(uint8_t flag, buffer_view_t payload_stream, size_t *payload_pos,
                          int *out_dx, int *out_dy, int *out_on_curve) {
  int triplet_code = flag & 0x7F;
  uint8_t b0;
  uint8_t b1;
  uint8_t b2;
  uint8_t b3;

  *out_on_curve = (flag & 0x80) == 0;

  if (triplet_code < 10) {
    if (!read_u8_stream(payload_stream, payload_pos, &b0)) {
      return 0;
    }
    *out_dx = 0;
    *out_dy = with_sign(triplet_code, ((triplet_code & 0x0E) << 7) + b0);
    return 1;
  }
  if (triplet_code < 20) {
    if (!read_u8_stream(payload_stream, payload_pos, &b0)) {
      return 0;
    }
    *out_dx = with_sign(triplet_code, (((triplet_code - 10) & 0x0E) << 7) + b0);
    *out_dy = 0;
    return 1;
  }
  if (triplet_code < 84) {
    if (!read_u8_stream(payload_stream, payload_pos, &b0)) {
      return 0;
    }
    triplet_code -= 20;
    *out_dx = with_sign(flag, 1 + (triplet_code & 0x30) + (b0 >> 4));
    *out_dy = with_sign(flag >> 1, 1 + ((triplet_code & 0x0C) << 2) + (b0 & 0x0F));
    return 1;
  }
  if (triplet_code < 120) {
    if (!read_u8_stream(payload_stream, payload_pos, &b0) ||
        !read_u8_stream(payload_stream, payload_pos, &b1)) {
      return 0;
    }
    triplet_code -= 84;
    *out_dx = with_sign(flag, 1 + ((triplet_code / 12) << 8) + b0);
    *out_dy = with_sign(flag >> 1, 1 + (((triplet_code % 12) >> 2) << 8) + b1);
    return 1;
  }
  if (triplet_code < 124) {
    if (!read_u8_stream(payload_stream, payload_pos, &b0) ||
        !read_u8_stream(payload_stream, payload_pos, &b1) ||
        !read_u8_stream(payload_stream, payload_pos, &b2)) {
      return 0;
    }
    *out_dx = with_sign(flag, (b0 << 4) + (b1 >> 4));
    *out_dy = with_sign(flag >> 1, ((b1 & 0x0F) << 8) + b2);
    return 1;
  }
  if (!read_u8_stream(payload_stream, payload_pos, &b0) ||
      !read_u8_stream(payload_stream, payload_pos, &b1) ||
      !read_u8_stream(payload_stream, payload_pos, &b2) ||
      !read_u8_stream(payload_stream, payload_pos, &b3)) {
    return 0;
  }
  *out_dx = with_sign(flag, (b0 << 8) + b1);
  *out_dy = with_sign(flag >> 1, (b2 << 8) + b3);
  return 1;
}

static eot_status_t decode_push_values(buffer_view_t push_stream, size_t *push_pos,
                                       int push_count, int16_t **out_values) {
  if (push_count == 0) {
    *out_values = NULL;
    return EOT_OK;
  }

  int16_t *values = malloc((size_t)push_count * sizeof(int16_t));
  if (!values) {
    return EOT_ERR_ALLOCATION;
  }

  int value_index = 0;
  while (value_index < push_count) {
    if (*push_pos >= push_stream.length) {
      free(values);
      return EOT_ERR_CORRUPT_DATA;
    }

    uint8_t code = push_stream.data[*push_pos];
    if (code == 251 || code == 252) {
      int hop_count = code == 251 ? 3 : 5;
      int16_t repeated_value = 0;
      if (value_index < 2 || value_index + hop_count > push_count) {
        free(values);
        return EOT_ERR_CORRUPT_DATA;
      }

      repeated_value = values[value_index - 2];
      *push_pos += 1;
      values[value_index++] = repeated_value;

      int16_t middle_value = 0;
      if (!read_255_short(push_stream.data, push_stream.length, push_pos,
                          &middle_value)) {
        free(values);
        return EOT_ERR_CORRUPT_DATA;
      }
      values[value_index++] = middle_value;
      values[value_index++] = repeated_value;
      if (code == 252) {
        int16_t second_middle_value = 0;
        if (!read_255_short(push_stream.data, push_stream.length, push_pos,
                            &second_middle_value)) {
          free(values);
          return EOT_ERR_CORRUPT_DATA;
        }
        values[value_index++] = second_middle_value;
        values[value_index++] = repeated_value;
      }
      continue;
    }

    if (!read_255_short(push_stream.data, push_stream.length, push_pos,
                        &values[value_index])) {
      free(values);
      return EOT_ERR_CORRUPT_DATA;
    }
    value_index++;
  }

  *out_values = values;
  return EOT_OK;
}

static eot_status_t append_push_run(byte_writer_t *writer, const int16_t *values,
                                    int count, int byte_sized) {
  while (count > 0) {
    int chunk = count > 255 ? 255 : count;
    if (byte_sized) {
      if (chunk <= 8) {
        eot_status_t status = writer_append_u8(writer,
                                               (uint8_t)(TT_PUSHB_BASE + chunk - 1));
        if (status != EOT_OK) {
          return status;
        }
      } else {
        eot_status_t status = writer_append_u8(writer, TT_NPUSHB);
        if (status != EOT_OK) {
          return status;
        }
        status = writer_append_u8(writer, (uint8_t)chunk);
        if (status != EOT_OK) {
          return status;
        }
      }
      for (int i = 0; i < chunk; i++) {
        eot_status_t status = writer_append_u8(writer, (uint8_t)values[i]);
        if (status != EOT_OK) {
          return status;
        }
      }
    } else {
      if (chunk <= 8) {
        eot_status_t status = writer_append_u8(writer,
                                               (uint8_t)(TT_PUSHW_BASE + chunk - 1));
        if (status != EOT_OK) {
          return status;
        }
      } else {
        eot_status_t status = writer_append_u8(writer, TT_NPUSHW);
        if (status != EOT_OK) {
          return status;
        }
        status = writer_append_u8(writer, (uint8_t)chunk);
        if (status != EOT_OK) {
          return status;
        }
      }
      for (int i = 0; i < chunk; i++) {
        eot_status_t status = writer_append_i16be(writer, values[i]);
        if (status != EOT_OK) {
          return status;
        }
      }
    }

    values += chunk;
    count -= chunk;
  }

  return EOT_OK;
}

static eot_status_t append_push_instructions(byte_writer_t *writer,
                                             const int16_t *values, int count) {
  int run_start = 0;
  while (run_start < count) {
    int byte_sized = values[run_start] >= 0 && values[run_start] <= 255;
    int run_end = run_start + 1;
    while (run_end < count) {
      int next_byte_sized = values[run_end] >= 0 && values[run_end] <= 255;
      if (next_byte_sized != byte_sized) {
        break;
      }
      run_end++;
    }

    eot_status_t status = append_push_run(writer, values + run_start,
                                          run_end - run_start, byte_sized);
    if (status != EOT_OK) {
      return status;
    }

    run_start = run_end;
  }

  return EOT_OK;
}

static eot_status_t build_instruction_stream(buffer_view_t push_stream, size_t *push_pos,
                                             buffer_view_t code_stream, size_t *code_pos,
                                             int push_count, int code_size,
                                             byte_writer_t *instructions) {
  int16_t *push_values = NULL;
  eot_status_t status = decode_push_values(push_stream, push_pos, push_count,
                                           &push_values);
  if (status != EOT_OK) {
    return status;
  }

  if (push_count > 0) {
    status = append_push_instructions(instructions, push_values, push_count);
    free(push_values);
    if (status != EOT_OK) {
      return status;
    }
  }

  if (code_size > 0) {
    if (!buffer_view_has_range(code_stream, *code_pos, (size_t)code_size)) {
      return EOT_ERR_CORRUPT_DATA;
    }
    status = writer_append_bytes(instructions, code_stream.data + *code_pos,
                                 (size_t)code_size);
    if (status != EOT_OK) {
      return status;
    }
    *code_pos += (size_t)code_size;
  }

  return EOT_OK;
}

static eot_status_t append_flag_run(byte_writer_t *writer, uint8_t flag,
                                    size_t count) {
  while (count > 0) {
    size_t chunk = count > 256u ? 256u : count;
    uint8_t encoded_flag = flag;
    eot_status_t status;

    if (chunk > 1u) {
      encoded_flag |= TT_REPEAT_FLAG;
    }

    status = writer_append_u8(writer, encoded_flag);
    if (status != EOT_OK) {
      return status;
    }

    if (chunk > 1u) {
      status = writer_append_u8(writer, (uint8_t)(chunk - 1u));
      if (status != EOT_OK) {
        return status;
      }
    }

    count -= chunk;
  }

  return EOT_OK;
}

static eot_status_t append_compressed_flags(byte_writer_t *writer,
                                            const uint8_t *flags_data,
                                            size_t flags_size) {
  size_t run_start = 0;

  while (run_start < flags_size) {
    uint8_t flag = flags_data[run_start];
    size_t run_end = run_start + 1u;

    while (run_end < flags_size && flags_data[run_end] == flag) {
      run_end++;
    }

    eot_status_t status = append_flag_run(writer, flag, run_end - run_start);
    if (status != EOT_OK) {
      return status;
    }

    run_start = run_end;
  }

  return EOT_OK;
}

static eot_status_t append_simple_points(byte_writer_t *writer,
                                         const decoded_point_t *points,
                                         size_t num_points) {
  byte_writer_t flags;
  byte_writer_t x_bytes;
  byte_writer_t y_bytes;
  int last_x = 0;
  int last_y = 0;
  eot_status_t status = EOT_OK;

  writer_init(&flags);
  writer_init(&x_bytes);
  writer_init(&y_bytes);

  for (size_t i = 0; i < num_points; i++) {
    int dx = points[i].x - last_x;
    int dy = points[i].y - last_y;
    uint8_t flag = points[i].on_curve ? TT_ON_CURVE : 0;

    if (!value_fits_in_int16(dx) || !value_fits_in_int16(dy)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    if (dx == 0) {
      flag |= TT_X_SAME;
    } else if (dx > 0 && dx < 255) {
      flag |= (uint8_t)(TT_X_SHORT | TT_X_SAME);
      status = writer_append_u8(&x_bytes, (uint8_t)dx);
      if (status != EOT_OK) {
        goto cleanup;
      }
    } else if (dx < 0 && dx >= -255) {
      flag |= TT_X_SHORT;
      status = writer_append_u8(&x_bytes, (uint8_t)(-dx));
      if (status != EOT_OK) {
        goto cleanup;
      }
    } else {
      status = writer_append_i16be(&x_bytes, (int16_t)dx);
      if (status != EOT_OK) {
        goto cleanup;
      }
    }

    if (dy == 0) {
      flag |= TT_Y_SAME;
    } else if (dy > 0 && dy < 255) {
      flag |= (uint8_t)(TT_Y_SHORT | TT_Y_SAME);
      status = writer_append_u8(&y_bytes, (uint8_t)dy);
      if (status != EOT_OK) {
        goto cleanup;
      }
    } else if (dy < 0 && dy >= -255) {
      flag |= TT_Y_SHORT;
      status = writer_append_u8(&y_bytes, (uint8_t)(-dy));
      if (status != EOT_OK) {
        goto cleanup;
      }
    } else {
      status = writer_append_i16be(&y_bytes, (int16_t)dy);
      if (status != EOT_OK) {
        goto cleanup;
      }
    }

    status = writer_append_u8(&flags, flag);
    if (status != EOT_OK) {
      goto cleanup;
    }

    last_x = points[i].x;
    last_y = points[i].y;
  }

  status = append_compressed_flags(writer, flags.data, flags.size);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_bytes(writer, x_bytes.data, x_bytes.size);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_bytes(writer, y_bytes.data, y_bytes.size);

cleanup:
  writer_destroy(&flags);
  writer_destroy(&x_bytes);
  writer_destroy(&y_bytes);
  return status;
}

static eot_status_t decode_simple_glyph(buffer_view_t glyf_stream, size_t *glyf_pos,
                                        buffer_view_t push_stream, size_t *push_pos,
                                        buffer_view_t code_stream, size_t *code_pos,
                                        int16_t contour_count,
                                        byte_writer_t *glyf_data) {
  int16_t x_min = 0;
  int16_t y_min = 0;
  int16_t x_max = 0;
  int16_t y_max = 0;
  int explicit_bbox = 0;
  uint16_t *end_pts = NULL;
  decoded_point_t *points = NULL;
  size_t total_points = 0;
  byte_writer_t instructions;
  eot_status_t status = EOT_OK;

  writer_init(&instructions);

  if (contour_count == SIMPLE_GLYPH_BBOX_MARKER) {
    explicit_bbox = 1;
    if (!read_s16be_stream(glyf_stream, glyf_pos, &contour_count) ||
        !read_s16be_stream(glyf_stream, glyf_pos, &x_min) ||
        !read_s16be_stream(glyf_stream, glyf_pos, &y_min) ||
        !read_s16be_stream(glyf_stream, glyf_pos, &x_max) ||
        !read_s16be_stream(glyf_stream, glyf_pos, &y_max)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
  }

  if (contour_count < 0) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  if (contour_count > 0) {
    end_pts = malloc((size_t)contour_count * sizeof(uint16_t));
    if (!end_pts) {
      status = EOT_ERR_ALLOCATION;
      goto cleanup;
    }

    for (int i = 0; i < contour_count; i++) {
      int contour_points = read_255_ushort(glyf_stream.data, glyf_stream.length,
                                           glyf_pos);
      if (contour_points < 0) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      contour_points += i == 0 ? 1 : 0;
      if (contour_points <= 0 ||
          (size_t)contour_points > SIZE_MAX - total_points ||
          total_points + (size_t)contour_points > 0xFFFFu) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      total_points += (size_t)contour_points;
      end_pts[i] = (uint16_t)(total_points - 1);
    }
  }

  if (total_points > 0) {
    if (!buffer_view_has_range(glyf_stream, *glyf_pos, total_points)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    const uint8_t *flags = glyf_stream.data + *glyf_pos;
    size_t payload_pos = *glyf_pos + total_points;
    points = malloc(total_points * sizeof(decoded_point_t));
    if (!points) {
      status = EOT_ERR_ALLOCATION;
      goto cleanup;
    }

    int last_x = 0;
    int last_y = 0;
    for (size_t i = 0; i < total_points; i++) {
      int dx = 0;
      int dy = 0;
      int on_curve = 0;
      int next_x = 0;
      int next_y = 0;
      if (!decode_triplet(flags[i], glyf_stream, &payload_pos, &dx, &dy,
                          &on_curve)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }

      next_x = last_x + dx;
      next_y = last_y + dy;
      if (!value_fits_in_int16(next_x) || !value_fits_in_int16(next_y)) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }

      last_x = next_x;
      last_y = next_y;
      points[i].x = next_x;
      points[i].y = next_y;
      points[i].on_curve = on_curve;

      if (!explicit_bbox || i == 0) {
        if (i == 0) {
          x_min = x_max = (int16_t)next_x;
          y_min = y_max = (int16_t)next_y;
        } else {
          if (next_x < x_min) {
            x_min = (int16_t)next_x;
          }
          if (next_x > x_max) {
            x_max = (int16_t)next_x;
          }
          if (next_y < y_min) {
            y_min = (int16_t)next_y;
          }
          if (next_y > y_max) {
            y_max = (int16_t)next_y;
          }
        }
      }
    }

    *glyf_pos = payload_pos;
  }

  if (contour_count > 0) {
    int push_count = read_255_ushort(glyf_stream.data, glyf_stream.length,
                                     glyf_pos);
    int code_size = read_255_ushort(glyf_stream.data, glyf_stream.length,
                                    glyf_pos);
    if (push_count < 0 || code_size < 0) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    status = build_instruction_stream(push_stream, push_pos, code_stream, code_pos,
                                      push_count, code_size, &instructions);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  status = writer_append_i16be(glyf_data, contour_count);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_i16be(glyf_data, x_min);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_i16be(glyf_data, y_min);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_i16be(glyf_data, x_max);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_i16be(glyf_data, y_max);
  if (status != EOT_OK) {
    goto cleanup;
  }

  for (int i = 0; i < contour_count; i++) {
    status = writer_append_u16be(glyf_data, end_pts[i]);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (instructions.size > 0xFFFFu) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }
  status = writer_append_u16be(glyf_data, (uint16_t)instructions.size);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = writer_append_bytes(glyf_data, instructions.data, instructions.size);
  if (status != EOT_OK) {
    goto cleanup;
  }

  if (total_points > 0) {
    status = append_simple_points(glyf_data, points, total_points);
  }

cleanup:
  free(end_pts);
  free(points);
  writer_destroy(&instructions);
  return status;
}

static eot_status_t decode_composite_glyph(buffer_view_t glyf_stream, size_t *glyf_pos,
                                           buffer_view_t push_stream, size_t *push_pos,
                                           buffer_view_t code_stream, size_t *code_pos,
                                           byte_writer_t *glyf_data) {
  byte_writer_t instructions;
  uint16_t flags = 0;
  eot_status_t status = EOT_OK;

  writer_init(&instructions);

  status = writer_append_i16be(glyf_data, -1);
  if (status != EOT_OK) {
    goto cleanup;
  }

  for (int i = 0; i < 4; i++) {
    int16_t bbox_value = 0;
    if (!read_s16be_stream(glyf_stream, glyf_pos, &bbox_value)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    status = writer_append_i16be(glyf_data, bbox_value);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  do {
    uint16_t glyph_index = 0;
    size_t component_bytes = 0;

    if (!read_u16be_stream(glyf_stream, glyf_pos, &flags) ||
        !read_u16be_stream(glyf_stream, glyf_pos, &glyph_index)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    status = writer_append_u16be(glyf_data, flags);
    if (status != EOT_OK) {
      goto cleanup;
    }
    status = writer_append_u16be(glyf_data, glyph_index);
    if (status != EOT_OK) {
      goto cleanup;
    }

    component_bytes = (flags & COMPOSITE_ARG_WORDS) ? 4u : 2u;
    if (flags & COMPOSITE_HAVE_SCALE) {
      component_bytes += 2u;
    } else if (flags & COMPOSITE_HAVE_XY_SCALE) {
      component_bytes += 4u;
    } else if (flags & COMPOSITE_HAVE_TWO_BY_TWO) {
      component_bytes += 8u;
    }

    if (!buffer_view_has_range(glyf_stream, *glyf_pos, component_bytes)) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    status = writer_append_bytes(glyf_data, glyf_stream.data + *glyf_pos,
                                 component_bytes);
    if (status != EOT_OK) {
      goto cleanup;
    }
    *glyf_pos += component_bytes;
  } while ((flags & COMPOSITE_MORE_COMPONENTS) != 0);

  if ((flags & COMPOSITE_HAVE_INSTRUCTIONS) != 0) {
    int push_count = read_255_ushort(glyf_stream.data, glyf_stream.length,
                                     glyf_pos);
    int code_size = read_255_ushort(glyf_stream.data, glyf_stream.length,
                                    glyf_pos);
    if (push_count < 0 || code_size < 0) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }

    status = build_instruction_stream(push_stream, push_pos, code_stream, code_pos,
                                      push_count, code_size, &instructions);
    if (status != EOT_OK) {
      goto cleanup;
    }
    if (instructions.size > 0xFFFFu) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    status = writer_append_u16be(glyf_data, (uint16_t)instructions.size);
    if (status != EOT_OK) {
      goto cleanup;
    }
    status = writer_append_bytes(glyf_data, instructions.data, instructions.size);
  }

cleanup:
  writer_destroy(&instructions);
  return status;
}

static int write_short_loca_entry(uint8_t *loca_data, int glyph_id, size_t offset) {
  if ((offset & 1u) != 0 || offset / 2u > 0xFFFFu) {
    return 0;
  }
  write_u16be(loca_data + glyph_id * 2, (uint16_t)(offset / 2u));
  return 1;
}

static int write_loca_entry(uint8_t *loca_data, int glyph_id, size_t offset,
                            int index_to_loca_format) {
  if (index_to_loca_format == 0) {
    return write_short_loca_entry(loca_data, glyph_id, offset);
  }
  if (index_to_loca_format == 1) {
    if (offset > 0xFFFFFFFFu) {
      return 0;
    }
    write_u32be(loca_data + glyph_id * 4, (uint32_t)offset);
    return 1;
  }
  return 0;
}

eot_status_t glyf_decode_with_loca_format(buffer_view_t glyf_stream,
                                          buffer_view_t push_stream,
                                          buffer_view_t code_stream,
                                          int num_glyphs,
                                          int index_to_loca_format,
                                          uint8_t **out_glyf,
                                          size_t *out_glyf_size,
                                          uint8_t **out_loca,
                                          size_t *out_loca_size) {
  byte_writer_t glyf_data;
  size_t glyf_stream_pos = 0;
  size_t push_pos = 0;
  size_t code_pos = 0;
  size_t loca_size = 0;
  uint8_t *loca_data = NULL;

  if (num_glyphs < 0 || (index_to_loca_format != 0 && index_to_loca_format != 1)) {
    return EOT_ERR_CORRUPT_DATA;
  }

  writer_init(&glyf_data);

  loca_size = ((size_t)num_glyphs + 1u) * (index_to_loca_format == 0 ? 2u : 4u);
  loca_data = malloc(loca_size);
  if (!loca_data) {
    return EOT_ERR_ALLOCATION;
  }

  for (int glyph_id = 0; glyph_id <= num_glyphs; glyph_id++) {
    if (!write_loca_entry(loca_data, glyph_id, glyf_data.size,
                          index_to_loca_format)) {
      free(loca_data);
      writer_destroy(&glyf_data);
      return EOT_ERR_CORRUPT_DATA;
    }

    if (glyph_id == num_glyphs) {
      break;
    }

    int16_t contour_count = 0;
    size_t glyph_start = glyf_data.size;
    eot_status_t status = EOT_OK;

    if (!read_s16be_stream(glyf_stream, &glyf_stream_pos, &contour_count)) {
      free(loca_data);
      writer_destroy(&glyf_data);
      return EOT_ERR_CORRUPT_DATA;
    }

    if (contour_count == 0) {
      continue;
    }

    if (contour_count == -1) {
      status = decode_composite_glyph(glyf_stream, &glyf_stream_pos, push_stream,
                                      &push_pos, code_stream, &code_pos,
                                      &glyf_data);
    } else {
      status = decode_simple_glyph(glyf_stream, &glyf_stream_pos, push_stream,
                                   &push_pos, code_stream, &code_pos,
                                   contour_count, &glyf_data);
    }

    if (status != EOT_OK) {
      free(loca_data);
      writer_destroy(&glyf_data);
      return status;
    }

    size_t alignment = index_to_loca_format == 0 ? 2u : 4u;
    while (((glyf_data.size - glyph_start) % alignment) != 0) {
      status = writer_append_u8(&glyf_data, 0);
      if (status != EOT_OK) {
        free(loca_data);
        writer_destroy(&glyf_data);
        return status;
      }
    }
  }

  if (glyf_stream_pos != glyf_stream.length ||
      push_pos != push_stream.length ||
      code_pos != code_stream.length) {
    free(loca_data);
    writer_destroy(&glyf_data);
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_glyf = glyf_data.data;
  *out_glyf_size = glyf_data.size;
  *out_loca = loca_data;
  *out_loca_size = loca_size;
  return EOT_OK;
}

eot_status_t glyf_decode(buffer_view_t glyf_stream, buffer_view_t push_stream,
                         buffer_view_t code_stream, int num_glyphs,
                         uint8_t **out_glyf, size_t *out_glyf_size,
                         uint8_t **out_loca, size_t *out_loca_size) {
  return glyf_decode_with_loca_format(glyf_stream, push_stream, code_stream,
                                      num_glyphs, 0, out_glyf, out_glyf_size,
                                      out_loca, out_loca_size);
}
