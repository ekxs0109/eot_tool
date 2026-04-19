#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

extern "C" {
#include "tt_rebuilder.h"
#include "byte_io.h"
#include "glyf_codec.h"
}

#define TAG_glyf 0x676c7966u
#define TAG_head 0x68656164u
#define TAG_hhea 0x68686561u
#define TAG_hmtx 0x686d7478u
#define TAG_loca 0x6c6f6361u
#define TAG_maxp 0x6d617870u
#define TAG_post 0x706f7374u

#define HEAD_TABLE_LENGTH 54u
#define HHEA_TABLE_LENGTH 36u
#define HMTX_METRIC_LENGTH 4u
#define MAXP_TABLE_LENGTH 32u
#define POST_TABLE_HEADER_LENGTH 32u

#define HEAD_VERSION 0x00010000u
#define HEAD_MAGIC 0x5F0F3CF5u
#define HEAD_FLAGS 0x0003u
#define HEAD_UNITS_PER_EM 1000u
#define HEAD_LOWEST_REC_PPEM 8u
#define HEAD_FONT_DIRECTION_HINT 2
#define MAC_EPOCH_OFFSET 2082844800u

#define POST_FORMAT_2 0x00020000u
#define POST_FORMAT_3 0x00030000u

#define SIMPLE_GLYPH_BBOX_MARKER 0x7FFF

typedef struct {
  uint8_t *data;
  size_t size;
  size_t capacity;
} byte_writer_t;

typedef struct {
  int16_t x_min;
  int16_t y_min;
  int16_t x_max;
  int16_t y_max;
  uint16_t point_count;
  uint16_t contour_count;
  uint16_t advance_width;
  int empty;
} glyph_metrics_t;

static void writer_init(byte_writer_t *writer) {
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
}

static void writer_destroy(byte_writer_t *writer) {
  free(writer->data);
  writer->data = NULL;
  writer->size = 0;
  writer->capacity = 0;
}

static eot_status_t writer_reserve(byte_writer_t *writer, size_t additional) {
  if (additional > SIZE_MAX - writer->size) {
    return EOT_ERR_CORRUPT_DATA;
  }
  if (writer->size + additional <= writer->capacity) {
    return EOT_OK;
  }

  size_t new_capacity = writer->capacity == 0 ? 64u : writer->capacity;
  while (new_capacity < writer->size + additional) {
    if (new_capacity > SIZE_MAX / 2u) {
      new_capacity = writer->size + additional;
      break;
    }
    new_capacity *= 2u;
  }

  uint8_t *new_data = (uint8_t *)realloc(writer->data, new_capacity);
  if (new_data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  writer->data = new_data;
  writer->capacity = new_capacity;
  return EOT_OK;
}

static eot_status_t writer_append_bytes(byte_writer_t *writer,
                                        const uint8_t *data,
                                        size_t length) {
  eot_status_t status = writer_reserve(writer, length);
  if (status != EOT_OK) {
    return status;
  }

  if (length > 0) {
    memcpy(writer->data + writer->size, data, length);
  }
  writer->size += length;
  return EOT_OK;
}

static eot_status_t writer_append_u8(byte_writer_t *writer, uint8_t value) {
  return writer_append_bytes(writer, &value, 1u);
}

static eot_status_t writer_append_u16be(byte_writer_t *writer, uint16_t value) {
  uint8_t bytes[2];
  write_u16be(bytes, value);
  return writer_append_bytes(writer, bytes, sizeof(bytes));
}

static eot_status_t writer_append_s16be(byte_writer_t *writer, int16_t value) {
  return writer_append_u16be(writer, (uint16_t)value);
}

static eot_status_t writer_append_encoded_255_ushort(byte_writer_t *writer,
                                                     uint16_t value) {
  uint8_t encoded[3];
  size_t encoded_size = 0;
  eot_status_t status =
      glyf_encode_255_ushort(value, encoded, sizeof(encoded), &encoded_size);
  if (status != EOT_OK) {
    return status;
  }
  return writer_append_bytes(writer, encoded, encoded_size);
}

static int fits_in_int16(int value) {
  return value >= INT16_MIN && value <= INT16_MAX;
}

static const char *resolve_glyph_name(const tt_glyph_outline_t *outline,
                                      size_t glyph_index,
                                      char *generated_name,
                                      size_t generated_name_size) {
  if (outline->glyph_name != NULL && outline->glyph_name[0] != '\0') {
    return outline->glyph_name;
  }

  if (glyph_index == 0u) {
    return ".notdef";
  }

  if (snprintf(generated_name, generated_name_size, "glyph%zu", glyph_index) <
      0) {
    return NULL;
  }
  return generated_name;
}

static int is_post_format_2_name_usable(const char *name) {
  size_t i;
  size_t length;

  if (name == NULL) {
    return 0;
  }

  length = strlen(name);
  if (length == 0u || length > 255u) {
    return 0;
  }

  for (i = 0; i < length; i++) {
    unsigned char ch = (unsigned char)name[i];
    if (ch < 33u || ch > 126u) {
      return 0;
    }
  }

  return 1;
}

static eot_status_t compute_glyph_metrics(const tt_glyph_outline_t *outline,
                                          glyph_metrics_t *out_metrics) {
  size_t contour_index;
  size_t total_points = 0u;
  int first_point = 1;

  if (outline == NULL || out_metrics == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  memset(out_metrics, 0, sizeof(*out_metrics));
  out_metrics->advance_width = outline->advance_width;

  if (outline->num_contours == 0u) {
    out_metrics->empty = 1;
    return EOT_OK;
  }

  if (outline->contours == NULL || outline->num_contours > INT16_MAX) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (contour_index = 0u; contour_index < outline->num_contours;
       contour_index++) {
    const tt_contour_t *contour = &outline->contours[contour_index];
    size_t point_index;

    if (contour->points == NULL || contour->num_points == 0u) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    if (contour->num_points > UINT16_MAX ||
        total_points > UINT16_MAX - contour->num_points) {
      return EOT_ERR_CORRUPT_DATA;
    }

    total_points += contour->num_points;
    for (point_index = 0u; point_index < contour->num_points; point_index++) {
      const tt_outline_point_t *point = &contour->points[point_index];
      if (first_point) {
        out_metrics->x_min = point->x;
        out_metrics->y_min = point->y;
        out_metrics->x_max = point->x;
        out_metrics->y_max = point->y;
        first_point = 0;
        continue;
      }

      if (point->x < out_metrics->x_min) {
        out_metrics->x_min = point->x;
      }
      if (point->x > out_metrics->x_max) {
        out_metrics->x_max = point->x;
      }
      if (point->y < out_metrics->y_min) {
        out_metrics->y_min = point->y;
      }
      if (point->y > out_metrics->y_max) {
        out_metrics->y_max = point->y;
      }
    }
  }

  out_metrics->point_count = (uint16_t)total_points;
  out_metrics->contour_count = (uint16_t)outline->num_contours;
  out_metrics->empty = 0;
  return EOT_OK;
}

static eot_status_t append_outline_payload(const tt_glyph_outline_t *outline,
                                           byte_writer_t *glyf_stream) {
  byte_writer_t payload;
  int last_x = 0;
  int last_y = 0;
  size_t contour_index;
  eot_status_t status = EOT_OK;

  writer_init(&payload);

  for (contour_index = 0u; contour_index < outline->num_contours;
       contour_index++) {
    const tt_contour_t *contour = &outline->contours[contour_index];
    size_t point_index;

    for (point_index = 0u; point_index < contour->num_points; point_index++) {
      const tt_outline_point_t *point = &contour->points[point_index];
      uint8_t flag = 0u;
      uint8_t encoded[4];
      size_t encoded_size = 0u;
      int dx = (int)point->x - last_x;
      int dy = (int)point->y - last_y;

      status = glyf_encode_triplet(point->on_curve != 0, dx, dy, &flag, encoded,
                                   sizeof(encoded), &encoded_size);
      if (status != EOT_OK) {
        goto cleanup;
      }
      status = writer_append_u8(glyf_stream, flag);
      if (status != EOT_OK) {
        goto cleanup;
      }
      status = writer_append_bytes(&payload, encoded, encoded_size);
      if (status != EOT_OK) {
        goto cleanup;
      }

      last_x = point->x;
      last_y = point->y;
    }
  }

  status = writer_append_bytes(glyf_stream, payload.data, payload.size);

cleanup:
  writer_destroy(&payload);
  return status;
}

static eot_status_t append_simple_glyph_stream(const tt_glyph_outline_t *outline,
                                               const glyph_metrics_t *metrics,
                                               byte_writer_t *glyf_stream) {
  size_t contour_index;
  eot_status_t status;

  if (metrics->empty) {
    return writer_append_s16be(glyf_stream, 0);
  }

  status = writer_append_s16be(glyf_stream, SIMPLE_GLYPH_BBOX_MARKER);
  if (status == EOT_OK) {
    status = writer_append_s16be(glyf_stream, (int16_t)metrics->contour_count);
  }
  if (status == EOT_OK) {
    status = writer_append_s16be(glyf_stream, metrics->x_min);
  }
  if (status == EOT_OK) {
    status = writer_append_s16be(glyf_stream, metrics->y_min);
  }
  if (status == EOT_OK) {
    status = writer_append_s16be(glyf_stream, metrics->x_max);
  }
  if (status == EOT_OK) {
    status = writer_append_s16be(glyf_stream, metrics->y_max);
  }
  if (status != EOT_OK) {
    return status;
  }

  for (contour_index = 0u; contour_index < outline->num_contours;
       contour_index++) {
    uint16_t contour_points =
        (uint16_t)outline->contours[contour_index].num_points;
    uint16_t encoded_points =
        (uint16_t)(contour_points - (contour_index == 0u ? 1u : 0u));
    status = writer_append_encoded_255_ushort(glyf_stream, encoded_points);
    if (status != EOT_OK) {
      return status;
    }
  }

  status = append_outline_payload(outline, glyf_stream);
  if (status == EOT_OK) {
    status = writer_append_encoded_255_ushort(glyf_stream, 0u);
  }
  if (status == EOT_OK) {
    status = writer_append_encoded_255_ushort(glyf_stream, 0u);
  }
  return status;
}

static eot_status_t build_compressed_glyf_stream(
    const tt_glyph_outline_t *outlines, const glyph_metrics_t *metrics,
    size_t num_outlines, byte_writer_t *out_glyf_stream) {
  size_t glyph_index;

  writer_init(out_glyf_stream);
  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    eot_status_t status = append_simple_glyph_stream(
        &outlines[glyph_index], &metrics[glyph_index], out_glyf_stream);
    if (status != EOT_OK) {
      writer_destroy(out_glyf_stream);
      return status;
    }
  }

  return EOT_OK;
}

static eot_status_t build_glyf_and_loca_tables(
    const tt_glyph_outline_t *outlines, const glyph_metrics_t *metrics,
    size_t num_outlines, uint8_t **out_glyf_data, size_t *out_glyf_length,
    uint8_t **out_loca_data, size_t *out_loca_length,
    int16_t *out_index_to_loca_format) {
  byte_writer_t compressed_glyf;
  buffer_view_t empty_view = buffer_view_make(NULL, 0u);
  eot_status_t first_status = EOT_OK;
  int format;

  if (out_glyf_data == NULL || out_glyf_length == NULL || out_loca_data == NULL ||
      out_loca_length == NULL || out_index_to_loca_format == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *out_glyf_data = NULL;
  *out_glyf_length = 0u;
  *out_loca_data = NULL;
  *out_loca_length = 0u;
  *out_index_to_loca_format = 0;

  eot_status_t status = build_compressed_glyf_stream(outlines, metrics,
                                                     num_outlines,
                                                     &compressed_glyf);
  if (status != EOT_OK) {
    return status;
  }

  for (format = 0; format <= 1; format++) {
    uint8_t *glyf_data = NULL;
    size_t glyf_length = 0u;
    uint8_t *loca_data = NULL;
    size_t loca_length = 0u;

    status = glyf_decode_with_loca_format(
        buffer_view_make(compressed_glyf.data, compressed_glyf.size), empty_view,
        empty_view, (int)num_outlines, format, &glyf_data, &glyf_length,
        &loca_data, &loca_length);
    if (status == EOT_OK) {
      *out_glyf_data = glyf_data;
      *out_glyf_length = glyf_length;
      *out_loca_data = loca_data;
      *out_loca_length = loca_length;
      *out_index_to_loca_format = (int16_t)format;
      writer_destroy(&compressed_glyf);
      return EOT_OK;
    }

    free(glyf_data);
    free(loca_data);
    if (format == 0) {
      first_status = status;
    }
  }

  writer_destroy(&compressed_glyf);
  return first_status;
}

static const glyph_metrics_t *find_first_non_empty_glyph(
    const glyph_metrics_t *metrics, size_t num_outlines, size_t *out_index) {
  size_t glyph_index;

  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    if (!metrics[glyph_index].empty) {
      if (out_index != NULL) {
        *out_index = glyph_index;
      }
      return &metrics[glyph_index];
    }
  }

  return NULL;
}

static eot_status_t build_head_table(const glyph_metrics_t *metrics,
                                     size_t num_outlines,
                                     int16_t index_to_loca_format,
                                     uint8_t **out_data,
                                     size_t *out_length) {
  uint8_t *data;
  const glyph_metrics_t *first_non_empty;
  size_t glyph_index;
  int16_t x_min = 0;
  int16_t y_min = 0;
  int16_t x_max = 0;
  int16_t y_max = 0;
  uint64_t timestamp = (uint64_t)time(NULL) + (uint64_t)MAC_EPOCH_OFFSET;

  data = (uint8_t *)calloc(HEAD_TABLE_LENGTH, 1u);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  first_non_empty = find_first_non_empty_glyph(metrics, num_outlines, &glyph_index);
  if (first_non_empty != NULL) {
    x_min = first_non_empty->x_min;
    y_min = first_non_empty->y_min;
    x_max = first_non_empty->x_max;
    y_max = first_non_empty->y_max;
    for (glyph_index++; glyph_index < num_outlines; glyph_index++) {
      if (metrics[glyph_index].empty) {
        continue;
      }
      if (metrics[glyph_index].x_min < x_min) {
        x_min = metrics[glyph_index].x_min;
      }
      if (metrics[glyph_index].y_min < y_min) {
        y_min = metrics[glyph_index].y_min;
      }
      if (metrics[glyph_index].x_max > x_max) {
        x_max = metrics[glyph_index].x_max;
      }
      if (metrics[glyph_index].y_max > y_max) {
        y_max = metrics[glyph_index].y_max;
      }
    }
  }

  write_u32be(data, HEAD_VERSION);
  write_u32be(data + 4u, HEAD_VERSION);
  write_u32be(data + 8u, 0u);
  write_u32be(data + 12u, HEAD_MAGIC);
  write_u16be(data + 16u, HEAD_FLAGS);
  write_u16be(data + 18u, HEAD_UNITS_PER_EM);
  write_u32be(data + 20u, (uint32_t)(timestamp >> 32));
  write_u32be(data + 24u, (uint32_t)(timestamp & 0xFFFFFFFFu));
  write_u32be(data + 28u, (uint32_t)(timestamp >> 32));
  write_u32be(data + 32u, (uint32_t)(timestamp & 0xFFFFFFFFu));
  write_u16be(data + 36u, (uint16_t)x_min);
  write_u16be(data + 38u, (uint16_t)y_min);
  write_u16be(data + 40u, (uint16_t)x_max);
  write_u16be(data + 42u, (uint16_t)y_max);
  write_u16be(data + 44u, 0u);
  write_u16be(data + 46u, HEAD_LOWEST_REC_PPEM);
  write_u16be(data + 48u, (uint16_t)HEAD_FONT_DIRECTION_HINT);
  write_u16be(data + 50u, (uint16_t)index_to_loca_format);
  write_u16be(data + 52u, 0u);

  *out_data = data;
  *out_length = HEAD_TABLE_LENGTH;
  return EOT_OK;
}

static eot_status_t build_hhea_table(const glyph_metrics_t *metrics,
                                     size_t num_outlines,
                                     uint8_t **out_data,
                                     size_t *out_length) {
  uint8_t *data;
  const glyph_metrics_t *first_non_empty;
  size_t glyph_index;
  uint16_t advance_width_max = 0u;
  int16_t ascender = 0;
  int16_t descender = 0;
  int16_t min_left_side_bearing = 0;
  int16_t min_right_side_bearing = 0;
  int16_t x_max_extent = 0;

  data = (uint8_t *)calloc(HHEA_TABLE_LENGTH, 1u);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  if (num_outlines > 0u) {
    for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
      if (metrics[glyph_index].advance_width > advance_width_max) {
        advance_width_max = metrics[glyph_index].advance_width;
      }
    }

    first_non_empty = find_first_non_empty_glyph(metrics, num_outlines, &glyph_index);
    if (first_non_empty != NULL) {
      int first_right_side_bearing =
          (int)first_non_empty->advance_width - (int)first_non_empty->x_max;
      if (!fits_in_int16(first_right_side_bearing)) {
        free(data);
        return EOT_ERR_CORRUPT_DATA;
      }

      ascender = first_non_empty->y_max;
      descender = first_non_empty->y_min;
      min_left_side_bearing = first_non_empty->x_min;
      min_right_side_bearing = (int16_t)first_right_side_bearing;
      x_max_extent = first_non_empty->x_max;

      for (glyph_index++; glyph_index < num_outlines; glyph_index++) {
        int right_side_bearing;
        if (metrics[glyph_index].empty) {
          continue;
        }

        right_side_bearing =
            (int)metrics[glyph_index].advance_width - (int)metrics[glyph_index].x_max;
        if (!fits_in_int16(right_side_bearing)) {
          free(data);
          return EOT_ERR_CORRUPT_DATA;
        }

        if (metrics[glyph_index].y_max > ascender) {
          ascender = metrics[glyph_index].y_max;
        }
        if (metrics[glyph_index].y_min < descender) {
          descender = metrics[glyph_index].y_min;
        }
        if (metrics[glyph_index].x_min < min_left_side_bearing) {
          min_left_side_bearing = metrics[glyph_index].x_min;
        }
        if ((int16_t)right_side_bearing < min_right_side_bearing) {
          min_right_side_bearing = (int16_t)right_side_bearing;
        }
        if (metrics[glyph_index].x_max > x_max_extent) {
          x_max_extent = metrics[glyph_index].x_max;
        }
      }
    }
  }

  write_u32be(data, HEAD_VERSION);
  write_u16be(data + 4u, (uint16_t)ascender);
  write_u16be(data + 6u, (uint16_t)descender);
  write_u16be(data + 8u, 0u);
  write_u16be(data + 10u, advance_width_max);
  write_u16be(data + 12u, (uint16_t)min_left_side_bearing);
  write_u16be(data + 14u, (uint16_t)min_right_side_bearing);
  write_u16be(data + 16u, (uint16_t)x_max_extent);
  write_u16be(data + 18u, 1u);
  write_u16be(data + 20u, 0u);
  write_u16be(data + 22u, 0u);
  write_u16be(data + 24u, 0u);
  write_u16be(data + 26u, 0u);
  write_u16be(data + 28u, 0u);
  write_u16be(data + 30u, 0u);
  write_u16be(data + 32u, 0u);
  write_u16be(data + 34u, (uint16_t)num_outlines);

  *out_data = data;
  *out_length = HHEA_TABLE_LENGTH;
  return EOT_OK;
}

static eot_status_t build_hmtx_table(const glyph_metrics_t *metrics,
                                     size_t num_outlines,
                                     uint8_t **out_data,
                                     size_t *out_length) {
  uint8_t *data;
  size_t glyph_index;

  if (num_outlines > SIZE_MAX / HMTX_METRIC_LENGTH) {
    return EOT_ERR_CORRUPT_DATA;
  }

  data = (uint8_t *)malloc(num_outlines * HMTX_METRIC_LENGTH);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    size_t offset = glyph_index * HMTX_METRIC_LENGTH;
    write_u16be(data + offset, metrics[glyph_index].advance_width);
    write_u16be(data + offset + 2u, (uint16_t)metrics[glyph_index].x_min);
  }

  *out_data = data;
  *out_length = num_outlines * HMTX_METRIC_LENGTH;
  return EOT_OK;
}

static eot_status_t build_maxp_table(const glyph_metrics_t *metrics,
                                     size_t num_outlines,
                                     uint8_t **out_data,
                                     size_t *out_length) {
  uint8_t *data;
  size_t glyph_index;
  uint16_t max_points = 0u;
  uint16_t max_contours = 0u;

  data = (uint8_t *)calloc(MAXP_TABLE_LENGTH, 1u);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    if (metrics[glyph_index].point_count > max_points) {
      max_points = metrics[glyph_index].point_count;
    }
    if (metrics[glyph_index].contour_count > max_contours) {
      max_contours = metrics[glyph_index].contour_count;
    }
  }

  write_u32be(data, HEAD_VERSION);
  write_u16be(data + 4u, (uint16_t)num_outlines);
  write_u16be(data + 6u, max_points);
  write_u16be(data + 8u, max_contours);
  write_u16be(data + 10u, 0u);
  write_u16be(data + 12u, 0u);
  write_u16be(data + 14u, 1u);
  write_u16be(data + 16u, 0u);
  write_u16be(data + 18u, 0u);
  write_u16be(data + 20u, 0u);
  write_u16be(data + 22u, 0u);
  write_u16be(data + 24u, 0u);
  write_u16be(data + 26u, 0u);
  write_u16be(data + 28u, 0u);
  write_u16be(data + 30u, 0u);

  *out_data = data;
  *out_length = MAXP_TABLE_LENGTH;
  return EOT_OK;
}

static eot_status_t build_post_table(const tt_glyph_outline_t *outlines,
                                     size_t num_outlines,
                                     uint8_t **out_data,
                                     size_t *out_length) {
  size_t glyph_index;
  size_t total_length = POST_TABLE_HEADER_LENGTH + 2u;
  size_t custom_name_count = 0u;
  uint8_t *data;

  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    char generated_name[32];
    const char *name = resolve_glyph_name(&outlines[glyph_index], glyph_index,
                                          generated_name,
                                          sizeof(generated_name));
    if (!is_post_format_2_name_usable(name)) {
      data = (uint8_t *)calloc(POST_TABLE_HEADER_LENGTH, 1u);
      if (data == NULL) {
        return EOT_ERR_ALLOCATION;
      }
      write_u32be(data, POST_FORMAT_3);
      *out_data = data;
      *out_length = POST_TABLE_HEADER_LENGTH;
      return EOT_OK;
    }

    if (strcmp(name, ".notdef") == 0) {
      continue;
    }

    total_length += 1u + strlen(name);
    custom_name_count++;
  }

  total_length += num_outlines * 2u;
  data = (uint8_t *)calloc(total_length, 1u);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  write_u32be(data, POST_FORMAT_2);
  write_u16be(data + 32u, (uint16_t)num_outlines);

  size_t name_index_offset = 34u;
  size_t string_offset = 34u + num_outlines * 2u;
  uint16_t next_custom_name_index = 258u;

  (void)custom_name_count;
  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    char generated_name[32];
    const char *name = resolve_glyph_name(&outlines[glyph_index], glyph_index,
                                          generated_name,
                                          sizeof(generated_name));

    if (strcmp(name, ".notdef") == 0) {
      write_u16be(data + name_index_offset, 0u);
    } else {
      size_t name_length = strlen(name);
      write_u16be(data + name_index_offset, next_custom_name_index++);
      data[string_offset++] = (uint8_t)name_length;
      memcpy(data + string_offset, name, name_length);
      string_offset += name_length;
    }

    name_index_offset += 2u;
  }

  *out_data = data;
  *out_length = total_length;
  return EOT_OK;
}

static void free_tables(uint8_t **tables, size_t count) {
  size_t i;
  for (i = 0u; i < count; i++) {
    free(tables[i]);
    tables[i] = NULL;
  }
}

static eot_status_t add_table_and_release(sfnt_font_t *font, uint32_t tag,
                                          uint8_t **data, size_t length) {
  eot_status_t status = sfnt_font_add_table(font, tag, *data, length);
  free(*data);
  *data = NULL;
  return status;
}

extern "C" eot_status_t tt_rebuilder_build_font(
    const tt_glyph_outline_t *outlines, size_t num_outlines,
    sfnt_font_t *out_font) {
  glyph_metrics_t *metrics = NULL;
  uint8_t *head_data = NULL;
  size_t head_length = 0u;
  uint8_t *hhea_data = NULL;
  size_t hhea_length = 0u;
  uint8_t *hmtx_data = NULL;
  size_t hmtx_length = 0u;
  uint8_t *maxp_data = NULL;
  size_t maxp_length = 0u;
  uint8_t *glyf_data = NULL;
  size_t glyf_length = 0u;
  uint8_t *loca_data = NULL;
  size_t loca_length = 0u;
  uint8_t *post_data = NULL;
  size_t post_length = 0u;
  int16_t index_to_loca_format = 0;
  eot_status_t status = EOT_OK;
  size_t glyph_index;

  if (outlines == NULL || out_font == NULL || num_outlines == 0u ||
      num_outlines > UINT16_MAX) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  metrics =
      (glyph_metrics_t *)calloc(num_outlines, sizeof(glyph_metrics_t));
  if (metrics == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  for (glyph_index = 0u; glyph_index < num_outlines; glyph_index++) {
    status = compute_glyph_metrics(&outlines[glyph_index], &metrics[glyph_index]);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  status = build_glyf_and_loca_tables(outlines, metrics, num_outlines, &glyf_data,
                                      &glyf_length, &loca_data, &loca_length,
                                      &index_to_loca_format);
  if (status == EOT_OK) {
    status = build_head_table(metrics, num_outlines, index_to_loca_format,
                              &head_data, &head_length);
  }
  if (status == EOT_OK) {
    status = build_hhea_table(metrics, num_outlines, &hhea_data, &hhea_length);
  }
  if (status == EOT_OK) {
    status = build_hmtx_table(metrics, num_outlines, &hmtx_data, &hmtx_length);
  }
  if (status == EOT_OK) {
    status = build_maxp_table(metrics, num_outlines, &maxp_data, &maxp_length);
  }
  if (status == EOT_OK) {
    status = build_post_table(outlines, num_outlines, &post_data, &post_length);
  }
  if (status != EOT_OK) {
    goto cleanup;
  }

  sfnt_font_init(out_font);
  status = add_table_and_release(out_font, TAG_head, &head_data, head_length);
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_hhea, &hhea_data, hhea_length);
  }
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_hmtx, &hmtx_data, hmtx_length);
  }
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_maxp, &maxp_data, maxp_length);
  }
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_glyf, &glyf_data, glyf_length);
  }
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_loca, &loca_data, loca_length);
  }
  if (status == EOT_OK) {
    status = add_table_and_release(out_font, TAG_post, &post_data, post_length);
  }
  if (status != EOT_OK) {
    sfnt_font_destroy(out_font);
  }

cleanup:
  if (status != EOT_OK) {
    uint8_t *tables[] = {
        head_data, hhea_data, hmtx_data, maxp_data, glyf_data, loca_data,
        post_data,
    };
    free_tables(tables, sizeof(tables) / sizeof(tables[0]));
  }
  free(metrics);
  return status;
}
