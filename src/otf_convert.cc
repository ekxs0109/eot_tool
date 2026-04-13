#include "otf_convert.h"

#include <hb.h>
#include <hb-ot.h>

#include <cmath>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

#include "parallel_runtime.h"

extern "C" {
#include "byte_io.h"
#include "cu2qu.h"
#include "sfnt_reader.h"
#include "tt_rebuilder.h"
}

namespace {

constexpr uint32_t TAG_CFF = 0x43464620u;
constexpr uint32_t TAG_CFF2 = 0x43464632u;
constexpr uint32_t TAG_DSIG = 0x44534947u;
constexpr uint32_t TAG_HVAR = 0x48564152u;
constexpr uint32_t TAG_MVAR = 0x4d564152u;
constexpr uint32_t TAG_VORG = 0x564f5247u;
constexpr uint32_t TAG_VVAR = 0x56564152u;
constexpr uint32_t TAG_avar = 0x61766172u;
constexpr uint32_t TAG_cvar = 0x63766172u;
constexpr uint32_t TAG_fvar = 0x66766172u;
constexpr uint32_t TAG_gvar = 0x67766172u;
constexpr uint32_t TAG_glyf = 0x676c7966u;
constexpr uint32_t TAG_head = 0x68656164u;
constexpr uint32_t TAG_hhea = 0x68686561u;
constexpr uint32_t TAG_hmtx = 0x686d7478u;
constexpr uint32_t TAG_loca = 0x6c6f6361u;
constexpr uint32_t TAG_maxp = 0x6d617870u;
constexpr double kCu2QuMaxError = 1.0;

struct OutlineCapture {
  std::vector<std::vector<tt_outline_point_t>> contours;
  std::vector<tt_outline_point_t> current_contour;
  float contour_start_x = 0.0f;
  float contour_start_y = 0.0f;
  float current_x = 0.0f;
  float current_y = 0.0f;
  bool contour_open = false;
  eot_status_t status = EOT_OK;
};

struct GlyphBuildResult {
  tt_glyph_outline_t outline = {};
  eot_status_t status = EOT_OK;
};

struct GlyphBuildTaskContext {
  hb_face_t *face = nullptr;
  hb_draw_funcs_t *draw_funcs = nullptr;
  const std::vector<int> *normalized_coords = nullptr;
  unsigned int upem = 0u;
  std::vector<GlyphBuildResult> *results = nullptr;
};

bool PointEquals(const tt_outline_point_t &lhs, const tt_outline_point_t &rhs) {
  return lhs.x == rhs.x && lhs.y == rhs.y && lhs.on_curve == rhs.on_curve;
}

float Clamp(float value, float minimum, float maximum) {
  if (value < minimum) {
    return minimum;
  }
  if (value > maximum) {
    return maximum;
  }
  return value;
}

eot_status_t RoundToInt16(float value, int16_t *out_value) {
  long rounded;

  if (out_value == nullptr || !std::isfinite(value)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  rounded = std::lround(value);
  if (rounded < std::numeric_limits<int16_t>::min() ||
      rounded > std::numeric_limits<int16_t>::max()) {
    return EOT_ERR_CORRUPT_DATA;
  }

  *out_value = static_cast<int16_t>(rounded);
  return EOT_OK;
}

eot_status_t AppendPoint(OutlineCapture *capture, float x, float y, int on_curve) {
  tt_outline_point_t point = {};
  eot_status_t status;

  if (capture == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = RoundToInt16(x, &point.x);
  if (status != EOT_OK) {
    return status;
  }
  status = RoundToInt16(y, &point.y);
  if (status != EOT_OK) {
    return status;
  }
  point.on_curve = on_curve != 0 ? 1 : 0;

  if (!capture->current_contour.empty() &&
      PointEquals(capture->current_contour.back(), point)) {
    return EOT_OK;
  }

  capture->current_contour.push_back(point);
  return EOT_OK;
}

void FinishCurrentContour(OutlineCapture *capture) {
  if (capture == nullptr || !capture->contour_open) {
    return;
  }

  capture->contour_open = false;

  if (capture->status != EOT_OK) {
    capture->current_contour.clear();
    return;
  }

  if (capture->current_contour.size() <= 1u) {
    capture->current_contour.clear();
    return;
  }

  if (capture->current_contour.front().on_curve != 0 &&
      capture->current_contour.back().on_curve != 0 &&
      capture->current_contour.front().x == capture->current_contour.back().x &&
      capture->current_contour.front().y == capture->current_contour.back().y) {
    capture->current_contour.pop_back();
  }

  if (!capture->current_contour.empty()) {
    capture->contours.push_back(capture->current_contour);
  }
  capture->current_contour.clear();
}

void MoveToCallback(hb_draw_funcs_t *, void *draw_data, hb_draw_state_t *,
                    float to_x, float to_y, void *) {
  OutlineCapture *capture = static_cast<OutlineCapture *>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  FinishCurrentContour(capture);
  capture->contour_open = true;
  capture->current_contour.clear();
  capture->status = AppendPoint(capture, to_x, to_y, 1);
  if (capture->status == EOT_OK) {
    capture->contour_start_x = to_x;
    capture->contour_start_y = to_y;
    capture->current_x = to_x;
    capture->current_y = to_y;
  }
}

void LineToCallback(hb_draw_funcs_t *, void *draw_data, hb_draw_state_t *,
                    float to_x, float to_y, void *) {
  OutlineCapture *capture = static_cast<OutlineCapture *>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK || !capture->contour_open) {
    return;
  }

  capture->status = AppendPoint(capture, to_x, to_y, 1);
  if (capture->status == EOT_OK) {
    capture->current_x = to_x;
    capture->current_y = to_y;
  }
}

void QuadraticToCallback(hb_draw_funcs_t *, void *draw_data, hb_draw_state_t *,
                         float control_x, float control_y,
                         float to_x, float to_y, void *) {
  OutlineCapture *capture = static_cast<OutlineCapture *>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK || !capture->contour_open) {
    return;
  }

  capture->status = AppendPoint(capture, control_x, control_y, 0);
  if (capture->status == EOT_OK) {
    capture->status = AppendPoint(capture, to_x, to_y, 1);
  }
  if (capture->status == EOT_OK) {
    capture->current_x = to_x;
    capture->current_y = to_y;
  }
}

void CubicToCallback(hb_draw_funcs_t *, void *draw_data, hb_draw_state_t *,
                     float control1_x, float control1_y,
                     float control2_x, float control2_y,
                     float to_x, float to_y, void *) {
  OutlineCapture *capture = static_cast<OutlineCapture *>(draw_data);
  cubic_curve_t cubic = {};
  quadratic_spline_t spline = {};

  if (capture == nullptr || capture->status != EOT_OK || !capture->contour_open) {
    return;
  }

  cubic.p0.x = capture->current_x;
  cubic.p0.y = capture->current_y;
  cubic.p1.x = control1_x;
  cubic.p1.y = control1_y;
  cubic.p2.x = control2_x;
  cubic.p2.y = control2_y;
  cubic.p3.x = to_x;
  cubic.p3.y = to_y;

  capture->status = curve_to_quadratic(&cubic, kCu2QuMaxError, &spline);
  if (capture->status != EOT_OK) {
    return;
  }

  for (size_t i = 1; i < spline.num_points; ++i) {
    const int on_curve = (i + 1u == spline.num_points) ? 1 : 0;
    capture->status = AppendPoint(
        capture, static_cast<float>(spline.points[i].x),
        static_cast<float>(spline.points[i].y), on_curve);
    if (capture->status != EOT_OK) {
      break;
    }
  }

  quadratic_spline_destroy(&spline);
  if (capture->status == EOT_OK) {
    capture->current_x = to_x;
    capture->current_y = to_y;
  }
}

void ClosePathCallback(hb_draw_funcs_t *, void *draw_data, hb_draw_state_t *, void *) {
  FinishCurrentContour(static_cast<OutlineCapture *>(draw_data));
}

hb_draw_funcs_t *CreateDrawFuncs(void) {
  hb_draw_funcs_t *draw_funcs = hb_draw_funcs_create();
  if (draw_funcs == nullptr) {
    return nullptr;
  }

  hb_draw_funcs_set_move_to_func(draw_funcs, MoveToCallback, nullptr, nullptr);
  hb_draw_funcs_set_line_to_func(draw_funcs, LineToCallback, nullptr, nullptr);
  hb_draw_funcs_set_quadratic_to_func(draw_funcs, QuadraticToCallback, nullptr,
                                      nullptr);
  hb_draw_funcs_set_cubic_to_func(draw_funcs, CubicToCallback, nullptr, nullptr);
  hb_draw_funcs_set_close_path_func(draw_funcs, ClosePathCallback, nullptr, nullptr);
  hb_draw_funcs_make_immutable(draw_funcs);
  return draw_funcs;
}

char *DuplicateGlyphName(hb_font_t *font, hb_codepoint_t glyph_id) {
  char name[128];
  char *copy;
  size_t name_length;

  if (font == nullptr || !hb_font_get_glyph_name(font, glyph_id, name, sizeof(name))) {
    return nullptr;
  }

  name_length = std::strlen(name) + 1u;
  copy = static_cast<char *>(std::malloc(name_length));
  if (copy == nullptr) {
    return nullptr;
  }

  std::memcpy(copy, name, name_length);
  return copy;
}

eot_status_t BuildGlyphOutline(hb_font_t *font, hb_draw_funcs_t *draw_funcs,
                               hb_codepoint_t glyph_id,
                               tt_glyph_outline_t *out_outline) {
  OutlineCapture capture = {};
  hb_position_t advance_width;
  tt_contour_t *contours = nullptr;

  if (font == nullptr || draw_funcs == nullptr || out_outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (!hb_font_draw_glyph_or_fail(font, glyph_id, draw_funcs, &capture)) {
    return EOT_ERR_CORRUPT_DATA;
  }
  FinishCurrentContour(&capture);
  if (capture.status != EOT_OK) {
    return capture.status;
  }

  advance_width = hb_font_get_glyph_h_advance(font, glyph_id);
  if (advance_width < 0 || advance_width > std::numeric_limits<uint16_t>::max()) {
    return EOT_ERR_CORRUPT_DATA;
  }

  out_outline->advance_width = static_cast<uint16_t>(advance_width);
  out_outline->glyph_name = DuplicateGlyphName(font, glyph_id);
  if (out_outline->glyph_name == nullptr && glyph_id != 0u) {
    /* Glyph names are optional for rebuild; allocation failure is the only hard error. */
    char fallback_name[32];
    int written = std::snprintf(fallback_name, sizeof(fallback_name), "glyph%u",
                                static_cast<unsigned int>(glyph_id));
    if (written > 0 && static_cast<size_t>(written) < sizeof(fallback_name)) {
      size_t fallback_length = static_cast<size_t>(written) + 1u;
      out_outline->glyph_name = static_cast<char *>(std::malloc(fallback_length));
      if (out_outline->glyph_name == nullptr) {
        return EOT_ERR_ALLOCATION;
      }
      std::memcpy(out_outline->glyph_name, fallback_name, fallback_length);
    }
  }

  if (capture.contours.empty()) {
    return EOT_OK;
  }

  contours = static_cast<tt_contour_t *>(
      std::calloc(capture.contours.size(), sizeof(tt_contour_t)));
  if (contours == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  for (size_t contour_index = 0; contour_index < capture.contours.size();
       ++contour_index) {
    const std::vector<tt_outline_point_t> &source = capture.contours[contour_index];
    const size_t contour_size = source.size();
    tt_outline_point_t *points;

    if (contour_size == 0u) {
      continue;
    }

    points = static_cast<tt_outline_point_t *>(
        std::malloc(contour_size * sizeof(tt_outline_point_t)));
    if (points == nullptr) {
      for (size_t i = 0; i < contour_index; ++i) {
        std::free(contours[i].points);
      }
      std::free(contours);
      return EOT_ERR_ALLOCATION;
    }

    std::memcpy(points, source.data(), contour_size * sizeof(tt_outline_point_t));
    contours[contour_index].points = points;
    contours[contour_index].num_points = contour_size;
  }

  out_outline->contours = contours;
  out_outline->num_contours = capture.contours.size();
  return EOT_OK;
}

eot_status_t CreateTaskFont(const GlyphBuildTaskContext *context,
                            hb_font_t **out_font) {
  hb_font_t *font;

  if (out_font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  *out_font = nullptr;

  if (context == nullptr || context->face == nullptr || context->upem == 0u) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  font = hb_font_create(context->face);
  if (font == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  hb_ot_font_set_funcs(font);
  hb_font_set_scale(font, static_cast<int>(context->upem),
                    static_cast<int>(context->upem));
  if (context->normalized_coords != nullptr &&
      !context->normalized_coords->empty()) {
    hb_font_set_var_coords_normalized(
        font, context->normalized_coords->data(),
        static_cast<unsigned int>(context->normalized_coords->size()));
  }

  *out_font = font;
  return EOT_OK;
}

eot_status_t BuildGlyphOutlineTask(size_t index, void *context_ptr) {
  GlyphBuildTaskContext *context =
      static_cast<GlyphBuildTaskContext *>(context_ptr);
  GlyphBuildResult *result;
  hb_font_t *font = nullptr;
  eot_status_t status;

  if (context == nullptr || context->draw_funcs == nullptr ||
      context->results == nullptr || index >= context->results->size()) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  result = &(*context->results)[index];
  status = CreateTaskFont(context, &font);
  if (status != EOT_OK) {
    result->status = status;
    return EOT_OK;
  }

  result->status = BuildGlyphOutline(font, context->draw_funcs,
                                     static_cast<hb_codepoint_t>(index),
                                     &result->outline);
  hb_font_destroy(font);
  return EOT_OK;
}

eot_status_t FindFirstFailedStatus(const eot_status_t *statuses, size_t status_count,
                                   size_t *out_glyph_index) {
  if (statuses == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (out_glyph_index != nullptr) {
    *out_glyph_index = 0u;
  }
  for (size_t i = 0; i < status_count; ++i) {
    if (statuses[i] != EOT_OK) {
      if (out_glyph_index != nullptr) {
        *out_glyph_index = i;
      }
      return statuses[i];
    }
  }
  return EOT_OK;
}

bool ShouldCopySourceTable(uint32_t tag) {
  switch (tag) {
    case TAG_CFF:
    case TAG_CFF2:
    case TAG_DSIG:
    case TAG_HVAR:
    case TAG_MVAR:
    case TAG_VORG:
    case TAG_VVAR:
    case TAG_avar:
    case TAG_cvar:
    case TAG_fvar:
    case TAG_gvar:
    case TAG_glyf:
    case TAG_head:
    case TAG_hhea:
    case TAG_hmtx:
    case TAG_loca:
    case TAG_maxp:
      return false;
    default:
      return true;
  }
}

eot_status_t CopySourceTables(const sfnt_font_t *source, sfnt_font_t *destination) {
  if (source == nullptr || destination == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (size_t i = 0; i < source->num_tables; ++i) {
    const sfnt_table_t *table = &source->tables[i];
    if (!ShouldCopySourceTable(table->tag)) {
      continue;
    }

    eot_status_t status =
        sfnt_font_add_table(destination, table->tag, table->data, table->length);
    if (status != EOT_OK) {
      return status;
    }
  }

  return EOT_OK;
}

eot_status_t CopyAllTables(const sfnt_font_t *source, sfnt_font_t *destination) {
  if (source == nullptr || destination == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (size_t i = 0; i < source->num_tables; ++i) {
    const sfnt_table_t *table = &source->tables[i];
    eot_status_t status =
        sfnt_font_add_table(destination, table->tag, table->data, table->length);
    if (status != EOT_OK) {
      return status;
    }
  }

  return EOT_OK;
}

eot_status_t BuildResolvedNormalizedCoords(
    hb_face_t *face, const variation_location_t *location,
    std::vector<int> *normalized_coords) {
  std::vector<hb_ot_var_axis_info_t> axis_infos;
  unsigned int axis_count;

  if (face == nullptr || normalized_coords == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  normalized_coords->clear();
  if (location == nullptr || location->num_axes == 0u) {
    return EOT_OK;
  }
  if (location->axes == nullptr || !hb_ot_var_has_data(face)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  axis_count = hb_ot_var_get_axis_count(face);
  if (axis_count == 0u || location->num_axes != axis_count) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  axis_infos.resize(axis_count);
  if (axis_count > 0u) {
    unsigned int fetched_count = axis_count;
    hb_ot_var_get_axis_infos(face, 0u, &fetched_count, axis_infos.data());
    if (fetched_count != axis_count) {
      return EOT_ERR_CORRUPT_DATA;
    }
  }

  normalized_coords->reserve(axis_count);
  for (size_t i = 0; i < location->num_axes; ++i) {
    const variation_axis_value_t &source_axis = location->axes[i];

    if (hb_tag_from_string(source_axis.tag, 4) != axis_infos[i].tag) {
      return EOT_ERR_INVALID_ARGUMENT;
    }

    normalized_coords->push_back(static_cast<int>(
        std::lround(Clamp(source_axis.normalized_value, -1.0f, 1.0f) * 16384.0f)));
  }

  return EOT_OK;
}

}  // namespace

extern "C" eot_status_t otf_convert_to_truetype_sfnt(
    const uint8_t *font_bytes, size_t font_size,
    const variation_location_t *location, sfnt_font_t *out_font) {
  sfnt_font_t source_font;
  sfnt_font_t rebuilt_font;
  sfnt_font_t converted_font;
  hb_blob_t *blob = nullptr;
  hb_face_t *face = nullptr;
  hb_draw_funcs_t *draw_funcs = nullptr;
  std::vector<tt_glyph_outline_t> outlines;
  std::vector<GlyphBuildResult> glyph_results;
  std::vector<eot_status_t> glyph_statuses;
  std::vector<int> normalized_coords;
  GlyphBuildTaskContext task_context = {};
  unsigned int glyph_count;
  unsigned int upem;
  eot_status_t status = EOT_OK;

  if (font_bytes == nullptr || font_size == 0u || out_font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = sfnt_reader_parse(font_bytes, font_size, &source_font);
  if (status != EOT_OK) {
    return status;
  }
  sfnt_font_init(&rebuilt_font);
  sfnt_font_init(&converted_font);

  if (sfnt_font_has_table(&source_font, TAG_CFF2)) {
    /* CFF2 variable input is supported through HarfBuzz instancing. */
  } else if (!sfnt_font_has_table(&source_font, TAG_CFF)) {
    status = EOT_ERR_INVALID_ARGUMENT;
    goto cleanup;
  } else if (location != nullptr && location->num_axes > 0u) {
    status = EOT_ERR_INVALID_ARGUMENT;
    goto cleanup;
  }

  blob = hb_blob_create_or_fail(reinterpret_cast<const char *>(font_bytes),
                                static_cast<unsigned int>(font_size),
                                HB_MEMORY_MODE_READONLY, nullptr, nullptr);
  if (blob == nullptr) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  face = hb_face_create_or_fail(blob, 0u);
  if (face == nullptr) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  upem = hb_face_get_upem(face);
  if (upem == 0u) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  status = BuildResolvedNormalizedCoords(face, location, &normalized_coords);
  if (status != EOT_OK) {
    goto cleanup;
  }

  draw_funcs = CreateDrawFuncs();
  if (draw_funcs == nullptr) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  glyph_count = hb_face_get_glyph_count(face);
  if (glyph_count == 0u) {
    status = EOT_ERR_CORRUPT_DATA;
    goto cleanup;
  }

  glyph_results.resize(glyph_count);
  task_context.face = face;
  task_context.draw_funcs = draw_funcs;
  task_context.normalized_coords = &normalized_coords;
  task_context.upem = upem;
  task_context.results = &glyph_results;

  status = parallel_runtime_run_indexed_tasks(glyph_count, BuildGlyphOutlineTask,
                                              &task_context);
  if (status != EOT_OK) {
    goto cleanup;
  }

  glyph_statuses.resize(glyph_count);
  for (unsigned int glyph_index = 0; glyph_index < glyph_count; ++glyph_index) {
    glyph_statuses[glyph_index] = glyph_results[glyph_index].status;
  }
  status = FindFirstFailedStatus(glyph_statuses.data(), glyph_statuses.size(), nullptr);
  if (status != EOT_OK) {
    goto cleanup;
  }

  outlines.resize(glyph_count);
  for (unsigned int glyph_index = 0; glyph_index < glyph_count; ++glyph_index) {
    outlines[glyph_index] = glyph_results[glyph_index].outline;
    glyph_results[glyph_index].outline = {};
  }

  status = tt_rebuilder_build_font(outlines.data(), outlines.size(), &rebuilt_font);
  if (status != EOT_OK) {
    goto cleanup;
  }

  status = CopySourceTables(&source_font, &converted_font);
  if (status == EOT_OK) {
    status = CopyAllTables(&rebuilt_font, &converted_font);
  }
  if (status != EOT_OK) {
    goto cleanup;
  }

  *out_font = converted_font;
  sfnt_font_init(&converted_font);

cleanup:
  for (size_t i = 0; i < glyph_results.size(); ++i) {
    tt_glyph_outline_destroy(&glyph_results[i].outline);
  }
  for (size_t i = 0; i < outlines.size(); ++i) {
    tt_glyph_outline_destroy(&outlines[i]);
  }
  hb_draw_funcs_destroy(draw_funcs);
  hb_face_destroy(face);
  hb_blob_destroy(blob);
  sfnt_font_destroy(&converted_font);
  sfnt_font_destroy(&rebuilt_font);
  sfnt_font_destroy(&source_font);
  return status;
}

#if defined(FONTTOOL_TESTING)
extern "C" eot_status_t otf_convert_find_first_failed_status_for_testing(
    const eot_status_t *statuses, size_t status_count, size_t *out_glyph_index) {
  return FindFirstFailedStatus(statuses, status_count, out_glyph_index);
}
#endif
