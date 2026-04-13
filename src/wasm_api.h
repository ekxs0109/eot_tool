#ifndef EOT_TOOL_WASM_API_H_
#define EOT_TOOL_WASM_API_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#include "file_io.h"

typedef struct {
  uint8_t *data;
  size_t length;
} wasm_buffer_t;

typedef struct {
  size_t requested_threads;
  size_t effective_threads;
  const char *resolved_mode;
  const char *fallback_reason;
} wasm_runtime_diagnostics_t;

const char *wasm_runtime_thread_mode(void);
void wasm_buffer_destroy(wasm_buffer_t *buffer);
eot_status_t wasm_convert_otf_to_embedded_font(const uint8_t *input,
                                               size_t input_size,
                                               const char *output_kind,
                                               const char *variation_axes,
                                               wasm_buffer_t *out);
eot_status_t wasm_runtime_get_diagnostics(wasm_runtime_diagnostics_t *diagnostics);

#ifdef __cplusplus
}
#endif

#endif
