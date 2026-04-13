#include <stdio.h>
#include <string.h>

#include "../src/subset_args.h"

extern void test_register(const char *name, void (*fn)(void));
extern void test_fail_with_message(const char *message);

#define ASSERT_OK(expr) do { \
  eot_status_t status = (expr); \
  if (status != EOT_OK) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s returned %d", #expr, status); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s", #expr); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_EQ(actual, expected) do { \
  if ((actual) != (expected)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual: %d, expected: %d)", \
             #actual, #expected, (int)(actual), (int)(expected)); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static void test_subset_args_parses_text_mode(void) {
  const char *argv[] = {
    "fonttool", "subset", "in.eot", "out.eot", "--text", "你好ABC"
  };
  subset_request_t request;

  ASSERT_OK(subset_args_parse(6, argv, &request));
  ASSERT_EQ(request.selection_mode, SUBSET_SELECTION_TEXT);
  ASSERT_TRUE(strcmp(request.input_path, "in.eot") == 0);
  ASSERT_TRUE(strcmp(request.output_path, "out.eot") == 0);
  ASSERT_TRUE(strcmp(request.selection_data, "你好ABC") == 0);
  ASSERT_TRUE(!request.keep_gids);

  subset_request_destroy(&request);
}

static void test_subset_args_parses_keep_gids(void) {
  const char *argv[] = {
    "fonttool", "subset", "in.eot", "out.eot", "--glyph-ids", "0,1,2", "--keep-gids"
  };
  subset_request_t request;

  ASSERT_OK(subset_args_parse(7, argv, &request));
  ASSERT_EQ(request.selection_mode, SUBSET_SELECTION_GLYPH_IDS);
  ASSERT_TRUE(request.keep_gids);
  ASSERT_TRUE(strcmp(request.selection_data, "0,1,2") == 0);

  subset_request_destroy(&request);
}

static void test_subset_args_parses_unicode_ranges(void) {
  const char *argv[] = {
    "fonttool", "subset", "in.eot", "out.eot",
    "--unicodes", "U+0041-0043,U+4F60,U+597D"
  };
  subset_request_t request;

  ASSERT_OK(subset_args_parse(6, argv, &request));
  ASSERT_EQ(request.selection_mode, SUBSET_SELECTION_UNICODES);
  ASSERT_TRUE(strcmp(request.selection_data, "U+0041-0043,U+4F60,U+597D") == 0);

  subset_request_destroy(&request);
}

static void test_subset_args_rejects_missing_selection(void) {
  const char *argv[] = {
    "fonttool", "subset", "in.eot", "out.eot", "--keep-gids"
  };
  subset_request_t request;
  eot_status_t status = subset_args_parse(5, argv, &request);

  ASSERT_TRUE(status == EOT_ERR_INVALID_ARGUMENT);
}

static void test_subset_request_init_for_glyph_ids_keep_sets_flag(void) {
  subset_request_t request;

  ASSERT_OK(subset_request_init_for_glyph_ids_keep(&request, "0,35"));
  ASSERT_EQ(request.selection_mode, SUBSET_SELECTION_GLYPH_IDS);
  ASSERT_TRUE(request.keep_gids);
  ASSERT_TRUE(strcmp(request.selection_data, "0,35") == 0);

  subset_request_destroy(&request);
}

void register_subset_args_tests(void) {
  test_register("test_subset_args_parses_text_mode",
                test_subset_args_parses_text_mode);
  test_register("test_subset_args_parses_keep_gids",
                test_subset_args_parses_keep_gids);
  test_register("test_subset_args_parses_unicode_ranges",
                test_subset_args_parses_unicode_ranges);
  test_register("test_subset_args_rejects_missing_selection",
                test_subset_args_rejects_missing_selection);
  test_register("test_subset_request_init_for_glyph_ids_keep_sets_flag",
                test_subset_request_init_for_glyph_ids_keep_sets_flag);
}
