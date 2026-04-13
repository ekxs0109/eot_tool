#include "mtx_decode.h"

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "byte_io.h"
#include "cvt_codec.h"
#include "eot_header.h"
#include "glyf_codec.h"
#include "hdmx_codec.h"
#include "lzcomp.h"
#include "mtx_container.h"

#define TAG_head 0x68656164
#define TAG_hhea 0x68686561
#define TAG_hmtx 0x686d7478
#define TAG_maxp 0x6d617870
#define TAG_name 0x6e616d65
#define TAG_post 0x706f7374
#define TAG_OS2  0x4f532f32
#define TAG_cmap 0x636d6170
#define TAG_DSIG 0x44534947
#define TAG_cvt  0x63767420
#define TAG_fpgm 0x6670676d
#define TAG_glyf 0x676c7966
#define TAG_hdmx 0x68646d78
#define TAG_kern 0x6b65726e
#define TAG_loca 0x6c6f6361
#define TAG_prep 0x70726570
#define TAG_gasp 0x67617370
#define EOT_FLAG_PPT_XOR 0x10000000u

static void xor_buffer(uint8_t *data, size_t length, uint8_t mask) {
  for (size_t i = 0; i < length; i++) {
    data[i] ^= mask;
  }
}

static eot_status_t extract_tables_from_block1(buffer_view_t block1, sfnt_font_t *font,
                                               int preserve_loca,
                                               int *out_num_glyphs) {
  // Block1 uses MTX compact encoding:
  // - 12-byte header
  // - 16-byte table entries
  // - Table data follows

  if (block1.length < 12) {
    return EOT_ERR_CORRUPT_DATA;
  }

  // Check for MTX compact format (version 0x00020000)
  uint32_t version = read_u32be(block1.data);
  if (version != 0x00020000) {
    // Try standard SFNT format
    if (version != 0x00010000 && version != 0x74727565 && version != 0x4f54544f) {
      return EOT_ERR_CORRUPT_DATA;
    }
    // Standard SFNT - use original logic
    uint16_t num_tables = read_u16be(block1.data + 4);
    size_t table_dir_size = 12 + num_tables * 16;

    if (num_tables > 50 || block1.length < table_dir_size) {
      return EOT_ERR_CORRUPT_DATA;
    }

    *out_num_glyphs = 0;
    for (uint16_t i = 0; i < num_tables; i++) {
      size_t entry_offset = 12 + i * 16;
      uint32_t tag = read_u32be(block1.data + entry_offset);
      uint32_t table_offset = read_u32be(block1.data + entry_offset + 8);
      uint32_t table_length = read_u32be(block1.data + entry_offset + 12);

      if (tag == TAG_loca && !preserve_loca) continue;
      if (table_offset + table_length > block1.length) {
        return EOT_ERR_CORRUPT_DATA;
      }

      if (tag == TAG_maxp && table_length >= 6) {
        *out_num_glyphs = read_u16be(block1.data + table_offset + 4);
      }

      eot_status_t status = sfnt_font_add_table(font, tag, block1.data + table_offset, table_length);
      if (status != EOT_OK) return status;
    }
    return EOT_OK;
  }

  // MTX compact format
  uint16_t num_tables = read_u16be(block1.data + 4);
  size_t table_dir_offset = 12;
  size_t table_dir_size = num_tables * 16;

  if (num_tables > 50 || block1.length < table_dir_offset + table_dir_size) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_num_glyphs = 0;

  // MTX compact format (Microsoft's encoding, version 0x00020000)
  // This is different from sfntly's MTX format which uses standard SFNT (0x00010000)
  //
  // The test file testdata/wingdings3.eot uses Microsoft's MTX encoder.
  // sfntly only has an encoder (no decoder), so we need to either:
  // 1. Implement Microsoft's MTX decoder (complex, undocumented format)
  // 2. Generate a new test file using sfntly's encoder
  //
  // For now, return an error with a clear message

  fprintf(stderr, "ERROR: MTX compact format (version 0x00020000) not supported\n");
  fprintf(stderr, "       This file uses Microsoft's MTX encoding\n");
  fprintf(stderr, "       Expected sfntly MTX format with SFNT version 0x00010000\n");
  fprintf(stderr, "\n");
  fprintf(stderr, "SOLUTION: Generate a new test file using sfntly's Java encoder:\n");
  fprintf(stderr, "          java -jar sfnttool.jar -e -x input.ttf output.eot\n");

  return EOT_ERR_CORRUPT_DATA;
}

eot_status_t mtx_decode_eot_file(const char *path, sfnt_font_t *out_font) {
  file_buffer_t file_buf;
  eot_status_t status = file_io_read_all(path, &file_buf);
  if (status != EOT_OK) {
    return status;
  }

  eot_header_t header;
  buffer_view_t view = buffer_view_make(file_buf.data, file_buf.length);
  status = eot_header_parse(view, &header);
  if (status != EOT_OK) {
    file_io_free(&file_buf);
    return status;
  }

  size_t mtx_offset = header.header_length;
  if (mtx_offset >= file_buf.length) {
    eot_header_destroy(&header);
    file_io_free(&file_buf);
    return EOT_ERR_CORRUPT_DATA;
  }

  if ((header.flags & EOT_FLAG_PPT_XOR) != 0u) {
    xor_buffer(file_buf.data + mtx_offset, header.font_data_size, 0x50u);
  }

  buffer_view_t mtx_view = buffer_view_make(file_buf.data + mtx_offset,
                                             file_buf.length - mtx_offset);

  mtx_container_t container;
  status = mtx_container_parse(mtx_view, &container);
  if (status != EOT_OK) {
    eot_header_destroy(&header);
    file_io_free(&file_buf);
    return status;
  }

  uint8_t *block1_data = NULL;
  size_t block1_size = 0;
  uint8_t *block2_data = NULL;
  size_t block2_size = 0;
  uint8_t *block3_data = NULL;
  size_t block3_size = 0;

  buffer_view_t compressed1 = buffer_view_make(container.payload_data + 10,
                                                container.offset_block2 - 10);
  status = lzcomp_decompress(compressed1, &block1_data, &block1_size);
  if (status != EOT_OK) {
    goto cleanup;
  }

  if (container.num_blocks >= 2) {
    size_t block2_size_compressed = container.offset_block3 - container.offset_block2;
    buffer_view_t compressed2 = buffer_view_make(
        container.payload_data + container.offset_block2,
        block2_size_compressed);
    status = lzcomp_decompress(compressed2, &block2_data, &block2_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (container.num_blocks >= 3) {
    size_t block3_size_compressed = container.payload_size - container.offset_block3;
    buffer_view_t compressed3 = buffer_view_make(
        container.payload_data + container.offset_block3,
        block3_size_compressed);
    status = lzcomp_decompress(compressed3, &block3_data, &block3_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }


  // Debug: write block1 to file for analysis if needed
  // FILE *debug_file = fopen("debug_block1.bin", "wb");
  // if (debug_file) {
  //   fwrite(block1_data, 1, block1_size, debug_file);
  //   fclose(debug_file);
  // }

  sfnt_font_init(out_font);

  buffer_view_t block1_view = buffer_view_make(block1_data, block1_size);
  int num_glyphs = 0;
  status = extract_tables_from_block1(block1_view, out_font,
                                      container.num_blocks < 3, &num_glyphs);
  if (status != EOT_OK) {
    goto cleanup;
  }

  sfnt_table_t *cvt_table = sfnt_font_get_table(out_font, TAG_cvt);
  if (cvt_table) {
    uint8_t *decoded_cvt = NULL;
    size_t decoded_cvt_size = 0;
    buffer_view_t cvt_view = buffer_view_make(cvt_table->data, cvt_table->length);
    status = cvt_decode(cvt_view, &decoded_cvt, &decoded_cvt_size);
    if (status != EOT_OK) {
      goto cleanup;
    }

    free(cvt_table->data);
    cvt_table->data = decoded_cvt;
    cvt_table->length = decoded_cvt_size;
  }

  sfnt_table_t *hdmx_table = sfnt_font_get_table(out_font, TAG_hdmx);
  if (hdmx_table) {
    sfnt_table_t *hmtx = sfnt_font_get_table(out_font, TAG_hmtx);
    sfnt_table_t *hhea = sfnt_font_get_table(out_font, TAG_hhea);
    sfnt_table_t *head = sfnt_font_get_table(out_font, TAG_head);
    sfnt_table_t *maxp = sfnt_font_get_table(out_font, TAG_maxp);
    if (hmtx && hhea && head && maxp) {
      uint8_t *decoded_hdmx = NULL;
      size_t decoded_hdmx_size = 0;
      buffer_view_t hdmx_view = buffer_view_make(hdmx_table->data, hdmx_table->length);
      buffer_view_t hmtx_view = buffer_view_make(hmtx->data, hmtx->length);
      buffer_view_t hhea_view = buffer_view_make(hhea->data, hhea->length);
      buffer_view_t head_view = buffer_view_make(head->data, head->length);
      buffer_view_t maxp_view = buffer_view_make(maxp->data, maxp->length);
      status = hdmx_decode(hdmx_view, hmtx_view, hhea_view, head_view, maxp_view,
                          &decoded_hdmx, &decoded_hdmx_size);
      if (status != EOT_OK) {
        goto cleanup;
      }

      free(hdmx_table->data);
      hdmx_table->data = decoded_hdmx;
      hdmx_table->length = decoded_hdmx_size;
    }
  }

  {
    sfnt_table_t *glyf_table = sfnt_font_get_table(out_font, TAG_glyf);
    sfnt_table_t *head_table = sfnt_font_get_table(out_font, TAG_head);
    sfnt_table_t *loca_table = sfnt_font_get_table(out_font, TAG_loca);
    int needs_loca_rebuild =
        (loca_table == NULL || loca_table->length == 0) && num_glyphs > 0;

    if (needs_loca_rebuild && glyf_table && head_table && head_table->length >= 52) {
      uint8_t *glyf_data = NULL;
      size_t glyf_size = 0;
      uint8_t *loca_data = NULL;
      size_t loca_size = 0;
      int index_to_loca_format = (int16_t)read_u16be(head_table->data + 50);

      buffer_view_t glyf_view = buffer_view_make(glyf_table->data, glyf_table->length);
      buffer_view_t push_view = buffer_view_make(block2_data, block2_size);
      buffer_view_t code_view = buffer_view_make(block3_data, block3_size);

      status = glyf_decode_with_loca_format(glyf_view, push_view, code_view,
                                            num_glyphs, index_to_loca_format,
                                            &glyf_data, &glyf_size,
                                            &loca_data, &loca_size);
      if (status == EOT_OK) {
        free(glyf_table->data);
        glyf_table->data = glyf_data;
        glyf_table->length = glyf_size;

        status = sfnt_font_add_table(out_font, TAG_loca, loca_data, loca_size);
        free(loca_data);
        if (status != EOT_OK) {
          goto cleanup;
        }
      }
    }
  }

  /* Any embedded-font roundtrip invalidates the original digital signature. */
  sfnt_font_remove_table(out_font, TAG_DSIG);

cleanup:
  free(block1_data);
  free(block2_data);
  free(block3_data);
  eot_header_destroy(&header);
  file_io_free(&file_buf);

  if (status != EOT_OK) {
    sfnt_font_destroy(out_font);
  }

  return status;
}
