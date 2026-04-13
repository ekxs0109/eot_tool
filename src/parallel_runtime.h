#ifndef EOT_TOOL_PARALLEL_RUNTIME_H
#define EOT_TOOL_PARALLEL_RUNTIME_H

#include <stddef.h>

#include "file_io.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef eot_status_t (*parallel_indexed_task_fn)(size_t index, void *context);
typedef eot_status_t (*parallel_task_fn)(void *task);

size_t parallel_runtime_effective_threads(void);
eot_status_t parallel_runtime_run_indexed_tasks(size_t task_count,
                                                parallel_indexed_task_fn fn,
                                                void *context);
eot_status_t parallel_runtime_run_task_list(void *tasks,
                                            size_t task_count,
                                            size_t task_size,
                                            parallel_task_fn fn);
eot_status_t parallel_runtime_set_test_env(const char *name, const char *value);
void parallel_runtime_clear_test_env(void);
size_t parallel_runtime_last_run_task_count(void);
size_t parallel_runtime_last_run_requested_threads(void);
size_t parallel_runtime_last_run_effective_threads(void);

#ifdef __cplusplus
}
#endif

#endif
