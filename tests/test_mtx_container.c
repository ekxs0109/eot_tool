#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/byte_io.h"
#include "../src/eot_header.h"
#include "../src/file_io.h"
#include "../src/lzcomp.h"
#include "../src/mtx_container.h"

void test_register(const char *name, void (*fn)(void));

typedef struct {
  uint8_t *block1;
  size_t block1_size;
  uint8_t *block2;
  size_t block2_size;
  uint8_t *block3;
  size_t block3_size;
} decoded_blocks_t;

static void decoded_blocks_destroy(decoded_blocks_t *blocks) {
  if (blocks == NULL) {
    return;
  }
  free(blocks->block1);
  free(blocks->block2);
  free(blocks->block3);
  blocks->block1 = NULL;
  blocks->block2 = NULL;
  blocks->block3 = NULL;
  blocks->block1_size = 0;
  blocks->block2_size = 0;
  blocks->block3_size = 0;
}

static buffer_view_t extract_mtx_payload(const char *eot_path, file_buffer_t *file_buf, eot_header_t *header) {
  eot_status_t status = file_io_read_all(eot_path, file_buf);
  if (status != EOT_OK) {
    fprintf(stderr, "Failed to read EOT file: %s\n", eot_status_to_string(status));
    return buffer_view_make(NULL, 0);
  }

  status = eot_header_parse(buffer_view_make(file_buf->data, file_buf->length), header);
  if (status != EOT_OK) {
    fprintf(stderr, "Failed to parse EOT header: %s\n", eot_status_to_string(status));
    file_io_free(file_buf);
    return buffer_view_make(NULL, 0);
  }

  if (header->header_length >= file_buf->length) {
    fprintf(stderr, "Invalid header_length: %u >= %zu\n", header->header_length, file_buf->length);
    eot_header_destroy(header);
    file_io_free(file_buf);
    return buffer_view_make(NULL, 0);
  }

  const uint8_t *mtx_data = file_buf->data + header->header_length;
  size_t mtx_size = file_buf->length - header->header_length;

  return buffer_view_make(mtx_data, mtx_size);
}

static eot_status_t decompress_mtx_blocks(const mtx_container_t *container, decoded_blocks_t *blocks) {
  eot_status_t status;

  memset(blocks, 0, sizeof(decoded_blocks_t));

  size_t block1_compressed_size = container->offset_block2 - 10;
  buffer_view_t block1_view = buffer_view_make(
      container->payload_data + 10,
      block1_compressed_size);

  status = lzcomp_decompress(block1_view, &blocks->block1, &blocks->block1_size);
  if (status != EOT_OK) {
    fprintf(stderr, "Failed to decompress block 1: %s\n", eot_status_to_string(status));
    return status;
  }

  if (container->num_blocks >= 2) {
    size_t block2_compressed_size;
    if (container->num_blocks == 3) {
      block2_compressed_size = container->offset_block3 - container->offset_block2;
    } else {
      block2_compressed_size = container->payload_size - container->offset_block2;
    }

    buffer_view_t block2_view = buffer_view_make(
        container->payload_data + container->offset_block2,
        block2_compressed_size);

    status = lzcomp_decompress(block2_view, &blocks->block2, &blocks->block2_size);
    if (status != EOT_OK) {
      fprintf(stderr, "Failed to decompress block 2: %s\n", eot_status_to_string(status));
      decoded_blocks_destroy(blocks);
      return status;
    }
  }

  if (container->num_blocks >= 3) {
    size_t block3_compressed_size = container->payload_size - container->offset_block3;

    buffer_view_t block3_view = buffer_view_make(
        container->payload_data + container->offset_block3,
        block3_compressed_size);

    status = lzcomp_decompress(block3_view, &blocks->block3, &blocks->block3_size);
    if (status != EOT_OK) {
      fprintf(stderr, "Failed to decompress block 3: %s\n", eot_status_to_string(status));
      decoded_blocks_destroy(blocks);
      return status;
    }
  }

  return EOT_OK;
}

static void test_parse_mtx_offsets_from_fixture(void) {
  file_buffer_t file_buf = {0};
  eot_header_t header = {0};

  buffer_view_t mtx_view = extract_mtx_payload("testdata/wingdings3.eot", &file_buf, &header);
  if (mtx_view.data == NULL) {
    fprintf(stderr, "Failed to extract MTX payload\n");
    return;
  }

  mtx_container_t container;
  eot_status_t status = mtx_container_parse(mtx_view, &container);

  if (status != EOT_OK) {
    fprintf(stderr, "Failed to parse MTX container: %s\n", eot_status_to_string(status));
    eot_header_destroy(&header);
    file_io_free(&file_buf);
    return;
  }

  // Verify expected values
  assert(container.num_blocks == 3);
  assert(container.offset_block2 > 10);
  assert(container.offset_block3 > container.offset_block2);

  eot_header_destroy(&header);
  file_io_free(&file_buf);
}

static void test_decompress_wingdings3_blocks(void) {
  file_buffer_t file_buf = {0};
  eot_header_t header = {0};

  buffer_view_t mtx_view = extract_mtx_payload("testdata/wingdings3.eot", &file_buf, &header);
  if (mtx_view.data == NULL) {
    fprintf(stderr, "Failed to extract MTX payload\n");
    return;
  }

  mtx_container_t container;
  eot_status_t status = mtx_container_parse(mtx_view, &container);
  assert(status == EOT_OK);

  decoded_blocks_t blocks;
  status = decompress_mtx_blocks(&container, &blocks);

  if (status != EOT_OK) {
    fprintf(stderr, "Failed to decompress MTX blocks: %s\n", eot_status_to_string(status));
    eot_header_destroy(&header);
    file_io_free(&file_buf);
    return;
  }

  // Verify all 3 blocks were decompressed
  assert(blocks.block1 != NULL);
  assert(blocks.block1_size > 0);
  assert(blocks.block2 != NULL);
  assert(blocks.block2_size > 0);
  assert(blocks.block3 != NULL);
  assert(blocks.block3_size > 0);

  decoded_blocks_destroy(&blocks);
  eot_header_destroy(&header);
  file_io_free(&file_buf);
}

static void test_mtx_rejects_invalid_block_order(void) {
  // Create MTX data with invalid block order (offset_block3 < offset_block2)
  uint8_t invalid_mtx[] = {
    0x03,                    // num_blocks = 3
    0x00, 0x00, 0x00, 0x01,  // copy_dist = 1
    0x00, 0x00, 0x00, 0x64,  // offset_block2 = 100
    0x00, 0x00, 0x00, 0x32   // offset_block3 = 50 (INVALID: < offset_block2)
  };

  buffer_view_t invalid_view = buffer_view_make(invalid_mtx, sizeof(invalid_mtx));
  mtx_container_t container;

  eot_status_t status = mtx_container_parse(invalid_view, &container);
  assert(status != EOT_OK);
}

static void test_mtx_rejects_invalid_copy_distance(void) {
  // Create MTX data with copy_dist = 0 (invalid)
  uint8_t invalid_mtx[] = {
    0x02,                    // num_blocks = 2
    0x00, 0x00, 0x00, 0x00,  // copy_dist = 0 (INVALID)
    0x00, 0x00, 0x00, 0x64,  // offset_block2 = 100
    0x00, 0x00, 0x00, 0x00   // offset_block3 = 0 (unused)
  };

  buffer_view_t invalid_view = buffer_view_make(invalid_mtx, sizeof(invalid_mtx));
  mtx_container_t container;

  eot_status_t status = mtx_container_parse(invalid_view, &container);
  assert(status != EOT_OK);
}

static void test_byte_io_u24be(void) {
  uint8_t data1[] = {0x12, 0x34, 0x56};
  uint32_t result1 = read_u24be(data1);
  assert(result1 == 0x123456);

  uint8_t data2[] = {0x00, 0x00, 0x01};
  uint32_t result2 = read_u24be(data2);
  assert(result2 == 0x000001);

  uint8_t data3[] = {0xFF, 0xFF, 0xFF};
  uint32_t result3 = read_u24be(data3);
  assert(result3 == 0xFFFFFF);
}

static void test_error_codes(void) {
  assert(strcmp(eot_status_to_string(EOT_OK), "ok") == 0);
  assert(strcmp(eot_status_to_string(EOT_ERR_CORRUPT_DATA), "corrupt data") == 0);
  assert(strcmp(eot_status_to_string(EOT_ERR_DECOMPRESS_FAILED), "decompression failed") == 0);
}

static void test_mtx_pack_sets_copy_distance_from_largest_block(void) {
  uint8_t block1[16] = {0};
  uint8_t block2[32] = {0};
  uint8_t block3[24] = {0};
  encoded_blocks_t blocks = {
    .block1 = block1,
    .block1_size = sizeof(block1),
    .block2 = block2,
    .block2_size = sizeof(block2),
    .block3 = block3,
    .block3_size = sizeof(block3),
  };
  uint8_t *packed = NULL;
  size_t packed_size = 0;

  eot_status_t status = mtx_container_pack(&blocks, &packed, &packed_size);
  assert(status == EOT_OK);
  assert(packed != NULL);
  assert(packed_size == 10u + sizeof(block1) + sizeof(block2) + sizeof(block3));
  assert(read_u24be(packed + 1) == 7168u + sizeof(block2));

  free(packed);
}

void register_mtx_tests(void) {
  test_register("byte_io_u24be", test_byte_io_u24be);
  test_register("error_codes", test_error_codes);
  test_register("parse_mtx_offsets_from_fixture", test_parse_mtx_offsets_from_fixture);
  test_register("decompress_wingdings3_blocks", test_decompress_wingdings3_blocks);
  test_register("mtx_rejects_invalid_block_order", test_mtx_rejects_invalid_block_order);
  test_register("mtx_rejects_invalid_copy_distance", test_mtx_rejects_invalid_copy_distance);
  test_register("mtx_pack_sets_copy_distance_from_largest_block",
                test_mtx_pack_sets_copy_distance_from_largest_block);
}
