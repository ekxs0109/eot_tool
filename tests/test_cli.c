#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "../src/eot_header.h"
#include "../src/file_io.h"
#include "../src/sfnt_font.h"
#include "../src/sfnt_reader.h"
#include "../src/sfnt_writer.h"

#define EOT_FLAG_PPT_XOR 0x10000000u

void test_register(const char *name, void (*fn)(void));
void test_capture_command(int argc, char *argv[], int expected_status,
                          const char *expected_fragment);
void test_capture_stderr_command(int argc, char *argv[], int expected_status,
                                 const char *expected_fragment);
void test_capture_command_streams(int argc, char *argv[], int *actual_status,
                                  char *stdout_output, size_t stdout_output_size,
                                  char *stderr_output, size_t stderr_output_size);
void test_assert_stderr_occurrences(const char *expected_fragment,
                                    int expected_count);
void test_fail_with_message(const char *message);

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

static void write_u16be_local(uint8_t *dest, uint16_t value) {
  dest[0] = (uint8_t)(value >> 8);
  dest[1] = (uint8_t)(value & 0xFF);
}

static int write_minimal_vdmx_ttf_fixture(const char *path) {
  sfnt_font_t font;
  uint8_t head[54] = {0};
  uint8_t os2[86] = {0};
  uint8_t maxp[6] = {0};
  uint8_t loca[4] = {0};
  uint8_t vdmx[4] = {0x00, 0x01, 0x00, 0x00};
  uint8_t *serialized = NULL;
  size_t serialized_size = 0;
  eot_status_t status;

  sfnt_font_init(&font);

  write_u16be_local(head + 18, 1000);
  write_u16be_local(head + 50, 0);
  write_u16be_local(os2 + 4, 400);
  write_u16be_local(maxp + 4, 1);

  status = sfnt_font_add_table(&font, 0x68656164u, head, sizeof(head));
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&font, 0x4f532f32u, os2, sizeof(os2));
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&font, 0x6d617870u, maxp, sizeof(maxp));
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&font, 0x676c7966u, NULL, 0);
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&font, 0x6c6f6361u, loca, sizeof(loca));
  }
  if (status == EOT_OK) {
    status = sfnt_font_add_table(&font, 0x56444d58u, vdmx, sizeof(vdmx));
  }
  if (status == EOT_OK) {
    status = sfnt_writer_serialize(&font, &serialized, &serialized_size);
  }
  if (status == EOT_OK) {
    status = file_io_write_all(path, serialized, serialized_size);
  }

  free(serialized);
  sfnt_font_destroy(&font);

  if (status != EOT_OK) {
    test_fail_with_message("failed to write minimal VDMX TTF fixture");
    return 0;
  }

  return 1;
}

static int write_obfuscated_fixture_copy(const char *source_path,
                                         const char *dest_path) {
  file_buffer_t input = {0};
  eot_header_t header;
  buffer_view_t view;
  uint32_t flags;
  eot_status_t status = file_io_read_all(source_path, &input);
  if (status != EOT_OK) {
    test_fail_with_message("failed to read source EOT fixture");
    return 0;
  }

  view = buffer_view_make(input.data, input.length);
  status = eot_header_parse(view, &header);
  if (status != EOT_OK) {
    file_io_free(&input);
    test_fail_with_message("failed to parse source EOT fixture");
    return 0;
  }

  if (header.header_length >= input.length ||
      header.header_length + header.font_data_size > input.length) {
    eot_header_destroy(&header);
    file_io_free(&input);
    test_fail_with_message("source EOT fixture has invalid size metadata");
    return 0;
  }

  flags = read_u32le(input.data + 12);
  write_u32le(input.data + 12, flags | EOT_FLAG_PPT_XOR);

  for (size_t i = 0; i < header.font_data_size; i++) {
    input.data[header.header_length + i] ^= 0x50u;
  }

  status = file_io_write_all(dest_path, input.data, input.length);
  eot_header_destroy(&header);
  file_io_free(&input);

  if (status != EOT_OK) {
    test_fail_with_message("failed to write obfuscated fixture");
    return 0;
  }

  return 1;
}

static void test_cli_help_prints_usage(void) {
  char *argv[] = {"fonttool", "--help"};
  test_capture_command(2, argv, 0, "usage: fonttool <encode|decode>");
}

static void test_cli_without_help_reports_not_implemented(void) {
  char *argv[] = {"fonttool"};
  test_capture_command(1, argv, 2, "error: command not implemented yet");
}

static void test_decode_command_writes_valid_ttf(void) {
  char *argv[] = {
    "fonttool", "decode", "testdata/wingdings3.eot", "build/out/wingdings3.ttf"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/wingdings3.ttf");

  test_capture_command(4, argv, 0, "Decoded testdata/wingdings3.eot");

  if (access("build/out/wingdings3.ttf", F_OK) != 0) {
    test_fail_with_message("decoded TTF file was not created");
    return;
  }
}

static void test_roundtrip_open_sans_writes_decodeable_ttf(void) {
  char *encode_argv[] = {
    "fonttool", "encode", "testdata/OpenSans-Regular.ttf",
    "build/out/OpenSans-Regular.eot"
  };
  char *decode_argv[] = {
    "fonttool", "decode", "build/out/OpenSans-Regular.eot",
    "build/out/OpenSans-Regular.roundtrip.ttf"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/OpenSans-Regular.eot");
  unlink("build/out/OpenSans-Regular.roundtrip.ttf");

  test_capture_command(4, encode_argv, 0, "Encoded testdata/OpenSans-Regular.ttf");
  test_capture_command(4, decode_argv, 0, "Decoded build/out/OpenSans-Regular.eot");

  if (access("build/out/OpenSans-Regular.roundtrip.ttf", F_OK) != 0) {
    test_fail_with_message("roundtrip TTF file was not created");
    return;
  }

  sfnt_font_t font;
  eot_status_t status =
      sfnt_reader_load_file("build/out/OpenSans-Regular.roundtrip.ttf", &font);
  if (status != EOT_OK) {
    test_fail_with_message("roundtrip TTF could not be parsed as SFNT");
    return;
  }

  if (!sfnt_font_has_table(&font, 0x68656164u) ||
      !sfnt_font_has_table(&font, 0x6e616d65u) ||
      !sfnt_font_has_table(&font, 0x636d6170u) ||
      !sfnt_font_has_table(&font, 0x68686561u) ||
      !sfnt_font_has_table(&font, 0x686d7478u) ||
      !sfnt_font_has_table(&font, 0x6d617870u) ||
      !sfnt_font_has_table(&font, 0x676c7966u) ||
      !sfnt_font_has_table(&font, 0x6c6f6361u)) {
    sfnt_font_destroy(&font);
    test_fail_with_message("roundtrip TTF is missing required tables");
    return;
  }

  sfnt_font_destroy(&font);
}

static void test_decode_obfuscated_fixture_copy_writes_parseable_ttf(void) {
  char *decode_argv[] = {
    "fonttool", "decode", "build/out/wingdings3-obfuscated.fntdata",
    "build/out/wingdings3-obfuscated.ttf"
  };
  sfnt_font_t font;

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/wingdings3-obfuscated.fntdata");
  unlink("build/out/wingdings3-obfuscated.ttf");

  if (!write_obfuscated_fixture_copy("testdata/wingdings3.eot",
                                     "build/out/wingdings3-obfuscated.fntdata")) {
    return;
  }

  test_capture_command(4, decode_argv, 0,
                       "Decoded build/out/wingdings3-obfuscated.fntdata");

  if (access("build/out/wingdings3-obfuscated.ttf", F_OK) != 0) {
    test_fail_with_message("obfuscated fixture TTF file was not created");
    return;
  }

  if (sfnt_reader_load_file("build/out/wingdings3-obfuscated.ttf", &font) != EOT_OK) {
    test_fail_with_message("decoded obfuscated fixture TTF could not be parsed as SFNT");
    return;
  }

  sfnt_font_destroy(&font);
}

static void test_encode_fntdata_sets_obfuscation_flag_and_decodes(void) {
  char *encode_argv[] = {
    "fonttool", "encode", "testdata/OpenSans-Regular.ttf",
    "build/out/OpenSans-Regular.fntdata"
  };
  char *decode_argv[] = {
    "fonttool", "decode", "build/out/OpenSans-Regular.fntdata",
    "build/out/OpenSans-Regular.fntdata.roundtrip.ttf"
  };
  eot_header_t header;

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/OpenSans-Regular.fntdata");
  unlink("build/out/OpenSans-Regular.fntdata.roundtrip.ttf");

  test_capture_command(4, encode_argv, 0, "Encoded testdata/OpenSans-Regular.ttf");

  if (eot_header_read_file("build/out/OpenSans-Regular.fntdata", &header) != EOT_OK) {
    test_fail_with_message("encoded .fntdata file did not parse as EOT");
    return;
  }

  if ((header.flags & 0x10000000u) == 0u) {
    eot_header_destroy(&header);
    test_fail_with_message("encoded .fntdata file did not set PPT XOR flag");
    return;
  }
  eot_header_destroy(&header);

  test_capture_command(4, decode_argv, 0, "Decoded build/out/OpenSans-Regular.fntdata");

  if (access("build/out/OpenSans-Regular.fntdata.roundtrip.ttf", F_OK) != 0) {
    test_fail_with_message(".fntdata roundtrip TTF file was not created");
    return;
  }
}

static void test_encode_command_warns_when_dropping_vdmx(void) {
  char *argv[] = {
    "fonttool", "encode", "build/out/minimal-vdmx.ttf", "build/out/minimal-vdmx.eot"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/minimal-vdmx.ttf");
  unlink("build/out/minimal-vdmx.eot");

  if (!write_minimal_vdmx_ttf_fixture("build/out/minimal-vdmx.ttf")) {
    return;
  }

  test_capture_stderr_command(
      4, argv, 0,
      "warning: unsupported VDMX in MTX encode/subset path; dropping table");

  if (access("build/out/minimal-vdmx.eot", F_OK) != 0) {
    test_fail_with_message("encode command did not write EOT output");
    return;
  }
}

static void test_cli_encode_accepts_static_cff_otf(void) {
  char *argv[] = {
    "fonttool", "encode", "testdata/cff-static.otf", "build/out/cff-static.eot"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/cff-static.eot");

  test_capture_command(4, argv, 0, "Encoded testdata/cff-static.otf");
}

static void test_encode_cff_static_otf_succeeds_with_single_thread_override(void) {
  char *argv[] = {
    "fonttool", "encode", "testdata/cff-static.otf",
    "build/out/cff-static-single-thread.eot"
  };
  const char *original_threads = getenv("EOT_TOOL_THREADS");
  char *saved_threads = NULL;
  int had_original_threads = original_threads != NULL;

  if (had_original_threads) {
    saved_threads = strdup(original_threads);
    if (saved_threads == NULL) {
      test_fail_with_message("failed to snapshot EOT_TOOL_THREADS");
      return;
    }
  }

  if (setenv("EOT_TOOL_THREADS", "1", 1) != 0) {
    test_fail_with_message("failed to set EOT_TOOL_THREADS=1");
    goto cleanup;
  }

  if (!ensure_build_output_dir()) {
    goto cleanup;
  }
  unlink("build/out/cff-static-single-thread.eot");

  test_capture_command(4, argv, 0, "Encoded testdata/cff-static.otf");

cleanup:
  if (had_original_threads) {
    if (setenv("EOT_TOOL_THREADS", saved_threads, 1) != 0) {
      test_fail_with_message("failed to restore EOT_TOOL_THREADS");
    }
  } else {
    if (unsetenv("EOT_TOOL_THREADS") != 0) {
      test_fail_with_message("failed to clear EOT_TOOL_THREADS");
    }
  }
  free(saved_threads);
}

static void test_subset_text_eot_to_eot_succeeds(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/wingdings3.eot",
    "build/out/wingdings3-text-subset.eot", "--text", "ABC"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/wingdings3-text-subset.eot");

  test_capture_command(6, argv, 0, "Subset testdata/wingdings3.eot");
}

static void test_subset_keep_gids_fntdata_to_fntdata_succeeds(void) {
  char *encode_argv[] = {
    "fonttool", "encode", "testdata/OpenSans-Regular.ttf",
    "build/out/OpenSans-Regular.fntdata"
  };
  char *subset_argv[] = {
    "fonttool", "subset", "build/out/OpenSans-Regular.fntdata",
    "build/out/OpenSans-Regular.keep.fntdata", "--glyph-ids", "0,35", "--keep-gids"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/OpenSans-Regular.fntdata");
  unlink("build/out/OpenSans-Regular.keep.fntdata");

  test_capture_command(4, encode_argv, 0, "Encoded testdata/OpenSans-Regular.ttf");
  test_capture_command(7, subset_argv, 0, "Subset build/out/OpenSans-Regular.fntdata");
}

static void test_subset_accepts_static_cff_otf_input(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/cff-static.otf",
    "build/out/cff-static-subset.eot", "--text", "ABC"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/cff-static-subset.eot");

  test_capture_command(6, argv, 0, "Subset testdata/cff-static.otf");
}

static void test_subset_accepts_cff2_instance_with_variation_args(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/cff2-variable.otf",
    "build/out/cff2-instance-subset.eot", "--text", "ABC", "--variation",
    "wght=700"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/cff2-instance-subset.eot");

  test_capture_command(8, argv, 0, "Subset testdata/cff2-variable.otf");
}

static void test_subset_rejects_variation_args_for_non_variable_input(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/cff-static.otf",
    "build/out/should-not-exist.eot", "--text", "ABC", "--variation", "wght=700"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/should-not-exist.eot");

  test_capture_command(8, argv, 1, "error: failed to subset testdata/cff-static.otf");
}

static void test_subset_cli_emits_extra_table_warnings_once(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/wingdings3.eot",
    "build/out/wingdings3-subset.eot", "--glyph-ids", "0,1,2"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/wingdings3-subset.eot");

  test_capture_stderr_command(
      6, argv, 0,
      "warning: unsupported VDMX in MTX encode/subset path; dropping table");
  test_assert_stderr_occurrences(
      "warning: unsupported VDMX in MTX encode/subset path; dropping table", 1);
  test_assert_stderr_occurrences(
      "warning: unsupported HDMX in subset path; dropping table", 1);
}

static void test_subset_rejects_malformed_selection_flags(void) {
  char *argv[] = {
    "fonttool", "subset", "testdata/wingdings3.eot",
    "build/out/should-not-exist.eot"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/should-not-exist.eot");

  test_capture_command(4, argv, 1, "error: invalid subset arguments");

  if (access("build/out/should-not-exist.eot", F_OK) == 0) {
    test_fail_with_message("subset created output for malformed selection flags");
    return;
  }
}

static void test_decode_rejects_non_eot_input(void) {
  char *argv[] = {
    "fonttool", "decode", "testdata/OpenSans-Regular.ttf",
    "build/out/should-not-exist.ttf"
  };

  if (!ensure_build_output_dir()) {
    return;
  }
  unlink("build/out/should-not-exist.ttf");

  test_capture_command(4, argv, 1, "invalid EOT magic number");

  if (access("build/out/should-not-exist.ttf", F_OK) == 0) {
    test_fail_with_message("decode created output for non-EOT input");
    return;
  }
}

void register_cli_tests(void) {
  test_register("cli_help_prints_usage", test_cli_help_prints_usage);
  test_register("cli_without_help_reports_not_implemented",
                test_cli_without_help_reports_not_implemented);
  test_register("decode_command_writes_valid_ttf",
                test_decode_command_writes_valid_ttf);
  test_register("roundtrip_open_sans_writes_decodeable_ttf",
                test_roundtrip_open_sans_writes_decodeable_ttf);
  test_register("decode_obfuscated_fixture_copy_writes_parseable_ttf",
                test_decode_obfuscated_fixture_copy_writes_parseable_ttf);
  test_register("encode_fntdata_sets_obfuscation_flag_and_decodes",
                test_encode_fntdata_sets_obfuscation_flag_and_decodes);
  test_register("encode_command_warns_when_dropping_vdmx",
                test_encode_command_warns_when_dropping_vdmx);
  test_register("test_cli_encode_accepts_static_cff_otf",
                test_cli_encode_accepts_static_cff_otf);
  test_register("test_encode_cff_static_otf_succeeds_with_single_thread_override",
                test_encode_cff_static_otf_succeeds_with_single_thread_override);
  test_register("subset_text_eot_to_eot_succeeds",
                test_subset_text_eot_to_eot_succeeds);
  test_register("subset_keep_gids_fntdata_to_fntdata_succeeds",
                test_subset_keep_gids_fntdata_to_fntdata_succeeds);
  test_register("test_subset_accepts_static_cff_otf_input",
                test_subset_accepts_static_cff_otf_input);
  test_register("test_subset_accepts_cff2_instance_with_variation_args",
                test_subset_accepts_cff2_instance_with_variation_args);
  test_register("test_subset_rejects_variation_args_for_non_variable_input",
                test_subset_rejects_variation_args_for_non_variable_input);
  test_register("subset_cli_emits_extra_table_warnings_once",
                test_subset_cli_emits_extra_table_warnings_once);
  test_register("subset_rejects_malformed_selection_flags",
                test_subset_rejects_malformed_selection_flags);
  test_register("decode_rejects_non_eot_input",
                test_decode_rejects_non_eot_input);
}
