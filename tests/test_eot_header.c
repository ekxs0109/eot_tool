#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "eot_header.h"
#include "file_io.h"

void register_cli_tests_original(void);
void test_register(const char *name, void (*fn)(void));
void test_capture_command(int argc, char *argv[], int expected_status,
                          const char *expected_fragment);

#define ASSERT_OK(expr) assert_status_ok((expr), #expr, __FILE__, __LINE__)
#define ASSERT_STATUS(expected, expr) \
  assert_status_eq((expected), (expr), #expr, __FILE__, __LINE__)
#define ASSERT_EQ_U32(expected, actual) \
  assert_eq_u32((expected), (actual), #actual, __FILE__, __LINE__)
#define ASSERT_EQ_U16(expected, actual) \
  assert_eq_u16((expected), (actual), #actual, __FILE__, __LINE__)
#define ASSERT_TRUE(expr) assert_true((expr), #expr, __FILE__, __LINE__)
#define ASSERT_EOT_ASCII(expected, actual) \
  assert_eot_ascii((expected), &(actual), #actual, __FILE__, __LINE__)
#define ASSERT_EQ_BYTES(expected, actual, length) \
  assert_eq_bytes((expected), (actual), (length), #actual, __FILE__, __LINE__)

static void assert_status_ok(eot_status_t status, const char *expr,
                             const char *file, int line) {
  if (status != 0) {
    fprintf(stderr, "%s:%d expected OK from %s, got %d\n", file, line, expr,
            status);
    exit(1);
  }
}

static void assert_status_eq(eot_status_t expected, eot_status_t actual,
                             const char *expr, const char *file, int line) {
  if (expected != actual) {
    fprintf(stderr, "%s:%d expected %s == %d, got %d\n", file, line, expr,
            expected, actual);
    exit(1);
  }
}

static void assert_eq_u32(uint32_t expected, uint32_t actual, const char *expr,
                          const char *file, int line) {
  if (expected != actual) {
    fprintf(stderr, "%s:%d expected %s == 0x%08x, got 0x%08x\n", file, line,
            expr, expected, actual);
    exit(1);
  }
}

static void assert_eq_u16(uint16_t expected, uint16_t actual, const char *expr,
                          const char *file, int line) {
  if (expected != actual) {
    fprintf(stderr, "%s:%d expected %s == 0x%04x, got 0x%04x\n", file, line,
            expr, expected, actual);
    exit(1);
  }
}

static void assert_true(int condition, const char *expr, const char *file,
                        int line) {
  if (!condition) {
    fprintf(stderr, "%s:%d expected true: %s\n", file, line, expr);
    exit(1);
  }
}

static void assert_eot_ascii(const char *expected, const eot_string_t *actual,
                             const char *expr, const char *file, int line) {
  size_t expected_length = strlen(expected);
  size_t i;

  if (expected_length == 0u) {
    if (actual->size != 0u) {
      fprintf(stderr, "%s:%d expected %s to be empty, got %u bytes\n", file,
              line, expr, actual->size);
      exit(1);
    }
    return;
  }

  if (actual->size != (uint16_t)(expected_length * 2u)) {
    fprintf(stderr, "%s:%d expected %s size %zu, got %u\n", file, line, expr,
            expected_length * 2u, actual->size);
    exit(1);
  }

  for (i = 0; i < expected_length; i++) {
    if (actual->data[i * 2u] != (uint8_t)expected[i] ||
        actual->data[i * 2u + 1u] != 0u) {
      fprintf(stderr, "%s:%d expected %s[%zu] to encode '%c'\n", file, line,
              expr, i, expected[i]);
      exit(1);
    }
  }

}

static void assert_eq_bytes(const uint8_t *expected, const uint8_t *actual,
                            size_t length, const char *expr, const char *file,
                            int line) {
  if (length > 0u && memcmp(expected, actual, length) != 0) {
    fprintf(stderr, "%s:%d expected %s bytes to match\n", file, line, expr);
    exit(1);
  }
}

static void write_utf16le_ascii(uint8_t *dst, const char *text) {
  size_t i;

  for (i = 0; text[i] != '\0'; i++) {
    dst[i * 2u] = (uint8_t)text[i];
    dst[i * 2u + 1u] = 0u;
  }
}

static size_t append_length_prefixed_ascii(uint8_t *dst, size_t offset,
                                           const char *text) {
  uint16_t length = (uint16_t)(strlen(text) * 2u);

  write_u16le(dst + offset, length);
  offset += 2u;
  write_utf16le_ascii(dst + offset, text);
  return offset + length;
}

static size_t build_synthetic_v20002_header(uint8_t *dst,
                                            const uint8_t *signature,
                                            uint16_t signature_size,
                                            const uint8_t *eudc_font_data,
                                            uint32_t eudc_font_size,
                                            uint32_t payload_size) {
  size_t offset = 0u;

  memset(dst, 0, 512u);
  offset = 82u;

  offset = append_length_prefixed_ascii(dst, offset, "Family");
  write_u16le(dst + offset, 0u);
  offset += 2u;
  offset = append_length_prefixed_ascii(dst, offset, "Style");
  write_u16le(dst + offset, 0u);
  offset += 2u;
  offset = append_length_prefixed_ascii(dst, offset, "Version");
  write_u16le(dst + offset, 0u);
  offset += 2u;
  offset = append_length_prefixed_ascii(dst, offset, "Full");
  write_u16le(dst + offset, 0u);
  offset += 2u;
  write_u16le(dst + offset, 0u);
  offset += 2u;

  write_u32le(dst + offset, 0x11223344u);
  offset += 4u;
  write_u32le(dst + offset, 0x55667788u);
  offset += 4u;
  write_u16le(dst + offset, 0u);
  offset += 2u;
  write_u16le(dst + offset, signature_size);
  offset += 2u;
  if (signature_size > 0u) {
    memcpy(dst + offset, signature, signature_size);
    offset += signature_size;
  }
  write_u32le(dst + offset, 0x99aabbccu);
  offset += 4u;
  write_u32le(dst + offset, eudc_font_size);
  offset += 4u;
  if (eudc_font_size > 0u) {
    memcpy(dst + offset, eudc_font_data, eudc_font_size);
    offset += eudc_font_size;
  }
  if (payload_size > 0u) {
    memset(dst + offset, 0x5au, payload_size);
  }

  write_u32le(dst + 0u, (uint32_t)(offset + payload_size));
  write_u32le(dst + 4u, payload_size);
  write_u32le(dst + 8u, 0x00020002u);
  write_u32le(dst + 12u, 0x4u);
  write_u32le(dst + 28u, 400u);
  write_u16le(dst + 32u, 0u);
  write_u16le(dst + 34u, 0x504cu);
  return offset;
}

static void write_temp_file(const char *path, const uint8_t *data,
                            size_t length) {
  FILE *stream = fopen(path, "wb");

  if (stream == NULL) {
    fprintf(stderr, "failed to open temp file: %s\n", path);
    exit(1);
  }

  if (fwrite(data, 1u, length, stream) != length) {
    fclose(stream);
    fprintf(stderr, "failed to write temp file: %s\n", path);
    exit(1);
  }

  fclose(stream);
}

static void test_parse_wingdings3_header(void) {
  eot_header_t header;

  ASSERT_OK(eot_header_read_file("testdata/wingdings3.eot", &header));
  ASSERT_EQ_U32(0x00020002u, header.version);
  ASSERT_EQ_U16(0x504cu, header.magic_number);
  ASSERT_TRUE((header.flags & 0x4u) != 0);
  ASSERT_EOT_ASCII("Wingdings 3", header.family_name);
  ASSERT_EOT_ASCII("Regular", header.style_name);
  ASSERT_EOT_ASCII("Version 5.03", header.version_name);
  ASSERT_EOT_ASCII("Wingdings 3", header.full_name);
  ASSERT_EOT_ASCII("", header.root_string);
  ASSERT_EQ_U16(0x0000u, header.padding2);
  ASSERT_EQ_U16(0x0000u, header.padding3);
  ASSERT_EQ_U16(0x0000u, header.padding4);
  ASSERT_EQ_U16(0x0000u, header.padding5);
  ASSERT_EQ_U32(0x50475342u, header.root_string_checksum);
  ASSERT_EQ_U32(0x00002710u, header.eudc_code_page);
  ASSERT_EQ_U16(0x0000u, header.padding6);
  ASSERT_EQ_U16(0x0000u, header.signature_size);
  ASSERT_EQ_U32(0x00000000u, header.eudc_flags);
  ASSERT_EQ_U32(0x00000000u, header.eudc_font_size);
  ASSERT_EQ_U32(202u, header.header_length);
  ASSERT_EQ_U32(header.eot_size - header.font_data_size, header.header_length);

  eot_header_destroy(&header);
}

static void test_rejects_truncated_header(void) {
  uint8_t bytes[81];

  memset(bytes, 0, sizeof(bytes));
  ASSERT_STATUS(EOT_ERR_TRUNCATED,
                eot_header_parse(buffer_view_make(bytes, sizeof(bytes)),
                                 &(eot_header_t){0}));
}

static void test_rejects_invalid_magic(void) {
  uint8_t bytes[82];

  memset(bytes, 0, sizeof(bytes));
  write_u16le(bytes + 34, 0x1234u);
  ASSERT_STATUS(EOT_ERR_INVALID_MAGIC,
                eot_header_parse(buffer_view_make(bytes, sizeof(bytes)),
                                 &(eot_header_t){0}));
}

static void test_rejects_truncated_post_root_trailer(void) {
  file_buffer_t fixture;

  ASSERT_OK(file_io_read_all("testdata/wingdings3.eot", &fixture));
  ASSERT_STATUS(EOT_ERR_TRUNCATED,
                eot_header_parse(buffer_view_make(fixture.data, 201u),
                                 &(eot_header_t){0}));
  file_io_free(&fixture);
}

static void test_rejects_eot_size_smaller_than_font_data_size(void) {
  file_buffer_t fixture;

  ASSERT_OK(file_io_read_all("testdata/wingdings3.eot", &fixture));
  write_u32le(fixture.data + 0u, 13679u);
  ASSERT_STATUS(EOT_ERR_INVALID_SIZE_METADATA,
                eot_header_parse(buffer_view_make(fixture.data, fixture.length),
                                 &(eot_header_t){0}));
  file_io_free(&fixture);
}

static void test_rejects_inconsistent_declared_header_length(void) {
  file_buffer_t fixture;

  ASSERT_OK(file_io_read_all("testdata/wingdings3.eot", &fixture));
  write_u32le(fixture.data + 4u, 13679u);
  ASSERT_STATUS(EOT_ERR_INVALID_SIZE_METADATA,
                eot_header_parse(buffer_view_make(fixture.data, fixture.length),
                                 &(eot_header_t){0}));
  file_io_free(&fixture);
}

static void test_parses_non_empty_signature_and_eudc_font_data(void) {
  static const uint8_t signature[] = {0xdeu, 0xadu, 0xbeu, 0xefu};
  static const uint8_t eudc_font_data[] = {0x11u, 0x22u, 0x33u};
  uint8_t bytes[512];
  eot_header_t header;
  size_t header_length;

  header_length = build_synthetic_v20002_header(bytes, signature,
                                                (uint16_t)sizeof(signature),
                                                eudc_font_data,
                                                (uint32_t)sizeof(eudc_font_data),
                                                5u);
  ASSERT_OK(eot_header_parse(buffer_view_make(bytes, header_length + 5u),
                             &header));
  ASSERT_EQ_U16((uint16_t)sizeof(signature), header.signature_size);
  ASSERT_EQ_BYTES(signature, header.signature, sizeof(signature));
  ASSERT_EQ_U32((uint32_t)sizeof(eudc_font_data), header.eudc_font_size);
  ASSERT_EQ_U32((uint32_t)header_length, header.header_length);
  ASSERT_EQ_U32((uint32_t)header_length + 5u, header.eot_size);
  ASSERT_EQ_U32(5u, header.font_data_size);
  ASSERT_EQ_U32(0x99aabbccu, header.eudc_flags);

  eot_header_destroy(&header);
}

static void test_rejects_truncated_signature_bytes(void) {
  static const uint8_t signature[] = {0xa1u, 0xb2u, 0xc3u, 0xd4u};
  uint8_t bytes[512];
  size_t header_length;

  header_length = build_synthetic_v20002_header(bytes, signature,
                                                (uint16_t)sizeof(signature),
                                                NULL, 0u, 0u);
  ASSERT_STATUS(EOT_ERR_TRUNCATED,
                eot_header_parse(buffer_view_make(bytes, header_length - 1u),
                                 &(eot_header_t){0}));
}

static void test_rejects_truncated_eudc_font_data(void) {
  static const uint8_t signature[] = {0x10u, 0x20u};
  static const uint8_t eudc_font_data[] = {0x31u, 0x32u, 0x33u, 0x34u};
  uint8_t bytes[512];
  size_t header_length;

  header_length = build_synthetic_v20002_header(bytes, signature,
                                                (uint16_t)sizeof(signature),
                                                eudc_font_data,
                                                (uint32_t)sizeof(eudc_font_data),
                                                0u);
  ASSERT_STATUS(EOT_ERR_TRUNCATED,
                eot_header_parse(buffer_view_make(bytes, header_length - 1u),
                                 &(eot_header_t){0}));
}

static void test_eot_header_suite(void) {
  test_parse_wingdings3_header();
  test_rejects_truncated_header();
  test_rejects_invalid_magic();
  test_rejects_truncated_post_root_trailer();
  test_rejects_eot_size_smaller_than_font_data_size();
  test_rejects_inconsistent_declared_header_length();
  test_parses_non_empty_signature_and_eudc_font_data();
  test_rejects_truncated_signature_bytes();
  test_rejects_truncated_eudc_font_data();
}

static void test_decode_parses_header_before_writing_output(void) {
  char *argv[] = {"fonttool", "decode", "testdata/wingdings3.eot",
                  "build/out/placeholder.ttf"};

  if (mkdir("build", 0777) != 0 && errno != EEXIST) {
    fprintf(stderr, "failed to create build directory\n");
    exit(1);
  }
  if (mkdir("build/out", 0777) != 0 && errno != EEXIST) {
    fprintf(stderr, "failed to create build/out directory\n");
    exit(1);
  }

  unlink("build/out/placeholder.ttf");
  test_capture_command(4, argv, 0, "Decoded testdata/wingdings3.eot");
  ASSERT_TRUE(access("build/out/placeholder.ttf", F_OK) == 0);
}

static void test_decode_rejects_inconsistent_size_metadata(void) {
  file_buffer_t fixture;
  char template_path[] = "build/bad-size-XXXXXX.eot";
  char *argv[] = {"fonttool", "decode", template_path,
                  "build/out/placeholder.ttf"};
  int fd;

  ASSERT_OK(file_io_read_all("testdata/wingdings3.eot", &fixture));
  write_u32le(fixture.data + 4u, 13679u);

  fd = mkstemps(template_path, 4);
  if (fd < 0) {
    file_io_free(&fixture);
    fprintf(stderr, "failed to create temp file\n");
    exit(1);
  }
  close(fd);

  write_temp_file(template_path, fixture.data, fixture.length);
  test_capture_command(4, argv, 1, "invalid size metadata");
  unlink(template_path);
  file_io_free(&fixture);
}

void register_cli_tests(void) {
  register_cli_tests_original();
  test_register("eot_header_", test_eot_header_suite);
  test_register("decode_parses_header_before_writing_output",
                test_decode_parses_header_before_writing_output);
  test_register("decode_rejects_inconsistent_size_metadata",
                test_decode_rejects_inconsistent_size_metadata);
  test_register("parse_wingdings3_header", test_parse_wingdings3_header);
  test_register("rejects_truncated_header", test_rejects_truncated_header);
  test_register("rejects_invalid_magic", test_rejects_invalid_magic);
  test_register("rejects_truncated_post_root_trailer",
                test_rejects_truncated_post_root_trailer);
  test_register("rejects_eot_size_smaller_than_font_data_size",
                test_rejects_eot_size_smaller_than_font_data_size);
  test_register("rejects_inconsistent_declared_header_length",
                test_rejects_inconsistent_declared_header_length);
  test_register("parses_non_empty_signature_and_eudc_font_data",
                test_parses_non_empty_signature_and_eudc_font_data);
  test_register("rejects_truncated_signature_bytes",
                test_rejects_truncated_signature_bytes);
  test_register("rejects_truncated_eudc_font_data",
                test_rejects_truncated_eudc_font_data);
}
