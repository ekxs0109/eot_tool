extern "C" {
#include "subset_backend_harfbuzz.h"
#include "sfnt_reader.h"
#include "sfnt_writer.h"
}

#include <cstdlib>
#include <limits>

#include <hb-subset.h>

#if !defined(HB_SUBSET_FLAGS_NO_LAYOUT_CLOSURE)
#define HB_SUBSET_FLAGS_NO_LAYOUT_CLOSURE 0u
#endif

#if !defined(HB_SUBSET_FLAGS_NO_BIDI_CLOSURE)
#define HB_SUBSET_FLAGS_NO_BIDI_CLOSURE 0u
#endif

#if defined(__EMSCRIPTEN__) && !defined(EOT_WASM_CUSTOM_HARFBUZZ)
#define HB_FACE_CREATE_OR_FAIL(blob, index) hb_face_create((blob), (index))
#else
#define HB_FACE_CREATE_OR_FAIL(blob, index) hb_face_create_or_fail((blob), (index))
#endif

static eot_status_t copy_face_blob_to_font(hb_face_t *face, sfnt_font_t *output) {
  hb_blob_t *blob = hb_face_reference_blob(face);
  unsigned int length = 0;
  const char *data = nullptr;
  eot_status_t status;

  if (blob == nullptr) {
    return EOT_ERR_CORRUPT_DATA;
  }

  data = hb_blob_get_data(blob, &length);
  if (data == nullptr || length == 0) {
    hb_blob_destroy(blob);
    return EOT_ERR_CORRUPT_DATA;
  }

  status = sfnt_reader_parse(reinterpret_cast<const uint8_t *>(data),
                             static_cast<size_t>(length),
                             output);
  hb_blob_destroy(blob);
  return status;
}

extern "C" eot_status_t subset_backend_harfbuzz_run(const sfnt_font_t *input,
                                                    const subset_plan_t *plan,
                                                    sfnt_font_t *output,
                                                    subset_warnings_t *warnings) {
  uint8_t *serialized = nullptr;
  size_t serialized_size = 0;
  hb_blob_t *blob = nullptr;
  hb_face_t *face = nullptr;
  hb_subset_input_t *subset_input = nullptr;
  hb_face_t *subset_face = nullptr;
  eot_status_t status = EOT_OK;
  unsigned int flags = HB_SUBSET_FLAGS_DEFAULT |
                       HB_SUBSET_FLAGS_PASSTHROUGH_UNRECOGNIZED |
                       HB_SUBSET_FLAGS_NOTDEF_OUTLINE |
                       HB_SUBSET_FLAGS_NO_LAYOUT_CLOSURE |
                       HB_SUBSET_FLAGS_NO_BIDI_CLOSURE;

  if (input == nullptr || plan == nullptr || output == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  (void)warnings;

  status = sfnt_writer_serialize(const_cast<sfnt_font_t *>(input),
                                 &serialized,
                                 &serialized_size);
  if (status != EOT_OK) {
    return status;
  }

  if (serialized_size > std::numeric_limits<unsigned int>::max()) {
    free(serialized);
    return EOT_ERR_INVALID_ARGUMENT;
  }

  blob = hb_blob_create(reinterpret_cast<const char *>(serialized),
                        static_cast<unsigned int>(serialized_size),
                        HB_MEMORY_MODE_WRITABLE,
                        serialized,
                        free);
  if (blob == nullptr) {
    free(serialized);
    return EOT_ERR_ALLOCATION;
  }

  face = HB_FACE_CREATE_OR_FAIL(blob, 0);
  if (face == nullptr) {
    hb_blob_destroy(blob);
    return EOT_ERR_CORRUPT_DATA;
  }

  subset_input = hb_subset_input_create_or_fail();
  if (subset_input == nullptr) {
    hb_face_destroy(face);
    hb_blob_destroy(blob);
    return EOT_ERR_ALLOCATION;
  }

  if (plan->keep_gids) {
#if HB_VERSION_ATLEAST(2, 9, 0)
    flags |= HB_SUBSET_FLAGS_RETAIN_GIDS;
#else
    /* This build cannot satisfy keep-gids semantics without retain-gids. */
    status = EOT_ERR_INVALID_ARGUMENT;
#endif
  }
  if (status != EOT_OK) {
    goto cleanup;
  }
  hb_subset_input_set_flags(subset_input, flags);

  {
    hb_set_t *glyph_set = hb_subset_input_glyph_set(subset_input);
    size_t i;

    for (i = 0; i < plan->num_glyphs; i++) {
      hb_set_add(glyph_set, plan->included_glyph_ids[i]);
    }

    if (!hb_set_allocation_successful(glyph_set)) {
      status = EOT_ERR_ALLOCATION;
    }
  }

  if (status == EOT_OK) {
    subset_face = hb_subset_or_fail(face, subset_input);
    if (subset_face == nullptr) {
      status = EOT_ERR_CORRUPT_DATA;
    }
  }

  if (status == EOT_OK) {
    status = copy_face_blob_to_font(subset_face, output);
  }

cleanup:
  if (subset_face != nullptr) {
    hb_face_destroy(subset_face);
  }
  if (subset_input != nullptr) {
    hb_subset_input_destroy(subset_input);
  }
  if (face != nullptr) {
    hb_face_destroy(face);
  }
  if (blob != nullptr) {
    hb_blob_destroy(blob);
  }

  return status;
}
