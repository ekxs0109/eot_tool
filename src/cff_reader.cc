#include "cff_reader.h"

#include <hb.h>
#include <hb-ot.h>

#include <new>

#include <cmath>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

extern "C" {
#include "byte_io.h"
#include "sfnt_font.h"
#include "sfnt_reader.h"
#include "sfnt_writer.h"
}

namespace {

#if defined(__EMSCRIPTEN__) && !defined(EOT_WASM_CUSTOM_HARFBUZZ)
#define HB_FACE_CREATE_OR_FAIL(blob, index) hb_face_create((blob), (index))
#define HB_DRAW_SET_MOVE_TO(dfuncs, func) hb_draw_funcs_set_move_to_func((dfuncs), (func))
#define HB_DRAW_SET_LINE_TO(dfuncs, func) hb_draw_funcs_set_line_to_func((dfuncs), (func))
#define HB_DRAW_SET_QUADRATIC_TO(dfuncs, func) \
  hb_draw_funcs_set_quadratic_to_func((dfuncs), (func))
#define HB_DRAW_SET_CUBIC_TO(dfuncs, func) hb_draw_funcs_set_cubic_to_func((dfuncs), (func))
#define HB_DRAW_SET_CLOSE_PATH(dfuncs, func) \
  hb_draw_funcs_set_close_path_func((dfuncs), (func))
#else
#define HB_FACE_CREATE_OR_FAIL(blob, index) hb_face_create_or_fail((blob), (index))
#define HB_DRAW_SET_MOVE_TO(dfuncs, func) \
  hb_draw_funcs_set_move_to_func((dfuncs), (func), nullptr, nullptr)
#define HB_DRAW_SET_LINE_TO(dfuncs, func) \
  hb_draw_funcs_set_line_to_func((dfuncs), (func), nullptr, nullptr)
#define HB_DRAW_SET_QUADRATIC_TO(dfuncs, func) \
  hb_draw_funcs_set_quadratic_to_func((dfuncs), (func), nullptr, nullptr)
#define HB_DRAW_SET_CUBIC_TO(dfuncs, func) \
  hb_draw_funcs_set_cubic_to_func((dfuncs), (func), nullptr, nullptr)
#define HB_DRAW_SET_CLOSE_PATH(dfuncs, func) \
  hb_draw_funcs_set_close_path_func((dfuncs), (func), nullptr, nullptr)
#endif

constexpr uint16_t kCffStandardStringCount = 391;
constexpr int kMaxSubrCallDepth = 16;

constexpr const char* kCffStandardStrings[] = {
  ".notdef",
  "space",
  "exclam",
  "quotedbl",
  "numbersign",
  "dollar",
  "percent",
  "ampersand",
  "quoteright",
  "parenleft",
  "parenright",
  "asterisk",
  "plus",
  "comma",
  "hyphen",
  "period",
  "slash",
  "zero",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "colon",
  "semicolon",
  "less",
  "equal",
  "greater",
  "question",
  "at",
  "A",
  "B",
  "C",
  "D",
  "E",
  "F",
  "G",
  "H",
  "I",
  "J",
  "K",
  "L",
  "M",
  "N",
  "O",
  "P",
  "Q",
  "R",
  "S",
  "T",
  "U",
  "V",
  "W",
  "X",
  "Y",
  "Z",
  "bracketleft",
  "backslash",
  "bracketright",
  "asciicircum",
  "underscore",
  "quoteleft",
  "a",
  "b",
  "c",
  "d",
  "e",
  "f",
  "g",
  "h",
  "i",
  "j",
  "k",
  "l",
  "m",
  "n",
  "o",
  "p",
  "q",
  "r",
  "s",
  "t",
  "u",
  "v",
  "w",
  "x",
  "y",
  "z",
  "braceleft",
  "bar",
  "braceright",
  "asciitilde",
  "exclamdown",
  "cent",
  "sterling",
  "fraction",
  "yen",
  "florin",
  "section",
  "currency",
  "quotesingle",
  "quotedblleft",
  "guillemotleft",
  "guilsinglleft",
  "guilsinglright",
  "fi",
  "fl",
  "endash",
  "dagger",
  "daggerdbl",
  "periodcentered",
  "paragraph",
  "bullet",
  "quotesinglbase",
  "quotedblbase",
  "quotedblright",
  "guillemotright",
  "ellipsis",
  "perthousand",
  "questiondown",
  "grave",
  "acute",
  "circumflex",
  "tilde",
  "macron",
  "breve",
  "dotaccent",
  "dieresis",
  "ring",
  "cedilla",
  "hungarumlaut",
  "ogonek",
  "caron",
  "emdash",
  "AE",
  "ordfeminine",
  "Lslash",
  "Oslash",
  "OE",
  "ordmasculine",
  "ae",
  "dotlessi",
  "lslash",
  "oslash",
  "oe",
  "germandbls",
  "onesuperior",
  "logicalnot",
  "mu",
  "trademark",
  "Eth",
  "onehalf",
  "plusminus",
  "Thorn",
  "onequarter",
  "divide",
  "brokenbar",
  "degree",
  "thorn",
  "threequarters",
  "twosuperior",
  "registered",
  "minus",
  "eth",
  "multiply",
  "threesuperior",
  "copyright",
  "Aacute",
  "Acircumflex",
  "Adieresis",
  "Agrave",
  "Aring",
  "Atilde",
  "Ccedilla",
  "Eacute",
  "Ecircumflex",
  "Edieresis",
  "Egrave",
  "Iacute",
  "Icircumflex",
  "Idieresis",
  "Igrave",
  "Ntilde",
  "Oacute",
  "Ocircumflex",
  "Odieresis",
  "Ograve",
  "Otilde",
  "Scaron",
  "Uacute",
  "Ucircumflex",
  "Udieresis",
  "Ugrave",
  "Yacute",
  "Ydieresis",
  "Zcaron",
  "aacute",
  "acircumflex",
  "adieresis",
  "agrave",
  "aring",
  "atilde",
  "ccedilla",
  "eacute",
  "ecircumflex",
  "edieresis",
  "egrave",
  "iacute",
  "icircumflex",
  "idieresis",
  "igrave",
  "ntilde",
  "oacute",
  "ocircumflex",
  "odieresis",
  "ograve",
  "otilde",
  "scaron",
  "uacute",
  "ucircumflex",
  "udieresis",
  "ugrave",
  "yacute",
  "ydieresis",
  "zcaron",
  "exclamsmall",
  "Hungarumlautsmall",
  "dollaroldstyle",
  "dollarsuperior",
  "ampersandsmall",
  "Acutesmall",
  "parenleftsuperior",
  "parenrightsuperior",
  "twodotenleader",
  "onedotenleader",
  "zerooldstyle",
  "oneoldstyle",
  "twooldstyle",
  "threeoldstyle",
  "fouroldstyle",
  "fiveoldstyle",
  "sixoldstyle",
  "sevenoldstyle",
  "eightoldstyle",
  "nineoldstyle",
  "commasuperior",
  "threequartersemdash",
  "periodsuperior",
  "questionsmall",
  "asuperior",
  "bsuperior",
  "centsuperior",
  "dsuperior",
  "esuperior",
  "isuperior",
  "lsuperior",
  "msuperior",
  "nsuperior",
  "osuperior",
  "rsuperior",
  "ssuperior",
  "tsuperior",
  "ff",
  "ffi",
  "ffl",
  "parenleftinferior",
  "parenrightinferior",
  "Circumflexsmall",
  "hyphensuperior",
  "Gravesmall",
  "Asmall",
  "Bsmall",
  "Csmall",
  "Dsmall",
  "Esmall",
  "Fsmall",
  "Gsmall",
  "Hsmall",
  "Ismall",
  "Jsmall",
  "Ksmall",
  "Lsmall",
  "Msmall",
  "Nsmall",
  "Osmall",
  "Psmall",
  "Qsmall",
  "Rsmall",
  "Ssmall",
  "Tsmall",
  "Usmall",
  "Vsmall",
  "Wsmall",
  "Xsmall",
  "Ysmall",
  "Zsmall",
  "colonmonetary",
  "onefitted",
  "rupiah",
  "Tildesmall",
  "exclamdownsmall",
  "centoldstyle",
  "Lslashsmall",
  "Scaronsmall",
  "Zcaronsmall",
  "Dieresissmall",
  "Brevesmall",
  "Caronsmall",
  "Dotaccentsmall",
  "Macronsmall",
  "figuredash",
  "hypheninferior",
  "Ogoneksmall",
  "Ringsmall",
  "Cedillasmall",
  "questiondownsmall",
  "oneeighth",
  "threeeighths",
  "fiveeighths",
  "seveneighths",
  "onethird",
  "twothirds",
  "zerosuperior",
  "foursuperior",
  "fivesuperior",
  "sixsuperior",
  "sevensuperior",
  "eightsuperior",
  "ninesuperior",
  "zeroinferior",
  "oneinferior",
  "twoinferior",
  "threeinferior",
  "fourinferior",
  "fiveinferior",
  "sixinferior",
  "seveninferior",
  "eightinferior",
  "nineinferior",
  "centinferior",
  "dollarinferior",
  "periodinferior",
  "commainferior",
  "Agravesmall",
  "Aacutesmall",
  "Acircumflexsmall",
  "Atildesmall",
  "Adieresissmall",
  "Aringsmall",
  "AEsmall",
  "Ccedillasmall",
  "Egravesmall",
  "Eacutesmall",
  "Ecircumflexsmall",
  "Edieresissmall",
  "Igravesmall",
  "Iacutesmall",
  "Icircumflexsmall",
  "Idieresissmall",
  "Ethsmall",
  "Ntildesmall",
  "Ogravesmall",
  "Oacutesmall",
  "Ocircumflexsmall",
  "Otildesmall",
  "Odieresissmall",
  "OEsmall",
  "Oslashsmall",
  "Ugravesmall",
  "Uacutesmall",
  "Ucircumflexsmall",
  "Udieresissmall",
  "Yacutesmall",
  "Thornsmall",
  "Ydieresissmall",
  "001.000",
  "001.001",
  "001.002",
  "001.003",
  "Black",
  "Bold",
  "Book",
  "Light",
  "Medium",
  "Regular",
  "Roman",
  "Semibold",
};

uint32_t MakeTag(char a, char b, char c, char d) {
  return (static_cast<uint32_t>(static_cast<uint8_t>(a)) << 24) |
         (static_cast<uint32_t>(static_cast<uint8_t>(b)) << 16) |
         (static_cast<uint32_t>(static_cast<uint8_t>(c)) << 8) |
         static_cast<uint32_t>(static_cast<uint8_t>(d));
}

struct CffFontImpl {
  CffFontImpl() { sfnt_font_init(&sfnt); }
  ~CffFontImpl() { sfnt_font_destroy(&sfnt); }

  sfnt_font_t sfnt;
};

struct CffIndexView {
  const uint8_t* base;
  size_t length;
  size_t count;
  uint8_t off_size;
  size_t offsets_offset;
  size_t data_offset;
};

struct TopDictInfo {
  size_t charset_offset = 0;
  size_t charstrings_offset = 0;
  size_t private_offset = 0;
  size_t private_size = 0;
  bool has_charset = false;
  bool has_charstrings = false;
  bool has_private = false;
};

struct PrivateDictInfo {
  size_t subrs_offset = 0;
  bool has_subrs = false;
};

struct OutlineBuilder {
  std::vector<cubic_curve_t> cubics;
  std::vector<size_t> contour_end_indices;
  cff_point_t current = {0.0, 0.0};
  cff_point_t contour_start = {0.0, 0.0};
  bool contour_open = false;
};

struct CharStringContext {
  const uint8_t* cff_data;
  size_t cff_length;
  const CffIndexView* local_subrs;
  OutlineBuilder* outline;
  std::vector<double> stack;
  int call_depth;
  bool width_seen;
};

bool HasRange(const uint8_t* data, size_t length, size_t offset, size_t span) {
  return buffer_view_has_range(buffer_view_make(data, length), offset, span) != 0;
}

const sfnt_table_t* FindTable(const sfnt_font_t* font, uint32_t tag) {
  if (font == nullptr) {
    return nullptr;
  }

  for (size_t i = 0; i < font->num_tables; ++i) {
    if (font->tables[i].tag == tag) {
      return &font->tables[i];
    }
  }

  return nullptr;
}

float Fixed16_16ToFloat(uint32_t raw_value) {
  const int32_t signed_value = static_cast<int32_t>(raw_value);
  return static_cast<float>(signed_value / 65536.0);
}

float F2Dot14ToFloat(uint16_t raw_value) {
  const int16_t signed_value = static_cast<int16_t>(raw_value);
  return static_cast<float>(signed_value / 16384.0);
}

eot_status_t ParseCffIndex(const uint8_t* data, size_t length, size_t offset,
                           CffIndexView* out_index, size_t* out_next_offset) {
  if (data == nullptr || out_index == nullptr || out_next_offset == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (!HasRange(data, length, offset, 2)) {
    return EOT_ERR_TRUNCATED;
  }

  const uint16_t count = read_u16be(data + offset);
  size_t cursor = offset + 2;

  out_index->base = data;
  out_index->length = length;
  out_index->count = count;
  out_index->off_size = 0;
  out_index->offsets_offset = cursor;
  out_index->data_offset = cursor;

  if (count == 0) {
    *out_next_offset = cursor;
    return EOT_OK;
  }

  if (!HasRange(data, length, cursor, 1)) {
    return EOT_ERR_TRUNCATED;
  }

  out_index->off_size = data[cursor];
  if (out_index->off_size < 1 || out_index->off_size > 4) {
    return EOT_ERR_CORRUPT_DATA;
  }
  cursor += 1;

  const size_t offsets_size =
      static_cast<size_t>(count + 1) * static_cast<size_t>(out_index->off_size);
  if (!HasRange(data, length, cursor, offsets_size)) {
    return EOT_ERR_TRUNCATED;
  }

  out_index->offsets_offset = cursor;
  cursor += offsets_size;
  out_index->data_offset = cursor;

  uint32_t last_offset = 0;
  for (size_t i = 0; i <= count; ++i) {
    const uint8_t* encoded =
        data + out_index->offsets_offset + i * out_index->off_size;
    uint32_t value = 0;
    for (uint8_t j = 0; j < out_index->off_size; ++j) {
      value = (value << 8) | encoded[j];
    }
    if (i == 0 && value != 1) {
      return EOT_ERR_CORRUPT_DATA;
    }
    if (i > 0 && value < last_offset) {
      return EOT_ERR_CORRUPT_DATA;
    }
    last_offset = value;
  }

  const size_t data_size = static_cast<size_t>(last_offset) - 1;
  if (!HasRange(data, length, out_index->data_offset, data_size)) {
    return EOT_ERR_TRUNCATED;
  }

  *out_next_offset = out_index->data_offset + data_size;
  return EOT_OK;
}

eot_status_t GetCffIndexObject(const CffIndexView& index, size_t object_index,
                               const uint8_t** out_data, size_t* out_length) {
  if (out_data == nullptr || out_length == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (object_index >= index.count) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  const uint8_t* offset_bytes =
      index.base + index.offsets_offset + object_index * index.off_size;
  const uint8_t* next_offset_bytes = offset_bytes + index.off_size;
  uint32_t start = 0;
  uint32_t end = 0;

  for (uint8_t i = 0; i < index.off_size; ++i) {
    start = (start << 8) | offset_bytes[i];
    end = (end << 8) | next_offset_bytes[i];
  }

  if (start == 0 || end < start) {
    return EOT_ERR_CORRUPT_DATA;
  }

  const size_t object_offset = index.data_offset + static_cast<size_t>(start) - 1;
  const size_t object_length = static_cast<size_t>(end - start);
  if (!HasRange(index.base, index.length, object_offset, object_length)) {
    return EOT_ERR_TRUNCATED;
  }

  *out_data = index.base + object_offset;
  *out_length = object_length;
  return EOT_OK;
}

eot_status_t ReadDictNumber(const uint8_t* data, size_t length, size_t* cursor,
                            int32_t* out_value) {
  if (data == nullptr || cursor == nullptr || out_value == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (*cursor >= length) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t b0 = data[*cursor];
  (*cursor)++;

  if (b0 >= 32 && b0 <= 246) {
    *out_value = static_cast<int32_t>(b0) - 139;
    return EOT_OK;
  }

  if (b0 >= 247 && b0 <= 250) {
    if (*cursor >= length) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value =
        (static_cast<int32_t>(b0) - 247) * 256 + data[*cursor] + 108;
    (*cursor)++;
    return EOT_OK;
  }

  if (b0 >= 251 && b0 <= 254) {
    if (*cursor >= length) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value =
        -((static_cast<int32_t>(b0) - 251) * 256) - data[*cursor] - 108;
    (*cursor)++;
    return EOT_OK;
  }

  if (b0 == 28) {
    if (!HasRange(data, length, *cursor, 2)) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value = static_cast<int16_t>(read_u16be(data + *cursor));
    *cursor += 2;
    return EOT_OK;
  }

  if (b0 == 29) {
    if (!HasRange(data, length, *cursor, 4)) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value = static_cast<int32_t>(read_u32be(data + *cursor));
    *cursor += 4;
    return EOT_OK;
  }

  if (b0 == 30) {
    while (*cursor < length) {
      const uint8_t byte = data[*cursor];
      (*cursor)++;
      if ((byte & 0x0f) == 0x0f || (byte >> 4) == 0x0f) {
        *out_value = 0;
        return EOT_OK;
      }
    }
    return EOT_ERR_TRUNCATED;
  }

  if (b0 == 255) {
    if (!HasRange(data, length, *cursor, 4)) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value = static_cast<int32_t>(read_u32be(data + *cursor));
    *cursor += 4;
    return EOT_OK;
  }

  return EOT_ERR_CORRUPT_DATA;
}

eot_status_t ParseTopDict(const uint8_t* data, size_t length,
                          TopDictInfo* out_top_dict) {
  if (data == nullptr || out_top_dict == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  std::vector<int32_t> operands;
  size_t cursor = 0;
  while (cursor < length) {
    const uint8_t b0 = data[cursor];
    const bool is_operator = (b0 <= 21 && b0 != 28 && b0 != 29 && b0 != 30);

    if (!is_operator) {
      int32_t value = 0;
      const eot_status_t status = ReadDictNumber(data, length, &cursor, &value);
      if (status != EOT_OK) {
        return status;
      }
      operands.push_back(value);
      continue;
    }

    cursor++;
    int op = b0;
    if (op == 12) {
      if (cursor >= length) {
        return EOT_ERR_TRUNCATED;
      }
      op = 1200 + data[cursor];
      cursor++;
    }

    if (op == 15 && !operands.empty()) {
      out_top_dict->charset_offset = static_cast<size_t>(operands.back());
      out_top_dict->has_charset = true;
    } else if (op == 17 && !operands.empty()) {
      out_top_dict->charstrings_offset = static_cast<size_t>(operands.back());
      out_top_dict->has_charstrings = true;
    } else if (op == 18 && operands.size() >= 2) {
      out_top_dict->private_size =
          static_cast<size_t>(operands[operands.size() - 2]);
      out_top_dict->private_offset =
          static_cast<size_t>(operands[operands.size() - 1]);
      out_top_dict->has_private = true;
    }

    operands.clear();
  }

  if (!out_top_dict->has_charset || !out_top_dict->has_charstrings) {
    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_OK;
}

eot_status_t ParsePrivateDict(const uint8_t* data, size_t length,
                              PrivateDictInfo* out_private_dict) {
  if (data == nullptr || out_private_dict == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  std::vector<int32_t> operands;
  size_t cursor = 0;
  while (cursor < length) {
    const uint8_t b0 = data[cursor];
    const bool is_operator = (b0 <= 21 && b0 != 28 && b0 != 29 && b0 != 30);

    if (!is_operator) {
      int32_t value = 0;
      const eot_status_t status = ReadDictNumber(data, length, &cursor, &value);
      if (status != EOT_OK) {
        return status;
      }
      operands.push_back(value);
      continue;
    }

    cursor++;
    int op = b0;
    if (op == 12) {
      if (cursor >= length) {
        return EOT_ERR_TRUNCATED;
      }
      op = 1200 + data[cursor];
      cursor++;
    }

    if (op == 19 && !operands.empty()) {
      out_private_dict->subrs_offset = static_cast<size_t>(operands.back());
      out_private_dict->has_subrs = true;
    }

    operands.clear();
  }

  return EOT_OK;
}

bool MatchCustomString(const CffIndexView& string_index, uint16_t sid,
                       const char* glyph_name) {
  if (sid < kCffStandardStringCount || glyph_name == nullptr) {
    return false;
  }

  const size_t string_index_entry = sid - kCffStandardStringCount;
  const uint8_t* string_bytes = nullptr;
  size_t string_length = 0;
  if (GetCffIndexObject(string_index, string_index_entry, &string_bytes,
                        &string_length) != EOT_OK) {
    return false;
  }

  return std::strlen(glyph_name) == string_length &&
         std::memcmp(string_bytes, glyph_name, string_length) == 0;
}

bool GlyphNameMatchesSid(uint16_t sid, const CffIndexView& string_index,
                         const char* glyph_name) {
  if (glyph_name == nullptr) {
    return false;
  }

  if (sid < kCffStandardStringCount) {
    return std::strcmp(glyph_name, kCffStandardStrings[sid]) == 0;
  }

  return MatchCustomString(string_index, sid, glyph_name);
}

eot_status_t FindGlyphIdByName(const uint8_t* cff_data, size_t cff_length,
                               size_t charset_offset,
                               const CffIndexView& string_index,
                               size_t glyph_count, const char* glyph_name,
                               size_t* out_glyph_id) {
  if (cff_data == nullptr || glyph_name == nullptr || out_glyph_id == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (std::strcmp(glyph_name, ".notdef") == 0) {
    *out_glyph_id = 0;
    return EOT_OK;
  }
  if (!HasRange(cff_data, cff_length, charset_offset, 1)) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t format = cff_data[charset_offset];
  size_t cursor = charset_offset + 1;
  size_t glyph_id = 1;

  if (format == 0) {
    while (glyph_id < glyph_count) {
      if (!HasRange(cff_data, cff_length, cursor, 2)) {
        return EOT_ERR_TRUNCATED;
      }
      const uint16_t sid = read_u16be(cff_data + cursor);
      if (GlyphNameMatchesSid(sid, string_index, glyph_name)) {
        *out_glyph_id = glyph_id;
        return EOT_OK;
      }
      cursor += 2;
      glyph_id++;
    }
    return EOT_ERR_INVALID_ARGUMENT;
  }

  while (glyph_id < glyph_count) {
    uint16_t sid = 0;
    uint16_t n_left = 0;

    if (!HasRange(cff_data, cff_length, cursor, format == 1 ? 3 : 4)) {
      return EOT_ERR_TRUNCATED;
    }

    sid = read_u16be(cff_data + cursor);
    cursor += 2;
    if (format == 1) {
      n_left = cff_data[cursor];
      cursor += 1;
    } else if (format == 2) {
      n_left = read_u16be(cff_data + cursor);
      cursor += 2;
    } else {
      return EOT_ERR_CORRUPT_DATA;
    }

    for (uint16_t i = 0; i <= n_left && glyph_id < glyph_count; ++i) {
      if (GlyphNameMatchesSid(static_cast<uint16_t>(sid + i), string_index,
                              glyph_name)) {
        *out_glyph_id = glyph_id;
        return EOT_OK;
      }
      glyph_id++;
    }
  }

  return EOT_ERR_INVALID_ARGUMENT;
}

int LocalSubrBias(size_t subr_count) {
  if (subr_count < 1240) {
    return 107;
  }
  if (subr_count < 33900) {
    return 1131;
  }
  return 32768;
}

void AddLineAsCubic(OutlineBuilder* outline, cff_point_t start, cff_point_t end) {
  cubic_curve_t segment;
  const double dx = end.x - start.x;
  const double dy = end.y - start.y;

  segment.p0 = start;
  segment.p1 = {start.x + dx / 3.0, start.y + dy / 3.0};
  segment.p2 = {start.x + (dx * 2.0) / 3.0, start.y + (dy * 2.0) / 3.0};
  segment.p3 = end;

  outline->cubics.push_back(segment);
  outline->current = end;
}

eot_status_t AddLineDelta(OutlineBuilder* outline, double dx, double dy) {
  if (outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (!outline->contour_open) {
    return EOT_ERR_CORRUPT_DATA;
  }

  const cff_point_t start = outline->current;
  const cff_point_t end = {start.x + dx, start.y + dy};
  AddLineAsCubic(outline, start, end);
  return EOT_OK;
}

eot_status_t AddCurveDelta(OutlineBuilder* outline, double dx1, double dy1,
                           double dx2, double dy2, double dx3, double dy3) {
  if (outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (!outline->contour_open) {
    return EOT_ERR_CORRUPT_DATA;
  }

  cubic_curve_t segment;
  segment.p0 = outline->current;
  segment.p1 = {segment.p0.x + dx1, segment.p0.y + dy1};
  segment.p2 = {segment.p1.x + dx2, segment.p1.y + dy2};
  segment.p3 = {segment.p2.x + dx3, segment.p2.y + dy3};
  outline->cubics.push_back(segment);
  outline->current = segment.p3;
  return EOT_OK;
}

eot_status_t CloseContour(OutlineBuilder* outline) {
  if (outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (!outline->contour_open) {
    return EOT_OK;
  }

  if (outline->current.x != outline->contour_start.x ||
      outline->current.y != outline->contour_start.y) {
    AddLineAsCubic(outline, outline->current, outline->contour_start);
  }

  if (outline->cubics.empty()) {
    return EOT_ERR_CORRUPT_DATA;
  }

  outline->contour_end_indices.push_back(outline->cubics.size() - 1);
  outline->current = outline->contour_start;
  outline->contour_open = false;
  return EOT_OK;
}

eot_status_t MoveToDelta(OutlineBuilder* outline, double dx, double dy) {
  if (outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  const eot_status_t close_status = CloseContour(outline);
  if (close_status != EOT_OK) {
    return close_status;
  }

  outline->current.x += dx;
  outline->current.y += dy;
  outline->contour_start = outline->current;
  outline->contour_open = true;
  return EOT_OK;
}

eot_status_t AddQuadraticCurve(OutlineBuilder* outline, double control_x,
                               double control_y, double to_x, double to_y) {
  cubic_curve_t segment;
  if (outline == nullptr || !outline->contour_open) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  segment.p0 = outline->current;
  segment.p1 = {
      segment.p0.x + (2.0 / 3.0) * (control_x - segment.p0.x),
      segment.p0.y + (2.0 / 3.0) * (control_y - segment.p0.y),
  };
  segment.p2 = {
      to_x + (2.0 / 3.0) * (control_x - to_x),
      to_y + (2.0 / 3.0) * (control_y - to_y),
  };
  segment.p3 = {to_x, to_y};
  outline->cubics.push_back(segment);
  outline->current = segment.p3;
  return EOT_OK;
}

struct HbOutlineCapture {
  OutlineBuilder* outline;
  eot_status_t status;
};

static void HbMoveToImpl(void* draw_data, float to_x, float to_y) {
  HbOutlineCapture* capture = static_cast<HbOutlineCapture*>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  capture->status = MoveToDelta(
      capture->outline, to_x - capture->outline->current.x,
      to_y - capture->outline->current.y);
}

static void HbLineToImpl(void* draw_data, float to_x, float to_y) {
  HbOutlineCapture* capture = static_cast<HbOutlineCapture*>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  capture->status = AddLineDelta(
      capture->outline, to_x - capture->outline->current.x,
      to_y - capture->outline->current.y);
}

static void HbQuadraticToImpl(void* draw_data, float control_x, float control_y,
                              float to_x, float to_y) {
  HbOutlineCapture* capture = static_cast<HbOutlineCapture*>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  capture->status = AddQuadraticCurve(capture->outline, control_x, control_y,
                                      to_x, to_y);
}

static void HbCubicToImpl(void* draw_data, float control1_x, float control1_y,
                          float control2_x, float control2_y,
                          float to_x, float to_y) {
  HbOutlineCapture* capture = static_cast<HbOutlineCapture*>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  capture->status = AddCurveDelta(
      capture->outline, control1_x - capture->outline->current.x,
      control1_y - capture->outline->current.y, control2_x - control1_x,
      control2_y - control1_y, to_x - control2_x, to_y - control2_y);
}

static void HbClosePathImpl(void* draw_data) {
  HbOutlineCapture* capture = static_cast<HbOutlineCapture*>(draw_data);
  if (capture == nullptr || capture->status != EOT_OK) {
    return;
  }

  capture->status = CloseContour(capture->outline);
}

#if defined(__EMSCRIPTEN__) && !defined(EOT_WASM_CUSTOM_HARFBUZZ)
void HbMoveToCallback(hb_position_t to_x, hb_position_t to_y, void* user_data) {
  HbMoveToImpl(user_data, static_cast<float>(to_x), static_cast<float>(to_y));
}

void HbLineToCallback(hb_position_t to_x, hb_position_t to_y, void* user_data) {
  HbLineToImpl(user_data, static_cast<float>(to_x), static_cast<float>(to_y));
}

void HbQuadraticToCallback(hb_position_t control_x, hb_position_t control_y,
                           hb_position_t to_x, hb_position_t to_y, void* user_data) {
  HbQuadraticToImpl(user_data, static_cast<float>(control_x),
                    static_cast<float>(control_y), static_cast<float>(to_x),
                    static_cast<float>(to_y));
}

void HbCubicToCallback(hb_position_t control1_x, hb_position_t control1_y,
                       hb_position_t control2_x, hb_position_t control2_y,
                       hb_position_t to_x, hb_position_t to_y, void* user_data) {
  HbCubicToImpl(user_data, static_cast<float>(control1_x),
                static_cast<float>(control1_y), static_cast<float>(control2_x),
                static_cast<float>(control2_y), static_cast<float>(to_x),
                static_cast<float>(to_y));
}

void HbClosePathCallback(void* user_data) {
  HbClosePathImpl(user_data);
}
#else
void HbMoveToCallback(hb_draw_funcs_t*, void* draw_data, hb_draw_state_t*,
                      float to_x, float to_y, void*) {
  HbMoveToImpl(draw_data, to_x, to_y);
}

void HbLineToCallback(hb_draw_funcs_t*, void* draw_data, hb_draw_state_t*,
                      float to_x, float to_y, void*) {
  HbLineToImpl(draw_data, to_x, to_y);
}

void HbQuadraticToCallback(hb_draw_funcs_t*, void* draw_data, hb_draw_state_t*,
                           float control_x, float control_y,
                           float to_x, float to_y, void*) {
  HbQuadraticToImpl(draw_data, control_x, control_y, to_x, to_y);
}

void HbCubicToCallback(hb_draw_funcs_t*, void* draw_data, hb_draw_state_t*,
                       float control1_x, float control1_y,
                       float control2_x, float control2_y,
                       float to_x, float to_y, void*) {
  HbCubicToImpl(draw_data, control1_x, control1_y, control2_x, control2_y,
                to_x, to_y);
}

void HbClosePathCallback(hb_draw_funcs_t*, void* draw_data, hb_draw_state_t*,
                         void*) {
  HbClosePathImpl(draw_data);
}
#endif

hb_draw_funcs_t* CreateHbOutlineDrawFuncs(void) {
  hb_draw_funcs_t* draw_funcs = hb_draw_funcs_create();
  if (draw_funcs == nullptr) {
    return nullptr;
  }

  HB_DRAW_SET_MOVE_TO(draw_funcs, HbMoveToCallback);
  HB_DRAW_SET_LINE_TO(draw_funcs, HbLineToCallback);
  HB_DRAW_SET_QUADRATIC_TO(draw_funcs, HbQuadraticToCallback);
  HB_DRAW_SET_CUBIC_TO(draw_funcs, HbCubicToCallback);
  HB_DRAW_SET_CLOSE_PATH(draw_funcs, HbClosePathCallback);
  hb_draw_funcs_make_immutable(draw_funcs);
  return draw_funcs;
}

bool ParseGlyphIdString(const char* glyph_name, hb_codepoint_t* glyph_id) {
  const char* cursor = glyph_name;
  char* parse_end = nullptr;
  unsigned long parsed_value;

  if (glyph_name == nullptr || glyph_id == nullptr) {
    return false;
  }
  if (std::strncmp(cursor, "gid", 3) == 0) {
    cursor += 3;
  }
  if (*cursor == '\0') {
    return false;
  }

  parsed_value = std::strtoul(cursor, &parse_end, 10);
  if (parse_end == cursor || *parse_end != '\0' ||
      parsed_value > std::numeric_limits<hb_codepoint_t>::max()) {
    return false;
  }

  *glyph_id = static_cast<hb_codepoint_t>(parsed_value);
  return true;
}

eot_status_t ExecuteCharString(const uint8_t* data, size_t length,
                               CharStringContext* context);

bool IsCharStringNumberByte(uint8_t b0) {
  return b0 == 28 || b0 == 255 || b0 >= 32;
}

bool TryStackInteger(double value, int32_t* out_value) {
  if (out_value == nullptr || !std::isfinite(value) ||
      value < static_cast<double>(std::numeric_limits<int32_t>::min()) ||
      value > static_cast<double>(std::numeric_limits<int32_t>::max()) ||
      std::floor(value) != value) {
    return false;
  }

  *out_value = static_cast<int32_t>(value);
  return true;
}

eot_status_t ReadCharStringNumber(const uint8_t* data, size_t length,
                                  size_t* cursor, double* out_value) {
  if (data == nullptr || cursor == nullptr || out_value == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (*cursor >= length) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t b0 = data[*cursor];
  (*cursor)++;

  if (b0 >= 32 && b0 <= 246) {
    *out_value = static_cast<double>(static_cast<int32_t>(b0) - 139);
    return EOT_OK;
  }
  if (b0 >= 247 && b0 <= 250) {
    if (*cursor >= length) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value =
        static_cast<double>((static_cast<int32_t>(b0) - 247) * 256 +
                            data[*cursor] + 108);
    (*cursor)++;
    return EOT_OK;
  }
  if (b0 >= 251 && b0 <= 254) {
    if (*cursor >= length) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value =
        static_cast<double>(-((static_cast<int32_t>(b0) - 251) * 256) -
                            data[*cursor] - 108);
    (*cursor)++;
    return EOT_OK;
  }
  if (b0 == 28) {
    if (!HasRange(data, length, *cursor, 2)) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value = static_cast<double>(
        static_cast<int16_t>(read_u16be(data + *cursor)));
    *cursor += 2;
    return EOT_OK;
  }
  if (b0 == 255) {
    if (!HasRange(data, length, *cursor, 4)) {
      return EOT_ERR_TRUNCATED;
    }
    *out_value =
        static_cast<double>(static_cast<int32_t>(read_u32be(data + *cursor))) /
        65536.0;
    *cursor += 4;
    return EOT_OK;
  }

  return EOT_ERR_CORRUPT_DATA;
}

eot_status_t NormalizeMoveStack(CharStringContext* context,
                                size_t required_operands) {
  if (context == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (!context->width_seen && context->stack.size() == required_operands + 1) {
    context->stack.erase(context->stack.begin());
  } else if (context->stack.size() != required_operands) {
    return EOT_ERR_CORRUPT_DATA;
  }

  context->width_seen = true;
  return EOT_OK;
}

eot_status_t ExecuteCallSubr(int32_t subr_number, CharStringContext* context) {
  if (context == nullptr || context->local_subrs == nullptr) {
    return EOT_ERR_CORRUPT_DATA;
  }
  if (context->call_depth >= kMaxSubrCallDepth) {
    return EOT_ERR_CORRUPT_DATA;
  }

  const int subr_index =
      subr_number + LocalSubrBias(context->local_subrs->count);
  if (subr_index < 0 ||
      static_cast<size_t>(subr_index) >= context->local_subrs->count) {
    return EOT_ERR_CORRUPT_DATA;
  }

  const uint8_t* subr_bytes = nullptr;
  size_t subr_length = 0;
  eot_status_t status = GetCffIndexObject(
      *context->local_subrs, static_cast<size_t>(subr_index), &subr_bytes,
      &subr_length);
  if (status != EOT_OK) {
    return status;
  }

  context->call_depth += 1;
  status = ExecuteCharString(subr_bytes, subr_length, context);
  context->call_depth -= 1;
  return status;
}

eot_status_t ExecuteCharString(const uint8_t* data, size_t length,
                               CharStringContext* context) {
  if (data == nullptr || context == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  size_t cursor = 0;
  while (cursor < length) {
    const uint8_t b0 = data[cursor];
    if (IsCharStringNumberByte(b0)) {
      double value = 0.0;
      const eot_status_t status =
          ReadCharStringNumber(data, length, &cursor, &value);
      if (status != EOT_OK) {
        return status;
      }
      context->stack.push_back(value);
      continue;
    }

    cursor++;

    if (b0 == 10) {
      if (context->stack.empty()) {
        return EOT_ERR_CORRUPT_DATA;
      }
      int32_t subr_number = 0;
      if (!TryStackInteger(context->stack.back(), &subr_number)) {
        return EOT_ERR_CORRUPT_DATA;
      }
      context->stack.pop_back();
      const eot_status_t status = ExecuteCallSubr(subr_number, context);
      if (status != EOT_OK) {
        return status;
      }
      continue;
    }

    if (b0 == 11) {
      return EOT_OK;
    }

    if (b0 == 14) {
      if (!context->width_seen && context->stack.size() == 1) {
        context->width_seen = true;
        context->stack.clear();
      }
      if (!context->stack.empty()) {
        return EOT_ERR_CORRUPT_DATA;
      }
      return CloseContour(context->outline);
    }

    if (b0 == 4) {
      const eot_status_t status = NormalizeMoveStack(context, 1);
      if (status != EOT_OK) {
        return status;
      }
      const double dy = context->stack[0];
      context->stack.clear();
      const eot_status_t move_status =
          MoveToDelta(context->outline, 0.0, dy);
      if (move_status != EOT_OK) {
        return move_status;
      }
      continue;
    }

    if (b0 == 21) {
      const eot_status_t status = NormalizeMoveStack(context, 2);
      if (status != EOT_OK) {
        return status;
      }
      const double dx = context->stack[0];
      const double dy = context->stack[1];
      context->stack.clear();
      const eot_status_t move_status =
          MoveToDelta(context->outline, dx, dy);
      if (move_status != EOT_OK) {
        return move_status;
      }
      continue;
    }

    if (b0 == 22) {
      const eot_status_t status = NormalizeMoveStack(context, 1);
      if (status != EOT_OK) {
        return status;
      }
      const double dx = context->stack[0];
      context->stack.clear();
      const eot_status_t move_status =
          MoveToDelta(context->outline, dx, 0.0);
      if (move_status != EOT_OK) {
        return move_status;
      }
      continue;
    }

    if (b0 == 5) {
      if (context->stack.empty() || context->stack.size() % 2 != 0) {
        return EOT_ERR_CORRUPT_DATA;
      }

      for (size_t i = 0; i < context->stack.size(); i += 2) {
        const eot_status_t status =
            AddLineDelta(context->outline, context->stack[i],
                         context->stack[i + 1]);
        if (status != EOT_OK) {
          return status;
        }
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 6) {
      if (context->stack.empty()) {
        return EOT_ERR_CORRUPT_DATA;
      }

      bool horizontal = true;
      for (size_t i = 0; i < context->stack.size(); ++i) {
        const eot_status_t status =
            AddLineDelta(context->outline, horizontal ? context->stack[i] : 0.0,
                         horizontal ? 0.0 : context->stack[i]);
        if (status != EOT_OK) {
          return status;
        }
        horizontal = !horizontal;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 7) {
      if (context->stack.empty()) {
        return EOT_ERR_CORRUPT_DATA;
      }

      bool vertical = true;
      for (size_t i = 0; i < context->stack.size(); ++i) {
        const eot_status_t status =
            AddLineDelta(context->outline, vertical ? 0.0 : context->stack[i],
                         vertical ? context->stack[i] : 0.0);
        if (status != EOT_OK) {
          return status;
        }
        vertical = !vertical;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 8) {
      if (context->stack.empty() || context->stack.size() % 6 != 0) {
        return EOT_ERR_CORRUPT_DATA;
      }

      for (size_t i = 0; i < context->stack.size(); i += 6) {
        const eot_status_t status = AddCurveDelta(
            context->outline, context->stack[i], context->stack[i + 1],
            context->stack[i + 2], context->stack[i + 3], context->stack[i + 4],
            context->stack[i + 5]);
        if (status != EOT_OK) {
          return status;
        }
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 24) {
      if (context->stack.size() < 8 || (context->stack.size() - 2) % 6 != 0) {
        return EOT_ERR_CORRUPT_DATA;
      }

      const size_t curve_args = context->stack.size() - 2;
      for (size_t i = 0; i < curve_args; i += 6) {
        const eot_status_t status = AddCurveDelta(
            context->outline, context->stack[i], context->stack[i + 1],
            context->stack[i + 2], context->stack[i + 3], context->stack[i + 4],
            context->stack[i + 5]);
        if (status != EOT_OK) {
          return status;
        }
      }

      const eot_status_t status = AddLineDelta(
          context->outline, context->stack[curve_args],
          context->stack[curve_args + 1]);
      if (status != EOT_OK) {
        return status;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 25) {
      if (context->stack.size() < 8 || context->stack.size() % 2 != 0) {
        return EOT_ERR_CORRUPT_DATA;
      }

      const size_t line_args = context->stack.size() - 6;
      for (size_t i = 0; i < line_args; i += 2) {
        const eot_status_t status =
            AddLineDelta(context->outline, context->stack[i],
                         context->stack[i + 1]);
        if (status != EOT_OK) {
          return status;
        }
      }

      const eot_status_t status = AddCurveDelta(
          context->outline, context->stack[line_args],
          context->stack[line_args + 1], context->stack[line_args + 2],
          context->stack[line_args + 3], context->stack[line_args + 4],
          context->stack[line_args + 5]);
      if (status != EOT_OK) {
        return status;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 26) {
      if (context->stack.size() < 4 || context->stack.size() % 4 > 1) {
        return EOT_ERR_CORRUPT_DATA;
      }

      size_t i = 0;
      double first_dx = 0.0;
      if (context->stack.size() % 4 == 1) {
        first_dx = context->stack[i++];
      }

      bool first_curve = true;
      while (i < context->stack.size()) {
        const eot_status_t status = AddCurveDelta(
            context->outline, first_curve ? first_dx : 0.0, context->stack[i],
            context->stack[i + 1], context->stack[i + 2], 0.0,
            context->stack[i + 3]);
        if (status != EOT_OK) {
          return status;
        }
        i += 4;
        first_curve = false;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 27) {
      if (context->stack.size() < 4 || context->stack.size() % 4 > 1) {
        return EOT_ERR_CORRUPT_DATA;
      }

      size_t i = 0;
      double first_dy = 0.0;
      if (context->stack.size() % 4 == 1) {
        first_dy = context->stack[i++];
      }

      bool first_curve = true;
      while (i < context->stack.size()) {
        const eot_status_t status = AddCurveDelta(
            context->outline, context->stack[i], first_curve ? first_dy : 0.0,
            context->stack[i + 1], context->stack[i + 2], context->stack[i + 3],
            0.0);
        if (status != EOT_OK) {
          return status;
        }
        i += 4;
        first_curve = false;
      }

      context->stack.clear();
      continue;
    }

    if (b0 == 30 || b0 == 31) {
      if (context->stack.size() < 4) {
        return EOT_ERR_CORRUPT_DATA;
      }

      bool horizontal = (b0 == 31);
      size_t i = 0;
      while (i < context->stack.size()) {
        const size_t remaining = context->stack.size() - i;
        const bool has_extra = remaining == 5;
        if (remaining < 4) {
          return EOT_ERR_CORRUPT_DATA;
        }

        eot_status_t status = EOT_OK;
        if (horizontal) {
          status = AddCurveDelta(
              context->outline, context->stack[i], 0.0, context->stack[i + 1],
              context->stack[i + 2], has_extra ? context->stack[i + 4] : 0.0,
              context->stack[i + 3]);
        } else {
          status = AddCurveDelta(
              context->outline, 0.0, context->stack[i], context->stack[i + 1],
              context->stack[i + 2], context->stack[i + 3],
              has_extra ? context->stack[i + 4] : 0.0);
        }
        if (status != EOT_OK) {
          return status;
        }

        i += has_extra ? 5 : 4;
        horizontal = !horizontal;
      }

      context->stack.clear();
      continue;
    }

    return EOT_ERR_CORRUPT_DATA;
  }

  return EOT_ERR_CORRUPT_DATA;
}

eot_status_t BuildOutline(const OutlineBuilder& builder,
                          cff_glyph_outline_t* outline) {
  if (outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (builder.cubics.empty() || builder.contour_end_indices.empty()) {
    return EOT_ERR_CORRUPT_DATA;
  }

  cubic_curve_t* cubics = static_cast<cubic_curve_t*>(
      std::malloc(builder.cubics.size() * sizeof(cubic_curve_t)));
  size_t* contour_end_indices = static_cast<size_t*>(
      std::malloc(builder.contour_end_indices.size() * sizeof(size_t)));
  if (cubics == nullptr || contour_end_indices == nullptr) {
    std::free(cubics);
    std::free(contour_end_indices);
    return EOT_ERR_ALLOCATION;
  }

  std::memcpy(cubics, builder.cubics.data(),
              builder.cubics.size() * sizeof(cubic_curve_t));
  std::memcpy(contour_end_indices, builder.contour_end_indices.data(),
              builder.contour_end_indices.size() * sizeof(size_t));

  outline->cubics = cubics;
  outline->num_cubics = builder.cubics.size();
  outline->contour_end_indices = contour_end_indices;
  outline->num_contours = builder.contour_end_indices.size();
  return EOT_OK;
}

eot_status_t ParseVariableAxes(const sfnt_table_t* fvar_table,
                               cff_font_t* font) {
  if (font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (fvar_table == nullptr) {
    return EOT_OK;
  }
  if (fvar_table->length < 16) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t* data = fvar_table->data;
  const uint16_t axes_offset = read_u16be(data + 4);
  const uint16_t axis_count = read_u16be(data + 8);
  const uint16_t axis_size = read_u16be(data + 10);

  if (axis_count == 0) {
    return EOT_OK;
  }
  if (axis_size < 20) {
    return EOT_ERR_CORRUPT_DATA;
  }
  if (!HasRange(data, fvar_table->length, axes_offset,
                static_cast<size_t>(axis_count) * axis_size)) {
    return EOT_ERR_TRUNCATED;
  }

  cff_axis_t* axes =
      static_cast<cff_axis_t*>(std::calloc(axis_count, sizeof(cff_axis_t)));
  if (axes == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  for (uint16_t i = 0; i < axis_count; ++i) {
    const uint8_t* axis = data + axes_offset + static_cast<size_t>(i) * axis_size;
    std::memcpy(axes[i].tag, axis, 4);
    axes[i].tag[4] = '\0';
    axes[i].min_value = Fixed16_16ToFloat(read_u32be(axis + 4));
    axes[i].default_value = Fixed16_16ToFloat(read_u32be(axis + 8));
    axes[i].max_value = Fixed16_16ToFloat(read_u32be(axis + 12));
  }

  font->axes = axes;
  font->num_axes = axis_count;
  return EOT_OK;
}

eot_status_t ParseAvarTable(const sfnt_table_t* avar_table, cff_font_t* font) {
  if (font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (avar_table == nullptr || font->axes == nullptr || font->num_axes == 0) {
    return EOT_OK;
  }
  if (avar_table->length < 8) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t* data = avar_table->data;
  const uint16_t axis_count = read_u16be(data + 6);
  size_t cursor = 8;

  if (axis_count != font->num_axes) {
    return EOT_ERR_CORRUPT_DATA;
  }

  for (size_t axis_index = 0; axis_index < font->num_axes; ++axis_index) {
    cff_axis_t* axis = &font->axes[axis_index];
    uint16_t mapping_count;
    cff_avar_mapping_t* mappings;

    if (!HasRange(data, avar_table->length, cursor, 2)) {
      return EOT_ERR_TRUNCATED;
    }
    mapping_count = read_u16be(data + cursor);
    cursor += 2;

    if (!HasRange(data, avar_table->length, cursor,
                  static_cast<size_t>(mapping_count) * 4u)) {
      return EOT_ERR_TRUNCATED;
    }

    if (mapping_count == 0) {
      continue;
    }

    mappings = static_cast<cff_avar_mapping_t*>(
        std::calloc(mapping_count, sizeof(cff_avar_mapping_t)));
    if (mappings == nullptr) {
      return EOT_ERR_ALLOCATION;
    }

    for (uint16_t mapping_index = 0; mapping_index < mapping_count; ++mapping_index) {
      mappings[mapping_index].from_coordinate =
          F2Dot14ToFloat(read_u16be(data + cursor));
      mappings[mapping_index].to_coordinate =
          F2Dot14ToFloat(read_u16be(data + cursor + 2));
      cursor += 4;
    }

    axis->avar_mappings = mappings;
    axis->num_avar_mappings = mapping_count;
  }

  return EOT_OK;
}

eot_status_t ExtractStaticCffOutline(const sfnt_table_t* cff_table,
                                     const char* glyph_name,
                                     cff_glyph_outline_t* outline) {
  if (cff_table == nullptr || glyph_name == nullptr || outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (cff_table->length < 4) {
    return EOT_ERR_TRUNCATED;
  }

  const uint8_t* cff = cff_table->data;
  const size_t cff_length = cff_table->length;
  const uint8_t header_size = cff[2];
  if (header_size < 4 || header_size > cff_length) {
    return EOT_ERR_CORRUPT_DATA;
  }

  size_t cursor = header_size;
  CffIndexView name_index = {};
  CffIndexView top_dict_index = {};
  CffIndexView string_index = {};
  CffIndexView global_subr_index = {};

  eot_status_t status = ParseCffIndex(cff, cff_length, cursor, &name_index, &cursor);
  if (status != EOT_OK) {
    return status;
  }
  status = ParseCffIndex(cff, cff_length, cursor, &top_dict_index, &cursor);
  if (status != EOT_OK) {
    return status;
  }
  status = ParseCffIndex(cff, cff_length, cursor, &string_index, &cursor);
  if (status != EOT_OK) {
    return status;
  }
  status = ParseCffIndex(cff, cff_length, cursor, &global_subr_index, &cursor);
  if (status != EOT_OK) {
    return status;
  }
  (void)name_index;
  (void)global_subr_index;

  if (top_dict_index.count == 0) {
    return EOT_ERR_CORRUPT_DATA;
  }

  const uint8_t* top_dict_bytes = nullptr;
  size_t top_dict_length = 0;
  status = GetCffIndexObject(top_dict_index, 0, &top_dict_bytes, &top_dict_length);
  if (status != EOT_OK) {
    return status;
  }

  TopDictInfo top_dict = {};
  status = ParseTopDict(top_dict_bytes, top_dict_length, &top_dict);
  if (status != EOT_OK) {
    return status;
  }

  CffIndexView charstrings_index = {};
  size_t next_offset = 0;
  status = ParseCffIndex(cff, cff_length, top_dict.charstrings_offset,
                         &charstrings_index, &next_offset);
  if (status != EOT_OK) {
    return status;
  }
  (void)next_offset;

  size_t glyph_id = 0;
  status = FindGlyphIdByName(cff, cff_length, top_dict.charset_offset,
                             string_index, charstrings_index.count, glyph_name,
                             &glyph_id);
  if (status != EOT_OK) {
    return status;
  }

  PrivateDictInfo private_dict = {};
  CffIndexView local_subrs = {};
  if (top_dict.has_private) {
    if (!HasRange(cff, cff_length, top_dict.private_offset, top_dict.private_size)) {
      return EOT_ERR_TRUNCATED;
    }
    status = ParsePrivateDict(cff + top_dict.private_offset, top_dict.private_size,
                              &private_dict);
    if (status != EOT_OK) {
      return status;
    }
    if (private_dict.has_subrs) {
      status = ParseCffIndex(cff, cff_length,
                             top_dict.private_offset + private_dict.subrs_offset,
                             &local_subrs, &next_offset);
      if (status != EOT_OK) {
        return status;
      }
    }
  }

  const uint8_t* charstring_bytes = nullptr;
  size_t charstring_length = 0;
  status = GetCffIndexObject(charstrings_index, glyph_id, &charstring_bytes,
                             &charstring_length);
  if (status != EOT_OK) {
    return status;
  }

  OutlineBuilder builder;
  CharStringContext context = {};
  context.cff_data = cff;
  context.cff_length = cff_length;
  context.local_subrs = private_dict.has_subrs ? &local_subrs : nullptr;
  context.outline = &builder;
  context.call_depth = 0;
  context.width_seen = false;

  status = ExecuteCharString(charstring_bytes, charstring_length, &context);
  if (status != EOT_OK) {
    return status;
  }

  return BuildOutline(builder, outline);
}

eot_status_t BuildNormalizedCoords(const cff_font_t* font,
                                   const variation_location_t* location,
                                   std::vector<int>* normalized_coords) {
  if (font == nullptr || normalized_coords == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  normalized_coords->assign(font->num_axes, 0);
  if (location == nullptr || location->num_axes == 0) {
    return EOT_OK;
  }
  if (location->axes == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (location->num_axes != font->num_axes) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  for (size_t axis_index = 0; axis_index < font->num_axes; ++axis_index) {
    float normalized_value;
    if (std::memcmp(font->axes[axis_index].tag, location->axes[axis_index].tag, 4) !=
        0) {
      return EOT_ERR_INVALID_ARGUMENT;
    }

    normalized_value = location->axes[axis_index].normalized_value;
    if (normalized_value < -1.0f) {
      normalized_value = -1.0f;
    } else if (normalized_value > 1.0f) {
      normalized_value = 1.0f;
    }

    (*normalized_coords)[axis_index] =
        static_cast<int>(std::lround(normalized_value * 16384.0f));
  }

  return EOT_OK;
}

eot_status_t ExtractVariableCffOutline(const cff_font_t* font,
                                       const char* glyph_name,
                                       const variation_location_t* location,
                                       cff_glyph_outline_t* outline) {
  const CffFontImpl* impl;
  uint8_t* sfnt_bytes = nullptr;
  size_t sfnt_length = 0;
  hb_blob_t* blob = nullptr;
  hb_face_t* face = nullptr;
  hb_font_t* hb_font = nullptr;
  hb_draw_funcs_t* draw_funcs = nullptr;
  hb_codepoint_t glyph_id = 0;
  std::vector<int> normalized_coords;
  OutlineBuilder builder;
  HbOutlineCapture capture = {&builder, EOT_OK};
  eot_status_t status;

  if (font == nullptr || glyph_name == nullptr || outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  impl = static_cast<const CffFontImpl*>(font->impl);
  if (impl == nullptr || !font->is_cff2) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = sfnt_writer_serialize(const_cast<sfnt_font_t*>(&impl->sfnt), &sfnt_bytes,
                                 &sfnt_length);
  if (status != EOT_OK) {
    return status;
  }

  blob = hb_blob_create_or_fail(reinterpret_cast<const char*>(sfnt_bytes),
                                static_cast<unsigned int>(sfnt_length),
                                HB_MEMORY_MODE_WRITABLE, sfnt_bytes, std::free);
  if (blob == nullptr) {
    std::free(sfnt_bytes);
    return EOT_ERR_ALLOCATION;
  }

  face = HB_FACE_CREATE_OR_FAIL(blob, 0);
  if (face == nullptr) {
    hb_blob_destroy(blob);
    return EOT_ERR_ALLOCATION;
  }

  hb_font = hb_font_create(face);
  if (hb_font == nullptr) {
    hb_face_destroy(face);
    hb_blob_destroy(blob);
    return EOT_ERR_ALLOCATION;
  }

  hb_ot_font_set_funcs(hb_font);
  hb_font_set_scale(hb_font, hb_face_get_upem(face), hb_face_get_upem(face));

  status = BuildNormalizedCoords(font, location, &normalized_coords);
  if (status != EOT_OK) {
    goto cleanup;
  }
  if (!normalized_coords.empty()) {
    hb_font_set_var_coords_normalized(hb_font, normalized_coords.data(),
                                      static_cast<unsigned int>(normalized_coords.size()));
  }

  if (!hb_font_glyph_from_string(hb_font, glyph_name, -1, &glyph_id) &&
      !ParseGlyphIdString(glyph_name, &glyph_id)) {
    status = EOT_ERR_INVALID_ARGUMENT;
    goto cleanup;
  }
  if (glyph_id >= hb_face_get_glyph_count(face)) {
    status = EOT_ERR_INVALID_ARGUMENT;
    goto cleanup;
  }

  draw_funcs = CreateHbOutlineDrawFuncs();
  if (draw_funcs == nullptr) {
    status = EOT_ERR_ALLOCATION;
    goto cleanup;
  }

  std::memset(outline, 0, sizeof(*outline));
  hb_font_draw_glyph(hb_font, glyph_id, draw_funcs, &capture);
  if (capture.status != EOT_OK) {
    status = capture.status;
    goto cleanup;
  }

  status = CloseContour(&builder);
  if (status != EOT_OK) {
    goto cleanup;
  }
  status = BuildOutline(builder, outline);

cleanup:
  hb_draw_funcs_destroy(draw_funcs);
  hb_font_destroy(hb_font);
  hb_face_destroy(face);
  hb_blob_destroy(blob);
  return status;
}

}  // namespace

extern "C" void cff_glyph_outline_destroy(cff_glyph_outline_t* outline) {
  if (outline == nullptr) {
    return;
  }

  std::free(outline->cubics);
  std::free(outline->contour_end_indices);
  outline->cubics = nullptr;
  outline->num_cubics = 0;
  outline->contour_end_indices = nullptr;
  outline->num_contours = 0;
}

extern "C" void cff_font_init(cff_font_t* font) {
  if (font == nullptr) {
    return;
  }

  std::memset(font, 0, sizeof(*font));
}

extern "C" void cff_font_destroy(cff_font_t* font) {
  if (font == nullptr) {
    return;
  }

  delete static_cast<CffFontImpl*>(font->impl);
  if (font->axes != nullptr) {
    for (size_t i = 0; i < font->num_axes; ++i) {
      std::free(font->axes[i].avar_mappings);
      font->axes[i].avar_mappings = nullptr;
      font->axes[i].num_avar_mappings = 0;
    }
  }
  std::free(font->axes);
  font->impl = nullptr;
  font->axes = nullptr;
  font->num_axes = 0;
  font->is_cff2 = 0;
}

extern "C" eot_status_t cff_reader_load_file(const char* path, cff_font_t* font) {
  file_buffer_t buffer = {};
  eot_status_t status;

  if (path == nullptr || font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = file_io_read_all(path, &buffer);
  if (status != EOT_OK) {
    return status;
  }

  status = cff_reader_load_memory(buffer.data, buffer.length, font);
  file_io_free(&buffer);
  return status;
}

extern "C" eot_status_t cff_reader_load_memory(const uint8_t* data, size_t size,
                                               cff_font_t* font) {
  cff_font_t loaded = {};
  eot_status_t status;

  if (data == nullptr || size == 0 || font == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  CffFontImpl* impl = new (std::nothrow) CffFontImpl();
  if (impl == nullptr) {
    return EOT_ERR_ALLOCATION;
  }

  status = sfnt_reader_parse(data, size, &impl->sfnt);
  if (status != EOT_OK) {
    delete impl;
    return status;
  }

  const sfnt_table_t* cff_table = FindTable(&impl->sfnt, MakeTag('C', 'F', 'F', ' '));
  const sfnt_table_t* cff2_table = FindTable(&impl->sfnt, MakeTag('C', 'F', 'F', '2'));
  if (cff_table == nullptr && cff2_table == nullptr) {
    delete impl;
    return EOT_ERR_CORRUPT_DATA;
  }

  loaded.impl = impl;
  loaded.is_cff2 = cff2_table != nullptr ? 1 : 0;

  status = ParseVariableAxes(FindTable(&impl->sfnt, MakeTag('f', 'v', 'a', 'r')),
                             &loaded);
  if (status == EOT_OK) {
    status = ParseAvarTable(FindTable(&impl->sfnt, MakeTag('a', 'v', 'a', 'r')),
                            &loaded);
  }
  if (status != EOT_OK) {
    cff_font_destroy(&loaded);
    return status;
  }

  cff_font_destroy(font);
  *font = loaded;
  return EOT_OK;
}

extern "C" eot_status_t cff_reader_extract_glyph_outline(
    const cff_font_t* font, const char* glyph_name,
    const variation_location_t* location, cff_glyph_outline_t* outline) {
  if (font == nullptr || glyph_name == nullptr || outline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  std::memset(outline, 0, sizeof(*outline));

  const CffFontImpl* impl = static_cast<const CffFontImpl*>(font->impl);
  if (impl == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (font->is_cff2) {
    return ExtractVariableCffOutline(font, glyph_name, location, outline);
  }
  if (location != nullptr && location->num_axes > 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  return ExtractStaticCffOutline(
      FindTable(&impl->sfnt, MakeTag('C', 'F', 'F', ' ')), glyph_name, outline);
}

extern "C" size_t cff_font_axis_count(const cff_font_t* font) {
  if (font == nullptr) {
    return 0;
  }
  return font->num_axes;
}
