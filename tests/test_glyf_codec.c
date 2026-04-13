#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/glyf_codec.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define ASSERT_OK(expr) do { \
  eot_status_t status__ = (expr); \
  if (status__ != EOT_OK) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s returned %d", #expr, status__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s", #expr); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), \
             "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_BYTES_EQ(actual, actual_size, expected, expected_size) do { \
  size_t actual_size__ = (actual_size); \
  size_t expected_size__ = (expected_size); \
  if (actual_size__ != expected_size__ || \
      memcmp((actual), (expected), expected_size__) != 0) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), \
             "assertion failed: byte arrays differ (%s size=%zu, %s size=%zu)", \
             #actual, actual_size__, #expected, expected_size__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

static uint16_t read_u16be_local(const uint8_t *data) {
  return ((uint16_t)data[0] << 8) | (uint16_t)data[1];
}

static size_t write_u16be_local(uint8_t *dest, uint16_t value) {
  dest[0] = (uint8_t)(value >> 8);
  dest[1] = (uint8_t)(value & 0xFF);
  return 2;
}

static size_t write_255_ushort_reference(uint8_t *dest, uint16_t value) {
  if (value < 253) {
    dest[0] = (uint8_t)value;
    return 1;
  }
  if (value < 506) {
    dest[0] = 255;
    dest[1] = (uint8_t)(value - 253);
    return 2;
  }
  if (value < 762) {
    dest[0] = 254;
    dest[1] = (uint8_t)(value - 506);
    return 2;
  }
  dest[0] = 253;
  dest[1] = (uint8_t)(value >> 8);
  dest[2] = (uint8_t)(value & 0xFF);
  return 3;
}

static size_t write_255_short_reference(uint8_t *dest, int16_t value) {
  if (value >= 750 || value <= -750) {
    dest[0] = 253;
    dest[1] = (uint8_t)(((uint16_t)value) >> 8);
    dest[2] = (uint8_t)((uint16_t)value & 0xFF);
    return 3;
  }

  size_t pos = 0;
  int short_value = value;
  if (short_value < 0) {
    dest[pos++] = 250;
    short_value = -short_value;
  }

  if (short_value >= 250) {
    short_value -= 250;
    if (short_value >= 250) {
      short_value -= 250;
      dest[pos++] = 254;
    } else {
      dest[pos++] = 255;
    }
  }

  dest[pos++] = (uint8_t)short_value;
  return pos;
}

static size_t write_triplet_reference(uint8_t *flag_dest, uint8_t *coord_dest,
                                      size_t coord_pos, int on_curve,
                                      int dx, int dy) {
  int abs_x = dx < 0 ? -dx : dx;
  int abs_y = dy < 0 ? -dy : dy;
  int on_curve_bit = on_curve ? 0 : 128;
  int x_sign_bit = dx < 0 ? 0 : 1;
  int y_sign_bit = dy < 0 ? 0 : 1;
  int xy_sign_bits = x_sign_bit + 2 * y_sign_bit;

  if (dx == 0 && abs_y < 1280) {
    *flag_dest = (uint8_t)(on_curve_bit + ((abs_y & 0xF00) >> 7) + y_sign_bit);
    coord_dest[coord_pos++] = (uint8_t)(abs_y & 0xFF);
  } else if (dy == 0 && abs_x < 1280) {
    *flag_dest = (uint8_t)(on_curve_bit + 10 + ((abs_x & 0xF00) >> 7) + x_sign_bit);
    coord_dest[coord_pos++] = (uint8_t)(abs_x & 0xFF);
  } else if (abs_x < 65 && abs_y < 65) {
    *flag_dest = (uint8_t)(on_curve_bit + 20 + ((abs_x - 1) & 0x30) +
                           (((abs_y - 1) & 0x30) >> 2) + xy_sign_bits);
    coord_dest[coord_pos++] = (uint8_t)((((abs_x - 1) & 0x0F) << 4) |
                                        ((abs_y - 1) & 0x0F));
  } else if (abs_x < 769 && abs_y < 769) {
    *flag_dest = (uint8_t)(on_curve_bit + 84 +
                           12 * (((abs_x - 1) & 0x300) >> 8) +
                           (((abs_y - 1) & 0x300) >> 6) + xy_sign_bits);
    coord_dest[coord_pos++] = (uint8_t)((abs_x - 1) & 0xFF);
    coord_dest[coord_pos++] = (uint8_t)((abs_y - 1) & 0xFF);
  } else if (abs_x < 4096 && abs_y < 4096) {
    *flag_dest = (uint8_t)(on_curve_bit + 120 + xy_sign_bits);
    coord_dest[coord_pos++] = (uint8_t)(abs_x >> 4);
    coord_dest[coord_pos++] = (uint8_t)(((abs_x & 0x0F) << 4) | (abs_y >> 8));
    coord_dest[coord_pos++] = (uint8_t)(abs_y & 0xFF);
  } else {
    *flag_dest = (uint8_t)(on_curve_bit + 124 + xy_sign_bits);
    coord_dest[coord_pos++] = (uint8_t)(abs_x >> 8);
    coord_dest[coord_pos++] = (uint8_t)(abs_x & 0xFF);
    coord_dest[coord_pos++] = (uint8_t)(abs_y >> 8);
    coord_dest[coord_pos++] = (uint8_t)(abs_y & 0xFF);
  }

  return coord_pos;
}

static size_t build_triangle_glyph_stream(uint8_t *dest, int push_count,
                                          int code_size) {
  uint8_t flags[3];
  uint8_t coord_bytes[8];
  size_t pos = 0;
  size_t coord_pos = 0;

  pos += write_u16be_local(dest + pos, 1);
  pos += write_255_ushort_reference(dest + pos, 2);

  coord_pos = write_triplet_reference(&flags[0], coord_bytes, coord_pos, 1, 0, 10);
  coord_pos = write_triplet_reference(&flags[1], coord_bytes, coord_pos, 1, 10, 0);
  coord_pos = write_triplet_reference(&flags[2], coord_bytes, coord_pos, 1, 0, -10);

  memcpy(dest + pos, flags, sizeof(flags));
  pos += sizeof(flags);
  memcpy(dest + pos, coord_bytes, coord_pos);
  pos += coord_pos;
  pos += write_255_ushort_reference(dest + pos, (uint16_t)push_count);
  pos += write_255_ushort_reference(dest + pos, (uint16_t)code_size);

  return pos;
}

static size_t build_simple_glyph_stream_from_deltas(uint8_t *dest,
                                                    const int *dx_values,
                                                    const int *dy_values,
                                                    size_t point_count,
                                                    int push_count,
                                                    int code_size) {
  uint8_t flags[16];
  uint8_t coord_bytes[64];
  size_t pos = 0;
  size_t coord_pos = 0;

  pos += write_u16be_local(dest + pos, 1);
  pos += write_255_ushort_reference(dest + pos, (uint16_t)(point_count - 1));

  for (size_t i = 0; i < point_count; i++) {
    coord_pos = write_triplet_reference(&flags[i], coord_bytes, coord_pos, 1,
                                        dx_values[i], dy_values[i]);
  }

  memcpy(dest + pos, flags, point_count);
  pos += point_count;
  memcpy(dest + pos, coord_bytes, coord_pos);
  pos += coord_pos;
  pos += write_255_ushort_reference(dest + pos, (uint16_t)push_count);
  pos += write_255_ushort_reference(dest + pos, (uint16_t)code_size);

  return pos;
}

static void test_glyf_encode_255_ushort_examples(void) {
  const uint16_t values[] = { 0, 252, 253, 505, 506, 761, 762, 65535 };

  for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
    uint8_t actual[3];
    uint8_t expected[3];
    size_t actual_size = 0;
    size_t expected_size = write_255_ushort_reference(expected, values[i]);

    ASSERT_OK(glyf_encode_255_ushort(values[i], actual, sizeof(actual),
                                     &actual_size));
    ASSERT_BYTES_EQ(actual, actual_size, expected, expected_size);
  }
}

static void test_glyf_encode_255_short_examples(void) {
  const int16_t values[] = { 0, 249, 250, 499, 500, 749, 750,
                             -1, -250, -500, -749, -750 };

  for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
    uint8_t actual[3];
    uint8_t expected[3];
    size_t actual_size = 0;
    size_t expected_size = write_255_short_reference(expected, values[i]);

    ASSERT_OK(glyf_encode_255_short(values[i], actual, sizeof(actual),
                                    &actual_size));
    ASSERT_BYTES_EQ(actual, actual_size, expected, expected_size);
  }
}

static void test_glyf_encode_triplet_examples(void) {
  struct triplet_case {
    int on_curve;
    int dx;
    int dy;
  };
  const struct triplet_case cases[] = {
    { 1, 0, 10 },
    { 1, 10, 0 },
    { 0, 5, -5 },
    { 1, 300, 500 },
    { 1, 1000, 2048 },
    { 0, -5000, 6000 }
  };

  for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
    uint8_t actual_flag = 0;
    uint8_t expected_flag = 0;
    uint8_t actual_bytes[4];
    uint8_t expected_bytes[4];
    size_t actual_size = 0;
    size_t expected_size = write_triplet_reference(&expected_flag, expected_bytes,
                                                   0, cases[i].on_curve,
                                                   cases[i].dx, cases[i].dy);

    ASSERT_OK(glyf_encode_triplet(cases[i].on_curve, cases[i].dx, cases[i].dy,
                                  &actual_flag, actual_bytes, sizeof(actual_bytes),
                                  &actual_size));
    ASSERT_EQ(actual_flag, expected_flag);
    ASSERT_BYTES_EQ(actual_bytes, actual_size, expected_bytes, expected_size);
  }
}

static void test_glyf_split_push_code_examples(void) {
  uint8_t instructions[] = { 0xB1, 11, 22, 0xB0, 11, 0xB0, 33, 0xB0, 11, 0x2B };
  uint8_t expected_push[] = { 11, 22, 251, 33 };
  uint8_t expected_code[] = { 0x2B };
  uint8_t *push_stream = NULL;
  uint8_t *code_stream = NULL;
  size_t push_size = 0;
  size_t code_size = 0;
  int push_count = 0;

  ASSERT_OK(glyf_split_push_code((buffer_view_t){ instructions, sizeof(instructions) },
                                 &push_count, &push_stream, &push_size,
                                 &code_stream, &code_size));

  ASSERT_EQ(push_count, 5);
  ASSERT_BYTES_EQ(push_stream, push_size, expected_push, sizeof(expected_push));
  ASSERT_BYTES_EQ(code_stream, code_size, expected_code, sizeof(expected_code));

  free(push_stream);
  free(code_stream);
}

static void test_glyf_split_push_code_truncated_leading_push_is_corrupt(void) {
  uint8_t instructions[] = { 0xB1, 11 };
  uint8_t *push_stream = NULL;
  uint8_t *code_stream = NULL;
  size_t push_size = 0;
  size_t code_size = 0;
  int push_count = 0;
  eot_status_t status = glyf_split_push_code(
    (buffer_view_t){ instructions, sizeof(instructions) },
    &push_count, &push_stream, &push_size, &code_stream, &code_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
  free(push_stream);
  free(code_stream);
}

static void test_glyf_decode_empty_glyphs(void) {
  uint8_t glyf_stream[] = {
    0x00, 0x00,
    0x00, 0x00
  };
  buffer_view_t glyf_view = { glyf_stream, sizeof(glyf_stream) };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 2,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 0);
  ASSERT_EQ(loca_size, 6);
  ASSERT_EQ(read_u16be_local(loca_out + 0), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 4), 0);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_simple_glyph(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 20);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(loca_out + 0), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 10);

  ASSERT_EQ(read_u16be_local(glyf_out + 0), 1);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 2), 0);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 4), 0);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 6), 10);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 8), 10);
  ASSERT_EQ(read_u16be_local(glyf_out + 10), 2);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 0);
  ASSERT_EQ(glyf_out[14], 0x35);
  ASSERT_EQ(glyf_out[15], 0x33);
  ASSERT_EQ(glyf_out[16], 0x15);
  ASSERT_EQ(glyf_out[17], 10);
  ASSERT_EQ(glyf_out[18], 10);
  ASSERT_EQ(glyf_out[19], 10);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_long_loca_pads_glyphs_to_four_bytes(void) {
  uint8_t glyf_stream[32];
  const int dx_values[] = { 0, 10 };
  const int dy_values[] = { 10, 0 };
  size_t glyf_stream_size =
      build_simple_glyph_stream_from_deltas(glyf_stream, dx_values, dy_values,
                                            2, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode_with_loca_format(glyf_view, push_view, code_view, 1, 1,
                                         &glyf_out, &glyf_size,
                                         &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 20);
  ASSERT_EQ(loca_size, 8);
  ASSERT_EQ((int)read_u32be(loca_out + 0), 0);
  ASSERT_EQ((int)read_u32be(loca_out + 4), 20);
  ASSERT_EQ(glyf_out[18], 0);
  ASSERT_EQ(glyf_out[19], 0);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_rebuilds_split_push_instructions(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 5, 1);
  uint8_t push_stream[] = { 11, 22, 251, 33 };
  uint8_t code_stream[] = { 0x2B };
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { push_stream, sizeof(push_stream) };
  buffer_view_t code_view = { code_stream, sizeof(code_stream) };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 28);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 14);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 7);
  ASSERT_EQ(glyf_out[14], 0xB4);
  ASSERT_EQ(glyf_out[15], 11);
  ASSERT_EQ(glyf_out[16], 22);
  ASSERT_EQ(glyf_out[17], 11);
  ASSERT_EQ(glyf_out[18], 33);
  ASSERT_EQ(glyf_out[19], 11);
  ASSERT_EQ(glyf_out[20], 0x2B);
  ASSERT_EQ(glyf_out[27], 0x00);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_compresses_repeated_point_flags(void) {
  uint8_t glyf_stream[32];
  const int dx_values[] = { 0, 0, 0, 0 };
  const int dy_values[] = { 1, 1, 1, 1 };
  size_t glyf_stream_size =
      build_simple_glyph_stream_from_deltas(glyf_stream, dx_values, dy_values,
                                            4, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 20);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 10);
  ASSERT_EQ(glyf_out[14], 0x3D);
  ASSERT_EQ(glyf_out[15], 0x03);
  ASSERT_EQ(glyf_out[16], 0x01);
  ASSERT_EQ(glyf_out[17], 0x01);
  ASSERT_EQ(glyf_out[18], 0x01);
  ASSERT_EQ(glyf_out[19], 0x01);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_keeps_255_deltas_as_word_coordinates(void) {
  uint8_t glyf_stream[32];
  const int dx_values[] = { 255 };
  const int dy_values[] = { 255 };
  size_t glyf_stream_size =
      build_simple_glyph_stream_from_deltas(glyf_stream, dx_values, dy_values,
                                            1, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 20);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 10);
  ASSERT_EQ(glyf_out[14], 0x01);
  ASSERT_EQ(glyf_out[15], 0x00);
  ASSERT_EQ(glyf_out[16], 0xFF);
  ASSERT_EQ(glyf_out[17], 0x00);
  ASSERT_EQ(glyf_out[18], 0xFF);
  ASSERT_EQ(glyf_out[19], 0x00);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_keeps_negative_255_deltas_as_short_coordinates(void) {
  uint8_t glyf_stream[32];
  const int dx_values[] = { -255 };
  const int dy_values[] = { -255 };
  size_t glyf_stream_size =
      build_simple_glyph_stream_from_deltas(glyf_stream, dx_values, dy_values,
                                            1, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 18);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 0);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 9);
  ASSERT_EQ(glyf_out[14], 0x07);
  ASSERT_EQ(glyf_out[15], 0xFF);
  ASSERT_EQ(glyf_out[16], 0xFF);
  ASSERT_EQ(glyf_out[17], 0x00);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_composite_glyph(void) {
  uint8_t glyf_stream[32];
  size_t pos = 0;
  uint8_t push_stream[8];
  size_t push_size = 0;
  uint8_t code_stream[] = { 0x2B };
  buffer_view_t glyf_view;
  buffer_view_t push_view;
  buffer_view_t code_view = { code_stream, sizeof(code_stream) };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;

  pos += write_u16be_local(glyf_stream + pos, 0xFFFF);
  pos += write_u16be_local(glyf_stream + pos, 0xFFFF);
  pos += write_u16be_local(glyf_stream + pos, 0xFFFE);
  pos += write_u16be_local(glyf_stream + pos, 10);
  pos += write_u16be_local(glyf_stream + pos, 20);
  pos += write_u16be_local(glyf_stream + pos, 0x0101);
  pos += write_u16be_local(glyf_stream + pos, 5);
  pos += write_u16be_local(glyf_stream + pos, 1);
  pos += write_u16be_local(glyf_stream + pos, 0xFFFF);
  pos += write_255_ushort_reference(glyf_stream + pos, 2);
  pos += write_255_ushort_reference(glyf_stream + pos, 1);

  push_size += write_255_short_reference(push_stream + push_size, 7);
  push_size += write_255_short_reference(push_stream + push_size, 8);

  glyf_view.data = glyf_stream;
  glyf_view.length = pos;
  push_view.data = push_stream;
  push_view.length = push_size;

  ASSERT_OK(glyf_decode(glyf_view, push_view, code_view, 1,
                        &glyf_out, &glyf_size, &loca_out, &loca_size));

  ASSERT_EQ(glyf_size, 24);
  ASSERT_EQ(loca_size, 4);
  ASSERT_EQ(read_u16be_local(loca_out + 2), 12);
  ASSERT_EQ(read_u16be_local(glyf_out + 0), 0xFFFF);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 2), -1);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 4), -2);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 6), 10);
  ASSERT_EQ((int16_t)read_u16be_local(glyf_out + 8), 20);
  ASSERT_EQ(read_u16be_local(glyf_out + 10), 0x0101);
  ASSERT_EQ(read_u16be_local(glyf_out + 12), 5);
  ASSERT_EQ(read_u16be_local(glyf_out + 14), 1);
  ASSERT_EQ(read_u16be_local(glyf_out + 16), 0xFFFF);
  ASSERT_EQ(read_u16be_local(glyf_out + 18), 4);
  ASSERT_EQ(glyf_out[20], 0xB1);
  ASSERT_EQ(glyf_out[21], 7);
  ASSERT_EQ(glyf_out[22], 8);
  ASSERT_EQ(glyf_out[23], 0x2B);

  free(glyf_out);
  free(loca_out);
}

static void test_glyf_decode_corrupt_truncated_triplet_data(void) {
  uint8_t glyf_stream[] = {
    0x00, 0x01,
    0x02,
    0x01, 0x0B, 0x00,
    0x0A, 0x0A,
    0x00,
    0x00
  };
  buffer_view_t glyf_view = { glyf_stream, sizeof(glyf_stream) };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_corrupt_truncated_push_stream(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 5, 0);
  uint8_t push_stream[] = { 11, 22 };
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { push_stream, sizeof(push_stream) };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_corrupt_truncated_code_stream(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 0, 1);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_corrupt_coordinate_overflow(void) {
  const int dx_values[] = { 32767, 1 };
  const int dy_values[] = { 0, 0 };
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_simple_glyph_stream_from_deltas(
    glyf_stream, dx_values, dy_values, 2, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_corrupt_point_delta_overflow(void) {
  const int dx_values[] = { 32767, -65535 };
  const int dy_values[] = { 0, 0 };
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_simple_glyph_stream_from_deltas(
    glyf_stream, dx_values, dy_values, 2, 0, 0);
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_rejects_trailing_glyf_stream_bytes(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 0, 0);
  glyf_stream[glyf_stream_size++] = 0xAA;
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_rejects_trailing_push_stream_bytes(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 0, 0);
  uint8_t push_stream[] = { 0xAA };
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { push_stream, sizeof(push_stream) };
  buffer_view_t code_view = { NULL, 0 };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_glyf_decode_rejects_trailing_code_stream_bytes(void) {
  uint8_t glyf_stream[32];
  size_t glyf_stream_size = build_triangle_glyph_stream(glyf_stream, 0, 0);
  uint8_t code_stream[] = { 0x2B };
  buffer_view_t glyf_view = { glyf_stream, glyf_stream_size };
  buffer_view_t push_view = { NULL, 0 };
  buffer_view_t code_view = { code_stream, sizeof(code_stream) };
  uint8_t *glyf_out = NULL;
  size_t glyf_size = 0;
  uint8_t *loca_out = NULL;
  size_t loca_size = 0;
  eot_status_t status = glyf_decode(glyf_view, push_view, code_view, 1,
                                    &glyf_out, &glyf_size, &loca_out, &loca_size);

  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

void register_glyf_codec_tests(void) {
  test_register("test_glyf_encode_255_ushort_examples",
                test_glyf_encode_255_ushort_examples);
  test_register("test_glyf_encode_255_short_examples",
                test_glyf_encode_255_short_examples);
  test_register("test_glyf_encode_triplet_examples",
                test_glyf_encode_triplet_examples);
  test_register("test_glyf_split_push_code_examples",
                test_glyf_split_push_code_examples);
  test_register("test_glyf_split_push_code_truncated_leading_push_is_corrupt",
                test_glyf_split_push_code_truncated_leading_push_is_corrupt);
  test_register("test_glyf_decode_empty_glyphs", test_glyf_decode_empty_glyphs);
  test_register("test_glyf_decode_simple_glyph", test_glyf_decode_simple_glyph);
  test_register("test_glyf_decode_long_loca_pads_glyphs_to_four_bytes",
                test_glyf_decode_long_loca_pads_glyphs_to_four_bytes);
  test_register("test_glyf_decode_rebuilds_split_push_instructions",
                test_glyf_decode_rebuilds_split_push_instructions);
  test_register("test_glyf_decode_compresses_repeated_point_flags",
                test_glyf_decode_compresses_repeated_point_flags);
  test_register("test_glyf_decode_keeps_255_deltas_as_word_coordinates",
                test_glyf_decode_keeps_255_deltas_as_word_coordinates);
  test_register("test_glyf_decode_keeps_negative_255_deltas_as_short_coordinates",
                test_glyf_decode_keeps_negative_255_deltas_as_short_coordinates);
  test_register("test_glyf_decode_composite_glyph",
                test_glyf_decode_composite_glyph);
  test_register("test_glyf_decode_corrupt_truncated_triplet_data",
                test_glyf_decode_corrupt_truncated_triplet_data);
  test_register("test_glyf_decode_corrupt_truncated_push_stream",
                test_glyf_decode_corrupt_truncated_push_stream);
  test_register("test_glyf_decode_corrupt_truncated_code_stream",
                test_glyf_decode_corrupt_truncated_code_stream);
  test_register("test_glyf_decode_corrupt_coordinate_overflow",
                test_glyf_decode_corrupt_coordinate_overflow);
  test_register("test_glyf_decode_corrupt_point_delta_overflow",
                test_glyf_decode_corrupt_point_delta_overflow);
  test_register("test_glyf_decode_rejects_trailing_glyf_stream_bytes",
                test_glyf_decode_rejects_trailing_glyf_stream_bytes);
  test_register("test_glyf_decode_rejects_trailing_push_stream_bytes",
                test_glyf_decode_rejects_trailing_push_stream_bytes);
  test_register("test_glyf_decode_rejects_trailing_code_stream_bytes",
                test_glyf_decode_rejects_trailing_code_stream_bytes);
}
