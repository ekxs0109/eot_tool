#ifndef EOT_TOOL_FILE_IO_H
#define EOT_TOOL_FILE_IO_H

#include <stddef.h>
#include <stdint.h>

typedef enum {
  EOT_OK = 0,
  EOT_ERR_INVALID_ARGUMENT = 1,
  EOT_ERR_IO = 2,
  EOT_ERR_TRUNCATED = 3,
  EOT_ERR_INVALID_MAGIC = 4,
  EOT_ERR_ALLOCATION = 5,
  EOT_ERR_INVALID_STRING_LENGTH = 6,
  EOT_ERR_INVALID_PADDING = 7,
  EOT_ERR_INVALID_SIZE_METADATA = 8,
  EOT_ERR_CORRUPT_DATA = 9,
  EOT_ERR_DECOMPRESS_FAILED = 10
} eot_status_t;

typedef struct {
  uint8_t *data;
  size_t length;
} file_buffer_t;

eot_status_t file_io_read_all(const char *path, file_buffer_t *out);
eot_status_t file_io_write_all(const char *path, const uint8_t *data, size_t length);
void file_io_free(file_buffer_t *buffer);
const char *eot_status_to_string(eot_status_t status);

#endif
