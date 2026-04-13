#ifndef EOT_TOOL_PARALLEL_RUNTIME_H
#define EOT_TOOL_PARALLEL_RUNTIME_H

#include <stddef.h>

#include "file_io.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef eot_status_t (*parallel_indexed_task_fn)(size_t index, void *context);
typedef eot_status_t (*parallel_task_fn)(void *task);

/*
 * Returns the process-wide preferred worker count for a non-empty run after
 * applying environment and requested-mode overrides, but before clamping to a
 * specific task count.
 */
size_t parallel_runtime_effective_threads(void);
eot_status_t parallel_runtime_run_indexed_tasks(size_t task_count,
                                                parallel_indexed_task_fn fn,
                                                void *context);
eot_status_t parallel_runtime_run_task_list(void *tasks,
                                            size_t task_count,
                                            size_t task_size,
                                            parallel_task_fn fn);
/*
 * Process-wide requested-mode override.
 * Passing NULL or "auto" clears the override.
 * "single" forces serial execution and reports requested_threads as 1.
 * "threaded" only re-enables normal native parallel resolution; it does not
 * guarantee that more than one worker will be used.
 */
eot_status_t parallel_runtime_set_requested_mode(const char *mode);
eot_status_t parallel_runtime_set_test_env(const char *name, const char *value);
/* Clears test-only overrides, including the requested mode override. */
void parallel_runtime_clear_test_env(void);
/*
 * Process-wide diagnostics for the most recent run. These values are global,
 * overwritten by each call to parallel_runtime_run_*(), and not thread-local.
 */
size_t parallel_runtime_last_run_task_count(void);
size_t parallel_runtime_last_run_requested_threads(void);
size_t parallel_runtime_last_run_effective_threads(void);
/* Returned strings point to static storage and stay valid for the process lifetime. */
const char *parallel_runtime_last_run_resolved_mode(void);
const char *parallel_runtime_last_run_fallback_reason(void);

#ifdef __cplusplus
}
#endif

#endif
