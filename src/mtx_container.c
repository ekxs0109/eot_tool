#include "mtx_container.h"

#include <stdlib.h>
#include <string.h>

#define MTX_HEADER_SIZE 10
#define MTX_PRELOAD_SIZE 7168

eot_status_t mtx_container_parse(buffer_view_t view, mtx_container_t *out) {
  if (out == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (!buffer_view_has_range(view, 0, MTX_HEADER_SIZE)) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t *data = view.data;

  out->num_blocks = data[0];
  out->copy_dist = read_u24be(data + 1);
  out->offset_block2 = read_u24be(data + 4);
  out->offset_block3 = read_u24be(data + 7);
  out->payload_data = view.data;
  out->payload_size = view.length;

  if (out->num_blocks < 1 || out->num_blocks > 3) {
    return EOT_ERR_CORRUPT_DATA;
  }

  if (out->copy_dist == 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  if (out->offset_block2 < MTX_HEADER_SIZE) {
    return EOT_ERR_CORRUPT_DATA;
  }

  if (out->num_blocks >= 2 && out->offset_block2 >= view.length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  if (out->num_blocks >= 3) {
    if (out->offset_block3 < out->offset_block2 || out->offset_block3 >= view.length) {
      return EOT_ERR_CORRUPT_DATA;
    }
  }

  return EOT_OK;
}

eot_status_t mtx_container_pack(const encoded_blocks_t *blocks, uint8_t **out_data, size_t *out_size) {
  if (blocks == NULL || out_data == NULL || out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  uint8_t num_blocks = 0;
  if (blocks->block1_size > 0) num_blocks++;
  if (blocks->block2_size > 0) num_blocks++;
  if (blocks->block3_size > 0) num_blocks++;

  if (num_blocks == 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  size_t total_size = MTX_HEADER_SIZE + blocks->block1_size + blocks->block2_size + blocks->block3_size;
  uint8_t *data = (uint8_t *)malloc(total_size);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  size_t max_block_size = blocks->block1_size;
  if (blocks->block2_size > max_block_size) {
    max_block_size = blocks->block2_size;
  }
  if (blocks->block3_size > max_block_size) {
    max_block_size = blocks->block3_size;
  }

  uint32_t offset_block2 = MTX_HEADER_SIZE + blocks->block1_size;
  uint32_t offset_block3 = offset_block2 + blocks->block2_size;
  uint32_t copy_dist = (uint32_t)(MTX_PRELOAD_SIZE + max_block_size);

  data[0] = num_blocks;
  write_u24be(data + 1, copy_dist);
  write_u24be(data + 4, offset_block2);
  write_u24be(data + 7, offset_block3);

  size_t offset = MTX_HEADER_SIZE;
  if (blocks->block1_size > 0) {
    memcpy(data + offset, blocks->block1, blocks->block1_size);
    offset += blocks->block1_size;
  }
  if (blocks->block2_size > 0) {
    memcpy(data + offset, blocks->block2, blocks->block2_size);
    offset += blocks->block2_size;
  }
  if (blocks->block3_size > 0) {
    memcpy(data + offset, blocks->block3, blocks->block3_size);
    offset += blocks->block3_size;
  }

  *out_data = data;
  *out_size = total_size;
  return EOT_OK;
}
