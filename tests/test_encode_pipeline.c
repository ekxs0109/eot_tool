#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "../src/mtx_encode.h"
#include "../src/parallel_runtime.h"
#include "../src/byte_io.h"
#include "../src/eot_header.h"
#include "../src/lzcomp.h"
#include "../src/mtx_decode.h"
#include "../src/mtx_container.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_reader.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);
extern void test_capture_stderr(void (*fn)(void *context), void *context,
                                char *stderr_output, size_t stderr_output_size);

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

#define ASSERT_EQ_U32(expected, actual) do { \
  uint32_t exp = (expected); \
  uint32_t act = (actual); \
  if (exp != act) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: expected 0x%08x, got 0x%08x", exp, act); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define TAG_head 0x68656164u
#define TAG_name 0x6e616d65u
#define TAG_OS_2 0x4f532f32u
#define TAG_cmap 0x636d6170u
#define TAG_hhea 0x68686561u
#define TAG_hmtx 0x686d7478u
#define TAG_maxp 0x6d617870u
#define TAG_glyf 0x676c7966u
#define TAG_loca 0x6c6f6361u
#define TAG_cvt  0x63767420u
#define TAG_hdmx 0x68646d78u
#define TAG_VDMX 0x56444d58u
#define EOT_FLAG_PPT_XOR 0x10000000u

static int ensure_build_output_dir(void) {
  if (mkdir("build", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build directory");
    return 0;
  }
  if (mkdir("build/out", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build/out directory");
    return 0;
  }
  return 1;
}

static void write_u16be_local(uint8_t *dest, uint16_t value) {
  dest[0] = (uint8_t)(value >> 8);
  dest[1] = (uint8_t)(value & 0xFF);
}

static void write_u32be_local(uint8_t *dest, uint32_t value) {
  dest[0] = (uint8_t)(value >> 24);
  dest[1] = (uint8_t)((value >> 16) & 0xFF);
  dest[2] = (uint8_t)((value >> 8) & 0xFF);
  dest[3] = (uint8_t)(value & 0xFF);
}

static void assert_matching_table(const sfnt_font_t *source_font,
                                  const sfnt_font_t *decoded_font,
                                  uint32_t tag) {
  sfnt_table_t *source_table = sfnt_font_get_table((sfnt_font_t *)source_font, tag);
  sfnt_table_t *decoded_table = sfnt_font_get_table((sfnt_font_t *)decoded_font, tag);
  ASSERT_TRUE(source_table != NULL);
  ASSERT_TRUE(decoded_table != NULL);
  ASSERT_TRUE(source_table->length == decoded_table->length);
  ASSERT_TRUE(memcmp(source_table->data, decoded_table->data,
                     source_table->length) == 0);
}

static eot_status_t decode_eot_buffer_to_font(const byte_buffer_t *eot,
                                              const char *path,
                                              sfnt_font_t *decoded_font) {
  eot_status_t status;

  if (!ensure_build_output_dir()) {
    return EOT_ERR_IO;
  }

  status = file_io_write_all(path, eot->data, eot->length);
  if (status != EOT_OK) {
    return status;
  }

  return mtx_decode_eot_file(path, decoded_font);
}

static void init_minimal_font_with_hdmx(sfnt_font_t *font) {
  /* Synthetic tracked-in-test font: two glyphs with shared trailing advance width. */
  uint8_t head[54] = {0};
  uint8_t hhea[36] = {0};
  uint8_t os2[86] = {0};
  uint8_t maxp[6] = {0};
  uint8_t hmtx[6] = {0};
  uint8_t loca[6] = {0};
  uint8_t hdmx[12] = {0};

  sfnt_font_init(font);

  write_u16be_local(head + 18, 1000);
  write_u16be_local(head + 50, 0);
  write_u16be_local(hhea + 34, 1);
  write_u16be_local(os2 + 4, 400);
  write_u16be_local(maxp + 4, 2);
  write_u16be_local(hmtx, 500);
  write_u16be_local(hdmx + 0, 0);
  write_u16be_local(hdmx + 2, 1);
  write_u32be_local(hdmx + 4, 4);
  hdmx[8] = 12;
  hdmx[9] = 7;
  hdmx[10] = 7;
  hdmx[11] = 7;

  ASSERT_OK(sfnt_font_add_table(font, TAG_head, head, sizeof(head)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_hhea, hhea, sizeof(hhea)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_OS_2, os2, sizeof(os2)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_maxp, maxp, sizeof(maxp)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_hmtx, hmtx, sizeof(hmtx)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_glyf, NULL, 0));
  ASSERT_OK(sfnt_font_add_table(font, TAG_loca, loca, sizeof(loca)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_hdmx, hdmx, sizeof(hdmx)));
}

static void init_minimal_font_with_vdmx(sfnt_font_t *font) {
  uint8_t head[54] = {0};
  uint8_t os2[86] = {0};
  uint8_t maxp[6] = {0};
  uint8_t loca[4] = {0};
  uint8_t vdmx[4] = {0x00, 0x01, 0x00, 0x00};

  sfnt_font_init(font);

  write_u16be_local(head + 18, 1000);
  write_u16be_local(head + 50, 0);
  write_u16be_local(os2 + 4, 400);
  write_u16be_local(maxp + 4, 1);

  ASSERT_OK(sfnt_font_add_table(font, TAG_head, head, sizeof(head)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_OS_2, os2, sizeof(os2)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_maxp, maxp, sizeof(maxp)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_glyf, NULL, 0));
  ASSERT_OK(sfnt_font_add_table(font, TAG_loca, loca, sizeof(loca)));
  ASSERT_OK(sfnt_font_add_table(font, TAG_VDMX, vdmx, sizeof(vdmx)));
}

static void extract_block1_font(const byte_buffer_t *eot,
                                uint8_t **out_block1_data,
                                size_t *out_block1_size,
                                sfnt_font_t *out_font,
                                eot_header_t *out_header) {
  buffer_view_t eot_view = buffer_view_make(eot->data, eot->length);
  ASSERT_OK(eot_header_parse(eot_view, out_header));

  buffer_view_t mtx_view = buffer_view_make(
      eot->data + out_header->header_length, eot->length - out_header->header_length);
  mtx_container_t container;
  ASSERT_OK(mtx_container_parse(mtx_view, &container));

  buffer_view_t compressed1 =
      buffer_view_make(container.payload_data + 10, container.offset_block2 - 10);
  ASSERT_OK(lzcomp_decompress(compressed1, out_block1_data, out_block1_size));
  ASSERT_OK(sfnt_reader_parse(*out_block1_data, *out_block1_size, out_font));
}

static void test_encode_opensans_uses_split_glyf_blocks(void) {
  byte_buffer_t eot;
  sfnt_font_t source_font;
  ASSERT_OK(mtx_encode_ttf_file("testdata/OpenSans-Regular.ttf", &eot));
  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &source_font));
  ASSERT_EQ_U32(0x00020002u, read_u32le(eot.data + 8));
  ASSERT_TRUE((read_u32le(eot.data + 12) & 0x4u) != 0);

  // Structural check: block1 must be a standard SFNT stream after MTX+LZ decode,
  // not a concatenated raw-table blob.
  eot_header_t header;
  buffer_view_t eot_view = buffer_view_make(eot.data, eot.length);
  ASSERT_OK(eot_header_parse(eot_view, &header));

  ASSERT_TRUE(header.header_length <= eot.length);
  buffer_view_t mtx_view =
      buffer_view_make(eot.data + header.header_length, eot.length - header.header_length);

  mtx_container_t container;
  ASSERT_OK(mtx_container_parse(mtx_view, &container));
  ASSERT_EQ_U32(3u, container.num_blocks);
  ASSERT_TRUE(container.offset_block2 > 10u);
  ASSERT_TRUE(container.offset_block3 > container.offset_block2);

  buffer_view_t compressed1 =
      buffer_view_make(container.payload_data + 10, container.offset_block2 - 10);
  buffer_view_t compressed2 =
      buffer_view_make(container.payload_data + container.offset_block2,
                       container.offset_block3 - container.offset_block2);
  buffer_view_t compressed3 =
      buffer_view_make(container.payload_data + container.offset_block3,
                       container.payload_size - container.offset_block3);

  uint8_t *block1_data = NULL;
  size_t block1_size = 0;
  uint8_t *block2_data = NULL;
  size_t block2_size = 0;
  uint8_t *block3_data = NULL;
  size_t block3_size = 0;
  ASSERT_OK(lzcomp_decompress(compressed1, &block1_data, &block1_size));
  ASSERT_OK(lzcomp_decompress(compressed2, &block2_data, &block2_size));
  ASSERT_OK(lzcomp_decompress(compressed3, &block3_data, &block3_size));
  ASSERT_TRUE(block2_size > 0);
  ASSERT_TRUE(block3_size > 0);

  sfnt_font_t font;
  ASSERT_OK(sfnt_reader_parse(block1_data, block1_size, &font));

  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_head));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_name));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_OS_2));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_cmap));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_hhea));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_hmtx));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_maxp));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_glyf));
  ASSERT_TRUE(sfnt_font_has_table(&font, TAG_loca));

  sfnt_table_t *source_loca = sfnt_font_get_table(&source_font, TAG_loca);
  sfnt_table_t *source_glyf = sfnt_font_get_table(&source_font, TAG_glyf);
  sfnt_table_t *block1_loca = sfnt_font_get_table(&font, TAG_loca);
  sfnt_table_t *block1_glyf = sfnt_font_get_table(&font, TAG_glyf);
  ASSERT_TRUE(source_loca != NULL);
  ASSERT_TRUE(source_glyf != NULL);
  ASSERT_TRUE(block1_loca != NULL);
  ASSERT_TRUE(block1_glyf != NULL);
  ASSERT_TRUE(source_loca->length > 0);
  ASSERT_TRUE(source_glyf->length > 0);
  ASSERT_TRUE(block1_loca->length == 0);
  ASSERT_TRUE(block1_glyf->length != source_glyf->length);

  sfnt_font_destroy(&font);
  sfnt_font_destroy(&source_font);
  free(block1_data);
  free(block2_data);
  free(block3_data);
  eot_header_destroy(&header);
  byte_buffer_destroy(&eot);
}

static void test_encode_opensans_matches_with_single_thread_override(void) {
  byte_buffer_t serial = {0};
  byte_buffer_t parallel = {0};
  eot_status_t status = EOT_OK;
  char failure[256] = {0};

  parallel_runtime_clear_test_env();
  status = parallel_runtime_set_test_env("EOT_TOOL_THREADS", "1");
  if (status != EOT_OK) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_set_test_env(\"EOT_TOOL_THREADS\", \"1\") returned %d",
             status);
    goto cleanup;
  }
  status = mtx_encode_ttf_file("testdata/OpenSans-Regular.ttf", &serial);
  if (status != EOT_OK) {
    snprintf(failure, sizeof(failure),
             "assertion failed: mtx_encode_ttf_file(\"testdata/OpenSans-Regular.ttf\", &serial) returned %d",
             status);
    goto cleanup;
  }
  if (parallel_runtime_last_run_effective_threads() != 1u) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_last_run_effective_threads() == 1u");
    goto cleanup;
  }

  parallel_runtime_clear_test_env();
  status = parallel_runtime_set_test_env("EOT_TOOL_THREADS", "3");
  if (status != EOT_OK) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_set_test_env(\"EOT_TOOL_THREADS\", \"3\") returned %d",
             status);
    goto cleanup;
  }
  status = mtx_encode_ttf_file("testdata/OpenSans-Regular.ttf", &parallel);
  if (status != EOT_OK) {
    snprintf(failure, sizeof(failure),
             "assertion failed: mtx_encode_ttf_file(\"testdata/OpenSans-Regular.ttf\", &parallel) returned %d",
             status);
    goto cleanup;
  }
  if (parallel_runtime_last_run_task_count() != 3u) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_last_run_task_count() == 3u");
    goto cleanup;
  }

  if (parallel_runtime_last_run_effective_threads() <= 1u) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_last_run_effective_threads() > 1u");
    goto cleanup;
  }
  if (parallel_runtime_last_run_effective_threads() != 3u) {
    snprintf(failure, sizeof(failure),
             "assertion failed: parallel_runtime_last_run_effective_threads() == 3u");
    goto cleanup;
  }
  if (serial.length != parallel.length) {
    snprintf(failure, sizeof(failure), "assertion failed: serial.length == parallel.length");
    goto cleanup;
  }
  if (memcmp(serial.data, parallel.data, serial.length) != 0) {
    snprintf(failure, sizeof(failure),
             "assertion failed: memcmp(serial.data, parallel.data, serial.length) == 0");
    goto cleanup;
  }

cleanup:
  parallel_runtime_clear_test_env();
  byte_buffer_destroy(&serial);
  byte_buffer_destroy(&parallel);
  if (failure[0] != '\0') {
    test_fail_with_message(failure);
  }
}

static void test_encode_excludes_vdmx_from_block1(void) {
  sfnt_font_t source_font;
  byte_buffer_t eot;
  uint8_t *block1_data = NULL;
  size_t block1_size = 0;
  sfnt_font_t block1_font;
  eot_header_t header;

  init_minimal_font_with_vdmx(&source_font);
  byte_buffer_init(&eot);

  ASSERT_OK(mtx_encode_font(&source_font, &eot));
  extract_block1_font(&eot, &block1_data, &block1_size, &block1_font, &header);

  ASSERT_TRUE(sfnt_font_has_table(&source_font, TAG_VDMX));
  ASSERT_TRUE(!sfnt_font_has_table(&block1_font, TAG_VDMX));

  sfnt_font_destroy(&block1_font);
  eot_header_destroy(&header);
  free(block1_data);
  byte_buffer_destroy(&eot);
  sfnt_font_destroy(&source_font);
}

static void test_encode_warns_when_dropping_vdmx(void) {
  sfnt_font_t source_font;
  byte_buffer_t eot;
  mtx_encode_warnings_t warnings;

  init_minimal_font_with_vdmx(&source_font);
  byte_buffer_init(&eot);
  mtx_encode_warnings_init(&warnings);

  ASSERT_OK(mtx_encode_font_with_warnings(&source_font, &eot, &warnings));
  ASSERT_TRUE(warnings.dropped_vdmx == 1);
  ASSERT_TRUE(eot.length > 0);

  byte_buffer_destroy(&eot);
  sfnt_font_destroy(&source_font);
}

static void test_encode_preserves_cvt_table_when_present(void) {
  sfnt_font_t source_font;
  sfnt_font_t decoded_font;
  byte_buffer_t eot;

  ASSERT_OK(sfnt_reader_load_file("testdata/OpenSans-Regular.ttf", &source_font));
  ASSERT_TRUE(sfnt_font_has_table(&source_font, TAG_cvt));
  ASSERT_OK(mtx_encode_ttf_file("testdata/OpenSans-Regular.ttf", &eot));
  ASSERT_TRUE(eot.length > 0);
  ASSERT_OK(decode_eot_buffer_to_font(&eot, "build/out/preserve-cvt.eot",
                                      &decoded_font));
  assert_matching_table(&source_font, &decoded_font, TAG_cvt);

  sfnt_font_destroy(&decoded_font);
  byte_buffer_destroy(&eot);
  sfnt_font_destroy(&source_font);
}

static void test_encode_preserves_hdmx_table_when_present(void) {
  sfnt_font_t source_font;
  sfnt_font_t decoded_font;
  byte_buffer_t eot;

  init_minimal_font_with_hdmx(&source_font);
  ASSERT_OK(mtx_encode_font(&source_font, &eot));
  ASSERT_TRUE(eot.length > 0);
  ASSERT_OK(decode_eot_buffer_to_font(&eot, "build/out/preserve-hdmx.eot",
                                      &decoded_font));
  assert_matching_table(&source_font, &decoded_font, TAG_hdmx);

  sfnt_font_destroy(&decoded_font);
  byte_buffer_destroy(&eot);
  sfnt_font_destroy(&source_font);
}

static void test_encode_ppt_xor_only_obfuscates_font_data_region(void) {
  byte_buffer_t normal_eot;
  byte_buffer_t obfuscated_eot;
  eot_header_t normal_header;
  eot_header_t obfuscated_header;

  ASSERT_OK(mtx_encode_ttf_file("testdata/OpenSans-Regular.ttf", &normal_eot));
  ASSERT_OK(mtx_encode_ttf_file_with_ppt_xor("testdata/OpenSans-Regular.ttf",
                                             &obfuscated_eot));
  ASSERT_OK(eot_header_parse(buffer_view_make(normal_eot.data, normal_eot.length),
                             &normal_header));
  ASSERT_OK(eot_header_parse(buffer_view_make(obfuscated_eot.data, obfuscated_eot.length),
                             &obfuscated_header));

  ASSERT_TRUE(normal_eot.length == obfuscated_eot.length);
  ASSERT_TRUE(normal_header.header_length == obfuscated_header.header_length);
  ASSERT_TRUE(normal_header.font_data_size == obfuscated_header.font_data_size);
  ASSERT_TRUE(normal_header.version == obfuscated_header.version);
  ASSERT_TRUE((normal_header.flags & EOT_FLAG_PPT_XOR) == 0u);
  ASSERT_TRUE((obfuscated_header.flags & EOT_FLAG_PPT_XOR) != 0u);
  ASSERT_TRUE((normal_header.flags | EOT_FLAG_PPT_XOR) == obfuscated_header.flags);

  ASSERT_TRUE(memcmp(normal_eot.data, obfuscated_eot.data, 12) == 0);
  ASSERT_TRUE(memcmp(normal_eot.data + 16, obfuscated_eot.data + 16,
                     normal_header.header_length - 16) == 0);

  for (size_t i = 0; i < normal_header.font_data_size; i++) {
    size_t offset = normal_header.header_length + i;
    ASSERT_TRUE((uint8_t)(normal_eot.data[offset] ^ 0x50u) == obfuscated_eot.data[offset]);
  }

  eot_header_destroy(&normal_header);
  eot_header_destroy(&obfuscated_header);
  byte_buffer_destroy(&normal_eot);
  byte_buffer_destroy(&obfuscated_eot);
}

void register_encode_pipeline_tests(void) {
  test_register("test_encode_opensans_matches_with_single_thread_override",
                test_encode_opensans_matches_with_single_thread_override);
  test_register("test_encode_opensans_uses_split_glyf_blocks",
                test_encode_opensans_uses_split_glyf_blocks);
  test_register("test_encode_excludes_vdmx_from_block1",
                test_encode_excludes_vdmx_from_block1);
  test_register("test_encode_warns_when_dropping_vdmx",
                test_encode_warns_when_dropping_vdmx);
  test_register("test_encode_preserves_cvt_table_when_present",
                test_encode_preserves_cvt_table_when_present);
  test_register("test_encode_preserves_hdmx_table_when_present",
                test_encode_preserves_hdmx_table_when_present);
  test_register("test_encode_ppt_xor_only_obfuscates_font_data_region",
                test_encode_ppt_xor_only_obfuscates_font_data_region);
}
