#include "sfnt_subset.h"

#include <ctype.h>
#include <stdlib.h>
#include <string.h>

#include "byte_io.h"
#include "parallel_runtime.h"
#include "subset_backend_harfbuzz.h"
#include "table_policy.h"

#define TAG_OS_2 0x4f532f32u
#define TAG_cmap 0x636d6170u
#define TAG_cvt 0x63767420u
#define TAG_glyf 0x676c7966u
#define TAG_hdmx 0x68646d78u
#define TAG_head 0x68656164u
#define TAG_hhea 0x68686561u
#define TAG_hmtx 0x686d7478u
#define TAG_loca 0x6c6f6361u
#define TAG_maxp 0x6d617870u
#define TAG_name 0x6e616d65u
#define TAG_post 0x706f7374u
#define TAG_VDMX 0x56444d58u

#define CMAP_FORMAT_4 4
#define CMAP_FORMAT_12 12

#define GLYF_COMPOSITE_ARG_WORDS 0x0001
#define GLYF_COMPOSITE_HAVE_SCALE 0x0008
#define GLYF_COMPOSITE_MORE_COMPONENTS 0x0020
#define GLYF_COMPOSITE_HAVE_XY_SCALE 0x0040
#define GLYF_COMPOSITE_HAVE_TWO_BY_TWO 0x0080
#define GLYF_COMPOSITE_HAVE_INSTRUCTIONS 0x0100

typedef struct {
  buffer_view_t data;
  uint16_t format;
} cmap_lookup_t;

typedef struct {
  uint16_t advance_width;
  int16_t left_side_bearing;
} long_hor_metric_t;

typedef struct {
  uint32_t codepoint;
  uint16_t glyph_id;
} cmap_entry_t;

typedef struct {
  uint16_t start_code;
  uint16_t end_code;
  int16_t delta;
} cmap4_segment_t;

typedef struct {
  uint32_t start_code;
  uint32_t end_code;
  uint32_t start_glyph_id;
} cmap12_group_t;

typedef struct {
  const sfnt_font_t *input;
  const subset_plan_t *plan;
  sfnt_font_t *output;
  subset_warnings_t *warnings;
  eot_status_t status;
} subset_font_task_t;

static sfnt_table_t *get_table(const sfnt_font_t *font, uint32_t tag) {
  return sfnt_font_get_table((sfnt_font_t *)font, tag);
}

static int16_t read_s16be_local(const uint8_t *data) {
  return (int16_t)read_u16be(data);
}

static const char *skip_ascii_space(const char *cursor) {
  while (*cursor != '\0' && isspace((unsigned char)*cursor)) {
    cursor++;
  }
  return cursor;
}

static int parse_decimal_value(const char **cursor, uint32_t *out_value) {
  uint32_t value = 0;
  int has_digit = 0;

  while (**cursor >= '0' && **cursor <= '9') {
    uint32_t digit = (uint32_t)(**cursor - '0');
    has_digit = 1;
    if (value > UINT32_MAX / 10u ||
        (value == UINT32_MAX / 10u && digit > UINT32_MAX % 10u)) {
      return 0;
    }
    value = value * 10u + digit;
    (*cursor)++;
  }

  if (!has_digit) {
    return 0;
  }

  *out_value = value;
  return 1;
}

static int parse_hex_value(const char **cursor, uint32_t *out_value) {
  uint32_t value = 0;
  int has_digit = 0;

  while (**cursor != '\0') {
    char ch = **cursor;
    uint32_t digit;

    if (ch >= '0' && ch <= '9') {
      digit = (uint32_t)(ch - '0');
    } else if (ch >= 'A' && ch <= 'F') {
      digit = (uint32_t)(ch - 'A') + 10u;
    } else if (ch >= 'a' && ch <= 'f') {
      digit = (uint32_t)(ch - 'a') + 10u;
    } else {
      break;
    }

    has_digit = 1;
    if (value > (UINT32_MAX >> 4)) {
      return 0;
    }
    value = (value << 4) | digit;
    (*cursor)++;
  }

  if (!has_digit) {
    return 0;
  }

  *out_value = value;
  return 1;
}

static eot_status_t decode_utf8_codepoint(const char **cursor,
                                          uint32_t *out_codepoint) {
  const unsigned char *text = (const unsigned char *)*cursor;
  uint32_t codepoint;
  size_t length;
  size_t remaining;

  remaining = strlen(*cursor);
  if (remaining == 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (text[0] < 0x80) {
    codepoint = text[0];
    length = 1;
  } else if ((text[0] & 0xE0) == 0xC0) {
    if (remaining < 2) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    if ((text[1] & 0xC0) != 0x80) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    codepoint = ((uint32_t)(text[0] & 0x1F) << 6) |
                (uint32_t)(text[1] & 0x3F);
    if (codepoint < 0x80) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    length = 2;
  } else if ((text[0] & 0xF0) == 0xE0) {
    if (remaining < 3) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    if ((text[1] & 0xC0) != 0x80 || (text[2] & 0xC0) != 0x80) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    codepoint = ((uint32_t)(text[0] & 0x0F) << 12) |
                ((uint32_t)(text[1] & 0x3F) << 6) |
                (uint32_t)(text[2] & 0x3F);
    if (codepoint < 0x800 || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    length = 3;
  } else if ((text[0] & 0xF8) == 0xF0) {
    if (remaining < 4) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    if ((text[1] & 0xC0) != 0x80 ||
        (text[2] & 0xC0) != 0x80 ||
        (text[3] & 0xC0) != 0x80) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    codepoint = ((uint32_t)(text[0] & 0x07) << 18) |
                ((uint32_t)(text[1] & 0x3F) << 12) |
                ((uint32_t)(text[2] & 0x3F) << 6) |
                (uint32_t)(text[3] & 0x3F);
    if (codepoint < 0x10000 || codepoint > 0x10FFFF) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    length = 4;
  } else {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  *cursor += length;
  *out_codepoint = codepoint;
  return EOT_OK;
}

static int is_valid_unicode_scalar(uint32_t codepoint) {
  return codepoint <= 0x10FFFFu &&
         (codepoint < 0xD800u || codepoint > 0xDFFFu);
}

static eot_status_t get_num_glyphs(const sfnt_font_t *font, uint16_t *out_num_glyphs) {
  sfnt_table_t *maxp = get_table(font, TAG_maxp);

  if (maxp == NULL || maxp->length < 6) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_num_glyphs = read_u16be(maxp->data + 4);
  if (*out_num_glyphs == 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_OK;
}

static eot_status_t get_index_to_loca_format(const sfnt_font_t *font,
                                             int16_t *out_format) {
  sfnt_table_t *head = get_table(font, TAG_head);

  if (head == NULL || head->length < 52) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_format = read_s16be_local(head->data + 50);
  if (*out_format != 0 && *out_format != 1) {
    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_OK;
}

static eot_status_t get_glyph_range(const sfnt_font_t *font, uint16_t glyph_id,
                                    uint16_t num_glyphs, size_t *out_offset,
                                    size_t *out_length) {
  sfnt_table_t *loca;
  sfnt_table_t *glyf;
  int16_t index_to_loca_format;
  size_t loca_entry_offset;
  size_t start;
  size_t end;
  eot_status_t status;

  if (glyph_id >= num_glyphs) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  loca = get_table(font, TAG_loca);
  glyf = get_table(font, TAG_glyf);
  if (loca == NULL || glyf == NULL) {
    return EOT_ERR_CORRUPT_DATA;
  }

  status = get_index_to_loca_format(font, &index_to_loca_format);
  if (status != EOT_OK) {
    return status;
  }

  if (index_to_loca_format == 0) {
    loca_entry_offset = (size_t)glyph_id * 2;
    if (!buffer_view_has_range(buffer_view_make(loca->data, loca->length), loca_entry_offset, 4)) {
      return EOT_ERR_CORRUPT_DATA;
    }
    start = (size_t)read_u16be(loca->data + loca_entry_offset) * 2u;
    end = (size_t)read_u16be(loca->data + loca_entry_offset + 2) * 2u;
  } else {
    loca_entry_offset = (size_t)glyph_id * 4;
    if (!buffer_view_has_range(buffer_view_make(loca->data, loca->length), loca_entry_offset, 8)) {
      return EOT_ERR_CORRUPT_DATA;
    }
    start = (size_t)read_u32be(loca->data + loca_entry_offset);
    end = (size_t)read_u32be(loca->data + loca_entry_offset + 4);
  }

  if (end < start || end > glyf->length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_offset = start;
  *out_length = end - start;
  return EOT_OK;
}

static eot_status_t init_best_cmap_lookup(const sfnt_font_t *font,
                                          int allow_symbol_format4,
                                          cmap_lookup_t *out_cmap) {
  sfnt_table_t *cmap = get_table(font, TAG_cmap);
  buffer_view_t cmap_view;
  uint16_t num_tables;
  size_t best_offset = 0;
  uint16_t best_format = 0;
  int best_score = -1;
  size_t i;

  if (cmap == NULL || cmap->length < 4) {
    return EOT_ERR_CORRUPT_DATA;
  }

  cmap_view = buffer_view_make(cmap->data, cmap->length);
  num_tables = read_u16be(cmap->data + 2);
  if (!buffer_view_has_range(cmap_view, 4, (size_t)num_tables * 8)) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (i = 0; i < num_tables; i++) {
    const uint8_t *record = cmap->data + 4 + i * 8;
    uint16_t platform_id = read_u16be(record);
    uint16_t encoding_id = read_u16be(record + 2);
    uint32_t subtable_offset = read_u32be(record + 4);
    uint16_t format;
    int score = -1;

    if (!buffer_view_has_range(cmap_view, subtable_offset, 2)) {
      return EOT_ERR_CORRUPT_DATA;
    }

    format = read_u16be(cmap->data + subtable_offset);
    if (format == CMAP_FORMAT_12) {
      if (platform_id == 3 && encoding_id == 10) {
        score = 4;
      } else if (platform_id == 0) {
        score = 3;
      }
    } else if (format == CMAP_FORMAT_4) {
      if (platform_id == 3 && encoding_id == 1) {
        score = 2;
      } else if (platform_id == 0) {
        score = 1;
      } else if (allow_symbol_format4 &&
                 platform_id == 3 && encoding_id == 0) {
        score = 0;
      }
    }

    if (score > best_score) {
      best_offset = subtable_offset;
      best_format = format;
      best_score = score;
    }
  }

  if (best_score < 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  out_cmap->data = buffer_view_make(cmap->data + best_offset, cmap->length - best_offset);
  out_cmap->format = best_format;
  return EOT_OK;
}

static eot_status_t cmap_lookup_format4(cmap_lookup_t cmap, uint32_t codepoint,
                                        uint16_t *out_gid) {
  const uint8_t *data = cmap.data.data;
  size_t length;
  uint16_t seg_count;
  size_t end_codes_offset;
  size_t start_codes_offset;
  size_t id_deltas_offset;
  size_t id_range_offsets_offset;
  uint16_t i;

  *out_gid = 0;
  if (codepoint > 0xFFFF || cmap.data.length < 16) {
    return EOT_OK;
  }

  length = read_u16be(data + 2);
  if (length > cmap.data.length || length < 16) {
    return EOT_ERR_CORRUPT_DATA;
  }

  seg_count = (uint16_t)(read_u16be(data + 6) / 2);
  end_codes_offset = 14;
  start_codes_offset = end_codes_offset + (size_t)seg_count * 2 + 2;
  id_deltas_offset = start_codes_offset + (size_t)seg_count * 2;
  id_range_offsets_offset = id_deltas_offset + (size_t)seg_count * 2;

  if (id_range_offsets_offset + (size_t)seg_count * 2 > length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (i = 0; i < seg_count; i++) {
    uint16_t end_code = read_u16be(data + end_codes_offset + (size_t)i * 2);
    uint16_t start_code;
    uint16_t id_range_offset;
    uint16_t glyph;
    int16_t id_delta;

    if (codepoint > end_code) {
      continue;
    }

    start_code = read_u16be(data + start_codes_offset + (size_t)i * 2);
    if (codepoint < start_code) {
      return EOT_OK;
    }

    id_delta = read_s16be_local(data + id_deltas_offset + (size_t)i * 2);
    id_range_offset = read_u16be(data + id_range_offsets_offset + (size_t)i * 2);
    if (id_range_offset == 0) {
      *out_gid = (uint16_t)((codepoint + (uint32_t)id_delta) & 0xFFFFu);
      return EOT_OK;
    }

    {
      size_t id_range_pos = id_range_offsets_offset + (size_t)i * 2;
      size_t glyph_index_pos = id_range_pos + id_range_offset +
                               (size_t)(codepoint - start_code) * 2;
      if (glyph_index_pos + 2 > length) {
        return EOT_ERR_CORRUPT_DATA;
      }
      glyph = read_u16be(data + glyph_index_pos);
      if (glyph == 0) {
        *out_gid = 0;
      } else {
        *out_gid = (uint16_t)((glyph + (uint16_t)id_delta) & 0xFFFFu);
      }
      return EOT_OK;
    }
  }

  return EOT_OK;
}

static eot_status_t cmap_lookup_format12(cmap_lookup_t cmap, uint32_t codepoint,
                                         uint16_t *out_gid) {
  const uint8_t *data = cmap.data.data;
  uint32_t length;
  uint32_t num_groups;
  uint32_t i;

  *out_gid = 0;
  if (cmap.data.length < 16) {
    return EOT_ERR_CORRUPT_DATA;
  }

  length = read_u32be(data + 4);
  if (length > cmap.data.length || length < 16) {
    return EOT_ERR_CORRUPT_DATA;
  }

  num_groups = read_u32be(data + 12);
  if ((size_t)length < 16u + (size_t)num_groups * 12u) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (i = 0; i < num_groups; i++) {
    const uint8_t *group = data + 16 + (size_t)i * 12;
    uint32_t start_char = read_u32be(group);
    uint32_t end_char = read_u32be(group + 4);

    if (codepoint < start_char) {
      return EOT_OK;
    }
    if (codepoint <= end_char) {
      uint32_t glyph = read_u32be(group + 8) + (codepoint - start_char);
      if (glyph > UINT16_MAX) {
        return EOT_ERR_CORRUPT_DATA;
      }
      *out_gid = (uint16_t)glyph;
      return EOT_OK;
    }
  }

  return EOT_OK;
}

static eot_status_t cmap_lookup_gid(cmap_lookup_t cmap, uint32_t codepoint,
                                    uint16_t *out_gid) {
  if (cmap.format == CMAP_FORMAT_12) {
    return cmap_lookup_format12(cmap, codepoint, out_gid);
  }
  if (cmap.format == CMAP_FORMAT_4) {
    return cmap_lookup_format4(cmap, codepoint, out_gid);
  }
  return EOT_ERR_CORRUPT_DATA;
}

static eot_status_t mark_gid(uint8_t *included, uint16_t num_glyphs, uint16_t gid) {
  if (gid >= num_glyphs) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  included[gid] = 1;
  return EOT_OK;
}

static eot_status_t add_text_selection(const char *text, cmap_lookup_t cmap,
                                       uint8_t *included, uint16_t num_glyphs) {
  const char *cursor;

  if (text == NULL || *text == '\0') {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  cursor = text;
  while (*cursor != '\0') {
    uint32_t codepoint;
    uint16_t gid;
    eot_status_t status = decode_utf8_codepoint(&cursor, &codepoint);
    if (status != EOT_OK) {
      return status;
    }

    status = cmap_lookup_gid(cmap, codepoint, &gid);
    if (status != EOT_OK) {
      return status;
    }
    if (gid >= num_glyphs) {
      return EOT_ERR_CORRUPT_DATA;
    }
    included[gid] = 1;
  }

  return EOT_OK;
}

static eot_status_t add_unicode_selection(const char *selection, cmap_lookup_t cmap,
                                          uint8_t *included, uint16_t num_glyphs) {
  const char *cursor;
  int parsed_any = 0;

  if (selection == NULL || *selection == '\0') {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  cursor = selection;
  while (1) {
    uint32_t range_start;
    uint32_t range_end;

    cursor = skip_ascii_space(cursor);
    if (*cursor == '\0') {
      break;
    }

    if (cursor[0] != 'U' && cursor[0] != 'u') {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    cursor++;
    if (*cursor != '+') {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    cursor++;
    if (!parse_hex_value(&cursor, &range_start)) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    range_end = range_start;

    if (*cursor == '-') {
      cursor++;
      if (!parse_hex_value(&cursor, &range_end) || range_end < range_start) {
        return EOT_ERR_INVALID_ARGUMENT;
      }
    }
    if (!is_valid_unicode_scalar(range_start) ||
        !is_valid_unicode_scalar(range_end)) {
      return EOT_ERR_INVALID_ARGUMENT;
    }

    while (range_start <= range_end) {
      uint16_t gid;
      eot_status_t status = cmap_lookup_gid(cmap, range_start, &gid);
      if (status != EOT_OK) {
        return status;
      }
      if (gid >= num_glyphs) {
        return EOT_ERR_CORRUPT_DATA;
      }
      included[gid] = 1;
      range_start++;
    }

    parsed_any = 1;
    cursor = skip_ascii_space(cursor);
    if (*cursor == '\0') {
      break;
    }
    if (*cursor != ',') {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    cursor++;
  }

  return parsed_any ? EOT_OK : EOT_ERR_INVALID_ARGUMENT;
}

static eot_status_t add_glyph_id_selection(const char *selection, uint8_t *included,
                                           uint16_t num_glyphs) {
  const char *cursor;
  int parsed_any = 0;

  if (selection == NULL || *selection == '\0') {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  cursor = selection;
  while (1) {
    uint32_t value;

    cursor = skip_ascii_space(cursor);
    if (*cursor == '\0') {
      break;
    }
    if (!parse_decimal_value(&cursor, &value) || value >= num_glyphs) {
      return EOT_ERR_INVALID_ARGUMENT;
    }

    included[value] = 1;
    parsed_any = 1;

    cursor = skip_ascii_space(cursor);
    if (*cursor == '\0') {
      break;
    }
    if (*cursor != ',') {
      return EOT_ERR_INVALID_ARGUMENT;
    }
    cursor++;
  }

  return parsed_any ? EOT_OK : EOT_ERR_INVALID_ARGUMENT;
}

static eot_status_t enqueue_initial_glyphs(const uint8_t *included,
                                           uint16_t num_glyphs,
                                           uint16_t *queue,
                                           size_t *out_queue_length) {
  size_t queue_length = 0;
  uint16_t gid;

  for (gid = 0; gid < num_glyphs; gid++) {
    if (included[gid]) {
      queue[queue_length++] = gid;
    }
  }

  *out_queue_length = queue_length;
  return EOT_OK;
}

static eot_status_t add_composite_closure(const sfnt_font_t *font, uint16_t num_glyphs,
                                          uint8_t *included,
                                          size_t *out_added_dependencies) {
  uint16_t *queue;
  size_t head = 0;
  size_t tail = 0;
  eot_status_t status;

  queue = (uint16_t *)malloc((size_t)num_glyphs * sizeof(uint16_t));
  if (queue == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  status = enqueue_initial_glyphs(included, num_glyphs, queue, &tail);
  if (status != EOT_OK) {
    free(queue);
    return status;
  }

  while (head < tail) {
    uint16_t glyph_id = queue[head++];
    size_t glyph_offset;
    size_t glyph_length;
    sfnt_table_t *glyf = get_table(font, TAG_glyf);
    const uint8_t *glyph_data;
    size_t position = 0;
    uint16_t flags = 0;

    status = get_glyph_range(font, glyph_id, num_glyphs, &glyph_offset, &glyph_length);
    if (status != EOT_OK) {
      free(queue);
      return status;
    }

    if (glyph_length == 0) {
      continue;
    }
    if (glyph_length < 10) {
      free(queue);
      return EOT_ERR_CORRUPT_DATA;
    }

    glyph_data = glyf->data + glyph_offset;
    if (read_s16be_local(glyph_data) >= 0) {
      continue;
    }

    position = 10;
    do {
      uint16_t component_gid;

      if (position + 4 > glyph_length) {
        free(queue);
        return EOT_ERR_CORRUPT_DATA;
      }

      flags = read_u16be(glyph_data + position);
      component_gid = read_u16be(glyph_data + position + 2);
      position += 4;

      if (component_gid >= num_glyphs) {
        free(queue);
        return EOT_ERR_CORRUPT_DATA;
      }
      if (!included[component_gid]) {
        included[component_gid] = 1;
        queue[tail++] = component_gid;
        (*out_added_dependencies)++;
      }

      position += (flags & GLYF_COMPOSITE_ARG_WORDS) ? 4u : 2u;
      if (flags & GLYF_COMPOSITE_HAVE_SCALE) {
        position += 2;
      } else if (flags & GLYF_COMPOSITE_HAVE_XY_SCALE) {
        position += 4;
      } else if (flags & GLYF_COMPOSITE_HAVE_TWO_BY_TWO) {
        position += 8;
      }

      if (position > glyph_length) {
        free(queue);
        return EOT_ERR_CORRUPT_DATA;
      }
    } while ((flags & GLYF_COMPOSITE_MORE_COMPONENTS) != 0);

    if (flags & GLYF_COMPOSITE_HAVE_INSTRUCTIONS) {
      uint16_t instruction_length;
      if (position + 2 > glyph_length) {
        free(queue);
        return EOT_ERR_CORRUPT_DATA;
      }
      instruction_length = read_u16be(glyph_data + position);
      position += 2u + instruction_length;
      if (position > glyph_length) {
        free(queue);
        return EOT_ERR_CORRUPT_DATA;
      }
    }
  }

  free(queue);
  return EOT_OK;
}

static eot_status_t build_plan(const uint8_t *included, uint16_t num_glyphs,
                               int keep_gids, size_t added_dependencies,
                               subset_plan_t *out_plan) {
  subset_plan_t plan;
  uint16_t gid;
  size_t count = 0;
  uint16_t dense_gid = 0;

  subset_plan_init(&plan);

  for (gid = 0; gid < num_glyphs; gid++) {
    if (included[gid]) {
      count++;
    }
  }

  plan.included_glyph_ids = (uint16_t *)malloc(count * sizeof(uint16_t));
  plan.old_to_new_gid = (uint16_t *)malloc((size_t)num_glyphs * sizeof(uint16_t));
  if ((count > 0 && plan.included_glyph_ids == NULL) || plan.old_to_new_gid == NULL) {
    subset_plan_destroy(&plan);
    return EOT_ERR_ALLOCATION;
  }

  for (gid = 0; gid < num_glyphs; gid++) {
    plan.old_to_new_gid[gid] = SUBSET_GID_NOT_INCLUDED;
  }

  for (gid = 0; gid < num_glyphs; gid++) {
    if (!included[gid]) {
      continue;
    }
    plan.included_glyph_ids[dense_gid] = gid;
    plan.old_to_new_gid[gid] = keep_gids ? gid : dense_gid;
    dense_gid++;
  }

  plan.num_glyphs = count;
  plan.total_input_glyphs = num_glyphs;
  plan.added_composite_dependencies = added_dependencies;
  plan.keep_gids = keep_gids;
  *out_plan = plan;
  return EOT_OK;
}

static int highest_power_of_two_at_most(int value) {
  int result = 1;

  while (result <= value / 2) {
    result <<= 1;
  }
  return result;
}

static int floor_log2_positive(int value) {
  int result = 0;

  while (value > 1) {
    value >>= 1;
    result++;
  }
  return result;
}

static int plan_includes_gid(const subset_plan_t *plan, uint16_t gid) {
  return gid < plan->total_input_glyphs &&
         plan->old_to_new_gid[gid] != SUBSET_GID_NOT_INCLUDED;
}

static uint16_t output_num_glyphs_for_plan(const subset_plan_t *plan) {
  return (uint16_t)(plan->keep_gids ? plan->total_input_glyphs : plan->num_glyphs);
}

static uint16_t source_gid_for_output_gid(const subset_plan_t *plan, uint16_t output_gid) {
  return plan->keep_gids ? output_gid : plan->included_glyph_ids[output_gid];
}

static eot_status_t get_number_of_hmetrics(const sfnt_font_t *font,
                                           uint16_t *out_number_of_hmetrics) {
  sfnt_table_t *hhea = get_table(font, TAG_hhea);

  if (hhea == NULL || hhea->length < 36) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_number_of_hmetrics = read_u16be(hhea->data + 34);
  if (*out_number_of_hmetrics == 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_OK;
}

static eot_status_t read_hmetric(const sfnt_font_t *font, uint16_t glyph_id,
                                 uint16_t *out_advance_width,
                                 int16_t *out_left_side_bearing) {
  sfnt_table_t *hmtx = get_table(font, TAG_hmtx);
  uint16_t num_glyphs;
  uint16_t number_of_hmetrics;
  size_t metric_offset;
  eot_status_t status;

  if (hmtx == NULL) {
    return EOT_ERR_CORRUPT_DATA;
  }

  status = get_num_glyphs(font, &num_glyphs);
  if (status != EOT_OK) {
    return status;
  }
  if (glyph_id >= num_glyphs) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = get_number_of_hmetrics(font, &number_of_hmetrics);
  if (status != EOT_OK) {
    return status;
  }
  if (number_of_hmetrics > num_glyphs ||
      hmtx->length < (size_t)number_of_hmetrics * 4u +
                     (size_t)(num_glyphs - number_of_hmetrics) * 2u) {
    return EOT_ERR_CORRUPT_DATA;
  }

  if (glyph_id < number_of_hmetrics) {
    metric_offset = (size_t)glyph_id * 4u;
    *out_advance_width = read_u16be(hmtx->data + metric_offset);
    *out_left_side_bearing = read_s16be_local(hmtx->data + metric_offset + 2u);
  } else {
    metric_offset = (size_t)(number_of_hmetrics - 1u) * 4u;
    *out_advance_width = read_u16be(hmtx->data + metric_offset);
    metric_offset = (size_t)number_of_hmetrics * 4u +
                    (size_t)(glyph_id - number_of_hmetrics) * 2u;
    *out_left_side_bearing = read_s16be_local(hmtx->data + metric_offset);
  }

  return EOT_OK;
}

static eot_status_t copy_table(const sfnt_font_t *input, sfnt_font_t *output, uint32_t tag) {
  sfnt_table_t *table = get_table(input, tag);

  if (table == NULL) {
    return EOT_ERR_CORRUPT_DATA;
  }
  return sfnt_font_add_table(output, tag, table->data, table->length);
}

static void set_subset_warning_flag(subset_warnings_t *warnings, uint32_t tag) {
  if (warnings == NULL) {
    return;
  }

  if (tag == TAG_hdmx) {
    warnings->dropped_hdmx = 1;
  } else if (tag == TAG_VDMX) {
    warnings->dropped_vdmx = 1;
  }
}

eot_status_t sfnt_subset_apply_output_table_policy(sfnt_font_t *output,
                                                   subset_warnings_t *warnings) {
  static const uint32_t extra_tags[] = {TAG_cvt, TAG_hdmx, TAG_VDMX};
  size_t i;

  if (output == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (i = 0; i < sizeof(extra_tags) / sizeof(extra_tags[0]); i++) {
    uint32_t tag = extra_tags[i];

    if (subset_table_policy_for_tag(tag) != TABLE_POLICY_DROP_WITH_WARNING ||
        !sfnt_font_has_table(output, tag)) {
      continue;
    }

    set_subset_warning_flag(warnings, tag);
    {
      eot_status_t status = sfnt_font_remove_table(output, tag);
      if (status != EOT_OK) {
        return status;
      }
    }
  }

  return EOT_OK;
}

static eot_status_t run_subset_font_task(void *task_context) {
  subset_font_task_t *task = (subset_font_task_t *)task_context;

  if (task == NULL || task->input == NULL || task->plan == NULL ||
      task->output == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  task->status = subset_backend_harfbuzz_run(task->input, task->plan,
                                             task->output, task->warnings);
  if (task->status == EOT_OK) {
    task->status = sfnt_subset_apply_output_table_policy(task->output,
                                                         task->warnings);
  }
  return task->status;
}

static eot_status_t build_head_table(const sfnt_font_t *input, uint8_t **out_data,
                                     size_t *out_length) {
  sfnt_table_t *head = get_table(input, TAG_head);
  uint8_t *data;

  if (head == NULL || head->length < 52) {
    return EOT_ERR_CORRUPT_DATA;
  }

  data = (uint8_t *)malloc(head->length);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }
  memcpy(data, head->data, head->length);
  write_u32be(data + 8, 0u);

  *out_data = data;
  *out_length = head->length;
  return EOT_OK;
}

static eot_status_t build_maxp_table(const sfnt_font_t *input, uint16_t num_glyphs,
                                     uint8_t **out_data, size_t *out_length) {
  sfnt_table_t *maxp = get_table(input, TAG_maxp);
  uint8_t *data;

  if (maxp == NULL || maxp->length < 6) {
    return EOT_ERR_CORRUPT_DATA;
  }

  data = (uint8_t *)malloc(maxp->length);
  if (data == NULL) {
    return EOT_ERR_ALLOCATION;
  }
  memcpy(data, maxp->data, maxp->length);
  write_u16be(data + 4, num_glyphs);

  *out_data = data;
  *out_length = maxp->length;
  return EOT_OK;
}

static eot_status_t build_hmtx_and_hhea_tables(const sfnt_font_t *input,
                                               const subset_plan_t *plan,
                                               uint8_t **out_hmtx_data,
                                               size_t *out_hmtx_length,
                                               uint8_t **out_hhea_data,
                                               size_t *out_hhea_length) {
  sfnt_table_t *hhea = get_table(input, TAG_hhea);
  uint16_t output_num_glyphs = output_num_glyphs_for_plan(plan);
  long_hor_metric_t *metrics = NULL;
  uint8_t *hmtx_data = NULL;
  uint8_t *hhea_data = NULL;
  uint16_t output_num_hmetrics = output_num_glyphs;
  size_t hmtx_length;
  uint16_t gid;
  eot_status_t status = EOT_OK;

  if (hhea == NULL || hhea->length < 36) {
    return EOT_ERR_CORRUPT_DATA;
  }

  metrics = (long_hor_metric_t *)malloc((size_t)output_num_glyphs * sizeof(long_hor_metric_t));
  hhea_data = (uint8_t *)malloc(hhea->length);
  if ((output_num_glyphs > 0 && metrics == NULL) || hhea_data == NULL) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  for (gid = 0; gid < output_num_glyphs; gid++) {
    uint16_t source_gid = source_gid_for_output_gid(plan, gid);
    status = read_hmetric(input, source_gid, &metrics[gid].advance_width,
                          &metrics[gid].left_side_bearing);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (output_num_glyphs > 0) {
    uint16_t last_advance_width = metrics[output_num_glyphs - 1u].advance_width;
    while (output_num_hmetrics > 1u &&
           metrics[output_num_hmetrics - 2u].advance_width == last_advance_width) {
      output_num_hmetrics--;
    }
  }

  hmtx_length = (size_t)output_num_hmetrics * 4u +
                (size_t)(output_num_glyphs - output_num_hmetrics) * 2u;
  hmtx_data = (uint8_t *)malloc(hmtx_length);
  if (hmtx_data == NULL) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  for (gid = 0; gid < output_num_hmetrics; gid++) {
    size_t metric_offset = (size_t)gid * 4u;
    write_u16be(hmtx_data + metric_offset, metrics[gid].advance_width);
    write_u16be(hmtx_data + metric_offset + 2u,
                (uint16_t)metrics[gid].left_side_bearing);
  }
  for (gid = output_num_hmetrics; gid < output_num_glyphs; gid++) {
    size_t lsb_offset = (size_t)output_num_hmetrics * 4u +
                        (size_t)(gid - output_num_hmetrics) * 2u;
    write_u16be(hmtx_data + lsb_offset, (uint16_t)metrics[gid].left_side_bearing);
  }

  memcpy(hhea_data, hhea->data, hhea->length);
  write_u16be(hhea_data + 34, output_num_hmetrics);

  *out_hmtx_data = hmtx_data;
  *out_hmtx_length = hmtx_length;
  *out_hhea_data = hhea_data;
  *out_hhea_length = hhea->length;

cleanup:
  if (status != EOT_OK) {
    free(hmtx_data);
    free(hhea_data);
  }
  free(metrics);
  return status;
}

static eot_status_t renumber_glyph_data(const uint8_t *glyph_data, size_t glyph_length,
                                        const subset_plan_t *plan,
                                        uint8_t **out_glyph_data) {
  uint8_t *copy;
  size_t position = 10;
  uint16_t flags = 0;

  if (glyph_length == 0) {
    *out_glyph_data = NULL;
    return EOT_OK;
  }
  if (glyph_length < 10) {
    return EOT_ERR_CORRUPT_DATA;
  }

  copy = (uint8_t *)malloc(glyph_length);
  if (copy == NULL) {
    return EOT_ERR_ALLOCATION;
  }
  memcpy(copy, glyph_data, glyph_length);

  if (plan->keep_gids || read_s16be_local(copy) >= 0) {
    *out_glyph_data = copy;
    return EOT_OK;
  }

  do {
    uint16_t component_gid;
    uint16_t new_component_gid;

    if (position + 4u > glyph_length) {
      free(copy);
      return EOT_ERR_CORRUPT_DATA;
    }

    flags = read_u16be(copy + position);
    component_gid = read_u16be(copy + position + 2u);
    if (component_gid >= plan->total_input_glyphs ||
        plan->old_to_new_gid[component_gid] == SUBSET_GID_NOT_INCLUDED) {
      free(copy);
      return EOT_ERR_CORRUPT_DATA;
    }
    new_component_gid = plan->old_to_new_gid[component_gid];
    write_u16be(copy + position + 2u, new_component_gid);
    position += 4u;

    position += (flags & GLYF_COMPOSITE_ARG_WORDS) ? 4u : 2u;
    if (flags & GLYF_COMPOSITE_HAVE_SCALE) {
      position += 2u;
    } else if (flags & GLYF_COMPOSITE_HAVE_XY_SCALE) {
      position += 4u;
    } else if (flags & GLYF_COMPOSITE_HAVE_TWO_BY_TWO) {
      position += 8u;
    }

    if (position > glyph_length) {
      free(copy);
      return EOT_ERR_CORRUPT_DATA;
    }
  } while ((flags & GLYF_COMPOSITE_MORE_COMPONENTS) != 0);

  if (flags & GLYF_COMPOSITE_HAVE_INSTRUCTIONS) {
    uint16_t instruction_length;
    if (position + 2u > glyph_length) {
      free(copy);
      return EOT_ERR_CORRUPT_DATA;
    }
    instruction_length = read_u16be(copy + position);
    position += 2u + instruction_length;
    if (position > glyph_length) {
      free(copy);
      return EOT_ERR_CORRUPT_DATA;
    }
  }

  *out_glyph_data = copy;
  return EOT_OK;
}

static eot_status_t build_glyf_and_loca_tables(const sfnt_font_t *input,
                                               const subset_plan_t *plan,
                                               uint8_t **out_glyf_data,
                                               size_t *out_glyf_length,
                                               uint8_t **out_loca_data,
                                               size_t *out_loca_length) {
  sfnt_table_t *source_glyf = get_table(input, TAG_glyf);
  uint16_t output_num_glyphs = output_num_glyphs_for_plan(plan);
  int16_t index_to_loca_format;
  uint8_t *glyf_data = NULL;
  uint8_t *loca_data = NULL;
  size_t glyf_offset = 0;
  uint16_t gid;
  eot_status_t status;

  if (source_glyf == NULL) {
    return EOT_ERR_CORRUPT_DATA;
  }

  status = get_index_to_loca_format(input, &index_to_loca_format);
  if (status != EOT_OK) {
    return status;
  }

  glyf_data = (uint8_t *)malloc(source_glyf->length > 0 ? source_glyf->length : 1u);
  if (glyf_data == NULL) {
    return EOT_ERR_ALLOCATION;
  }
  loca_data = (uint8_t *)malloc((size_t)(output_num_glyphs + 1u) *
                                (index_to_loca_format == 0 ? 2u : 4u));
  if (loca_data == NULL) {
    free(glyf_data);
    return EOT_ERR_ALLOCATION;
  }

  for (gid = 0; gid < output_num_glyphs; gid++) {
    uint16_t source_gid = source_gid_for_output_gid(plan, gid);
    size_t glyph_offset = 0;
    size_t glyph_length = 0;

    if (index_to_loca_format == 0) {
      if ((glyf_offset & 1u) != 0u || (glyf_offset >> 1) > UINT16_MAX) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      write_u16be(loca_data + (size_t)gid * 2u, (uint16_t)(glyf_offset >> 1));
    } else {
      if (glyf_offset > UINT32_MAX) {
        status = EOT_ERR_CORRUPT_DATA;
        goto cleanup;
      }
      write_u32be(loca_data + (size_t)gid * 4u, (uint32_t)glyf_offset);
    }

    if (plan->keep_gids && !plan_includes_gid(plan, source_gid)) {
      continue;
    }

    status = get_glyph_range(input, source_gid, (uint16_t)plan->total_input_glyphs,
                             &glyph_offset, &glyph_length);
    if (status != EOT_OK) {
      goto cleanup;
    }
    if (glyph_length > 0) {
      uint8_t *glyph_copy = NULL;
      status = renumber_glyph_data(source_glyf->data + glyph_offset, glyph_length,
                                   plan, &glyph_copy);
      if (status != EOT_OK) {
        goto cleanup;
      }
      memcpy(glyf_data + glyf_offset, glyph_copy, glyph_length);
      glyf_offset += glyph_length;
      free(glyph_copy);
    }
  }

  if (index_to_loca_format == 0) {
    if ((glyf_offset & 1u) != 0u || (glyf_offset >> 1) > UINT16_MAX) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    write_u16be(loca_data + (size_t)output_num_glyphs * 2u,
                (uint16_t)(glyf_offset >> 1));
  } else {
    if (glyf_offset > UINT32_MAX) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    write_u32be(loca_data + (size_t)output_num_glyphs * 4u, (uint32_t)glyf_offset);
  }

  *out_glyf_data = glyf_data;
  *out_glyf_length = glyf_offset;
  *out_loca_data = loca_data;
  *out_loca_length = (size_t)(output_num_glyphs + 1u) *
                     (index_to_loca_format == 0 ? 2u : 4u);
  return EOT_OK;

cleanup:
  free(glyf_data);
  free(loca_data);
  return status;
}

static eot_status_t append_cmap_entry(cmap_entry_t **entries, size_t *count,
                                      size_t *capacity, uint32_t codepoint,
                                      uint16_t glyph_id) {
  cmap_entry_t *new_entries;
  size_t new_capacity;

  if (*count > 0 && (*entries)[*count - 1u].codepoint == codepoint) {
    (*entries)[*count - 1u].glyph_id = glyph_id;
    return EOT_OK;
  }

  if (*count == *capacity) {
    new_capacity = *capacity == 0 ? 32u : *capacity * 2u;
    new_entries = (cmap_entry_t *)realloc(*entries, new_capacity * sizeof(cmap_entry_t));
    if (new_entries == NULL) {
      return EOT_ERR_ALLOCATION;
    }
    *entries = new_entries;
    *capacity = new_capacity;
  }

  (*entries)[*count].codepoint = codepoint;
  (*entries)[*count].glyph_id = glyph_id;
  (*count)++;
  return EOT_OK;
}

static eot_status_t collect_cmap_entries_format4(cmap_lookup_t cmap,
                                                 const subset_plan_t *plan,
                                                 cmap_entry_t **entries,
                                                 size_t *count,
                                                 size_t *capacity) {
  const uint8_t *data = cmap.data.data;
  size_t length = read_u16be(data + 2);
  uint16_t seg_count = (uint16_t)(read_u16be(data + 6) / 2u);
  size_t end_codes_offset = 14u;
  size_t start_codes_offset = end_codes_offset + (size_t)seg_count * 2u + 2u;
  size_t id_deltas_offset = start_codes_offset + (size_t)seg_count * 2u;
  size_t id_range_offsets_offset = id_deltas_offset + (size_t)seg_count * 2u;
  uint16_t i;

  if (length > cmap.data.length || length < 16u ||
      id_range_offsets_offset + (size_t)seg_count * 2u > length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (i = 0; i < seg_count; i++) {
    uint16_t start_code = read_u16be(data + start_codes_offset + (size_t)i * 2u);
    uint16_t end_code = read_u16be(data + end_codes_offset + (size_t)i * 2u);
    uint16_t id_range_offset =
        read_u16be(data + id_range_offsets_offset + (size_t)i * 2u);
    int16_t id_delta = read_s16be_local(data + id_deltas_offset + (size_t)i * 2u);
    uint32_t codepoint;

    if (start_code == 0xFFFFu && end_code == 0xFFFFu) {
      break;
    }
    if (end_code < start_code) {
      return EOT_ERR_CORRUPT_DATA;
    }

    for (codepoint = start_code; codepoint <= end_code; codepoint++) {
      uint16_t glyph_id;
      if (id_range_offset == 0) {
        glyph_id = (uint16_t)((codepoint + (uint32_t)id_delta) & 0xFFFFu);
      } else {
        size_t id_range_pos = id_range_offsets_offset + (size_t)i * 2u;
        size_t glyph_index_pos =
            id_range_pos + id_range_offset + (size_t)(codepoint - start_code) * 2u;
        if (glyph_index_pos + 2u > length) {
          return EOT_ERR_CORRUPT_DATA;
        }
        glyph_id = read_u16be(data + glyph_index_pos);
        if (glyph_id != 0) {
          glyph_id = (uint16_t)((glyph_id + (uint16_t)id_delta) & 0xFFFFu);
        }
      }

      if (glyph_id != 0 && glyph_id < plan->total_input_glyphs &&
          plan->old_to_new_gid[glyph_id] != SUBSET_GID_NOT_INCLUDED) {
        eot_status_t status = append_cmap_entry(entries, count, capacity, codepoint,
                                               plan->old_to_new_gid[glyph_id]);
        if (status != EOT_OK) {
          return status;
        }
      }

      if (codepoint == 0xFFFFu) {
        break;
      }
    }
  }

  return EOT_OK;
}

static eot_status_t collect_cmap_entries_format12(cmap_lookup_t cmap,
                                                  const subset_plan_t *plan,
                                                  cmap_entry_t **entries,
                                                  size_t *count,
                                                  size_t *capacity) {
  const uint8_t *data = cmap.data.data;
  uint32_t length = read_u32be(data + 4);
  uint32_t num_groups = read_u32be(data + 12);
  uint32_t i;

  if (cmap.data.length < 16u || length > cmap.data.length ||
      length < 16u + num_groups * 12u) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (i = 0; i < num_groups; i++) {
    const uint8_t *group = data + 16u + (size_t)i * 12u;
    uint32_t start_char = read_u32be(group);
    uint32_t end_char = read_u32be(group + 4u);
    uint32_t start_glyph_id = read_u32be(group + 8u);
    uint32_t codepoint;

    if (end_char < start_char) {
      return EOT_ERR_CORRUPT_DATA;
    }

    for (codepoint = start_char; codepoint <= end_char; codepoint++) {
      uint32_t glyph_id = start_glyph_id + (codepoint - start_char);
      if (glyph_id != 0 && glyph_id < plan->total_input_glyphs &&
          plan->old_to_new_gid[glyph_id] != SUBSET_GID_NOT_INCLUDED) {
        eot_status_t status = append_cmap_entry(entries, count, capacity, codepoint,
                                               plan->old_to_new_gid[glyph_id]);
        if (status != EOT_OK) {
          return status;
        }
      }
      if (codepoint == 0xFFFFFFFFu) {
        break;
      }
    }
  }

  return EOT_OK;
}

static eot_status_t collect_cmap_entries(const sfnt_font_t *input,
                                         const subset_plan_t *plan,
                                         cmap_entry_t **out_entries,
                                         size_t *out_count) {
  cmap_lookup_t cmap;
  cmap_entry_t *entries = NULL;
  size_t count = 0;
  size_t capacity = 0;
  eot_status_t status = init_best_cmap_lookup(input, 1, &cmap);

  if (status != EOT_OK) {
    return status;
  }

  if (cmap.format == CMAP_FORMAT_4) {
    status = collect_cmap_entries_format4(cmap, plan, &entries, &count, &capacity);
  } else if (cmap.format == CMAP_FORMAT_12) {
    status = collect_cmap_entries_format12(cmap, plan, &entries, &count, &capacity);
  } else {
    status = EOT_ERR_CORRUPT_DATA;
  }

  if (status != EOT_OK) {
    free(entries);
    return status;
  }

  *out_entries = entries;
  *out_count = count;
  return EOT_OK;
}

static eot_status_t build_cmap4_subtable(const cmap_entry_t *entries, size_t count,
                                         uint8_t **out_data, size_t *out_length) {
  cmap4_segment_t *segments = NULL;
  size_t segment_count = 0;
  size_t segment_capacity = 0;
  size_t i;
  uint16_t seg_count;
  uint16_t seg_count_x2;
  int search_power;
  int entry_selector;
  int search_range;
  uint8_t *data;
  size_t length;

  for (i = 0; i < count; i++) {
    uint32_t codepoint = entries[i].codepoint;
    int16_t delta;

    if (codepoint > 0xFFFEu) {
      continue;
    }

    delta = (int16_t)((entries[i].glyph_id - codepoint) & 0xFFFFu);
    if (segment_count > 0 &&
        segments[segment_count - 1u].end_code + 1u == (uint16_t)codepoint &&
        segments[segment_count - 1u].delta == delta) {
      segments[segment_count - 1u].end_code = (uint16_t)codepoint;
      continue;
    }

    if (segment_count == segment_capacity) {
      size_t new_capacity = segment_capacity == 0 ? 16u : segment_capacity * 2u;
      cmap4_segment_t *new_segments =
          (cmap4_segment_t *)realloc(segments, new_capacity * sizeof(cmap4_segment_t));
      if (new_segments == NULL) {
        free(segments);
        return EOT_ERR_ALLOCATION;
      }
      segments = new_segments;
      segment_capacity = new_capacity;
    }

    segments[segment_count].start_code = (uint16_t)codepoint;
    segments[segment_count].end_code = (uint16_t)codepoint;
    segments[segment_count].delta = delta;
    segment_count++;
  }

  seg_count = (uint16_t)(segment_count + 1u);
  seg_count_x2 = (uint16_t)(seg_count * 2u);
  search_power = highest_power_of_two_at_most(seg_count);
  entry_selector = floor_log2_positive(search_power);
  search_range = search_power * 2;
  length = 16u + (size_t)seg_count * 8u;

  data = (uint8_t *)calloc(length, 1u);
  if (data == NULL) {
    free(segments);
    return EOT_ERR_ALLOCATION;
  }

  write_u16be(data, CMAP_FORMAT_4);
  write_u16be(data + 2u, (uint16_t)length);
  write_u16be(data + 6u, seg_count_x2);
  write_u16be(data + 8u, (uint16_t)search_range);
  write_u16be(data + 10u, (uint16_t)entry_selector);
  write_u16be(data + 12u, (uint16_t)(seg_count_x2 - search_range));

  for (i = 0; i < segment_count; i++) {
    write_u16be(data + 14u + i * 2u, segments[i].end_code);
  }
  write_u16be(data + 14u + segment_count * 2u, 0xFFFFu);
  write_u16be(data + 14u + (size_t)seg_count * 2u, 0u);

  for (i = 0; i < segment_count; i++) {
    write_u16be(data + 16u + (size_t)seg_count * 2u + i * 2u, segments[i].start_code);
  }
  write_u16be(data + 16u + (size_t)seg_count * 2u + segment_count * 2u, 0xFFFFu);

  for (i = 0; i < segment_count; i++) {
    write_u16be(data + 16u + (size_t)seg_count * 4u + i * 2u,
                (uint16_t)segments[i].delta);
  }
  write_u16be(data + 16u + (size_t)seg_count * 4u + segment_count * 2u, 1u);

  *out_data = data;
  *out_length = length;
  free(segments);
  return EOT_OK;
}

static eot_status_t build_cmap12_subtable(const cmap_entry_t *entries, size_t count,
                                          uint8_t **out_data, size_t *out_length) {
  cmap12_group_t *groups = NULL;
  size_t group_count = 0;
  size_t group_capacity = 0;
  size_t i;
  uint8_t *data;
  size_t length;

  for (i = 0; i < count; i++) {
    if (group_count > 0 &&
        groups[group_count - 1u].end_code + 1u == entries[i].codepoint &&
        groups[group_count - 1u].start_glyph_id +
                (groups[group_count - 1u].end_code - groups[group_count - 1u].start_code) + 1u ==
            entries[i].glyph_id) {
      groups[group_count - 1u].end_code = entries[i].codepoint;
      continue;
    }

    if (group_count == group_capacity) {
      size_t new_capacity = group_capacity == 0 ? 16u : group_capacity * 2u;
      cmap12_group_t *new_groups =
          (cmap12_group_t *)realloc(groups, new_capacity * sizeof(cmap12_group_t));
      if (new_groups == NULL) {
        free(groups);
        return EOT_ERR_ALLOCATION;
      }
      groups = new_groups;
      group_capacity = new_capacity;
    }

    groups[group_count].start_code = entries[i].codepoint;
    groups[group_count].end_code = entries[i].codepoint;
    groups[group_count].start_glyph_id = entries[i].glyph_id;
    group_count++;
  }

  length = 16u + group_count * 12u;
  data = (uint8_t *)calloc(length, 1u);
  if (data == NULL) {
    free(groups);
    return EOT_ERR_ALLOCATION;
  }

  write_u16be(data, CMAP_FORMAT_12);
  write_u32be(data + 4u, (uint32_t)length);
  write_u32be(data + 12u, (uint32_t)group_count);
  for (i = 0; i < group_count; i++) {
    size_t offset = 16u + i * 12u;
    write_u32be(data + offset, groups[i].start_code);
    write_u32be(data + offset + 4u, groups[i].end_code);
    write_u32be(data + offset + 8u, groups[i].start_glyph_id);
  }

  *out_data = data;
  *out_length = length;
  free(groups);
  return EOT_OK;
}

static eot_status_t build_cmap_table(const sfnt_font_t *input, const subset_plan_t *plan,
                                     uint8_t **out_data, size_t *out_length) {
  cmap_entry_t *entries = NULL;
  cmap_entry_t *format12_entries = NULL;
  size_t entry_count = 0;
  size_t format12_count = 0;
  uint8_t *format4 = NULL;
  size_t format4_length = 0;
  uint8_t *format12 = NULL;
  size_t format12_length = 0;
  uint8_t *data = NULL;
  size_t num_tables = 1;
  size_t offset;
  size_t i;
  eot_status_t status = collect_cmap_entries(input, plan, &entries, &entry_count);

  if (status != EOT_OK) {
    return status;
  }

  status = build_cmap4_subtable(entries, entry_count, &format4, &format4_length);
  if (status != EOT_OK) {
    free(entries);
    return status;
  }

  for (i = 0; i < entry_count; i++) {
    if (entries[i].codepoint > 0xFFFFu) {
      format12_count++;
    }
  }
  if (format12_count > 0) {
    format12_entries = (cmap_entry_t *)malloc(entry_count * sizeof(cmap_entry_t));
    if (format12_entries == NULL) {
      free(entries);
      free(format4);
      return EOT_ERR_ALLOCATION;
    }
    for (i = 0; i < entry_count; i++) {
      format12_entries[i] = entries[i];
    }
    status = build_cmap12_subtable(format12_entries, entry_count, &format12, &format12_length);
    if (status != EOT_OK) {
      free(entries);
      free(format12_entries);
      free(format4);
      return status;
    }
    num_tables = 2;
  }

  *out_length = 4u + num_tables * 8u + format4_length + format12_length;
  data = (uint8_t *)calloc(*out_length, 1u);
  if (data == NULL) {
    free(entries);
    free(format12_entries);
    free(format4);
    free(format12);
    return EOT_ERR_ALLOCATION;
  }

  write_u16be(data + 2u, (uint16_t)num_tables);
  write_u16be(data + 4u, 3u);
  write_u16be(data + 6u, 1u);
  write_u32be(data + 8u, (uint32_t)(4u + num_tables * 8u));

  offset = 4u + num_tables * 8u;
  memcpy(data + offset, format4, format4_length);
  offset += format4_length;

  if (num_tables == 2) {
    write_u16be(data + 12u, 3u);
    write_u16be(data + 14u, 10u);
    write_u32be(data + 16u, (uint32_t)offset);
    memcpy(data + offset, format12, format12_length);
  }

  free(entries);
  free(format12_entries);
  free(format4);
  free(format12);

  *out_data = data;
  return EOT_OK;
}

static eot_status_t add_rebuilt_tables(const sfnt_font_t *input, const subset_plan_t *plan,
                                       sfnt_font_t *output) {
  uint8_t *head_data = NULL;
  size_t head_length = 0;
  uint8_t *maxp_data = NULL;
  size_t maxp_length = 0;
  uint8_t *hmtx_data = NULL;
  size_t hmtx_length = 0;
  uint8_t *hhea_data = NULL;
  size_t hhea_length = 0;
  uint8_t *glyf_data = NULL;
  size_t glyf_length = 0;
  uint8_t *loca_data = NULL;
  size_t loca_length = 0;
  uint8_t *cmap_data = NULL;
  size_t cmap_length = 0;
  eot_status_t status;

  status = build_head_table(input, &head_data, &head_length);
  if (status == EOT_OK) {
    status = build_maxp_table(input, output_num_glyphs_for_plan(plan),
                              &maxp_data, &maxp_length);
  }
  if (status == EOT_OK) {
    status = build_hmtx_and_hhea_tables(input, plan, &hmtx_data, &hmtx_length,
                                        &hhea_data, &hhea_length);
  }
  if (status == EOT_OK) {
    status = build_glyf_and_loca_tables(input, plan, &glyf_data, &glyf_length,
                                        &loca_data, &loca_length);
  }
  if (status == EOT_OK) {
    status = build_cmap_table(input, plan, &cmap_data, &cmap_length);
  }

  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_head, head_data, head_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_hhea, hhea_data, hhea_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_hmtx, hmtx_data, hmtx_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_maxp, maxp_data, maxp_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_glyf, glyf_data, glyf_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_loca, loca_data, loca_length);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(output, TAG_cmap, cmap_data, cmap_length);
  }
  if (status == EOT_OK) {
    status = copy_table(input, output, TAG_name);
  }
  if (status == EOT_OK) {
    status = copy_table(input, output, TAG_post);
  }
  if (status == EOT_OK) {
    status = copy_table(input, output, TAG_OS_2);
  }

  free(head_data);
  free(maxp_data);
  free(hmtx_data);
  free(hhea_data);
  free(glyf_data);
  free(loca_data);
  free(cmap_data);
  return status;
}

void subset_plan_init(subset_plan_t *plan) {
  if (plan == NULL) {
    return;
  }

  plan->included_glyph_ids = NULL;
  plan->old_to_new_gid = NULL;
  plan->num_glyphs = 0;
  plan->total_input_glyphs = 0;
  plan->added_composite_dependencies = 0;
  plan->keep_gids = 0;
}

void subset_plan_destroy(subset_plan_t *plan) {
  if (plan == NULL) {
    return;
  }

  free(plan->included_glyph_ids);
  free(plan->old_to_new_gid);
  subset_plan_init(plan);
}

void subset_warnings_init(subset_warnings_t *warnings) {
  if (warnings == NULL) {
    return;
  }

  warnings->dropped_hdmx = 0;
  warnings->dropped_vdmx = 0;
}

eot_status_t sfnt_subset_plan(const sfnt_font_t *font,
                              const subset_request_t *request,
                              subset_plan_t *out_plan) {
  uint16_t num_glyphs;
  uint8_t *included = NULL;
  cmap_lookup_t cmap;
  eot_status_t status;
  size_t added_dependencies = 0;

  if (font == NULL || request == NULL || out_plan == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = get_num_glyphs(font, &num_glyphs);
  if (status != EOT_OK) {
    return status;
  }

  included = (uint8_t *)calloc(num_glyphs, sizeof(uint8_t));
  if (included == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  status = mark_gid(included, num_glyphs, 0);
  if (status != EOT_OK) {
    free(included);
    return status;
  }

  switch (request->selection_mode) {
    case SUBSET_SELECTION_TEXT:
      status = init_best_cmap_lookup(font, 1, &cmap);
      if (status == EOT_OK) {
        status = add_text_selection(request->selection_data, cmap, included, num_glyphs);
      }
      break;
    case SUBSET_SELECTION_UNICODES:
      status = init_best_cmap_lookup(font, 0, &cmap);
      if (status == EOT_OK) {
        status = add_unicode_selection(request->selection_data, cmap, included, num_glyphs);
      }
      break;
    case SUBSET_SELECTION_GLYPH_IDS:
      status = add_glyph_id_selection(request->selection_data, included, num_glyphs);
      break;
    case SUBSET_SELECTION_NONE:
    default:
      status = EOT_ERR_INVALID_ARGUMENT;
      break;
  }

  if (status == EOT_OK) {
    status = add_composite_closure(font, num_glyphs, included, &added_dependencies);
  }
  if (status == EOT_OK) {
    status = build_plan(included, num_glyphs, request->keep_gids,
                        added_dependencies, out_plan);
  }

  free(included);
  return status;
}

eot_status_t sfnt_subset_font_with_warnings(const sfnt_font_t *input,
                                            const subset_request_t *request,
                                            sfnt_font_t *output,
                                            subset_warnings_t *warnings) {
  subset_plan_t plan;
  subset_font_task_t task;
  eot_status_t status;

  if (input == NULL || request == NULL || output == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (request->variation_axis_map != NULL && request->variation_axis_map[0] != '\0') {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  /* Task 1 routes runtime subsetting through HarfBuzz; keep the legacy
   * rebuilder referenced so the translation unit stays warning-clean while the
   * remaining migration work lands in later tasks. */
  (void)&add_rebuilt_tables;

  subset_plan_init(&plan);
  subset_warnings_init(warnings);
  sfnt_font_init(output);

  status = sfnt_subset_plan(input, request, &plan);
  if (status != EOT_OK) {
    subset_plan_destroy(&plan);
    sfnt_font_destroy(output);
    return status;
  }

  /* Keep subsetting itself serial until the backend exposes safe internal
   * parallel work, but route the stage through the shared runtime so its
   * execution model and diagnostics are explicit and testable. */
  task.input = input;
  task.plan = &plan;
  task.output = output;
  task.warnings = warnings;
  task.status = EOT_OK;
  status = parallel_runtime_run_task_list(&task, 1u, sizeof(task),
                                          run_subset_font_task);
  if (status == EOT_OK) {
    status = task.status;
  }

  subset_plan_destroy(&plan);
  if (status != EOT_OK) {
    sfnt_font_destroy(output);
  }
  return status;
}

eot_status_t sfnt_subset_font(const sfnt_font_t *input,
                              const subset_request_t *request,
                              sfnt_font_t *output) {
  return sfnt_subset_font_with_warnings(input, request, output, NULL);
}
