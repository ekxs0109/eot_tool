#include <stdio.h>
#include <string.h>

extern "C" {
#include "../src/file_io.h"
#include "../src/wasm_api.h"
void test_register(const char *name, void (*fn)(void));
void test_fail_with_message(const char *message);
}

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

static void test_browser_wasm_api_converts_cff2_instance(void) {
  file_buffer_t input = {};
  wasm_buffer_t output = {};

  ASSERT_OK(file_io_read_all("testdata/cff2-variable.otf", &input));
  ASSERT_OK(wasm_convert_otf_to_embedded_font(input.data, input.length,
                                              "fntdata", "wght=700",
                                              &output));
  ASSERT_TRUE(output.length > 0);

  wasm_buffer_destroy(&output);
  file_io_free(&input);
}

static void test_wasm_runtime_mode_constant_is_exposed(void) {
  const char *mode = wasm_runtime_thread_mode();

  ASSERT_TRUE(mode != nullptr);
  ASSERT_TRUE(strcmp(mode, "single-thread") == 0 ||
              strcmp(mode, "pthreads") == 0);
}

static void test_wasm_api_exposes_runtime_diagnostics_struct(void) {
  wasm_runtime_diagnostics_t diagnostics = {};

  ASSERT_OK(wasm_runtime_get_diagnostics(&diagnostics));
  ASSERT_TRUE(diagnostics.effective_threads >= 1u);
}

extern "C" void register_wasm_api_tests(void) {
  test_register("test_browser_wasm_api_converts_cff2_instance",
                test_browser_wasm_api_converts_cff2_instance);
  test_register("test_wasm_runtime_mode_constant_is_exposed",
                test_wasm_runtime_mode_constant_is_exposed);
  test_register("test_wasm_api_exposes_runtime_diagnostics_struct",
                test_wasm_api_exposes_runtime_diagnostics_struct);
}
