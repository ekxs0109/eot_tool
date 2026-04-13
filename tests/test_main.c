#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef void (*test_fn)(void);
typedef void (*test_capture_fn)(void *context);

struct test_case {
  const char *name;
  test_fn fn;
};

static struct test_case tests[256];
static size_t test_count = 0;
static int has_failure = 0;
static char last_stderr_output[4096];

int fonttool_main(int argc, char *argv[]);
void register_cli_tests(void);
void register_lzcomp_tests(void);
void register_mtx_tests(void);
void register_decode_pipeline_tests(void);
void register_cvt_codec_tests(void);
void register_hdmx_codec_tests(void);
void register_glyf_codec_tests(void);
void register_sfnt_writer_tests(void);
void register_encode_pipeline_tests(void);
void register_table_policy_tests(void);
void register_subset_args_tests(void);
void register_sfnt_subset_tests(void);
void register_otf_convert_tests(void);
void register_otf_parity_tests(void);
void register_cu2qu_tests(void);
void register_cff_reader_tests(void);
void register_cff_variation_tests(void);
void register_ttf_rebuilder_tests(void);
void register_ttf_rebuilder_header_tests(void);
void register_wasm_api_tests(void);
void register_coretext_acceptance_tests(void);
void register_parallel_runtime_tests(void);

void test_register(const char *name, test_fn fn) {
  if (test_count >= sizeof(tests) / sizeof(tests[0])) {
    fprintf(stderr, "too many tests registered\n");
    exit(1);
  }
  tests[test_count].name = name;
  tests[test_count].fn = fn;
  test_count++;
}

void test_fail_with_message(const char *message) {
  fprintf(stderr, "%s\n", message);
  has_failure = 1;
}

static void read_capture_pipe(int fd, char *buffer, size_t buffer_size) {
  ssize_t read_total = 0;

  if (buffer_size == 0) {
    return;
  }

  while (read_total < (ssize_t)(buffer_size - 1)) {
    ssize_t n = read(fd, buffer + read_total, buffer_size - 1 - (size_t)read_total);
    if (n <= 0) {
      break;
    }
    read_total += n;
  }

  buffer[read_total] = '\0';
}

static int count_occurrences(const char *haystack, const char *needle) {
  int count = 0;
  size_t needle_length = strlen(needle);
  const char *cursor = haystack;

  if (needle_length == 0) {
    return 0;
  }

  while ((cursor = strstr(cursor, needle)) != NULL) {
    count++;
    cursor += needle_length;
  }

  return count;
}

void test_capture_stderr(test_capture_fn fn, void *context,
                         char *stderr_output, size_t stderr_output_size) {
  int capture_pipe[2];
  int stderr_saved = dup(STDERR_FILENO);

  if (stderr_saved < 0 || pipe(capture_pipe) != 0) {
    test_fail_with_message("test infrastructure failure: could not initialize stderr capture");
    if (stderr_output_size > 0) {
      stderr_output[0] = '\0';
    }
    return;
  }

  fflush(stderr);
  dup2(capture_pipe[1], STDERR_FILENO);
  close(capture_pipe[1]);

  fn(context);

  fflush(stderr);
  dup2(stderr_saved, STDERR_FILENO);
  close(stderr_saved);

  read_capture_pipe(capture_pipe[0], stderr_output, stderr_output_size);
  close(capture_pipe[0]);
}

void test_capture_command_streams(int argc, char *argv[], int *actual_status,
                                  char *stdout_output, size_t stdout_output_size,
                                  char *stderr_output, size_t stderr_output_size) {
  int stdout_pipe[2];
  int stderr_pipe[2];
  int stdout_saved = dup(STDOUT_FILENO);
  int stderr_saved = dup(STDERR_FILENO);

  if (stdout_saved < 0 || stderr_saved < 0 ||
      pipe(stdout_pipe) != 0 || pipe(stderr_pipe) != 0) {
    test_fail_with_message("test infrastructure failure: could not initialize stream capture");
    if (stdout_output_size > 0) {
      stdout_output[0] = '\0';
    }
    if (stderr_output_size > 0) {
      stderr_output[0] = '\0';
    }
    if (actual_status != NULL) {
      *actual_status = -1;
    }
    return;
  }

  fflush(stdout);
  fflush(stderr);

  dup2(stdout_pipe[1], STDOUT_FILENO);
  dup2(stderr_pipe[1], STDERR_FILENO);
  close(stdout_pipe[1]);
  close(stderr_pipe[1]);

  if (actual_status != NULL) {
    *actual_status = fonttool_main(argc, argv);
  } else {
    (void)fonttool_main(argc, argv);
  }

  fflush(stdout);
  fflush(stderr);

  dup2(stdout_saved, STDOUT_FILENO);
  dup2(stderr_saved, STDERR_FILENO);
  close(stdout_saved);
  close(stderr_saved);

  read_capture_pipe(stdout_pipe[0], stdout_output, stdout_output_size);
  read_capture_pipe(stderr_pipe[0], stderr_output, stderr_output_size);
  close(stdout_pipe[0]);
  close(stderr_pipe[0]);

  snprintf(last_stderr_output, sizeof(last_stderr_output), "%s", stderr_output);
}

void test_capture_command(int argc, char *argv[], int expected_status,
                          const char *expected_fragment) {
  char stdout_output[2048];
  char stderr_output[2048];
  char combined_output[4096];
  int status = 0;

  test_capture_command_streams(argc, argv, &status,
                               stdout_output, sizeof(stdout_output),
                               stderr_output, sizeof(stderr_output));

  snprintf(combined_output, sizeof(combined_output), "%s%s",
           stdout_output, stderr_output);

  if (status != expected_status) {
    fprintf(stderr, "expected exit status: %d\nactual exit status: %d\n",
            expected_status, status);
    has_failure = 1;
  }

  if (strstr(combined_output, expected_fragment) == NULL) {
    fprintf(stderr, "expected output fragment: %s\nactual output: %s\n",
            expected_fragment, combined_output);
    has_failure = 1;
  }
}

void test_capture_stderr_command(int argc, char *argv[], int expected_status,
                                 const char *expected_fragment) {
  char stdout_output[2048];
  char stderr_output[2048];
  int status = 0;

  test_capture_command_streams(argc, argv, &status,
                               stdout_output, sizeof(stdout_output),
                               stderr_output, sizeof(stderr_output));

  if (status != expected_status) {
    fprintf(stderr, "expected exit status: %d\nactual exit status: %d\n",
            expected_status, status);
    has_failure = 1;
  }

  if (count_occurrences(stderr_output, expected_fragment) != 1) {
    fprintf(stderr, "expected stderr fragment once: %s\nactual stderr: %s\n",
            expected_fragment, stderr_output);
    has_failure = 1;
  }
}

void test_assert_stderr_occurrences(const char *expected_fragment,
                                    int expected_count) {
  int actual_count = count_occurrences(last_stderr_output, expected_fragment);

  if (actual_count != expected_count) {
    fprintf(stderr,
            "expected stderr fragment occurrences: %d\nactual occurrences: %d\nfragment: %s\nactual stderr: %s\n",
            expected_count, actual_count, expected_fragment, last_stderr_output);
    has_failure = 1;
  }
}

int main(void) {
  const char *test_filter = getenv("TESTCASE");
  size_t executed = 0;
  size_t i;

  register_cli_tests();
  register_lzcomp_tests();
  register_mtx_tests();
  register_decode_pipeline_tests();
  register_cvt_codec_tests();
  register_hdmx_codec_tests();
  register_glyf_codec_tests();
  register_sfnt_writer_tests();
  register_encode_pipeline_tests();
  register_table_policy_tests();
  register_subset_args_tests();
  register_sfnt_subset_tests();
  register_otf_convert_tests();
  register_otf_parity_tests();
  register_cu2qu_tests();
  register_cff_reader_tests();
  register_cff_variation_tests();
  register_ttf_rebuilder_tests();
  register_ttf_rebuilder_header_tests();
  register_wasm_api_tests();
  register_coretext_acceptance_tests();
  register_parallel_runtime_tests();

  for (i = 0; i < test_count; i++) {
    if (test_filter != NULL && test_filter[0] != '\0' &&
        strcmp(test_filter, tests[i].name) != 0) {
      continue;
    }

    tests[i].fn();
    executed++;
  }

  if (executed == 0) {
    fprintf(stderr, "no tests executed\n");
    return 1;
  }

  if (has_failure) {
    return 1;
  }

  return 0;
}
