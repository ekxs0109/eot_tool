#include "native_bridge.h"

#include <hb.h>
#include <hb-ot.h>
#include <unistd.h>
#include <cstring>
#include <cstdio>
#include <cstdlib>

#include "otf_convert.h"
#include "sfnt_font.h"
#include "sfnt_writer.h"

extern "C" int fonttool_cff_convert_static_otf_to_ttf(
    const uint8_t* input_bytes,
    size_t input_length,
    fonttool_cff_native_buffer_t* out_buffer) {
  static const char kEnterMessage[] = "native_bridge: entered\n";
  write(2, kEnterMessage, sizeof(kEnterMessage) - 1);
  if (input_bytes == nullptr || input_length == 0 || out_buffer == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  out_buffer->data = nullptr;
  out_buffer->length = 0;

  sfnt_font_t converted_font;
  sfnt_font_init(&converted_font);
  std::fprintf(stderr, "native_bridge: starting convert, len=%zu\n", input_length);
  std::fprintf(stderr, "native_bridge: stack align=%zu input align=%zu out align=%zu\n",
               static_cast<size_t>(reinterpret_cast<uintptr_t>(&converted_font) & 0xFu),
               static_cast<size_t>(reinterpret_cast<uintptr_t>(input_bytes) & 0xFu),
               static_cast<size_t>(reinterpret_cast<uintptr_t>(out_buffer) & 0xFu));
  if (input_length >= 8) {
    std::fprintf(stderr,
                 "native_bridge: first-bytes=%02X %02X %02X %02X %02X %02X %02X %02X\n",
                 input_bytes[0], input_bytes[1], input_bytes[2], input_bytes[3],
                 input_bytes[4], input_bytes[5], input_bytes[6], input_bytes[7]);
  }
  std::fflush(stderr);
  char* copied_bytes = static_cast<char*>(std::malloc(input_length));
  if (copied_bytes != nullptr) {
    memcpy(copied_bytes, input_bytes, input_length);
    hb_blob_t* copied_blob = hb_blob_create_or_fail(
        copied_bytes,
        static_cast<unsigned int>(input_length),
        HB_MEMORY_MODE_WRITABLE,
        copied_bytes,
        [](void* user_data) { std::free(user_data); });
    if (copied_blob != nullptr) {
      hb_face_t* copied_face = hb_face_create_or_fail(copied_blob, 0u);
      if (copied_face != nullptr) {
        std::fprintf(stderr, "native_bridge: hb copied glyphs=%u upem=%u\n",
                     hb_face_get_glyph_count(copied_face), hb_face_get_upem(copied_face));
        std::fflush(stderr);
        hb_face_destroy(copied_face);
      }
      hb_blob_destroy(copied_blob);
      copied_bytes = nullptr;
    }
    if (copied_bytes != nullptr) {
      std::free(copied_bytes);
    }
  }

  hb_blob_t* direct_blob = hb_blob_create_or_fail(
      reinterpret_cast<const char*>(input_bytes),
      static_cast<unsigned int>(input_length),
      HB_MEMORY_MODE_READONLY,
      nullptr,
      nullptr);
  if (direct_blob != nullptr) {
    hb_face_t* direct_face = hb_face_create_or_fail(direct_blob, 0u);
    if (direct_face != nullptr) {
      std::fprintf(stderr, "native_bridge: hb direct glyphs=%u upem=%u\n",
                   hb_face_get_glyph_count(direct_face), hb_face_get_upem(direct_face));
      std::fflush(stderr);
      hb_face_destroy(direct_face);
    }
    hb_blob_destroy(direct_blob);
  }

  eot_status_t status =
      otf_convert_to_truetype_sfnt(input_bytes, input_length, nullptr, &converted_font);
  std::fprintf(stderr, "native_bridge: otf_convert status=%d tables=%zu\n", status,
               converted_font.num_tables);
  std::fflush(stderr);
  if (status != EOT_OK) {
    sfnt_font_destroy(&converted_font);
    return status;
  }

  uint8_t* output_bytes = nullptr;
  size_t output_length = 0u;
  status = sfnt_writer_serialize(&converted_font, &output_bytes, &output_length);
  std::fprintf(stderr, "native_bridge: sfnt_writer status=%d len=%zu\n", status,
               output_length);
  std::fflush(stderr);
  sfnt_font_destroy(&converted_font);
  if (status != EOT_OK) {
    std::free(output_bytes);
    return status;
  }

  out_buffer->data = output_bytes;
  out_buffer->length = output_length;
  return EOT_OK;
}

extern "C" void fonttool_cff_native_buffer_free(fonttool_cff_native_buffer_t* buffer) {
  if (buffer == nullptr) {
    return;
  }

  std::free(buffer->data);
  buffer->data = nullptr;
  buffer->length = 0;
}
