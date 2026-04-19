#include "sfnt_writer.h"
#include "byte_io.h"

#include <stdlib.h>
#include <string.h>

#define TAG_head 0x68656164u
#define HEAD_CHECKSUM_ADJUSTMENT_OFFSET 8u
#define HEAD_TABLE_MIN_LENGTH 12u
#define SFNT_CHECKSUM_MAGIC 0xB1B0AFBAu

static uint32_t calc_checksum(const uint8_t *data, size_t length) {
  uint32_t sum = 0;
  size_t nlongs = (length + 3) / 4;

  for (size_t i = 0; i < nlongs; i++) {
    uint32_t value = 0;
    for (int j = 0; j < 4; j++) {
      size_t offset = i * 4 + j;
      if (offset < length) {
        value = (value << 8) | data[offset];
      }
    }
    sum += value;
  }

  return sum;
}

static int compare_tags(const void *a, const void *b) {
  uint32_t tag_a = ((const sfnt_table_t *)a)->tag;
  uint32_t tag_b = ((const sfnt_table_t *)b)->tag;
  if (tag_a < tag_b) return -1;
  if (tag_a > tag_b) return 1;
  return 0;
}

static int highest_bit(int x) {
  int result = 0;
  while (x > 1) {
    x >>= 1;
    result++;
  }
  return result;
}

static void write_head_checksum_adjustment(uint8_t *output, size_t output_size,
                                           size_t head_entry_offset,
                                           size_t head_table_offset,
                                           size_t head_table_length) {
  uint32_t head_checksum = calc_checksum(output + head_table_offset,
                                         head_table_length);
  write_u32be(output + head_entry_offset + 4, head_checksum);

  uint32_t font_checksum = calc_checksum(output, output_size);
  uint32_t adjustment = SFNT_CHECKSUM_MAGIC - font_checksum;
  write_u32be(output + head_table_offset + HEAD_CHECKSUM_ADJUSTMENT_OFFSET,
              adjustment);
}

eot_status_t sfnt_writer_serialize(sfnt_font_t *font, uint8_t **out_data, size_t *out_size) {
  if (font->num_tables == 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  sfnt_table_t *sorted_tables = malloc(font->num_tables * sizeof(sfnt_table_t));
  if (!sorted_tables) {
    return EOT_ERR_ALLOCATION;
  }
  memcpy(sorted_tables, font->tables, font->num_tables * sizeof(sfnt_table_t));
  qsort(sorted_tables, font->num_tables, sizeof(sfnt_table_t), compare_tags);

  size_t header_size = 12 + font->num_tables * 16;
  size_t total_size = header_size;

  for (size_t i = 0; i < font->num_tables; i++) {
    if (sorted_tables[i].length > 0) {
      size_t aligned_length = (sorted_tables[i].length + 3) & ~3;
      total_size += aligned_length;
    }
  }

  uint8_t *output = malloc(total_size);
  if (!output) {
    free(sorted_tables);
    return EOT_ERR_ALLOCATION;
  }
  memset(output, 0, total_size);

  write_u32be(output, 0x00010000);
  write_u16be(output + 4, (uint16_t)font->num_tables);

  int search_range_shift = highest_bit((int)font->num_tables);
  int search_range = 1 << search_range_shift;
  write_u16be(output + 6, (uint16_t)(search_range * 16));
  write_u16be(output + 8, (uint16_t)search_range_shift);
  write_u16be(output + 10, (uint16_t)((font->num_tables - search_range) * 16));

  size_t data_offset = header_size;
  size_t head_entry_offset = 0;
  size_t head_table_offset = 0;
  size_t head_table_length = 0;
  int saw_head = 0;

  for (size_t i = 0; i < font->num_tables; i++) {
    size_t entry_offset = 12 + i * 16;
    sfnt_table_t *table = &sorted_tables[i];

    write_u32be(output + entry_offset, table->tag);

    uint32_t checksum = calc_checksum(table->data, table->length);
    write_u32be(output + entry_offset + 4, checksum);

    if (table->length == 0) {
      write_u32be(output + entry_offset + 8, 0);
      write_u32be(output + entry_offset + 12, 0);
      continue;
    }

    write_u32be(output + entry_offset + 8, (uint32_t)data_offset);
    write_u32be(output + entry_offset + 12, (uint32_t)table->length);

    memcpy(output + data_offset, table->data, table->length);

    if (table->tag == TAG_head && table->length >= HEAD_TABLE_MIN_LENGTH) {
      saw_head = 1;
      head_entry_offset = entry_offset;
      head_table_offset = data_offset;
      head_table_length = table->length;
      write_u32be(output + head_table_offset + HEAD_CHECKSUM_ADJUSTMENT_OFFSET, 0u);
      write_u32be(output + head_entry_offset + 4,
                  calc_checksum(output + head_table_offset, head_table_length));
    }

    size_t aligned_length = (table->length + 3) & ~3;
    data_offset += aligned_length;
  }

  if (saw_head) {
    write_head_checksum_adjustment(output, total_size, head_entry_offset,
                                   head_table_offset, head_table_length);
  }

  free(sorted_tables);

  *out_data = output;
  *out_size = total_size;

  return EOT_OK;
}
