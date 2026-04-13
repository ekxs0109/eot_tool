#include <stdio.h>
#include <atomic>
#include <cstring>
#include <limits>

extern "C" {
#include "../src/parallel_runtime.h"
void test_fail_with_message(const char *message);
void test_register(const char *name, void (*fn)(void));
}

#define ASSERT_OK(expr) do { \
  eot_status_t status__ = (expr); \
  if (status__ != EOT_OK) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s returned %d", #expr, status__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_TRUE(expr) do { \
  if (!(expr)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s", #expr); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_EQ_SIZE(actual, expected) do { \
  size_t actual__ = (size_t)(actual); \
  size_t expected__ = (size_t)(expected); \
  if (actual__ != expected__) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s == %s (actual=%zu expected=%zu)", \
             #actual, #expected, actual__, expected__); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

#define ASSERT_STREQ(actual, expected) do { \
  const char *actual__ = (actual); \
  const char *expected__ = (expected); \
  if (((actual__ == NULL) != (expected__ == NULL)) || \
      (actual__ != NULL && std::strcmp(actual__, expected__) != 0)) { \
    char msg__[256]; \
    snprintf(msg__, sizeof(msg__), "assertion failed: %s == %s (actual=%s expected=%s)", \
             #actual, #expected, actual__ != NULL ? actual__ : "(null)", \
             expected__ != NULL ? expected__ : "(null)"); \
    test_fail_with_message(msg__); \
    return; \
  } \
} while (0)

struct indexed_context {
  size_t seen_count;
  size_t seen_indices[16];
};

struct failing_context {
  std::atomic<int> seen[4];
};

struct failure_order_context {
  std::atomic<int> allow_index_zero_to_fail;
  std::atomic<int> seen[4];
};

static eot_status_t record_index_task(size_t index, void *context) {
  indexed_context *ctx = (indexed_context *)context;
  if (ctx == NULL || ctx->seen_count >= (sizeof(ctx->seen_indices) / sizeof(ctx->seen_indices[0]))) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  ctx->seen_indices[ctx->seen_count++] = index;
  return EOT_OK;
}

static eot_status_t ok_task(size_t index, void *context) {
  (void)index;
  (void)context;
  return EOT_OK;
}

static eot_status_t fail_at_index_two_task(size_t index, void *context) {
  failing_context *ctx = (failing_context *)context;
  if (ctx == NULL || index >= 4u) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  ctx->seen[index].store(1, std::memory_order_relaxed);
  if (index == 2u) {
    return EOT_ERR_CORRUPT_DATA;
  }
  return EOT_OK;
}

static eot_status_t fail_multiple_indices_task(size_t index, void *context) {
  failure_order_context *ctx = (failure_order_context *)context;
  if (ctx == NULL || index >= 4u) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  ctx->seen[index].store(1, std::memory_order_relaxed);

  if (index == 0u) {
    while (ctx->allow_index_zero_to_fail.load(std::memory_order_acquire) == 0) {
    }
    return EOT_ERR_CORRUPT_DATA;
  }
  if (index == 3u) {
    ctx->allow_index_zero_to_fail.store(1, std::memory_order_release);
    return EOT_ERR_INVALID_PADDING;
  }
  return EOT_OK;
}

static void test_parallel_runtime_defaults_to_at_least_one_thread(void) {
  parallel_runtime_clear_test_env();
  ASSERT_TRUE(parallel_runtime_effective_threads() >= 1u);
}

static void test_parallel_runtime_supports_single_thread_test_override(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "1"));
  ASSERT_EQ_SIZE(parallel_runtime_effective_threads(), 1u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_effective_threads_reports_preferred_workers_before_task_clamp(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_EQ_SIZE(parallel_runtime_effective_threads(), 8u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_runs_indexed_tasks_in_order(void) {
  indexed_context context = {0, {0}};

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "1"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(5u, record_index_task, &context));
  ASSERT_EQ_SIZE(context.seen_count, 5u);
  ASSERT_EQ_SIZE(context.seen_indices[0], 0u);
  ASSERT_EQ_SIZE(context.seen_indices[1], 1u);
  ASSERT_EQ_SIZE(context.seen_indices[2], 2u);
  ASSERT_EQ_SIZE(context.seen_indices[3], 3u);
  ASSERT_EQ_SIZE(context.seen_indices[4], 4u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_invalid_override_falls_back_to_at_least_one_thread(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "invalid"));
  ASSERT_TRUE(parallel_runtime_effective_threads() >= 1u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_uses_requested_parallelism_when_available(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "4"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(4u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), 4u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 4u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 4u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "threaded");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "");
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_reports_requested_and_effective_threads(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(3u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 8u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 3u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "threaded");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "task-count-clamped");
}

static void test_parallel_runtime_forces_single_thread_mode(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_set_requested_mode("single"));
  ASSERT_EQ_SIZE(parallel_runtime_effective_threads(), 1u);
  ASSERT_OK(parallel_runtime_run_indexed_tasks(4u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 1u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "single");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "requested-single");
}

static void test_parallel_runtime_reports_single_mode_after_task_clamp(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(1u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 8u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 1u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "single");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "task-count-clamped");
}

static void test_parallel_runtime_threaded_mode_restores_normal_parallel_resolution(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "8"));
  ASSERT_OK(parallel_runtime_set_requested_mode("single"));
  ASSERT_OK(parallel_runtime_set_requested_mode("threaded"));
  ASSERT_EQ_SIZE(parallel_runtime_effective_threads(), 8u);
  ASSERT_OK(parallel_runtime_run_indexed_tasks(4u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_requested_threads(), 8u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 4u);
  ASSERT_STREQ(parallel_runtime_last_run_resolved_mode(), "threaded");
  ASSERT_STREQ(parallel_runtime_last_run_fallback_reason(), "task-count-clamped");
}

static void test_parallel_runtime_clamps_effective_threads_to_task_count(void) {
  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "64"));
  ASSERT_OK(parallel_runtime_run_indexed_tasks(3u, ok_task, NULL));
  ASSERT_EQ_SIZE(parallel_runtime_last_run_task_count(), 3u);
  ASSERT_EQ_SIZE(parallel_runtime_last_run_effective_threads(), 3u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_waits_for_all_started_tasks_before_reporting_error(void) {
  failing_context context;
  size_t i;

  for (i = 0u; i < 4u; ++i) {
    context.seen[i].store(0, std::memory_order_relaxed);
  }

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "4"));
  ASSERT_TRUE(parallel_runtime_run_indexed_tasks(4u, fail_at_index_two_task, &context) ==
              EOT_ERR_CORRUPT_DATA);
  ASSERT_EQ_SIZE(context.seen[0].load(std::memory_order_relaxed), 1u);
  ASSERT_EQ_SIZE(context.seen[1].load(std::memory_order_relaxed), 1u);
  ASSERT_EQ_SIZE(context.seen[2].load(std::memory_order_relaxed), 1u);
  ASSERT_EQ_SIZE(context.seen[3].load(std::memory_order_relaxed), 1u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_uses_lowest_failing_index_regardless_of_completion_order(void) {
  failure_order_context context;
  size_t i;

  context.allow_index_zero_to_fail.store(0, std::memory_order_relaxed);
  for (i = 0u; i < 4u; ++i) {
    context.seen[i].store(0, std::memory_order_relaxed);
  }

  parallel_runtime_clear_test_env();
  ASSERT_OK(parallel_runtime_set_test_env("EOT_TOOL_THREADS", "4"));
  ASSERT_TRUE(parallel_runtime_run_indexed_tasks(4u, fail_multiple_indices_task, &context) ==
              EOT_ERR_CORRUPT_DATA);
  ASSERT_EQ_SIZE(context.seen[0].load(std::memory_order_relaxed), 1u);
  ASSERT_EQ_SIZE(context.seen[3].load(std::memory_order_relaxed), 1u);
  parallel_runtime_clear_test_env();
}

static void test_parallel_runtime_does_not_throw_on_allocation_failure(void) {
  bool threw = false;
  eot_status_t status = EOT_OK;

  try {
    status = parallel_runtime_run_indexed_tasks(std::numeric_limits<size_t>::max(), ok_task, NULL);
  } catch (...) {
    threw = true;
  }

  ASSERT_TRUE(!threw);
  ASSERT_TRUE(status == EOT_ERR_ALLOCATION);
}

extern "C" void register_parallel_runtime_tests(void) {
  test_register("test_parallel_runtime_defaults_to_at_least_one_thread",
                test_parallel_runtime_defaults_to_at_least_one_thread);
  test_register("test_parallel_runtime_supports_single_thread_test_override",
                test_parallel_runtime_supports_single_thread_test_override);
  test_register("test_parallel_runtime_effective_threads_reports_preferred_workers_before_task_clamp",
                test_parallel_runtime_effective_threads_reports_preferred_workers_before_task_clamp);
  test_register("test_parallel_runtime_runs_indexed_tasks_in_order",
                test_parallel_runtime_runs_indexed_tasks_in_order);
  test_register("test_parallel_runtime_invalid_override_falls_back_to_at_least_one_thread",
                test_parallel_runtime_invalid_override_falls_back_to_at_least_one_thread);
  test_register("test_parallel_runtime_uses_requested_parallelism_when_available",
                test_parallel_runtime_uses_requested_parallelism_when_available);
  test_register("test_parallel_runtime_reports_requested_and_effective_threads",
                test_parallel_runtime_reports_requested_and_effective_threads);
  test_register("test_parallel_runtime_forces_single_thread_mode",
                test_parallel_runtime_forces_single_thread_mode);
  test_register("test_parallel_runtime_reports_single_mode_after_task_clamp",
                test_parallel_runtime_reports_single_mode_after_task_clamp);
  test_register("test_parallel_runtime_threaded_mode_restores_normal_parallel_resolution",
                test_parallel_runtime_threaded_mode_restores_normal_parallel_resolution);
  test_register("test_parallel_runtime_clamps_effective_threads_to_task_count",
                test_parallel_runtime_clamps_effective_threads_to_task_count);
  test_register("test_parallel_runtime_waits_for_all_started_tasks_before_reporting_error",
                test_parallel_runtime_waits_for_all_started_tasks_before_reporting_error);
  test_register("test_parallel_runtime_uses_lowest_failing_index_regardless_of_completion_order",
                test_parallel_runtime_uses_lowest_failing_index_regardless_of_completion_order);
  test_register("test_parallel_runtime_does_not_throw_on_allocation_failure",
                test_parallel_runtime_does_not_throw_on_allocation_failure);
}
