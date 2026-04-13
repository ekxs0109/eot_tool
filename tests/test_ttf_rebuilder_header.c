#include <stdio.h>

#include "../src/tt_rebuilder.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s", #expr); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

static eot_status_t (*test_ttf_rebuilder_build_font_symbol)(
    const tt_glyph_outline_t *, size_t, sfnt_font_t *) = tt_rebuilder_build_font;

static void test_ttf_rebuilder_header_exports_c_declaration(void) {
  ASSERT_TRUE(test_ttf_rebuilder_build_font_symbol != NULL);
}

void register_ttf_rebuilder_header_tests(void) {
  test_register("test_ttf_rebuilder_header_exports_c_declaration",
                test_ttf_rebuilder_header_exports_c_declaration);
}
