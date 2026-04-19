#ifndef FONTTOOL_CFF_NATIVE_BRIDGE_H_
#define FONTTOOL_CFF_NATIVE_BRIDGE_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* data;
  size_t length;
} fonttool_cff_native_buffer_t;

int fonttool_cff_convert_static_otf_to_ttf(const uint8_t* input_bytes,
                                           size_t input_length,
                                           fonttool_cff_native_buffer_t* out_buffer);

void fonttool_cff_native_buffer_free(fonttool_cff_native_buffer_t* buffer);

#ifdef __cplusplus
}
#endif

#endif
