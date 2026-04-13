#include "parallel_runtime.h"

#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <algorithm>
#include <atomic>
#include <new>
#include <system_error>
#include <thread>
#include <vector>

namespace {

constexpr const char *kRequestedModeAuto = "auto";
constexpr const char *kRequestedModeSingle = "single";
constexpr const char *kRequestedModeThreaded = "threaded";
constexpr const char *kResolvedModeSingle = "single";
constexpr const char *kResolvedModeThreaded = "threaded";
constexpr const char *kFallbackReasonNone = "";
constexpr const char *kFallbackReasonRequestedSingle = "requested-single";
constexpr const char *kFallbackReasonThreadingUnavailable = "threading-unavailable";
constexpr const char *kFallbackReasonTaskCountClamped = "task-count-clamped";

enum parallel_requested_mode_t {
  PARALLEL_REQUESTED_MODE_AUTO = 0,
  PARALLEL_REQUESTED_MODE_SINGLE = 1,
  PARALLEL_REQUESTED_MODE_THREADED = 2
};

static int g_test_env_override_enabled = 0;
static char g_test_env_threads_value[32] = {0};
static parallel_requested_mode_t g_requested_mode = PARALLEL_REQUESTED_MODE_AUTO;
static size_t g_last_run_task_count = 0u;
static size_t g_last_run_requested_threads = 1u;
static size_t g_last_run_effective_threads = 1u;
static const char *g_last_run_resolved_mode = kResolvedModeSingle;
static const char *g_last_run_fallback_reason = kFallbackReasonNone;

typedef struct {
  unsigned char *tasks;
  size_t task_size;
  parallel_task_fn fn;
} parallel_task_list_context_t;

typedef struct {
  size_t requested_threads;
  size_t effective_threads;
  const char *resolved_mode;
  const char *fallback_reason;
} parallel_runtime_resolution_t;

static bool parallel_runtime_supports_threads(void) {
#if defined(__EMSCRIPTEN__) && !defined(__EMSCRIPTEN_PTHREADS__)
  return false;
#else
  return true;
#endif
}

static const char *parallel_runtime_get_env(const char *name) {
  if (name == NULL) {
    return NULL;
  }

  if (g_test_env_override_enabled != 0 && strcmp(name, "EOT_TOOL_THREADS") == 0) {
    return g_test_env_threads_value;
  }

  return getenv(name);
}

static size_t parse_threads_value(const char *value) {
  char *endptr = NULL;
  unsigned long long parsed = 0;

  if (value == NULL || value[0] == '\0') {
    return 0u;
  }

  errno = 0;
  parsed = strtoull(value, &endptr, 10);
  if (errno != 0 || endptr == value || endptr == NULL || *endptr != '\0') {
    return 0u;
  }
  if (parsed == 0u) {
    return 0u;
  }
  if (parsed > static_cast<unsigned long long>(SIZE_MAX)) {
    return 0u;
  }

  return static_cast<size_t>(parsed);
}

static size_t parallel_runtime_default_threads(void) {
  size_t native_threads = static_cast<size_t>(std::thread::hardware_concurrency());
  if (native_threads == 0u) {
    return 1u;
  }
  return native_threads;
}

static bool parallel_runtime_fallback_reason_is_none(const char *reason) {
  return reason == NULL || reason[0] == '\0';
}

static parallel_runtime_resolution_t parallel_runtime_resolve(size_t task_count) {
  parallel_runtime_resolution_t resolution;
  const char *configured_threads = parallel_runtime_get_env("EOT_TOOL_THREADS");
  size_t requested_threads = parse_threads_value(configured_threads);

  if (requested_threads == 0u) {
    requested_threads = parallel_runtime_default_threads();
  }

  resolution.requested_threads = requested_threads;
  resolution.effective_threads = requested_threads;
  resolution.resolved_mode = kResolvedModeThreaded;
  resolution.fallback_reason = kFallbackReasonNone;

  if (g_requested_mode == PARALLEL_REQUESTED_MODE_SINGLE) {
    resolution.requested_threads = 1u;
    resolution.effective_threads = 1u;
    resolution.resolved_mode = kResolvedModeSingle;
    resolution.fallback_reason = kFallbackReasonRequestedSingle;
  } else if (!parallel_runtime_supports_threads()) {
    resolution.effective_threads = 1u;
    resolution.resolved_mode = kResolvedModeSingle;
    if (resolution.requested_threads > 1u || g_requested_mode == PARALLEL_REQUESTED_MODE_THREADED) {
      resolution.fallback_reason = kFallbackReasonThreadingUnavailable;
    }
  } else if (resolution.requested_threads <= 1u) {
    resolution.effective_threads = 1u;
    resolution.resolved_mode = kResolvedModeSingle;
  }

  if (task_count == 0u) {
    resolution.effective_threads = 0u;
    return resolution;
  }

  if (resolution.effective_threads > task_count) {
    resolution.effective_threads = task_count;
    if (resolution.effective_threads > 0u && resolution.requested_threads > resolution.effective_threads &&
        parallel_runtime_fallback_reason_is_none(resolution.fallback_reason)) {
      resolution.fallback_reason = kFallbackReasonTaskCountClamped;
    }
  }

  if (resolution.effective_threads <= 1u) {
    resolution.resolved_mode = kResolvedModeSingle;
  } else {
    resolution.resolved_mode = kResolvedModeThreaded;
  }

  return resolution;
}

static void join_all_workers(std::vector<std::thread> *workers) {
  size_t i;
  if (workers == NULL) {
    return;
  }
  for (i = 0u; i < workers->size(); i++) {
    if ((*workers)[i].joinable()) {
      (*workers)[i].join();
    }
  }
}

static eot_status_t run_task_list_entry(size_t index, void *context) {
  parallel_task_list_context_t *task_list_context =
      static_cast<parallel_task_list_context_t *>(context);
  void *task;

  if (task_list_context == NULL || task_list_context->tasks == NULL ||
      task_list_context->task_size == 0u || task_list_context->fn == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  task = task_list_context->tasks + (index * task_list_context->task_size);
  return task_list_context->fn(task);
}

}  // namespace

extern "C" size_t parallel_runtime_effective_threads(void) {
  return parallel_runtime_resolve(SIZE_MAX).effective_threads;
}

extern "C" eot_status_t parallel_runtime_run_indexed_tasks(size_t task_count,
                                                           parallel_indexed_task_fn fn,
                                                           void *context) {
  parallel_runtime_resolution_t resolution;
  size_t effective_threads;
  size_t index;

  if (fn == NULL && task_count > 0u) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  g_last_run_task_count = task_count;
  resolution = parallel_runtime_resolve(task_count);
  g_last_run_requested_threads = resolution.requested_threads;
  g_last_run_effective_threads = resolution.effective_threads;
  g_last_run_resolved_mode = resolution.resolved_mode;
  g_last_run_fallback_reason = resolution.fallback_reason;

  if (task_count == 0u) {
    return EOT_OK;
  }
  effective_threads = std::max<size_t>(1u, resolution.effective_threads);

  try {
    std::vector<eot_status_t> statuses(task_count, EOT_OK);

    if (effective_threads == 1u) {
      for (index = 0u; index < task_count; index++) {
        statuses[index] = fn(index, context);
      }
    } else if (parallel_runtime_supports_threads()) {
      std::atomic<size_t> next_index(0u);
      std::vector<std::thread> workers;
      workers.reserve(effective_threads - 1u);

      auto worker = [&]() {
        for (;;) {
          size_t task_index = next_index.fetch_add(1u, std::memory_order_relaxed);
          if (task_index >= task_count) {
            return;
          }
          statuses[task_index] = fn(task_index, context);
        }
      };

      try {
        for (index = 0u; index + 1u < effective_threads; index++) {
          workers.emplace_back(worker);
        }
      } catch (...) {
        join_all_workers(&workers);
        return EOT_ERR_ALLOCATION;
      }
      worker();
      join_all_workers(&workers);
    }

    for (size_t i = 0; i < task_count; ++i) {
      if (statuses[i] != EOT_OK) {
        return statuses[i];
      }
    }
  } catch (const std::bad_alloc &) {
    return EOT_ERR_ALLOCATION;
  } catch (const std::system_error &) {
    return EOT_ERR_ALLOCATION;
  } catch (...) {
    return EOT_ERR_ALLOCATION;
  }

  return EOT_OK;
}

extern "C" eot_status_t parallel_runtime_run_task_list(void *tasks,
                                                       size_t task_count,
                                                       size_t task_size,
                                                       parallel_task_fn fn) {
  parallel_task_list_context_t context;

  if (task_count == 0u) {
    return parallel_runtime_run_indexed_tasks(0u, NULL, NULL);
  }
  if (tasks == NULL || task_size == 0u || fn == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  context.tasks = static_cast<unsigned char *>(tasks);
  context.task_size = task_size;
  context.fn = fn;
  return parallel_runtime_run_indexed_tasks(task_count, run_task_list_entry, &context);
}

extern "C" eot_status_t parallel_runtime_set_requested_mode(const char *mode) {
  if (mode == NULL || mode[0] == '\0' || strcmp(mode, kRequestedModeAuto) == 0) {
    g_requested_mode = PARALLEL_REQUESTED_MODE_AUTO;
    return EOT_OK;
  }
  if (strcmp(mode, kRequestedModeSingle) == 0) {
    g_requested_mode = PARALLEL_REQUESTED_MODE_SINGLE;
    return EOT_OK;
  }
  if (strcmp(mode, kRequestedModeThreaded) == 0) {
    g_requested_mode = PARALLEL_REQUESTED_MODE_THREADED;
    return EOT_OK;
  }
  return EOT_ERR_INVALID_ARGUMENT;
}

extern "C" eot_status_t parallel_runtime_set_test_env(const char *name, const char *value) {
  size_t value_length;

  if (name == NULL || value == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (strcmp(name, "EOT_TOOL_THREADS") != 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  value_length = strlen(value);
  if (value_length >= sizeof(g_test_env_threads_value)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  memcpy(g_test_env_threads_value, value, value_length + 1u);
  g_test_env_override_enabled = 1;
  return EOT_OK;
}

extern "C" void parallel_runtime_clear_test_env(void) {
  g_test_env_override_enabled = 0;
  g_test_env_threads_value[0] = '\0';
  g_requested_mode = PARALLEL_REQUESTED_MODE_AUTO;
}

extern "C" size_t parallel_runtime_last_run_task_count(void) {
  return g_last_run_task_count;
}

extern "C" size_t parallel_runtime_last_run_requested_threads(void) {
  return g_last_run_requested_threads;
}

extern "C" size_t parallel_runtime_last_run_effective_threads(void) {
  return g_last_run_effective_threads;
}

extern "C" const char *parallel_runtime_last_run_resolved_mode(void) {
  return g_last_run_resolved_mode;
}

extern "C" const char *parallel_runtime_last_run_fallback_reason(void) {
  return g_last_run_fallback_reason;
}
