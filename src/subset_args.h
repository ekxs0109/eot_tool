#ifndef EOT_TOOL_SUBSET_ARGS_H
#define EOT_TOOL_SUBSET_ARGS_H

#include "sfnt_subset.h"

eot_status_t subset_args_parse(int argc, const char *argv[],
                               subset_request_t *out_request);

#endif
