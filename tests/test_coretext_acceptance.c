#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#ifdef __APPLE__
#include <CoreFoundation/CoreFoundation.h>
#include <CoreText/CoreText.h>
#endif

#include "../src/file_io.h"
#include "../src/mtx_decode.h"
#include "../src/mtx_encode.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_writer.h"

void test_register(const char *name, void (*fn)(void));
void test_fail_with_message(const char *message);

#define FAIL_STATUS(expr_text, status_code) do { \
  char msg[256]; \
  snprintf(msg, sizeof(msg), "assertion failed: %s returned %d", \
           (expr_text), (int)(status_code)); \
  test_fail_with_message(msg); \
} while (0)

#ifdef __APPLE__
static int ensure_build_output_dir(void) {
  if (mkdir("build", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build directory");
    return 0;
  }
  if (mkdir("build/out", 0777) != 0 && errno != EEXIST) {
    test_fail_with_message("failed to create build/out directory");
    return 0;
  }
  return 1;
}
#endif
#ifdef __APPLE__
static void cfstring_to_cstr(CFStringRef str, char *out, size_t out_size) {
  if (out_size == 0) {
    return;
  }
  out[0] = '\0';
  if (str == NULL) {
    snprintf(out, out_size, "(null)");
    return;
  }
  if (!CFStringGetCString(str, out, (CFIndex)out_size, kCFStringEncodingUTF8)) {
    snprintf(out, out_size, "(unprintable)");
  }
}

static void fail_coretext_probe(const char *path_under_test,
                                int descriptor_creation_succeeded,
                                CFErrorRef error) {
  char error_description[256];
  CFIndex error_code = 0;
  char message[1024];

  error_description[0] = '\0';
  if (error != NULL) {
    CFStringRef desc = CFErrorCopyDescription(error);
    error_code = CFErrorGetCode(error);
    cfstring_to_cstr(desc, error_description, sizeof(error_description));
    if (desc != NULL) {
      CFRelease(desc);
    }
  } else {
    snprintf(error_description, sizeof(error_description), "(none)");
  }

  snprintf(message, sizeof(message),
           "CoreText acceptance probe failed\n"
           "file path: %s\n"
           "descriptor creation succeeded: %s\n"
           "CoreText error code: %ld\n"
           "CoreText error description: %s",
           path_under_test,
           descriptor_creation_succeeded ? "yes" : "no",
           (long)error_code,
           error_description);
  test_fail_with_message(message);
}
#endif

#ifdef __APPLE__
static void test_otf_cff_roundtrip_is_accepted_by_coretext(void) {
  const char *source_path =
      "testdata/aipptfonts/\351\246\231\350\225\211Plus__20220301185701917366.otf";
  const char *eot_path = "build/out/0213-coretext.eot";
  const char *roundtrip_path = "build/out/0213-coretext.roundtrip.ttf";
  byte_buffer_t eot = {};
  sfnt_font_t decoded = {};
  uint8_t *serialized_sfnt = NULL;
  size_t serialized_sfnt_size = 0u;
  CFURLRef url = NULL;
  CFArrayRef descriptors = NULL;
  CFErrorRef registration_error = NULL;
  Boolean registration_ok = 0;
  int descriptor_creation_succeeded = 0;
  eot_status_t status = EOT_OK;

  sfnt_font_init(&decoded);
  if (!ensure_build_output_dir()) {
    goto cleanup;
  }

  status = mtx_encode_ttf_file(source_path, &eot);
  if (status != EOT_OK) {
    FAIL_STATUS("mtx_encode_ttf_file(source_path, &eot)", status);
    goto cleanup;
  }
  status = file_io_write_all(eot_path, eot.data, eot.length);
  if (status != EOT_OK) {
    FAIL_STATUS("file_io_write_all(eot_path, eot.data, eot.length)", status);
    goto cleanup;
  }
  status = mtx_decode_eot_file(eot_path, &decoded);
  if (status != EOT_OK) {
    FAIL_STATUS("mtx_decode_eot_file(eot_path, &decoded)", status);
    goto cleanup;
  }
  status = sfnt_writer_serialize(&decoded, &serialized_sfnt, &serialized_sfnt_size);
  if (status != EOT_OK) {
    FAIL_STATUS("sfnt_writer_serialize(&decoded, &serialized_sfnt, &serialized_sfnt_size)",
                status);
    goto cleanup;
  }
  status = file_io_write_all(roundtrip_path, serialized_sfnt, serialized_sfnt_size);
  if (status != EOT_OK) {
    FAIL_STATUS("file_io_write_all(roundtrip_path, serialized_sfnt, serialized_sfnt_size)",
                status);
    goto cleanup;
  }

  url = CFURLCreateFromFileSystemRepresentation(
      kCFAllocatorDefault,
      (const UInt8 *)roundtrip_path,
      (CFIndex)strlen(roundtrip_path),
      false);
  if (url == NULL) {
    fail_coretext_probe(roundtrip_path, 0, NULL);
    goto cleanup;
  }

  descriptors = CTFontManagerCreateFontDescriptorsFromURL(url);
  descriptor_creation_succeeded =
      (descriptors != NULL && CFArrayGetCount(descriptors) > 0);
  if (!descriptor_creation_succeeded) {
    fail_coretext_probe(roundtrip_path, 0, NULL);
    goto cleanup;
  }

  registration_ok = CTFontManagerRegisterFontsForURL(
      url, kCTFontManagerScopeProcess, &registration_error);
  if (!registration_ok) {
    fail_coretext_probe(roundtrip_path, 1, registration_error);
    goto cleanup;
  }

cleanup:
  if (registration_ok) {
    CTFontManagerUnregisterFontsForURL(url, kCTFontManagerScopeProcess, NULL);
  }
  if (registration_error != NULL) {
    CFRelease(registration_error);
  }
  if (descriptors != NULL) {
    CFRelease(descriptors);
  }
  if (url != NULL) {
    CFRelease(url);
  }
  free(serialized_sfnt);
  sfnt_font_destroy(&decoded);
  byte_buffer_destroy(&eot);
}
#endif

void register_coretext_acceptance_tests(void) {
#ifdef __APPLE__
  test_register("test_otf_cff_roundtrip_is_accepted_by_coretext",
                test_otf_cff_roundtrip_is_accepted_by_coretext);
#endif
}
