#ifndef EOT_TOOL_MTX_CONTAINER_H
#define EOT_TOOL_MTX_CONTAINER_H

#include <stddef.h>
#include <stdint.h>

#include "byte_io.h"
#include "file_io.h"

typedef struct {
  uint8_t num_blocks;
  uint32_t copy_dist;
  uint32_t offset_block2;
  uint32_t offset_block3;
  const uint8_t *payload_data;
  size_t payload_size;
} mtx_container_t;

typedef struct {
  uint8_t *block1;
  size_t block1_size;
  uint8_t *block2;
  size_t block2_size;
  uint8_t *block3;
  size_t block3_size;
} encoded_blocks_t;

eot_status_t mtx_container_parse(buffer_view_t view, mtx_container_t *out);
eot_status_t mtx_container_pack(const encoded_blocks_t *blocks, uint8_t **out_data, size_t *out_size);

#endif
