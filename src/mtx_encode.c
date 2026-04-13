#include "mtx_encode.h"

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
#include "otf_convert.h"
#include "parallel_runtime.h"
#include "sfnt_reader.h"
#include "sfnt_writer.h"
#include "table_policy.h"

#define TAG_head 0x68656164
#define TAG_name 0x6e616d65
#define TAG_OS_2 0x4f532f32
#define TAG_cmap 0x636d6170
#define TAG_hhea 0x68686561
#define TAG_hmtx 0x686d7478
#define TAG_maxp 0x6d617870
#define TAG_post 0x706f7374
#define TAG_glyf 0x676c7966
#define TAG_loca 0x6c6f6361
#define TAG_cvt  0x63767420
#define TAG_hdmx 0x68646d78
#define TAG_VDMX 0x56444d58
#define TAG_prep 0x70726570
#define TAG_fpgm 0x6670676d
#define TAG_CFF  0x43464620
#define TAG_CFF2 0x43464632
#define SFNT_VERSION_OTTO 0x4f54544f
#define EOT_FLAG_COMPRESSED 0x00000004u
#define EOT_FLAG_PPT_XOR 0x10000000u

typedef struct {
  const byte_buffer_t *input;
  uint8_t **out_data;
  size_t *out_size;
  size_t block_index;
  eot_status_t status;
} compress_block_task_t;

void byte_buffer_init(byte_buffer_t *buf) {
  buf->data = NULL;
  buf->length = 0;
  buf->capacity = 0;
}

void mtx_encode_warnings_init(mtx_encode_warnings_t *warnings) {
  if (warnings != NULL) {
    warnings->dropped_vdmx = 0;
  }
}

eot_status_t byte_buffer_reserve(byte_buffer_t *buf, size_t capacity) {
  if (capacity <= buf->capacity) {
    return EOT_OK;
  }

  uint8_t *new_data = (uint8_t *)realloc(buf->data, capacity);
  if (new_data == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  buf->data = new_data;
  buf->capacity = capacity;
  return EOT_OK;
}

eot_status_t byte_buffer_append(byte_buffer_t *buf, const uint8_t *data, size_t length) {
  if (buf->length + length > buf->capacity) {
    size_t new_capacity = buf->capacity * 2;
    if (new_capacity < buf->length + length) {
      new_capacity = buf->length + length;
    }
    if (new_capacity < 1024) {
      new_capacity = 1024;
    }

    eot_status_t status = byte_buffer_reserve(buf, new_capacity);
    if (status != EOT_OK) {
      return status;
    }
  }

  memcpy(buf->data + buf->length, data, length);
  buf->length += length;
  return EOT_OK;
}

void byte_buffer_destroy(byte_buffer_t *buf) {
  if (buf->data != NULL) {
    free(buf->data);
    buf->data = NULL;
  }
  buf->length = 0;
  buf->capacity = 0;
}

static int should_copy_block1_table(uint32_t tag, mtx_encode_warnings_t *warnings) {
  table_policy_t policy;

  if (tag == TAG_head || tag == TAG_glyf || tag == TAG_loca) {
    return 0;
  }

  policy = table_policy_for_tag(tag);
  if (policy == TABLE_POLICY_DROP_WITH_WARNING) {
    if (warnings != NULL && tag == TAG_VDMX) {
      warnings->dropped_vdmx = 1;
    }
    return 0;
  }

  return policy == TABLE_POLICY_KEEP;
}

static eot_status_t build_encoded_blocks(const sfnt_font_t *font,
                                         byte_buffer_t *block1,
                                         byte_buffer_t *block2,
                                         byte_buffer_t *block3,
                                         mtx_encode_warnings_t *warnings) {
  sfnt_table_t *head = sfnt_font_get_table((sfnt_font_t *)font, TAG_head);
  sfnt_table_t *maxp = sfnt_font_get_table((sfnt_font_t *)font, TAG_maxp);
  sfnt_table_t *glyf = sfnt_font_get_table((sfnt_font_t *)font, TAG_glyf);
  sfnt_table_t *loca = sfnt_font_get_table((sfnt_font_t *)font, TAG_loca);
  sfnt_table_t *cvt = sfnt_font_get_table((sfnt_font_t *)font, TAG_cvt);
  sfnt_table_t *hdmx = sfnt_font_get_table((sfnt_font_t *)font, TAG_hdmx);
  sfnt_table_t *hmtx = sfnt_font_get_table((sfnt_font_t *)font, TAG_hmtx);
  sfnt_table_t *hhea = sfnt_font_get_table((sfnt_font_t *)font, TAG_hhea);
  sfnt_table_t *post = sfnt_font_get_table((sfnt_font_t *)font, TAG_post);
  sfnt_font_t subset;
  uint8_t *encoded_glyf = NULL;
  size_t encoded_glyf_size = 0;
  uint8_t *encoded_cvt = NULL;
  size_t encoded_cvt_size = 0;
  uint8_t *encoded_hdmx = NULL;
  size_t encoded_hdmx_size = 0;
  uint8_t *push_stream = NULL;
  size_t push_stream_size = 0;
  uint8_t *code_stream = NULL;
  size_t code_stream_size = 0;
  uint8_t *sfnt_data = NULL;
  size_t sfnt_size = 0;
  eot_status_t status = EOT_OK;
  int index_to_loca_format = 0;
  int num_glyphs = 0;

  if (!head || !maxp || !glyf || !loca ||
      head->length < 52 || maxp->length < 6) {
    return EOT_ERR_CORRUPT_DATA;
  }

  index_to_loca_format = (int16_t)read_u16be(head->data + 50);
  num_glyphs = read_u16be(maxp->data + 4);
  if (num_glyphs < 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  status = glyf_encode(buffer_view_make(glyf->data, glyf->length),
                       buffer_view_make(loca->data, loca->length),
                       index_to_loca_format, num_glyphs,
                       &encoded_glyf, &encoded_glyf_size,
                       &push_stream, &push_stream_size,
                       &code_stream, &code_stream_size);
  if (status != EOT_OK) {
    return status;
  }

  sfnt_font_init(&subset);
  for (size_t i = 0; i < font->num_tables; i++) {
    sfnt_table_t *table = &font->tables[i];
    if (!should_copy_block1_table(table->tag, warnings)) {
      continue;
    }

    status = sfnt_font_add_table(&subset, table->tag, table->data, table->length);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  status = sfnt_font_add_table(&subset, TAG_head, head->data, head->length);
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&subset, TAG_glyf, encoded_glyf, encoded_glyf_size);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&subset, TAG_loca, NULL, 0);
  }
  if (status != EOT_OK) {
    goto cleanup;
  }

  /* Preserve validated hhea/post data from the input font. Some downstream
   * platform validators are sensitive to these exact values. */
  if (hhea != NULL) {
    status = sfnt_font_remove_table(&subset, TAG_hhea);
    if (status == EOT_OK) {
      status = sfnt_font_add_table(&subset, TAG_hhea, hhea->data, hhea->length);
    }
    if (status != EOT_OK) {
      goto cleanup;
    }
  }
  if (post != NULL) {
    status = sfnt_font_remove_table(&subset, TAG_post);
    if (status == EOT_OK) {
      status = sfnt_font_add_table(&subset, TAG_post, post->data, post->length);
    }
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (cvt != NULL && table_policy_for_tag(TAG_cvt) == TABLE_POLICY_REENCODE) {
    status = cvt_encode(buffer_view_make(cvt->data, cvt->length),
                        &encoded_cvt, &encoded_cvt_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
    status = sfnt_font_add_table(&subset, TAG_cvt, encoded_cvt, encoded_cvt_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  if (hdmx != NULL && table_policy_for_tag(TAG_hdmx) == TABLE_POLICY_REENCODE) {
    if (hmtx == NULL || hhea == NULL) {
      status = EOT_ERR_CORRUPT_DATA;
      goto cleanup;
    }
    status = hdmx_encode(buffer_view_make(hdmx->data, hdmx->length),
                         buffer_view_make(hmtx->data, hmtx->length),
                         buffer_view_make(hhea->data, hhea->length),
                         buffer_view_make(head->data, head->length),
                         buffer_view_make(maxp->data, maxp->length),
                         &encoded_hdmx, &encoded_hdmx_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
    status = sfnt_font_add_table(&subset, TAG_hdmx, encoded_hdmx, encoded_hdmx_size);
    if (status != EOT_OK) {
      goto cleanup;
    }
  }

  status = sfnt_writer_serialize(&subset, &sfnt_data, &sfnt_size);
  if (status != EOT_OK) {
    goto cleanup;
  }

  byte_buffer_init(block1);
  byte_buffer_init(block2);
  byte_buffer_init(block3);
  status = byte_buffer_append(block1, sfnt_data, sfnt_size);
  if (status == EOT_OK) {
    status = byte_buffer_append(block2, push_stream, push_stream_size);
  }
  if (status == EOT_OK) {
    status = byte_buffer_append(block3, code_stream, code_stream_size);
  }
  if (status != EOT_OK) {
    byte_buffer_destroy(block1);
    byte_buffer_destroy(block2);
    byte_buffer_destroy(block3);
  }

cleanup:
  free(sfnt_data);
  free(encoded_glyf);
  free(encoded_cvt);
  free(encoded_hdmx);
  free(push_stream);
  free(code_stream);
  sfnt_font_destroy(&subset);
  return status;
}

static eot_status_t compress_block(const byte_buffer_t *input,
                                   uint8_t **out_data, size_t *out_size) {
  if (input->length == 0) {
    *out_data = NULL;
    *out_size = 0;
    return EOT_OK;
  }

  return lzcomp_compress(input->data, input->length, out_data, out_size);
}

static eot_status_t run_compress_block_task(void *task_context) {
  compress_block_task_t *task = (compress_block_task_t *)task_context;

  if (task == NULL || task->input == NULL ||
      task->out_data == NULL || task->out_size == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  task->status = compress_block(task->input, task->out_data, task->out_size);
  return task->status;
}

static eot_status_t compress_blocks_in_order(const byte_buffer_t *block1,
                                             const byte_buffer_t *block2,
                                             const byte_buffer_t *block3,
                                             uint8_t **compressed1,
                                             size_t *compressed1_size,
                                             uint8_t **compressed2,
                                             size_t *compressed2_size,
                                             uint8_t **compressed3,
                                             size_t *compressed3_size) {
  compress_block_task_t tasks[3];
  const byte_buffer_t *blocks[3];
  uint8_t **outputs[3];
  size_t *sizes[3];
  size_t task_count = 0;
  eot_status_t status;
  size_t i;

  blocks[0] = block1;
  blocks[1] = block2;
  blocks[2] = block3;
  outputs[0] = compressed1;
  outputs[1] = compressed2;
  outputs[2] = compressed3;
  sizes[0] = compressed1_size;
  sizes[1] = compressed2_size;
  sizes[2] = compressed3_size;

  for (i = 0; i < 3; i++) {
    *outputs[i] = NULL;
    *sizes[i] = 0;
    if (blocks[i]->length == 0) {
      continue;
    }

    tasks[task_count].input = blocks[i];
    tasks[task_count].out_data = outputs[i];
    tasks[task_count].out_size = sizes[i];
    tasks[task_count].block_index = i;
    tasks[task_count].status = EOT_OK;
    task_count++;
  }

  status = parallel_runtime_run_task_list(tasks, task_count, sizeof(tasks[0]),
                                          run_compress_block_task);
  if (status != EOT_OK) {
    return status;
  }

  for (i = 0; i < task_count; i++) {
    if (tasks[i].status != EOT_OK) {
      return tasks[i].status;
    }
  }

  return EOT_OK;
}

static void xor_buffer(uint8_t *data, size_t length, uint8_t mask) {
  for (size_t i = 0; i < length; i++) {
    data[i] ^= mask;
  }
}

static eot_status_t simple_encode_font(const sfnt_font_t *font, byte_buffer_t *out,
                                       int apply_ppt_xor,
                                       mtx_encode_warnings_t *warnings) {
  byte_buffer_t block1_buf;
  byte_buffer_t block2_buf;
  byte_buffer_t block3_buf;
  uint8_t *compressed1 = NULL;
  size_t compressed1_size = 0;
  uint8_t *compressed2 = NULL;
  size_t compressed2_size = 0;
  uint8_t *compressed3 = NULL;
  size_t compressed3_size = 0;
  eot_status_t status = EOT_OK;

  byte_buffer_init(&block1_buf);
  byte_buffer_init(&block2_buf);
  byte_buffer_init(&block3_buf);
  mtx_encode_warnings_init(warnings);

  status = build_encoded_blocks(font, &block1_buf, &block2_buf, &block3_buf, warnings);
  if (status != EOT_OK) {
    goto cleanup;
  }

  status = compress_blocks_in_order(&block1_buf, &block2_buf, &block3_buf,
                                    &compressed1, &compressed1_size,
                                    &compressed2, &compressed2_size,
                                    &compressed3, &compressed3_size);
  if (status != EOT_OK) {
    goto cleanup;
  }

  if (status != EOT_OK) {
    return status;
  }
  encoded_blocks_t blocks = {0};
  blocks.block1 = compressed1;
  blocks.block1_size = compressed1_size;
  blocks.block2 = compressed2;
  blocks.block2_size = compressed2_size;
  blocks.block3 = compressed3;
  blocks.block3_size = compressed3_size;

  uint8_t *mtx_data = NULL;
  size_t mtx_size = 0;
  status = mtx_container_pack(&blocks, &mtx_data, &mtx_size);
  if (status != EOT_OK) {
    goto cleanup;
  }
  sfnt_table_t *head = sfnt_font_get_table((sfnt_font_t *)font, TAG_head);
  sfnt_table_t *os2 = sfnt_font_get_table((sfnt_font_t *)font, TAG_OS_2);

  if (head == NULL || os2 == NULL) {
    free(mtx_data);
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }
  size_t header_size = 120;
  size_t total_size = header_size + mtx_size;

  byte_buffer_init(out);
  status = byte_buffer_reserve(out, total_size);
  if (status != EOT_OK) {
    free(mtx_data);
    goto cleanup;
  }

  uint8_t header[120] = {0};
  size_t pos = 0;

  // EOTSize
  write_u32le(header + pos, (uint32_t)total_size);
  pos += 4;

  // FontDataSize
  write_u32le(header + pos, (uint32_t)mtx_size);
  pos += 4;

  // Version
  write_u32le(header + pos, 0x00020002);
  pos += 4;

  // Flags (compressed, optionally PowerPoint-style XOR obfuscated)
  write_u32le(header + pos,
              EOT_FLAG_COMPRESSED | (apply_ppt_xor ? EOT_FLAG_PPT_XOR : 0u));
  pos += 4;

  // FontPANOSE (10 bytes from OS/2)
  if (os2->length >= 42) {
    memcpy(header + pos, os2->data + 32, 10);
  }
  pos += 10;

  // Charset
  header[pos++] = 1;

  // Italic
  if (os2->length >= 63) {
    uint16_t selection = read_u16be(os2->data + 62);
    header[pos++] = (selection & 1) ? 1 : 0;
  } else {
    header[pos++] = 0;
  }

  // Weight
  if (os2->length >= 6) {
    write_u32le(header + pos, read_u16be(os2->data + 4));
  }
  pos += 4;

  // fsType
  if (os2->length >= 10) {
    write_u16le(header + pos, read_u16be(os2->data + 8));
  }
  pos += 2;

  // MagicNumber
  write_u16le(header + pos, 0x504c);
  pos += 2;

  // UnicodeRange (16 bytes from OS/2)
  if (os2->length >= 58) {
    for (int i = 0; i < 4; i++) {
      write_u32le(header + pos, read_u32be(os2->data + 42 + i * 4));
      pos += 4;
    }
  } else {
    pos += 16;
  }

  // CodePageRange (8 bytes from OS/2)
  if (os2->length >= 86) {
    for (int i = 0; i < 2; i++) {
      write_u32le(header + pos, read_u32be(os2->data + 78 + i * 4));
      pos += 4;
    }
  } else {
    pos += 8;
  }

  // CheckSumAdjustment
  if (head->length >= 12) {
    write_u32le(header + pos, read_u32be(head->data + 8));
  }
  pos += 4;

  // Reserved (16 bytes)
  pos += 16;

  // Padding1
  write_u16le(header + pos, 0);
  pos += 2;

  // FamilyName (empty string)
  write_u16le(header + pos, 0);
  pos += 2;

  // Padding2
  write_u16le(header + pos, 0);
  pos += 2;

  // StyleName (empty string)
  write_u16le(header + pos, 0);
  pos += 2;

  // Padding3
  write_u16le(header + pos, 0);
  pos += 2;

  // VersionName (empty string)
  write_u16le(header + pos, 0);
  pos += 2;

  // Padding4
  write_u16le(header + pos, 0);
  pos += 2;

  // FullName (empty string)
  write_u16le(header + pos, 0);
  pos += 2;

  // Padding5
  write_u16le(header + pos, 0);
  pos += 2;

  // RootString (empty string)
  write_u16le(header + pos, 0);
  pos += 2;

  // RootStringChecksum
  write_u32le(header + pos, 0);
  pos += 4;

  // EUDCCodePage
  write_u32le(header + pos, 0);
  pos += 4;

  // Padding6
  write_u16le(header + pos, 0);
  pos += 2;

  // SignatureSize
  write_u16le(header + pos, 0);
  pos += 2;

  // EUDCFlags
  write_u32le(header + pos, 0);
  pos += 4;

  // EUDCFontSize
  write_u32le(header + pos, 0);
  pos += 4;

  status = byte_buffer_append(out, header, pos);
  if (status != EOT_OK) {
    free(mtx_data);
    goto cleanup;
  }

  if (apply_ppt_xor) {
    xor_buffer(mtx_data, mtx_size, 0x50u);
  }

  status = byte_buffer_append(out, mtx_data, mtx_size);
  free(mtx_data);
  if (status != EOT_OK) {
    goto cleanup;
  }

cleanup:
  byte_buffer_destroy(&block1_buf);
  byte_buffer_destroy(&block2_buf);
  byte_buffer_destroy(&block3_buf);
  free(compressed1);
  free(compressed2);
  free(compressed3);
  return status;
}

eot_status_t mtx_encode_font(const sfnt_font_t *font, byte_buffer_t *out) {
  return simple_encode_font(font, out, 0, NULL);
}

eot_status_t mtx_encode_font_with_ppt_xor(const sfnt_font_t *font, byte_buffer_t *out) {
  return simple_encode_font(font, out, 1, NULL);
}

eot_status_t mtx_encode_ttf_file(const char *path, byte_buffer_t *out) {
  return mtx_encode_ttf_file_with_warnings(path, out, NULL);
}

eot_status_t mtx_encode_font_with_warnings(const sfnt_font_t *font, byte_buffer_t *out,
                                           mtx_encode_warnings_t *warnings) {
  return simple_encode_font(font, out, 0, warnings);
}

eot_status_t mtx_encode_font_with_ppt_xor_and_warnings(const sfnt_font_t *font,
                                                       byte_buffer_t *out,
                                                       mtx_encode_warnings_t *warnings) {
  return simple_encode_font(font, out, 1, warnings);
}

static eot_status_t load_sfnt_for_encode(const char *path, sfnt_font_t *font) {
  file_buffer_t buffer = {0};
  sfnt_font_t parsed;
  sfnt_table_t *source_post = NULL;
  eot_status_t status;

  if (path == NULL || font == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  sfnt_font_init(font);

  status = file_io_read_all(path, &buffer);
  if (status != EOT_OK) {
    return status;
  }

  status = sfnt_reader_parse(buffer.data, buffer.length, &parsed);
  if (status != EOT_OK) {
    file_io_free(&buffer);
    return status;
  }

  if (buffer.length >= 4 &&
      read_u32be(buffer.data) == SFNT_VERSION_OTTO &&
      sfnt_font_has_table(&parsed, TAG_CFF)) {
    source_post = sfnt_font_get_table(&parsed, TAG_post);

    status = otf_convert_to_truetype_sfnt(buffer.data, buffer.length, NULL, font);
    if (status == EOT_OK && source_post != NULL) {
      status = sfnt_font_add_table(font, TAG_post, source_post->data, source_post->length);
    }
    if (status != EOT_OK) {
      sfnt_font_destroy(font);
    }
    sfnt_font_destroy(&parsed);
    file_io_free(&buffer);
    return status;
  }

  if (sfnt_font_has_table(&parsed, TAG_CFF2)) {
    sfnt_font_destroy(&parsed);
    file_io_free(&buffer);
    return EOT_ERR_INVALID_ARGUMENT;
  }

  file_io_free(&buffer);
  *font = parsed;
  return EOT_OK;
}

eot_status_t mtx_encode_ttf_file_with_warnings(const char *path, byte_buffer_t *out,
                                               mtx_encode_warnings_t *warnings) {
  sfnt_font_t font;
  eot_status_t status = load_sfnt_for_encode(path, &font);
  if (status != EOT_OK) {
    return status;
  }

  status = mtx_encode_font_with_warnings(&font, out, warnings);
  sfnt_font_destroy(&font);

  return status;
}

eot_status_t mtx_encode_ttf_file_with_ppt_xor(const char *path, byte_buffer_t *out) {
  return mtx_encode_ttf_file_with_ppt_xor_and_warnings(path, out, NULL);
}

eot_status_t mtx_encode_ttf_file_with_ppt_xor_and_warnings(const char *path,
                                                           byte_buffer_t *out,
                                                           mtx_encode_warnings_t *warnings) {
  sfnt_font_t font;
  eot_status_t status = load_sfnt_for_encode(path, &font);
  if (status != EOT_OK) {
    return status;
  }

  status = mtx_encode_font_with_ppt_xor_and_warnings(&font, out, warnings);
  sfnt_font_destroy(&font);

  return status;
}
