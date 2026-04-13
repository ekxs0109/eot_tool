#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/hdmx_codec.h"

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

static uint32_t read_u32be_local(const uint8_t *data) {
  return ((uint32_t)data[0] << 24) | ((uint32_t)data[1] << 16) |
         ((uint32_t)data[2] << 8) | (uint32_t)data[3];
}

static void init_hhea(uint8_t *hhea, size_t length, uint16_t num_h_metrics) {
  memset(hhea, 0, length);
  if (length >= 36) {
    hhea[34] = (uint8_t)(num_h_metrics >> 8);
    hhea[35] = (uint8_t)(num_h_metrics & 0xFF);
  }
}

static void test_hdmx_decode_basic(void) {
  // Create minimal encoded HDMX data
  // Format: version(2) + numRecords(2) + recordSize(4) + [ppem(1) + maxWidth(1)]* + magnitude_data
  // Minimum encoded length must be >= 12 bytes
  uint8_t encoded[] = {
    0x00, 0x00,        // version
    0x00, 0x01,        // numRecords = 1
    0x00, 0x00, 0x00, 0x03,  // recordSize = 3 (ppem + maxWidth + 1 glyph)
    12, 100,           // ppem=12, maxWidth=100
    139,               // magnitude value: 139 - 139 = 0 surprise for glyph 0
    0                  // padding to reach 12 bytes minimum
  };

  // Create minimal hmtx table (1 glyph with advanceWidth)
  uint8_t hmtx[] = {
    0x00, 0x64, 0x00, 0x00   // glyph 0: advanceWidth=100, lsb=0
  };

  // Create minimal head table (need units_per_em at offset 18, so need at least 20 bytes)
  uint8_t head[54];
  memset(head, 0, sizeof(head));
  head[18] = 0x04;  // unitsPerEm = 1024 (0x0400)
  head[19] = 0x00;
  uint8_t hhea[36];
  init_hhea(hhea, sizeof(hhea), 1);

  // Create minimal maxp table (need numGlyphs at offset 4, so need at least 6 bytes)
  uint8_t maxp[32];
  memset(maxp, 0, sizeof(maxp));
  maxp[4] = 0x00;   // numGlyphs = 1
  maxp[5] = 0x01;

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t hmtx_view = { hmtx, sizeof(hmtx) };
  buffer_view_t hhea_view = { hhea, sizeof(hhea) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(hdmx_decode(encoded_view, hmtx_view, hhea_view, head_view, maxp_view,
                        &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Output should be: 8 byte header + 1 record * 3 bytes = 11 bytes
  ASSERT_EQ(output_size, 11);

  // Check header
  ASSERT_EQ(read_u16be_local(output), 0);      // version
  ASSERT_EQ(read_u16be_local(output + 2), 1);  // numRecords
  ASSERT_EQ(read_u32be_local(output + 4), 3);  // recordSize

  // Check record
  ASSERT_EQ(output[8], 12);   // ppem
  ASSERT_EQ(output[9], 100);  // maxWidth

  free(output);
}

static void test_hdmx_decode_with_surprises(void) {
  // Test with non-zero surprise values
  uint8_t encoded[] = {
    0x00, 0x00,        // version
    0x00, 0x01,        // numRecords = 1
    0x00, 0x00, 0x00, 0x03,  // recordSize = 3 (ppem + maxWidth + 1 glyph)
    16, 120,           // ppem=16, maxWidth=120
    144,               // surprise = 144 - 139 = 5 for glyph 0
    0                  // padding to reach 12 bytes
  };

  uint8_t hmtx[] = {
    0x00, 0x64, 0x00, 0x00   // glyph 0: advanceWidth=100
  };

  uint8_t head[54];
  memset(head, 0, sizeof(head));
  head[18] = 0x04;  // unitsPerEm = 1024
  head[19] = 0x00;
  uint8_t hhea[36];
  init_hhea(hhea, sizeof(hhea), 1);

  uint8_t maxp[32];
  memset(maxp, 0, sizeof(maxp));
  maxp[4] = 0x00;   // numGlyphs = 1
  maxp[5] = 0x01;

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t hmtx_view = { hmtx, sizeof(hmtx) };
  buffer_view_t hhea_view = { hhea, sizeof(hhea) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(hdmx_decode(encoded_view, hmtx_view, hhea_view, head_view, maxp_view,
                        &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // The width should be calculated as: rounded_tt_aw + surprise
  // rounded_tt_aw = ((64 * 16 * 100 + 512) / 1024 + 32) / 64 = (100 + 32) / 64 = 2
  // width = 2 + 5 = 7
  ASSERT_EQ(output[10], 7);

  free(output);
}

static void test_hdmx_decode_multiple_records(void) {
  // Test with 2 records
  uint8_t encoded[] = {
    0x00, 0x00,        // version
    0x00, 0x02,        // numRecords = 2
    0x00, 0x00, 0x00, 0x03,  // recordSize = 3
    12, 100,           // record 0: ppem=12, maxWidth=100
    16, 120,           // record 1: ppem=16, maxWidth=120
    139, 139           // surprises: 0, 0 for 2 records * 1 glyph
  };

  uint8_t hmtx[] = {
    0x00, 0x64, 0x00, 0x00   // glyph 0: advanceWidth=100
  };

  uint8_t head[54];
  memset(head, 0, sizeof(head));
  head[18] = 0x04;  // unitsPerEm = 1024
  head[19] = 0x00;
  uint8_t hhea[36];
  init_hhea(hhea, sizeof(hhea), 1);

  uint8_t maxp[32];
  memset(maxp, 0, sizeof(maxp));
  maxp[4] = 0x00;   // numGlyphs = 1
  maxp[5] = 0x01;

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t hmtx_view = { hmtx, sizeof(hmtx) };
  buffer_view_t hhea_view = { hhea, sizeof(hhea) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(hdmx_decode(encoded_view, hmtx_view, hhea_view, head_view, maxp_view,
                        &output, &output_size));
  ASSERT_TRUE(output != NULL);

  // Output: 8 byte header + 2 records * 3 bytes = 14 bytes
  ASSERT_EQ(output_size, 14);
  ASSERT_EQ(read_u16be_local(output + 2), 2);  // numRecords

  // Check both records
  ASSERT_EQ(output[8], 12);   // record 0 ppem
  ASSERT_EQ(output[11], 16);  // record 1 ppem

  free(output);
}

static void test_hdmx_decode_corrupt_too_short(void) {
  uint8_t encoded[] = { 0x00, 0x00, 0x00 };  // Less than 12 bytes
  uint8_t dummy[32] = {0};

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t dummy_view = { dummy, sizeof(dummy) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = hdmx_decode(encoded_view, dummy_view, dummy_view, dummy_view,
                                    dummy_view, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_hdmx_decode_corrupt_head_too_short(void) {
  uint8_t encoded[12] = {0};
  encoded[2] = 0x00;  // numRecords = 0
  encoded[3] = 0x00;

  uint8_t head[10] = {0};  // Too short, needs at least 20 bytes
  uint8_t dummy[32] = {0};

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t dummy_view = { dummy, sizeof(dummy) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = hdmx_decode(encoded_view, dummy_view, dummy_view, head_view,
                                    dummy_view, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_hdmx_decode_corrupt_maxp_too_short(void) {
  uint8_t encoded[12] = {0};
  encoded[2] = 0x00;  // numRecords = 0
  encoded[3] = 0x00;

  uint8_t head[54] = {0};
  uint8_t maxp[4] = {0};  // Too short, needs at least 6 bytes

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };
  buffer_view_t dummy_view = { head, sizeof(head) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = hdmx_decode(encoded_view, dummy_view, dummy_view, head_view,
                                    maxp_view, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_hdmx_decode_corrupt_truncated_records(void) {
  // Claims 2 records but only provides 1
  uint8_t encoded[] = {
    0x00, 0x00,        // version
    0x00, 0x02,        // numRecords = 2 (but only 1 provided)
    0x00, 0x00, 0x00, 0x03,  // recordSize = 3
    12, 100            // Only 1 record header
  };

  uint8_t head[54] = {0};
  head[18] = 0x04;
  head[19] = 0x00;
  uint8_t hhea[36];
  init_hhea(hhea, sizeof(hhea), 1);

  uint8_t maxp[32] = {0};
  maxp[4] = 0x00;
  maxp[5] = 0x01;

  uint8_t hmtx[4] = {0};

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t hmtx_view = { hmtx, sizeof(hmtx) };
  buffer_view_t hhea_view = { hhea, sizeof(hhea) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };

  uint8_t *output = NULL;
  size_t output_size = 0;

  eot_status_t status = hdmx_decode(encoded_view, hmtx_view, hhea_view, head_view,
                                    maxp_view, &output, &output_size);
  ASSERT_TRUE(status == EOT_ERR_CORRUPT_DATA);
}

static void test_hdmx_decode_with_shared_advance_widths(void) {
  uint8_t encoded[] = {
    0x00, 0x00,
    0x00, 0x01,
    0x00, 0x00, 0x00, 0x04,
    12, 7,
    140, 140
  };
  uint8_t hmtx[] = {
    0x01, 0xF4, 0x00, 0x00,
    0x00, 0x00
  };
  uint8_t hhea[36];
  uint8_t head[54];
  uint8_t maxp[32];

  init_hhea(hhea, sizeof(hhea), 1);
  memset(head, 0, sizeof(head));
  head[18] = 0x03;
  head[19] = 0xE8;
  memset(maxp, 0, sizeof(maxp));
  maxp[4] = 0x00;
  maxp[5] = 0x02;

  buffer_view_t encoded_view = { encoded, sizeof(encoded) };
  buffer_view_t hmtx_view = { hmtx, sizeof(hmtx) };
  buffer_view_t hhea_view = { hhea, sizeof(hhea) };
  buffer_view_t head_view = { head, sizeof(head) };
  buffer_view_t maxp_view = { maxp, sizeof(maxp) };
  uint8_t *output = NULL;
  size_t output_size = 0;

  ASSERT_OK(hdmx_decode(encoded_view, hmtx_view, hhea_view, head_view, maxp_view,
                        &output, &output_size));
  ASSERT_EQ(output_size, 12);
  ASSERT_EQ(output[10], 7);
  ASSERT_EQ(output[11], 7);

  free(output);
}

void register_hdmx_codec_tests(void) {
  test_register("test_hdmx_decode_basic", test_hdmx_decode_basic);
  test_register("test_hdmx_decode_with_surprises", test_hdmx_decode_with_surprises);
  test_register("test_hdmx_decode_multiple_records", test_hdmx_decode_multiple_records);
  test_register("test_hdmx_decode_with_shared_advance_widths",
                test_hdmx_decode_with_shared_advance_widths);
  test_register("test_hdmx_decode_corrupt_too_short", test_hdmx_decode_corrupt_too_short);
  test_register("test_hdmx_decode_corrupt_head_too_short", test_hdmx_decode_corrupt_head_too_short);
  test_register("test_hdmx_decode_corrupt_maxp_too_short", test_hdmx_decode_corrupt_maxp_too_short);
  test_register("test_hdmx_decode_corrupt_truncated_records", test_hdmx_decode_corrupt_truncated_records);
}
