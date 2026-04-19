#include "sfnt_font.h"

#include <stdlib.h>
#include <string.h>

void sfnt_font_init(sfnt_font_t *font) {
  font->tables = NULL;
  font->num_tables = 0;
  font->capacity = 0;
}

eot_status_t sfnt_font_add_table(sfnt_font_t *font, uint32_t tag, const uint8_t *data, size_t length) {
  if (font == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (length > 0 && data == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (size_t i = 0; i < font->num_tables; i++) {
    if (font->tables[i].tag != tag) {
      continue;
    }

    uint8_t *table_data = NULL;
    if (length > 0) {
      table_data = malloc(length);
      if (!table_data) {
        return EOT_ERR_ALLOCATION;
      }
      memcpy(table_data, data, length);
    }

    free(font->tables[i].data);
    font->tables[i].data = table_data;
    font->tables[i].length = length;

    for (size_t duplicate = i + 1; duplicate < font->num_tables;) {
      if (font->tables[duplicate].tag != tag) {
        duplicate++;
        continue;
      }

      free(font->tables[duplicate].data);
      if (duplicate + 1 < font->num_tables) {
        memmove(&font->tables[duplicate],
                &font->tables[duplicate + 1],
                (font->num_tables - duplicate - 1) * sizeof(sfnt_table_t));
      }
      font->num_tables--;
    }
    return EOT_OK;
  }

  if (font->num_tables >= font->capacity) {
    size_t new_capacity = font->capacity == 0 ? 16 : font->capacity * 2;
    sfnt_table_t *new_tables = realloc(font->tables, new_capacity * sizeof(sfnt_table_t));
    if (!new_tables) {
      return EOT_ERR_ALLOCATION;
    }
    font->tables = new_tables;
    font->capacity = new_capacity;
  }

  uint8_t *table_data = NULL;
  if (length > 0) {
    table_data = malloc(length);
    if (!table_data) {
      return EOT_ERR_ALLOCATION;
    }
    memcpy(table_data, data, length);
  }

  font->tables[font->num_tables].tag = tag;
  font->tables[font->num_tables].data = table_data;
  font->tables[font->num_tables].length = length;
  font->num_tables++;

  return EOT_OK;
}

eot_status_t sfnt_font_remove_table(sfnt_font_t *font, uint32_t tag) {
  size_t i;

  if (font == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (i = 0; i < font->num_tables; i++) {
    if (font->tables[i].tag != tag) {
      continue;
    }

    free(font->tables[i].data);
    if (i + 1 < font->num_tables) {
      memmove(&font->tables[i],
              &font->tables[i + 1],
              (font->num_tables - i - 1) * sizeof(sfnt_table_t));
    }
    font->num_tables--;
    return EOT_OK;
  }

  return EOT_OK;
}

int sfnt_font_has_table(sfnt_font_t *font, uint32_t tag) {
  for (size_t i = 0; i < font->num_tables; i++) {
    if (font->tables[i].tag == tag) {
      return 1;
    }
  }
  return 0;
}

sfnt_table_t *sfnt_font_get_table(sfnt_font_t *font, uint32_t tag) {
  for (size_t i = 0; i < font->num_tables; i++) {
    if (font->tables[i].tag == tag) {
      return &font->tables[i];
    }
  }
  return NULL;
}

void sfnt_font_destroy(sfnt_font_t *font) {
  for (size_t i = 0; i < font->num_tables; i++) {
    free(font->tables[i].data);
  }
  free(font->tables);
  font->tables = NULL;
  font->num_tables = 0;
  font->capacity = 0;
}
