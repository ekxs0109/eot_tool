#include <stdio.h>
#include <string.h>

extern "C" {
#include "../src/file_io.h"
#include "../src/parallel_runtime.h"
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

#define ASSERT_EQ_SIZE(actual, expected) do { \
  size_t actual__ = (size_t)(actual); \
  size_t expected__ = (size_t)(expected); \
  if (actual__ != expected__) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual=%zu expected=%zu)", \
             #actual, #expected, actual__, expected__); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

#define ASSERT_STREQ(actual, expected) do { \
  const char *actual__ = (actual); \
  const char *expected__ = (expected); \
  if (((actual__ == nullptr) != (expected__ == nullptr)) || \
      (actual__ != nullptr && strcmp(actual__, expected__) != 0)) { \
    char msg[256]; \
    snprintf(msg, sizeof(msg), "assertion failed: %s == %s (actual=%s expected=%s)", \
             #actual, #expected, actual__ != nullptr ? actual__ : "(null)", \
             expected__ != nullptr ? expected__ : "(null)"); \
    test_fail_with_message(msg); \
    return; \
  } \
} while (0)

static eot_status_t ok_task(size_t index, void *context) {
  (void)index;
  (void)context;
  return EOT_OK;
}

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

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(3u, ok_task, nullptr));
  ASSERT_OK(wasm_runtime_get_diagnostics(&diagnostics));
  ASSERT_EQ_SIZE(diagnostics.requested_threads, 8u);
  ASSERT_EQ_SIZE(diagnostics.effective_threads, 3u);
  ASSERT_STREQ(diagnostics.resolved_mode, "threaded");
  ASSERT_STREQ(diagnostics.fallback_reason, "task-count-clamped");
  parallel_runtime_clear_test_env();
}

static void test_wasm_api_exposes_single_mode_runtime_diagnostics(void) {
  wasm_runtime_diagnostics_t diagnostics = {};

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_set_requested_mode("single"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(4u, ok_task, nullptr));
  ASSERT_OK(wasm_runtime_get_diagnostics(&diagnostics));
  ASSERT_EQ_SIZE(diagnostics.requested_threads, 1u);
  ASSERT_TRUE(diagnostics.effective_threads >= 1u);
  ASSERT_STREQ(diagnostics.resolved_mode, "single");
  ASSERT_STREQ(diagnostics.fallback_reason, "requested-single");
  parallel_runtime_clear_test_env();
}

static void test_wasm_api_rejects_null_diagnostics_pointer(void) {
  ASSERT_TRUE(wasm_runtime_get_diagnostics(nullptr) == EOT_ERR_INVALID_ARGUMENT);
}

static void test_wasm_api_reports_zero_effective_threads_for_zero_task_run(void) {
  wasm_runtime_diagnostics_t diagnostics = {};

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(0u, nullptr, nullptr));
  ASSERT_OK(wasm_runtime_get_diagnostics(&diagnostics));
  ASSERT_EQ_SIZE(diagnostics.requested_threads, 8u);
  ASSERT_EQ_SIZE(diagnostics.effective_threads, 0u);
  parallel_runtime_clear_test_env();
}

extern "C" void register_wasm_api_tests(void) {
  test_register("test_browser_wasm_api_converts_cff2_instance",
                test_browser_wasm_api_converts_cff2_instance);
  test_register("test_wasm_runtime_mode_constant_is_exposed",
                test_wasm_runtime_mode_constant_is_exposed);
  test_register("test_wasm_api_exposes_runtime_diagnostics_struct",
                test_wasm_api_exposes_runtime_diagnostics_struct);
  test_register("test_wasm_api_exposes_single_mode_runtime_diagnostics",
                test_wasm_api_exposes_single_mode_runtime_diagnostics);
  test_register("test_wasm_api_rejects_null_diagnostics_pointer",
                test_wasm_api_rejects_null_diagnostics_pointer);
  test_register("test_wasm_api_reports_zero_effective_threads_for_zero_task_run",
                test_wasm_api_reports_zero_effective_threads_for_zero_task_run);
}
