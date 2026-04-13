#include "subset_args.h"

#include <stdlib.h>
#include <string.h>

static char *duplicate_string(const char *value) {
  size_t length;
  char *copy;

  if (value == NULL) {
    return NULL;
  }

  length = strlen(value);
  copy = (char *)malloc(length + 1);
  if (copy == NULL) {
    return NULL;
  }

  memcpy(copy, value, length + 1);
  return copy;
}

void subset_request_init(subset_request_t *request) {
  if (request == NULL) {
    return;
  }

  request->input_path = NULL;
  request->output_path = NULL;
  request->selection_data = NULL;
  request->variation_axis_map = NULL;
  request->selection_mode = SUBSET_SELECTION_NONE;
  request->keep_gids = 0;
}

void subset_request_destroy(subset_request_t *request) {
  if (request == NULL) {
    return;
  }

  free(request->input_path);
  free(request->output_path);
  free(request->selection_data);
  free(request->variation_axis_map);
  subset_request_init(request);
}

static eot_status_t subset_request_set_selection(subset_request_t *request,
                                                 subset_selection_mode_t mode,
                                                 const char *selection_data) {
  char *copy;

  if (request == NULL || selection_data == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  copy = duplicate_string(selection_data);
  if (copy == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  free(request->selection_data);
  request->selection_data = copy;
  request->selection_mode = mode;
  return EOT_OK;
}

eot_status_t subset_request_init_for_text(subset_request_t *request, const char *text) {
  if (request == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  subset_request_init(request);
  return subset_request_set_selection(request, SUBSET_SELECTION_TEXT, text);
}

eot_status_t subset_request_init_for_glyph_ids(subset_request_t *request, const char *csv) {
  if (request == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  subset_request_init(request);
  return subset_request_set_selection(request, SUBSET_SELECTION_GLYPH_IDS, csv);
}

eot_status_t subset_request_init_for_glyph_ids_keep(subset_request_t *request, const char *csv) {
  eot_status_t status = subset_request_init_for_glyph_ids(request, csv);
  if (status != EOT_OK) {
    return status;
  }

  request->keep_gids = 1;
  return EOT_OK;
}

static eot_status_t subset_request_set_path(char **field, const char *path) {
  char *copy;

  if (field == NULL || path == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  copy = duplicate_string(path);
  if (copy == NULL) {
    return EOT_ERR_ALLOCATION;
  }

  free(*field);
  *field = copy;
  return EOT_OK;
}

static eot_status_t subset_request_set_variation_axis_map(subset_request_t *request,
                                                          const char *axis_map) {
  if (request == NULL || axis_map == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  return subset_request_set_path(&request->variation_axis_map, axis_map);
}

eot_status_t subset_args_parse(int argc, const char *argv[],
                               subset_request_t *out_request) {
  int i;
  eot_status_t status;

  if (argc < 6 || argv == NULL || out_request == NULL) {
    return EOT_ERR_INVALID_ARGUMENT;
  }
  if (strcmp(argv[1], "subset") != 0) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  subset_request_init(out_request);

  status = subset_request_set_path(&out_request->input_path, argv[2]);
  if (status != EOT_OK) {
    goto fail;
  }

  status = subset_request_set_path(&out_request->output_path, argv[3]);
  if (status != EOT_OK) {
    goto fail;
  }

  for (i = 4; i < argc; i++) {
    if (strcmp(argv[i], "--keep-gids") == 0) {
      out_request->keep_gids = 1;
      continue;
    }

    if (i + 1 >= argc) {
      status = EOT_ERR_INVALID_ARGUMENT;
      goto fail;
    }

    if (strcmp(argv[i], "--variation") == 0) {
      status = subset_request_set_variation_axis_map(out_request, argv[i + 1]);
      if (status != EOT_OK) {
        goto fail;
      }
      i++;
      continue;
    }

    if (out_request->selection_mode != SUBSET_SELECTION_NONE) {
      status = EOT_ERR_INVALID_ARGUMENT;
      goto fail;
    }

    if (strcmp(argv[i], "--text") == 0) {
      status = subset_request_set_selection(out_request, SUBSET_SELECTION_TEXT,
                                            argv[i + 1]);
    } else if (strcmp(argv[i], "--unicodes") == 0) {
      status = subset_request_set_selection(out_request, SUBSET_SELECTION_UNICODES,
                                            argv[i + 1]);
    } else if (strcmp(argv[i], "--glyph-ids") == 0) {
      status = subset_request_set_selection(out_request, SUBSET_SELECTION_GLYPH_IDS,
                                            argv[i + 1]);
    } else {
      status = EOT_ERR_INVALID_ARGUMENT;
    }

    if (status != EOT_OK) {
      goto fail;
    }

    i++;
  }

  if (out_request->selection_mode == SUBSET_SELECTION_NONE) {
    status = EOT_ERR_INVALID_ARGUMENT;
    goto fail;
  }

  return EOT_OK;

fail:
  subset_request_destroy(out_request);
  return status;
}
