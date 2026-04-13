#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "eot_header.h"
#include "cff_reader.h"
#include "cff_variation.h"
#include "file_io.h"
#include "mtx_decode.h"
#include "mtx_encode.h"
#include "otf_convert.h"
#include "sfnt_font.h"
#include "sfnt_reader.h"
#include "sfnt_subset.h"
#include "sfnt_writer.h"
#include "status.h"
#include "subset_args.h"

static int path_has_suffix(const char *path, const char *suffix) {
  size_t path_length = strlen(path);
  size_t suffix_length = strlen(suffix);

  if (path_length < suffix_length) {
    return 0;
  }

  return strcmp(path + path_length - suffix_length, suffix) == 0;
}

static int handle_decode_command(const char *input_path, const char *output_path) {
  sfnt_font_t font;
  uint8_t *output = NULL;
  size_t output_size = 0;
  eot_status_t status = mtx_decode_eot_file(input_path, &font);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to decode %s: %s\n",
            input_path, eot_status_to_string(status));
    return 1;
  }

  status = sfnt_writer_serialize(&font, &output, &output_size);
  sfnt_font_destroy(&font);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to serialize reconstructed font from %s: %s\n",
            input_path, eot_status_to_string(status));
    return 1;
  }

  status = file_io_write_all(output_path, output, output_size);
  free(output);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to write output to %s: %s\n",
            output_path, eot_status_to_string(status));
    return 1;
  }

  printf("Decoded %s -> %s (%zu bytes)\n", input_path, output_path, output_size);
  return FONTTOOL_STATUS_OK;
}

static int handle_encode_command(const char *input_path, const char *output_path) {
  byte_buffer_t output;
  mtx_encode_warnings_t warnings;
  byte_buffer_init(&output);
  mtx_encode_warnings_init(&warnings);

  eot_status_t status;
  if (path_has_suffix(output_path, ".fntdata")) {
    status = mtx_encode_ttf_file_with_ppt_xor_and_warnings(input_path, &output, &warnings);
  } else {
    status = mtx_encode_ttf_file_with_warnings(input_path, &output, &warnings);
  }
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to encode %s: %s\n",
            input_path, eot_status_to_string(status));
    byte_buffer_destroy(&output);
    return 1;
  }

  size_t output_size = output.length;

  status = file_io_write_all(output_path, output.data, output.length);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to write output to %s: %s\n",
            output_path, eot_status_to_string(status));
    byte_buffer_destroy(&output);
    return 1;
  }

  byte_buffer_destroy(&output);
  if (warnings.dropped_vdmx) {
    fputs("warning: unsupported VDMX in MTX encode/subset path; dropping table\n",
          stderr);
  }
  printf("Encoded %s -> %s (%zu bytes)\n", input_path, output_path, output_size);
  return FONTTOOL_STATUS_OK;
}

static int path_is_subset_container(const char *path) {
  return path_has_suffix(path, ".eot") || path_has_suffix(path, ".fntdata");
}

static int path_is_sfnt_input(const char *path) {
  return path_has_suffix(path, ".ttf") || path_has_suffix(path, ".otf");
}

static eot_status_t load_sfnt_for_subset(const subset_request_t *request,
                                         sfnt_font_t *font) {
  file_buffer_t buffer = {0};
  sfnt_font_t parsed_font;
  cff_font_t cff_font;
  variation_location_t location;
  eot_status_t status;

  if (request == NULL || font == NULL || request->input_path == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  if (path_is_subset_container(request->input_path)) {
    return mtx_decode_eot_file(request->input_path, font);
  }

  if (!path_is_sfnt_input(request->input_path)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  status = file_io_read_all(request->input_path, &buffer);
  if (status != EOT_OK) {
    return status;
  }

  status = sfnt_reader_parse(buffer.data, buffer.length, &parsed_font);
  if (status != EOT_OK) {
    file_io_free(&buffer);
    return status;
  }

  if (!sfnt_font_has_table(&parsed_font, 0x43464620u) &&
      !sfnt_font_has_table(&parsed_font, 0x43464632u)) {
    if (request->variation_axis_map != NULL && request->variation_axis_map[0] != '\0') {
      sfnt_font_destroy(&parsed_font);
      file_io_free(&buffer);
      return EOT_ERR_INVALID_ARGUMENT;
    }
    file_io_free(&buffer);
    *font = parsed_font;
    return EOT_OK;
  }

  sfnt_font_destroy(&parsed_font);
  cff_font_init(&cff_font);
  variation_location_init(&location);

  if (request->variation_axis_map != NULL && request->variation_axis_map[0] != '\0') {
    status = cff_reader_load_file(request->input_path, &cff_font);
    if (status == EOT_OK) {
      status = variation_location_init_from_axis_map(&location, request->variation_axis_map);
    }
    if (status == EOT_OK) {
      status = cff_variation_resolve_location(&cff_font, &location);
    }
  }

  if (status == EOT_OK) {
    status = otf_convert_to_truetype_sfnt(
        buffer.data, buffer.length,
        (request->variation_axis_map != NULL && request->variation_axis_map[0] != '\0')
            ? &location
            : NULL,
        font);
  }

  variation_location_destroy(&location);
  cff_font_destroy(&cff_font);
  file_io_free(&buffer);
  return status;
}

static void emit_subset_encode_warnings(const subset_warnings_t *subset_warnings,
                                        const mtx_encode_warnings_t *encode_warnings) {
  int dropped_vdmx = 0;

  if (subset_warnings != NULL && subset_warnings->dropped_hdmx) {
    fputs("warning: unsupported HDMX in subset path; dropping table\n", stderr);
  }

  if (subset_warnings != NULL && subset_warnings->dropped_vdmx) {
    dropped_vdmx = 1;
  }
  if (encode_warnings != NULL && encode_warnings->dropped_vdmx) {
    dropped_vdmx = 1;
  }
  if (dropped_vdmx) {
    fputs("warning: unsupported VDMX in MTX encode/subset path; dropping table\n",
          stderr);
  }
}

static int handle_subset_command(int argc, char *argv[]) {
  subset_request_t request;
  subset_request_t effective_request;
  sfnt_font_t input;
  sfnt_font_t output;
  subset_warnings_t subset_warnings;
  mtx_encode_warnings_t encode_warnings;
  byte_buffer_t encoded_output;
  eot_status_t status;
  size_t output_size = 0;

  subset_request_init(&request);
  status = subset_args_parse(argc, (const char **)argv, &request);
  if (status != EOT_OK) {
    fprintf(stderr, "error: invalid subset arguments: %s\n",
            eot_status_to_string(status));
    subset_request_destroy(&request);
    return 1;
  }

  if (!path_is_subset_container(request.input_path) &&
      !path_is_sfnt_input(request.input_path)) {
    fprintf(stderr, "error: subset input must use .eot, .fntdata, .ttf, or .otf: %s\n",
            request.input_path);
    subset_request_destroy(&request);
    return 1;
  }

  if (!path_is_subset_container(request.output_path)) {
    fprintf(stderr, "error: subset output must use .eot or .fntdata: %s\n",
            request.output_path);
    subset_request_destroy(&request);
    return 1;
  }

  status = load_sfnt_for_subset(&request, &input);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to subset %s: %s\n",
            request.input_path, eot_status_to_string(status));
    subset_request_destroy(&request);
    return 1;
  }
  effective_request = request;
  effective_request.variation_axis_map = NULL;
  status = sfnt_subset_font_with_warnings(&input, &effective_request, &output,
                                          &subset_warnings);
  sfnt_font_destroy(&input);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to subset %s: %s\n",
            request.input_path, eot_status_to_string(status));
    subset_request_destroy(&request);
    return 1;
  }

  byte_buffer_init(&encoded_output);
  mtx_encode_warnings_init(&encode_warnings);
  if (path_has_suffix(request.output_path, ".fntdata")) {
    status = mtx_encode_font_with_ppt_xor_and_warnings(&output, &encoded_output,
                                                       &encode_warnings);
  } else {
    status = mtx_encode_font_with_warnings(&output, &encoded_output,
                                           &encode_warnings);
  }
  sfnt_font_destroy(&output);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to encode subset to %s: %s\n",
            request.output_path, eot_status_to_string(status));
    byte_buffer_destroy(&encoded_output);
    subset_request_destroy(&request);
    return 1;
  }

  output_size = encoded_output.length;
  status = file_io_write_all(request.output_path, encoded_output.data,
                             encoded_output.length);
  if (status != EOT_OK) {
    fprintf(stderr, "error: failed to write output to %s: %s\n",
            request.output_path, eot_status_to_string(status));
    byte_buffer_destroy(&encoded_output);
    subset_request_destroy(&request);
    return 1;
  }

  byte_buffer_destroy(&encoded_output);
  emit_subset_encode_warnings(&subset_warnings, &encode_warnings);
  printf("Subset %s -> %s (%zu bytes)\n",
         request.input_path, request.output_path, output_size);
  subset_request_destroy(&request);
  return FONTTOOL_STATUS_OK;
}

int fonttool_main(int argc, char *argv[]) {
  if (argc == 2 && strcmp(argv[1], "--help") == 0) {
    fputs("usage: fonttool <encode|decode> <input> <output>\n", stdout);
    fputs("usage: fonttool subset <input.eot|input.fntdata> <output.eot|output.fntdata> "
          "[--text TEXT|--unicodes LIST|--glyph-ids LIST] [--keep-gids]\n",
          stdout);
    return FONTTOOL_STATUS_OK;
  }

  if (argc == 4 && strcmp(argv[1], "decode") == 0) {
    return handle_decode_command(argv[2], argv[3]);
  }

  if (argc == 4 && strcmp(argv[1], "encode") == 0) {
    return handle_encode_command(argv[2], argv[3]);
  }

  if (argc >= 2 && strcmp(argv[1], "subset") == 0) {
    return handle_subset_command(argc, argv);
  }

  fputs("error: command not implemented yet\n", stderr);
  return FONTTOOL_STATUS_NOT_IMPLEMENTED;
}

#ifndef FONTTOOL_NO_MAIN
int main(int argc, char **argv) {
  return fonttool_main(argc, argv);
}
#endif
