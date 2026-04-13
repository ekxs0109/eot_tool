#ifndef EOT_TOOL_SUBSET_BACKEND_HARFBUZZ_H
#define EOT_TOOL_SUBSET_BACKEND_HARFBUZZ_H

#include "sfnt_subset.h"

#ifdef __cplusplus
extern "C" {
#endif

eot_status_t subset_backend_harfbuzz_run(const sfnt_font_t *input,
                                         const subset_plan_t *plan,
                                         sfnt_font_t *output,
                                         subset_warnings_t *warnings);

#ifdef __cplusplus
}
#endif

#endif
