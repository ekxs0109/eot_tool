#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/byte_io.h"
#include "../src/lzcomp.h"

void test_register(const char *name, void (*fn)(void));

static void assert_decompressed_equals(buffer_view_t compressed,
                                       const uint8_t *expected,
                                       size_t expected_size) {
  uint8_t *out_data = NULL;
  size_t out_size = 0;

  eot_status_t status = lzcomp_decompress(compressed, &out_data, &out_size);
  assert(status == EOT_OK);
  assert(out_size == expected_size);
  assert(memcmp(out_data, expected, expected_size) == 0);

  free(out_data);
}

static void assert_roundtrip_equals(const uint8_t *input, size_t input_size) {
  uint8_t *compressed = NULL;
  size_t compressed_size = 0;
  uint8_t *decompressed = NULL;
  size_t decompressed_size = 0;

  eot_status_t status = lzcomp_compress(input, input_size, &compressed,
                                        &compressed_size);
  assert(status == EOT_OK);
  assert(compressed != NULL);
  assert(compressed_size > 0);

  status = lzcomp_decompress(buffer_view_make(compressed, compressed_size),
                             &decompressed, &decompressed_size);
  assert(status == EOT_OK);
  assert(decompressed_size == input_size);
  if (input_size > 0) {
    assert(memcmp(decompressed, input, input_size) == 0);
  } else {
    assert(decompressed == NULL);
  }

  free(compressed);
  free(decompressed);
}

static void test_lzcomp_rejects_truncated_stream(void) {
  // Test 1: Empty stream
  uint8_t empty_data[] = {};
  buffer_view_t empty_view = buffer_view_make(empty_data, 0);
  uint8_t *out_data = NULL;
  size_t out_size = 0;

  eot_status_t status = lzcomp_decompress(empty_view, &out_data, &out_size);
  assert(status != EOT_OK);

  // Test 2: Stream with only 1 byte (needs at least 2 for header)
  uint8_t one_byte[] = {0x01};
  buffer_view_t one_byte_view = buffer_view_make(one_byte, 1);

  status = lzcomp_decompress(one_byte_view, &out_data, &out_size);
  assert(status != EOT_OK);

  // Test 3: Stream with header but no data
  uint8_t header_only[] = {0x01, 0x00};
  buffer_view_t header_only_view = buffer_view_make(header_only, 2);

  status = lzcomp_decompress(header_only_view, &out_data, &out_size);
  assert(status != EOT_OK);

  // Test 4: Stream with incomplete literal run
  uint8_t incomplete_literal[] = {0x01, 0x00, 0x05};  // Says 5 literals but provides none
  buffer_view_t incomplete_literal_view = buffer_view_make(incomplete_literal, 3);

  status = lzcomp_decompress(incomplete_literal_view, &out_data, &out_size);
  assert(status != EOT_OK);
}

static void test_lzcomp_rejects_invalid_copy_distance(void) {
  // Test: Copy command with distance beyond output buffer
  // Format: [header] [literal count=1] [literal byte] [copy count=5] [copy distance=10]
  // This tries to copy from position -9 when we only have 1 byte in output
  uint8_t invalid_copy[] = {
    0x01, 0x00,  // Header
    0x01,        // 1 literal
    0x41,        // 'A'
    0x05,        // Copy 5 bytes
    0x0A         // From distance 10 (invalid - we only have 1 byte)
  };
  buffer_view_t invalid_copy_view = buffer_view_make(invalid_copy, sizeof(invalid_copy));
  uint8_t *out_data = NULL;
  size_t out_size = 0;

  eot_status_t status = lzcomp_decompress(invalid_copy_view, &out_data, &out_size);
  assert(status != EOT_OK);
}

static void test_lzcomp_rejects_invalid_length(void) {
  // Test: Stream with excessively large decompressed_length (> 100MB)
  uint8_t huge_length[] = {
    0xFF, 0xFF, 0xFF, 0xFF  // Very large length
  };
  buffer_view_t huge_length_view = buffer_view_make(huge_length, sizeof(huge_length));
  uint8_t *out_data = NULL;
  size_t out_size = 0;

  eot_status_t status = lzcomp_decompress(huge_length_view, &out_data, &out_size);
  assert(status != EOT_OK);
}

static void test_lzcomp_decodes_java_reference_literal_stream(void) {
  static const uint8_t compressed[] = {
    0x00, 0x00, 0x05, 0x04, 0xC2, 0x82, 0x31, 0x20,
    0x4C, 0x28, 0x23, 0x12, 0x04, 0xC2, 0x80
  };
  static const uint8_t expected[] = {
    0x00, 0x01, 0x02, 0x03, 0x04,
    0x05, 0x06, 0x07, 0x08, 0x09
  };

  assert_decompressed_equals(buffer_view_make(compressed, sizeof(compressed)),
                             expected, sizeof(expected));
}

static void test_lzcomp_decodes_java_reference_copy_stream(void) {
  static const uint8_t compressed[] = {
    0x00, 0x00, 0x08, 0x2A, 0x2A, 0x89, 0x80, 0xA8, 0x0C, 0x20
  };
  static const uint8_t expected[] = {
    'A', 'B', 'A', 'B', 'A', 'B', 'A', 'B',
    'A', 'B', 'A', 'B', 'A', 'B', 'A', 'B'
  };

  assert_decompressed_equals(buffer_view_make(compressed, sizeof(compressed)),
                             expected, sizeof(expected));
}

static void test_lzcomp_decodes_java_reference_word_copy_stream(void) {
  static const uint8_t compressed[] = {
    0x00, 0x00, 0x0D, 0xB5, 0x3E, 0x40, 0xBD, 0x3B,
    0x8A, 0x18, 0x60, 0xC3, 0x26, 0x20, 0x80
  };
  static const uint8_t expected[] = {
    'W', 'i', 'n', 'g', 'd', 'i', 'n', 'g', 's',
    'W', 'i', 'n', 'g', 'd', 'i', 'n', 'g', 's',
    'W', 'i', 'n', 'g', 'd', 'i', 'n', 'g', 's'
  };

  assert_decompressed_equals(buffer_view_make(compressed, sizeof(compressed)),
                             expected, sizeof(expected));
}

static void test_lzcomp_roundtrips_literal_data(void) {
  static const uint8_t input[] = {
    0x00, 0x01, 0x02, 0x03, 0x10, 0x11, 0x12, 0x13,
    0x20, 0x21, 0x22, 0x23, 0x30, 0x31, 0x32, 0x33
  };

  assert_roundtrip_equals(input, sizeof(input));
}

static void test_lzcomp_roundtrips_repeated_data(void) {
  static const uint8_t input[] = "WingdingsWingdingsWingdingsWingdings";

  assert_roundtrip_equals(input, sizeof(input) - 1u);
}

static void test_lzcomp_roundtrips_empty_data(void) {
  static const uint8_t input[] = "";

  assert_roundtrip_equals(input, 0);
}

void register_lzcomp_tests(void) {
  test_register("lzcomp_rejects_truncated_stream", test_lzcomp_rejects_truncated_stream);
  test_register("lzcomp_rejects_invalid_copy_distance", test_lzcomp_rejects_invalid_copy_distance);
  test_register("lzcomp_rejects_invalid_length", test_lzcomp_rejects_invalid_length);
  test_register("lzcomp_decodes_java_reference_literal_stream",
                test_lzcomp_decodes_java_reference_literal_stream);
  test_register("lzcomp_decodes_java_reference_copy_stream",
                test_lzcomp_decodes_java_reference_copy_stream);
  test_register("lzcomp_decodes_java_reference_word_copy_stream",
                test_lzcomp_decodes_java_reference_word_copy_stream);
  test_register("lzcomp_roundtrips_literal_data",
                test_lzcomp_roundtrips_literal_data);
  test_register("lzcomp_roundtrips_repeated_data",
                test_lzcomp_roundtrips_repeated_data);
  test_register("lzcomp_roundtrips_empty_data",
                test_lzcomp_roundtrips_empty_data);
}
