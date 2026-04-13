#include "wasm_api.h"

#include <cstring>
#include <cstdlib>

extern "C" {
#include "cff_reader.h"
#include "cff_variation.h"
#include "mtx_encode.h"
#include "otf_convert.h"
#include "sfnt_font.h"
#include "sfnt_reader.h"
}

namespace {

constexpr uint32_t TAG_CFF = 0x43464620u;
constexpr uint32_t TAG_CFF2 = 0x43464632u;

bool IsCffFlavor(const sfnt_font_t& font) {
  return sfnt_font_has_table(const_cast<sfnt_font_t*>(&font), TAG_CFF) ||
         sfnt_font_has_table(const_cast<sfnt_font_t*>(&font), TAG_CFF2);
}

const char *GetRuntimeThreadMode(void) {
#if defined(__EMSCRIPTEN_PTHREADS__) || (!defined(__EMSCRIPTEN__) && defined(_REENTRANT))
  return "pthreads";
#else
  return "single-thread";
#endif
}

eot_status_t ResolveVariationLocation(const uint8_t *input, size_t input_size,
                                      const char *variation_axes,
                                      variation_location_t *location) {
  cff_font_t cff_font;
  eot_status_t status;

  if (variation_axes == nullptr || variation_axes[0] == '\0') {
    return EOT_OK;
  }
  if (input == nullptr || input_size == 0 || location == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  cff_font_init(&cff_font);
  status = cff_reader_load_memory(input, input_size, &cff_font);
  if (status == EOT_OK) {
    status = variation_location_init_from_axis_map(location, variation_axes);
  }
  if (status == EOT_OK) {
    status = cff_variation_resolve_location(&cff_font, location);
  }
  cff_font_destroy(&cff_font);
  return status;
}

}  // namespace

extern "C" const char *wasm_runtime_thread_mode(void) {
  return GetRuntimeThreadMode();
}

extern "C" void wasm_buffer_destroy(wasm_buffer_t *buffer) {
  if (buffer == nullptr) {
    return;
  }

  std::free(buffer->data);
  buffer->data = nullptr;
  buffer->length = 0;
}

extern "C" eot_status_t wasm_convert_otf_to_embedded_font(const uint8_t *input,
                                                          size_t input_size,
                                                          const char *output_kind,
                                                          const char *variation_axes,
                                                          wasm_buffer_t *out) {
  sfnt_font_t parsed_font;
  sfnt_font_t converted_font;
  variation_location_t location;
  byte_buffer_t encoded;
  eot_status_t status;
  int output_fntdata;

  if (input == nullptr || input_size == 0 || output_kind == nullptr || out == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  output_fntdata = std::strcmp(output_kind, "fntdata") == 0;
  if (!output_fntdata && std::strcmp(output_kind, "eot") != 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  out->data = nullptr;
  out->length = 0;
  sfnt_font_init(&parsed_font);
  sfnt_font_init(&converted_font);
  variation_location_init(&location);
  byte_buffer_init(&encoded);

  status = sfnt_reader_parse(input, input_size, &parsed_font);
  if (status != EOT_OK) {
    goto cleanup;
  }

  if (IsCffFlavor(parsed_font)) {
    status = ResolveVariationLocation(input, input_size, variation_axes, &location);
    if (status != EOT_OK) {
      goto cleanup;
    }

    status = otf_convert_to_truetype_sfnt(
        input, input_size,
        (location.num_axes > 0 || (variation_axes != nullptr && variation_axes[0] != '\0'))
            ? &location
            : nullptr,
        &converted_font);
    if (status != EOT_OK) {
      goto cleanup;
    }
  } else {
    if (variation_axes != nullptr && variation_axes[0] != '\0') {
      status = EOT_ERR_INVALID_ARGUMENT;
      goto cleanup;
    }
    converted_font = parsed_font;
    sfnt_font_init(&parsed_font);
  }

  status = output_fntdata ? mtx_encode_font_with_ppt_xor(&converted_font, &encoded)
                          : mtx_encode_font(&converted_font, &encoded);
  if (status != EOT_OK) {
    goto cleanup;
  }

  out->data = encoded.data;
  out->length = encoded.length;
  encoded.data = nullptr;
  encoded.length = 0;
  encoded.capacity = 0;

cleanup:
  byte_buffer_destroy(&encoded);
  variation_location_destroy(&location);
  sfnt_font_destroy(&parsed_font);
  sfnt_font_destroy(&converted_font);
  return status;
}
