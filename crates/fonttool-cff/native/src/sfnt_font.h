#ifndef EOT_TOOL_SFNT_FONT_H
#define EOT_TOOL_SFNT_FONT_H

#include <stddef.h>
#include <stdint.h>

#include "file_io.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint32_t tag;
  uint8_t *data;
  size_t length;
} sfnt_table_t;

typedef struct {
  sfnt_table_t *tables;
  size_t num_tables;
  size_t capacity;
} sfnt_font_t;

void sfnt_font_init(sfnt_font_t *font);
eot_status_t sfnt_font_add_table(sfnt_font_t *font, uint32_t tag, const uint8_t *data, size_t length);
eot_status_t sfnt_font_remove_table(sfnt_font_t *font, uint32_t tag);
int sfnt_font_has_table(sfnt_font_t *font, uint32_t tag);
sfnt_table_t *sfnt_font_get_table(sfnt_font_t *font, uint32_t tag);
void sfnt_font_destroy(sfnt_font_t *font);

#ifdef __cplusplus
}
#endif

#endif
