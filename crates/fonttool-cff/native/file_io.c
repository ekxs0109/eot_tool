#include "file_io.h"

#include <stdio.h>
#include <stdlib.h>

eot_status_t file_io_read_all(const char *path, file_buffer_t *out) {
  FILE *stream;
  long file_size;
  uint8_t *data;
  size_t bytes_read;

  if (path == NULL || out == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  out->data = NULL;
  out->length = 0;

  stream = fopen(path, "rb");
  if (stream == NULL) {
    return EOT_ERR_IO;
  }

  if (fseek(stream, 0, SEEK_END) != 0) {
    fclose(stream);
    return EOT_ERR_IO;
  }

  file_size = ftell(stream);
  if (file_size < 0) {
    fclose(stream);
    return EOT_ERR_IO;
  }

  if (fseek(stream, 0, SEEK_SET) != 0) {
    fclose(stream);
    return EOT_ERR_IO;
  }

  if ((unsigned long)file_size > (unsigned long)SIZE_MAX) {
    fclose(stream);
    return EOT_ERR_IO;
  }

  data = NULL;
  if (file_size > 0) {
    data = (uint8_t *)malloc((size_t)file_size);
    if (data == NULL) {
      fclose(stream);
      return EOT_ERR_ALLOCATION;
    }

    bytes_read = fread(data, 1, (size_t)file_size, stream);
    if (bytes_read != (size_t)file_size) {
      free(data);
      fclose(stream);
      return EOT_ERR_IO;
    }
  }

  fclose(stream);

  out->data = data;
  out->length = (size_t)file_size;
  return EOT_OK;
}

void file_io_free(file_buffer_t *buffer) {
  if (buffer == NULL) {
    return;
  }

  free(buffer->data);
  buffer->data = NULL;
  buffer->length = 0;
}

eot_status_t file_io_write_all(const char *path, const uint8_t *data, size_t length) {
  FILE *stream;

  if (path == NULL || data == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  stream = fopen(path, "wb");
  if (stream == NULL) {
    return EOT_ERR_IO;
  }

  if (length > 0) {
    size_t bytes_written = fwrite(data, 1, length, stream);
    if (bytes_written != length) {
      fclose(stream);
      return EOT_ERR_IO;
    }
  }

  fclose(stream);
  return EOT_OK;
}

const char *eot_status_to_string(eot_status_t status) {
  switch (status) {
    case EOT_OK:
      return "ok";
    case EOT_ERR_INVALID_ARGUMENT:
      return "invalid argument";
    case EOT_ERR_IO:
      return "i/o error";
    case EOT_ERR_TRUNCATED:
      return "truncated header";
    case EOT_ERR_INVALID_MAGIC:
      return "invalid EOT magic number";
    case EOT_ERR_ALLOCATION:
      return "allocation failure";
    case EOT_ERR_INVALID_STRING_LENGTH:
      return "invalid string length";
    case EOT_ERR_INVALID_PADDING:
      return "invalid padding";
    case EOT_ERR_INVALID_SIZE_METADATA:
      return "invalid size metadata";
    case EOT_ERR_CORRUPT_DATA:
      return "corrupt data";
    case EOT_ERR_DECOMPRESS_FAILED:
      return "decompression failed";
  }

  return "unknown error";
}
