#include "cff_variation.h"

#include <ctype.h>

#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <vector>

namespace {

struct ParsedAxisValue {
  char tag[5];
  float value;
};

bool IsAsciiWhitespace(char ch) {
  return ch != '\0' && isspace(static_cast<unsigned char>(ch)) != 0;
}

const char *SkipLeadingWhitespace(const char *cursor) {
  while (cursor != nullptr && IsAsciiWhitespace(*cursor)) {
    ++cursor;
  }
  return cursor;
}

const char *TrimTrailingWhitespace(const char *begin, const char *end) {
  while (end > begin && IsAsciiWhitespace(*(end - 1))) {
    --end;
  }
  return end;
}

bool TagsEqual(const char *lhs, const char *rhs) {
  return std::memcmp(lhs, rhs, 4) == 0;
}

const cff_axis_t *FindAxis(const cff_font_t *font, const char *tag) {
  if (font == nullptr || tag == nullptr) {
    return nullptr;
  }

  for (size_t i = 0; i < font->num_axes; ++i) {
    if (TagsEqual(font->axes[i].tag, tag)) {
      return &font->axes[i];
    }
  }

  return nullptr;
}

const variation_axis_value_t *FindLocationAxis(
    const variation_location_t *location, const char *tag) {
  if (location == nullptr || tag == nullptr) {
    return nullptr;
  }

  for (size_t i = 0; i < location->num_axes; ++i) {
    if (TagsEqual(location->axes[i].tag, tag)) {
      return &location->axes[i];
    }
  }

  return nullptr;
}

float NormalizeAxisValue(const cff_axis_t *axis, float design_value) {
  if (axis == nullptr) {
    return 0.0f;
  }
  if (design_value <= axis->default_value) {
    if (axis->default_value <= axis->min_value) {
      return 0.0f;
    }
    return (design_value - axis->default_value) /
           (axis->default_value - axis->min_value);
  }
  if (axis->max_value <= axis->default_value) {
    return 0.0f;
  }
  return (design_value - axis->default_value) /
         (axis->max_value - axis->default_value);
}

float ApplyAvarMapping(const cff_axis_t *axis, float normalized_value) {
  if (axis == nullptr || axis->num_avar_mappings == 0 ||
      axis->avar_mappings == nullptr) {
    return normalized_value;
  }
  if (normalized_value <= axis->avar_mappings[0].from_coordinate) {
    return axis->avar_mappings[0].to_coordinate;
  }

  for (size_t i = 1; i < axis->num_avar_mappings; ++i) {
    const cff_avar_mapping_t &previous = axis->avar_mappings[i - 1];
    const cff_avar_mapping_t &current = axis->avar_mappings[i];
    if (normalized_value > current.from_coordinate) {
      continue;
    }

    const float span = current.from_coordinate - previous.from_coordinate;
    if (span == 0.0f) {
      return current.to_coordinate;
    }

    const float t = (normalized_value - previous.from_coordinate) / span;
    return previous.to_coordinate +
           t * (current.to_coordinate - previous.to_coordinate);
  }

  return axis->avar_mappings[axis->num_avar_mappings - 1].to_coordinate;
}

bool HasDuplicateTags(const variation_location_t *location) {
  if (location == nullptr) {
    return false;
  }

  for (size_t i = 0; i < location->num_axes; ++i) {
    for (size_t j = i + 1; j < location->num_axes; ++j) {
      if (TagsEqual(location->axes[i].tag, location->axes[j].tag)) {
        return true;
      }
    }
  }

  return false;
}

eot_status_t ParseAxisAssignment(const char *begin, const char *end,
                                 ParsedAxisValue *out_axis) {
  const char *equals;
  const char *tag_begin;
  const char *tag_end;
  const char *value_begin;
  const char *value_end;
  char *parse_end = nullptr;
  float parsed_value;

  if (begin == nullptr || end == nullptr || out_axis == nullptr || end <= begin) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  equals = static_cast<const char *>(std::memchr(begin, '=', end - begin));
  if (equals == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  tag_begin = SkipLeadingWhitespace(begin);
  tag_end = TrimTrailingWhitespace(tag_begin, equals);
  if (tag_end - tag_begin != 4) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  value_begin = SkipLeadingWhitespace(equals + 1);
  value_end = TrimTrailingWhitespace(value_begin, end);
  if (value_end <= value_begin) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  char value_buffer[64];
  const ptrdiff_t value_length = value_end - value_begin;
  if (value_length <= 0 ||
      static_cast<size_t>(value_length) >= sizeof(value_buffer)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  std::memcpy(out_axis->tag, tag_begin, 4);
  out_axis->tag[4] = '\0';

  std::memcpy(value_buffer, value_begin, static_cast<size_t>(value_length));
  value_buffer[value_length] = '\0';

  parsed_value = std::strtof(value_buffer, &parse_end);
  if (parse_end == value_buffer || *parse_end != '\0') {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  out_axis->value = parsed_value;
  return EOT_OK;
}

}  // namespace

extern "C" void variation_location_init(variation_location_t *location) {
  if (location == nullptr) {
    return;
  }

  location->axes = nullptr;
  location->num_axes = 0;
}

extern "C" void variation_location_destroy(variation_location_t *location) {
  if (location == nullptr) {
    return;
  }

  std::free(location->axes);
  location->axes = nullptr;
  location->num_axes = 0;
}

extern "C" eot_status_t variation_location_init_from_axis_map(
    variation_location_t *location, const char *axis_map) {
  std::vector<ParsedAxisValue> parsed_axes;
  const char *cursor;

  if (location == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  variation_location_destroy(location);
  if (axis_map == nullptr) {
    return EOT_OK;
  }

  cursor = axis_map;
  while (*cursor != '\0') {
    const char *segment_begin;
    const char *segment_end;
    ParsedAxisValue parsed_axis = {};

    cursor = SkipLeadingWhitespace(cursor);
    if (*cursor == '\0') {
      break;
    }

    segment_begin = cursor;
    while (*cursor != '\0' && *cursor != ',') {
      ++cursor;
    }
    segment_end = TrimTrailingWhitespace(segment_begin, cursor);
    if (segment_end <= segment_begin) {
      return EOT_ERR_INVALID_ARGUMENT;
    }

    eot_status_t status =
        ParseAxisAssignment(segment_begin, segment_end, &parsed_axis);
    if (status != EOT_OK) {
      return status;
    }

    parsed_axes.push_back(parsed_axis);
    if (*cursor == ',') {
      ++cursor;
    }
  }

  if (parsed_axes.empty()) {
    return EOT_OK;
  }

  location->axes = static_cast<variation_axis_value_t *>(
      std::calloc(parsed_axes.size(), sizeof(variation_axis_value_t)));
  if (location->axes == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  location->num_axes = parsed_axes.size();
  for (size_t i = 0; i < parsed_axes.size(); ++i) {
    std::memcpy(location->axes[i].tag, parsed_axes[i].tag,
                sizeof(location->axes[i].tag));
    location->axes[i].user_value = parsed_axes[i].value;
    location->axes[i].resolved_value = parsed_axes[i].value;
    location->axes[i].normalized_value = 0.0f;
  }

  if (HasDuplicateTags(location)) {
    variation_location_destroy(location);
    return EOT_ERR_INVALID_ARGUMENT;
  }

  return EOT_OK;
}

extern "C" eot_status_t cff_variation_resolve_location(
    const cff_font_t *font, variation_location_t *location) {
  variation_axis_value_t *resolved_axes = nullptr;
  if (font == nullptr || location == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (location->num_axes > 0 && location->axes == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (HasDuplicateTags(location)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (font->num_axes == 0) {
    return location->num_axes == 0 ? EOT_OK : EOT_ERR_INVALID_ARGUMENT;
  }

  for (size_t i = 0; i < location->num_axes; ++i) {
    if (FindAxis(font, location->axes[i].tag) == nullptr) {
      return EOT_ERR_INVALID_ARGUMENT;
    }
  }

  resolved_axes = static_cast<variation_axis_value_t *>(
      std::calloc(font->num_axes, sizeof(variation_axis_value_t)));
  if (resolved_axes == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  for (size_t axis_index = 0; axis_index < font->num_axes; ++axis_index) {
    const cff_axis_t *axis = &font->axes[axis_index];
    const variation_axis_value_t *input_axis =
        FindLocationAxis(location, axis->tag);
    float resolved_value = axis->default_value;

    std::memcpy(resolved_axes[axis_index].tag, axis->tag,
                sizeof(resolved_axes[axis_index].tag));
    if (input_axis != nullptr) {
      resolved_value = input_axis->user_value;
      if (resolved_value < axis->min_value) {
        resolved_value = axis->min_value;
      } else if (resolved_value > axis->max_value) {
        resolved_value = axis->max_value;
      }
    }

    resolved_axes[axis_index].user_value = resolved_value;
    resolved_axes[axis_index].resolved_value = resolved_value;
    resolved_axes[axis_index].normalized_value =
        ApplyAvarMapping(axis, NormalizeAxisValue(axis, resolved_value));
  }

  std::free(location->axes);
  location->axes = resolved_axes;
  location->num_axes = font->num_axes;
  return EOT_OK;
}
