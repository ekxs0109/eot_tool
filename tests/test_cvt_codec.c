#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/cvt_codec.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define ASSERT_OK(expr) do { \
  eot_status_t status = (expr); \
  if (status != EOT_OK) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s returned %d", #expr, status); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s", #expr); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static uint16_t read_u16be_local(const uint8_t *data) {
  return ((uint16_t)data[0] << 8) | (uint16_t)data[1];
}

static void test_cvt_decode_simple_deltas(void) {
  // Encoded CVT with 3 entries using simple delta codes
  // Entry count: 3
  // Deltas: 100, 5, 3 (from 0, so values are 100, 105, 108)
  // Note: codes 0-237 are direct positive delta values
  uint8_t encoded[] = {
    0x00, 0x03,  // num_entries = 3
    100,         // delta = 100 (value = 0 + 100 = 100)
    5,           // delta = 5 (value = 100 + 5 = 105)
    3            // delta = 3 (value = 105 + 3 = 108)
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(cvt_decode(input, &output, &output_size));
  ASSERT_TRUE(output != NULL);
  ASSERT_EQ(output_size, 6);  // 3 entries * 2 bytes

  ASSERT_EQ(read_u16be_local(output + 0), 100);
  ASSERT_EQ(read_u16be_local(output + 2), 105);
  ASSERT_EQ(read_u16be_local(output + 4), 108);

  free(output);
}

static void test_cvt_decode_word_code(void) {
  // Test CVT_WORDCODE (238) for large deltas
  uint8_t encoded[] = {
    0x00, 0x02,  // num_entries = 2
    238,         // CVT_WORDCODE
    0x10, 0x00,  // delta = 4096
    238,         // CVT_WORDCODE
    0xFF, 0xFF   // delta = -1
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(cvt_decode(input, &output, &output_size));
  ASSERT_TRUE(output != NULL);
  ASSERT_EQ(output_size, 4);

  ASSERT_EQ(read_u16be_local(output + 0), 4096);
  ASSERT_EQ(read_u16be_local(output + 2), 4095);  // 4096 + (-1)

  free(output);
}

static void test_cvt_decode_negative_range(void) {
  // Test negative range codes (239-247)
  uint8_t encoded[] = {
    0x00, 0x02,  // num_entries = 2
    239, 10,     // NEG0: -(0 * 238 + 10) = -10
    240, 20      // NEG1: -(1 * 238 + 20) = -258
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(cvt_decode(input, &output, &output_size));
  ASSERT_TRUE(output != NULL);
  ASSERT_EQ(output_size, 4);

  // Values are stored as unsigned but represent signed
  ASSERT_EQ((int16_t)read_u16be_local(output + 0), -10);
  ASSERT_EQ((int16_t)read_u16be_local(output + 2), -268);  // -10 + (-258)

  free(output);
}

static void test_cvt_decode_positive_range(void) {
  // Test positive range codes (248-255)
  uint8_t encoded[] = {
    0x00, 0x02,  // num_entries = 2
    248, 10,     // POS1: (1 * 238 + 10) = 248
    249, 20      // POS2: (2 * 238 + 20) = 496
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(cvt_decode(input, &output, &output_size));
  ASSERT_TRUE(output != NULL);
  ASSERT_EQ(output_size, 4);

  ASSERT_EQ(read_u16be_local(output + 0), 248);
  ASSERT_EQ(read_u16be_local(output + 2), 744);  // 248 + 496

  free(output);
}

static void test_cvt_decode_empty_input(void) {
  uint8_t encoded[] = { 0x00, 0x00 };  // 0 entries
  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(cvt_decode(input, &output, &output_size));
  ASSERT_TRUE(output != NULL);
  ASSERT_EQ(output_size, 0);

  free(output);
}

static void test_cvt_decode_corrupt_too_short(void) {
  uint8_t encoded[] = { 0x00 };  // Missing second byte of count
  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = cvt_decode(input, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
  ASSERT_TRUE(output == NULL);
}

static void test_cvt_decode_corrupt_truncated_data(void) {
  // Claims 3 entries but only provides data for 1
  uint8_t encoded[] = {
    0x00, 0x03,  // num_entries = 3
    100          // Only 1 delta provided
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = cvt_decode(input, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_cvt_decode_corrupt_wordcode_truncated(void) {
  // WORDCODE without following 2 bytes
  uint8_t encoded[] = {
    0x00, 0x01,  // num_entries = 1
    238,         // CVT_WORDCODE
    0x10         // Missing second byte
  };

  buffer_view_t input = { encoded, sizeof(encoded) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = cvt_decode(input, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

void register_cvt_codec_tests(void) {
  test_register("test_cvt_decode_simple_deltas", test_cvt_decode_simple_deltas);
  test_register("test_cvt_decode_word_code", test_cvt_decode_word_code);
  test_register("test_cvt_decode_negative_range", test_cvt_decode_negative_range);
  test_register("test_cvt_decode_positive_range", test_cvt_decode_positive_range);
  test_register("test_cvt_decode_empty_input", test_cvt_decode_empty_input);
  test_register("test_cvt_decode_corrupt_too_short", test_cvt_decode_corrupt_too_short);
  test_register("test_cvt_decode_corrupt_truncated_data", test_cvt_decode_corrupt_truncated_data);
  test_register("test_cvt_decode_corrupt_wordcode_truncated", test_cvt_decode_corrupt_wordcode_truncated);
}
