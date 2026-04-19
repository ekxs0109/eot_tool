#ifndef EOT_TOOL_CU2QU_H_
#define EOT_TOOL_CU2QU_H_

#include "cff_types.h"
#include "file_io.h"

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Converts one cubic Bezier to a quadratic spline.
 *
 * On entry, any existing storage owned by out_spline is released and the
 * structure is reset before validation or conversion.
 *
 * Returns EOT_ERR_CORRUPT_DATA if no quadratic approximation is found within
 * the implementation's segment search cap.
 */
eot_status_t curve_to_quadratic(const cubic_curve_t *curve,
                                double max_err,
                                quadratic_spline_t *out_spline);

#ifdef __cplusplus
}
#endif

#endif
