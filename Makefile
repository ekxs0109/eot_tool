CC ?= cc
CFLAGS ?= -std=c11 -Wall -Wextra -Werror -g
CXX ?= c++
CXXFLAGS ?= -std=c++17 -Wall -Wextra -Werror -g
LDFLAGS ?=
PKG_CONFIG ?= pkg-config
HB_PACKAGES := harfbuzz-subset harfbuzz
HB_CFLAGS := $(shell $(PKG_CONFIG) --cflags $(HB_PACKAGES) 2>/dev/null)
HB_LIBS := $(shell $(PKG_CONFIG) --libs $(HB_PACKAGES) 2>/dev/null)
UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
TEST_CORETEXT_LIBS := -framework CoreText -framework CoreFoundation
THREAD_FLAGS :=
else
THREAD_FLAGS := -pthread
endif

ROOT_DIR := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
WORKSPACE_ROOT := $(abspath $(ROOT_DIR)/../..)
BUILD_DIR := $(ROOT_DIR)/build
OBJ_DIR := $(BUILD_DIR)/obj
BIN_DIR := $(BUILD_DIR)/bin
VENV_DIR := $(BUILD_DIR)/venv
VENV_PYTHON := $(VENV_DIR)/bin/python

CFLAGS += -I$(ROOT_DIR)/src
CXXFLAGS += -I$(ROOT_DIR)/src $(HB_CFLAGS)
CXXFLAGS += $(THREAD_FLAGS)
LDFLAGS += $(THREAD_FLAGS)

FONTTOOL_BIN := $(BUILD_DIR)/fonttool
TEST_BIN := $(BIN_DIR)/test_runner

BYTE_IO_OBJ := $(OBJ_DIR)/byte_io.o
FILE_IO_OBJ := $(OBJ_DIR)/file_io.o
EOT_HEADER_OBJ := $(OBJ_DIR)/eot_header.o
MTX_CONTAINER_OBJ := $(OBJ_DIR)/mtx_container.o
LZCOMP_OBJ := $(OBJ_DIR)/lzcomp.o
SFNT_FONT_OBJ := $(OBJ_DIR)/sfnt_font.o
SFNT_WRITER_OBJ := $(OBJ_DIR)/sfnt_writer.o
CVT_CODEC_OBJ := $(OBJ_DIR)/cvt_codec.o
HDMX_CODEC_OBJ := $(OBJ_DIR)/hdmx_codec.o
GLYF_CODEC_OBJ := $(OBJ_DIR)/glyf_codec.o
MTX_DECODE_OBJ := $(OBJ_DIR)/mtx_decode.o
SFNT_READER_OBJ := $(OBJ_DIR)/sfnt_reader.o
MTX_ENCODE_OBJ := $(OBJ_DIR)/mtx_encode.o
TABLE_POLICY_OBJ := $(OBJ_DIR)/table_policy.o
SUBSET_ARGS_OBJ := $(OBJ_DIR)/subset_args.o
SFNT_SUBSET_OBJ := $(OBJ_DIR)/sfnt_subset.o
SUBSET_BACKEND_HARFBUZZ_OBJ := $(OBJ_DIR)/subset_backend_harfbuzz.o
CU2QU_OBJ := $(OBJ_DIR)/cu2qu.o
CFF_READER_OBJ := $(OBJ_DIR)/cff_reader.o
TT_REBUILDER_OBJ := $(OBJ_DIR)/tt_rebuilder.o
OTF_CONVERT_OBJ := $(OBJ_DIR)/otf_convert.o
OTF_CONVERT_TEST_OBJ := $(OBJ_DIR)/otf_convert_test.o
CFF_VARIATION_OBJ := $(OBJ_DIR)/cff_variation.o
WASM_API_OBJ := $(OBJ_DIR)/wasm_api.o
PARALLEL_RUNTIME_OBJ := $(OBJ_DIR)/parallel_runtime.o
COMMON_OBJ := $(BYTE_IO_OBJ) $(FILE_IO_OBJ) $(EOT_HEADER_OBJ) $(MTX_CONTAINER_OBJ) $(LZCOMP_OBJ) \
              $(SFNT_FONT_OBJ) $(SFNT_WRITER_OBJ) $(CVT_CODEC_OBJ) $(HDMX_CODEC_OBJ) \
              $(GLYF_CODEC_OBJ) $(MTX_DECODE_OBJ) $(SFNT_READER_OBJ) $(MTX_ENCODE_OBJ) \
              $(CU2QU_OBJ) $(CFF_READER_OBJ) $(CFF_VARIATION_OBJ) $(TT_REBUILDER_OBJ) $(OTF_CONVERT_OBJ) \
              $(WASM_API_OBJ) $(PARALLEL_RUNTIME_OBJ) \
              $(TABLE_POLICY_OBJ)
TEST_COMMON_OBJ := $(BYTE_IO_OBJ) $(FILE_IO_OBJ) $(EOT_HEADER_OBJ) $(MTX_CONTAINER_OBJ) $(LZCOMP_OBJ) \
                   $(SFNT_FONT_OBJ) $(SFNT_WRITER_OBJ) $(CVT_CODEC_OBJ) $(HDMX_CODEC_OBJ) \
                   $(GLYF_CODEC_OBJ) $(MTX_DECODE_OBJ) $(SFNT_READER_OBJ) $(MTX_ENCODE_OBJ) \
                   $(CU2QU_OBJ) $(CFF_READER_OBJ) $(CFF_VARIATION_OBJ) $(TT_REBUILDER_OBJ) $(OTF_CONVERT_TEST_OBJ) \
                   $(WASM_API_OBJ) $(PARALLEL_RUNTIME_OBJ) \
                   $(TABLE_POLICY_OBJ)
SUBSET_TEST_OBJ := $(SUBSET_ARGS_OBJ) $(SFNT_SUBSET_OBJ) $(SUBSET_BACKEND_HARFBUZZ_OBJ)

FONTTOOL_OBJ := $(OBJ_DIR)/main.o
TEST_MAIN_OBJ := $(OBJ_DIR)/test_main.o
TEST_CLI_OBJ := $(OBJ_DIR)/test_cli.o
TEST_EOT_HEADER_OBJ := $(OBJ_DIR)/test_eot_header.o
TEST_MTX_CONTAINER_OBJ := $(OBJ_DIR)/test_mtx_container.o
TEST_LZCOMP_OBJ := $(OBJ_DIR)/test_lzcomp.o
TEST_DECODE_PIPELINE_OBJ := $(OBJ_DIR)/test_decode_pipeline.o
TEST_CVT_CODEC_OBJ := $(OBJ_DIR)/test_cvt_codec.o
TEST_HDMX_CODEC_OBJ := $(OBJ_DIR)/test_hdmx_codec.o
TEST_GLYF_CODEC_OBJ := $(OBJ_DIR)/test_glyf_codec.o
TEST_SFNT_WRITER_OBJ := $(OBJ_DIR)/test_sfnt_writer.o
TEST_ENCODE_PIPELINE_OBJ := $(OBJ_DIR)/test_encode_pipeline.o
TEST_TABLE_POLICY_OBJ := $(OBJ_DIR)/test_table_policy.o
TEST_SUBSET_ARGS_OBJ := $(OBJ_DIR)/test_subset_args.o
TEST_SFNT_SUBSET_OBJ := $(OBJ_DIR)/test_sfnt_subset.o
TEST_OTF_CONVERT_OBJ := $(OBJ_DIR)/test_otf_convert.o
TEST_OTF_PARITY_OBJ := $(OBJ_DIR)/test_otf_parity.o
TEST_CU2QU_OBJ := $(OBJ_DIR)/test_cu2qu.o
TEST_CFF_READER_OBJ := $(OBJ_DIR)/test_cff_reader.o
TEST_CFF_VARIATION_OBJ := $(OBJ_DIR)/test_cff_variation.o
TEST_TTF_REBUILDER_OBJ := $(OBJ_DIR)/test_ttf_rebuilder.o
TEST_TTF_REBUILDER_HEADER_OBJ := $(OBJ_DIR)/test_ttf_rebuilder_header.o
TEST_WASM_API_OBJ := $(OBJ_DIR)/test_wasm_api.o
TEST_CORETEXT_ACCEPTANCE_OBJ := $(OBJ_DIR)/test_coretext_acceptance.o
TEST_PARALLEL_RUNTIME_OBJ := $(OBJ_DIR)/test_parallel_runtime.o
TEST_MAIN_SRC_OBJ := $(OBJ_DIR)/main_test.o

FIXTURE_SOURCE ?= $(WORKSPACE_ROOT)/font2.fntdata
FIXTURE_DEST := $(ROOT_DIR)/testdata/wingdings3.eot

.PHONY: all clean test fixtures python-venv verify-decode verify-roundtrip check-harfbuzz wasm wasm-single wasm-pthreads verify-wasm-artifacts

all: $(FONTTOOL_BIN)

check-harfbuzz:
	@$(PKG_CONFIG) --exists $(HB_PACKAGES) || \
		{ echo "error: HarfBuzz subset development packages not found. Install $(HB_PACKAGES) to build subset-enabled test targets." >&2; exit 1; }

$(BUILD_DIR) $(OBJ_DIR) $(BIN_DIR):
	mkdir -p $@

$(BYTE_IO_OBJ): $(ROOT_DIR)/src/byte_io.c $(ROOT_DIR)/src/byte_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(FILE_IO_OBJ): $(ROOT_DIR)/src/file_io.c $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(EOT_HEADER_OBJ): $(ROOT_DIR)/src/eot_header.c $(ROOT_DIR)/src/eot_header.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(MTX_CONTAINER_OBJ): $(ROOT_DIR)/src/mtx_container.c $(ROOT_DIR)/src/mtx_container.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(LZCOMP_OBJ): $(ROOT_DIR)/src/lzcomp.c $(ROOT_DIR)/src/lzcomp.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SFNT_FONT_OBJ): $(ROOT_DIR)/src/sfnt_font.c $(ROOT_DIR)/src/sfnt_font.h \
	$(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SFNT_WRITER_OBJ): $(ROOT_DIR)/src/sfnt_writer.c $(ROOT_DIR)/src/sfnt_writer.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(CVT_CODEC_OBJ): $(ROOT_DIR)/src/cvt_codec.c $(ROOT_DIR)/src/cvt_codec.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(HDMX_CODEC_OBJ): $(ROOT_DIR)/src/hdmx_codec.c $(ROOT_DIR)/src/hdmx_codec.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(GLYF_CODEC_OBJ): $(ROOT_DIR)/src/glyf_codec.c $(ROOT_DIR)/src/glyf_codec.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(MTX_DECODE_OBJ): $(ROOT_DIR)/src/mtx_decode.c $(ROOT_DIR)/src/mtx_decode.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/eot_header.h \
	$(ROOT_DIR)/src/mtx_container.h $(ROOT_DIR)/src/lzcomp.h \
	$(ROOT_DIR)/src/cvt_codec.h $(ROOT_DIR)/src/hdmx_codec.h \
	$(ROOT_DIR)/src/glyf_codec.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SFNT_READER_OBJ): $(ROOT_DIR)/src/sfnt_reader.c $(ROOT_DIR)/src/sfnt_reader.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/byte_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(MTX_ENCODE_OBJ): $(ROOT_DIR)/src/mtx_encode.c $(ROOT_DIR)/src/mtx_encode.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/sfnt_reader.h \
	$(ROOT_DIR)/src/file_io.h $(ROOT_DIR)/src/otf_convert.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TABLE_POLICY_OBJ): $(ROOT_DIR)/src/table_policy.c $(ROOT_DIR)/src/table_policy.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SUBSET_ARGS_OBJ): $(ROOT_DIR)/src/subset_args.c $(ROOT_DIR)/src/subset_args.h \
	$(ROOT_DIR)/src/sfnt_subset.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SFNT_SUBSET_OBJ): $(ROOT_DIR)/src/sfnt_subset.c $(ROOT_DIR)/src/sfnt_subset.h \
	$(ROOT_DIR)/src/subset_backend_harfbuzz.h $(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/byte_io.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(SUBSET_BACKEND_HARFBUZZ_OBJ): $(ROOT_DIR)/src/subset_backend_harfbuzz.cc \
	$(ROOT_DIR)/src/subset_backend_harfbuzz.h $(ROOT_DIR)/src/sfnt_subset.h \
	$(ROOT_DIR)/src/sfnt_reader.h $(ROOT_DIR)/src/sfnt_writer.h | $(OBJ_DIR) check-harfbuzz
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(CU2QU_OBJ): $(ROOT_DIR)/src/cu2qu.cc $(ROOT_DIR)/src/cu2qu.h \
	$(ROOT_DIR)/src/cff_types.h $(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(CFF_READER_OBJ): $(ROOT_DIR)/src/cff_reader.cc $(ROOT_DIR)/src/cff_reader.h \
	$(ROOT_DIR)/src/cff_types.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/sfnt_reader.h \
	$(ROOT_DIR)/src/byte_io.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(CFF_VARIATION_OBJ): $(ROOT_DIR)/src/cff_variation.cc $(ROOT_DIR)/src/cff_variation.h \
	$(ROOT_DIR)/src/cff_reader.h $(ROOT_DIR)/src/cff_types.h \
	$(ROOT_DIR)/src/file_io.h $(ROOT_DIR)/src/sfnt_font.h \
	$(ROOT_DIR)/src/sfnt_writer.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TT_REBUILDER_OBJ): $(ROOT_DIR)/src/tt_rebuilder.cc $(ROOT_DIR)/src/tt_rebuilder.h \
	$(ROOT_DIR)/src/cff_types.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/byte_io.h \
	$(ROOT_DIR)/src/glyf_codec.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(OTF_CONVERT_OBJ): $(ROOT_DIR)/src/otf_convert.cc $(ROOT_DIR)/src/otf_convert.h \
	$(ROOT_DIR)/src/cff_types.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/sfnt_reader.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/cu2qu.h \
	$(ROOT_DIR)/src/tt_rebuilder.h $(ROOT_DIR)/src/parallel_runtime.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(OTF_CONVERT_TEST_OBJ): $(ROOT_DIR)/src/otf_convert.cc $(ROOT_DIR)/src/otf_convert.h \
	$(ROOT_DIR)/src/cff_types.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/sfnt_font.h $(ROOT_DIR)/src/sfnt_reader.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/cu2qu.h \
	$(ROOT_DIR)/src/tt_rebuilder.h $(ROOT_DIR)/src/parallel_runtime.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -DFONTTOOL_TESTING -c $< -o $@

$(WASM_API_OBJ): $(ROOT_DIR)/src/wasm_api.cc $(ROOT_DIR)/src/wasm_api.h \
	$(ROOT_DIR)/src/otf_convert.h $(ROOT_DIR)/src/cff_variation.h \
	$(ROOT_DIR)/src/cff_reader.h $(ROOT_DIR)/src/mtx_encode.h \
	$(ROOT_DIR)/src/sfnt_reader.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(PARALLEL_RUNTIME_OBJ): $(ROOT_DIR)/src/parallel_runtime.cc $(ROOT_DIR)/src/parallel_runtime.h \
	$(ROOT_DIR)/src/file_io.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(FONTTOOL_OBJ): $(ROOT_DIR)/src/main.c $(ROOT_DIR)/src/status.h \
	$(ROOT_DIR)/src/eot_header.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/subset_args.h \
	$(ROOT_DIR)/src/sfnt_subset.h $(ROOT_DIR)/src/mtx_decode.h \
	$(ROOT_DIR)/src/mtx_encode.h $(ROOT_DIR)/src/cff_reader.h \
	$(ROOT_DIR)/src/cff_variation.h $(ROOT_DIR)/src/otf_convert.h \
	$(ROOT_DIR)/src/sfnt_reader.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_MAIN_SRC_OBJ): $(ROOT_DIR)/src/main.c $(ROOT_DIR)/src/status.h \
	$(ROOT_DIR)/src/eot_header.h $(ROOT_DIR)/src/file_io.h \
	$(ROOT_DIR)/src/byte_io.h $(ROOT_DIR)/src/subset_args.h \
	$(ROOT_DIR)/src/sfnt_subset.h $(ROOT_DIR)/src/mtx_decode.h \
	$(ROOT_DIR)/src/mtx_encode.h $(ROOT_DIR)/src/cff_reader.h \
	$(ROOT_DIR)/src/cff_variation.h $(ROOT_DIR)/src/otf_convert.h \
	$(ROOT_DIR)/src/sfnt_reader.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -DFONTTOOL_NO_MAIN -c $< -o $@

$(TEST_MAIN_OBJ): $(ROOT_DIR)/tests/test_main.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_CLI_OBJ): $(ROOT_DIR)/tests/test_cli.c $(ROOT_DIR)/src/sfnt_subset.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -Dregister_cli_tests=register_cli_tests_original -c $< -o $@

$(TEST_EOT_HEADER_OBJ): $(ROOT_DIR)/tests/test_eot_header.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_MTX_CONTAINER_OBJ): $(ROOT_DIR)/tests/test_mtx_container.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_LZCOMP_OBJ): $(ROOT_DIR)/tests/test_lzcomp.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_DECODE_PIPELINE_OBJ): $(ROOT_DIR)/tests/test_decode_pipeline.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_CVT_CODEC_OBJ): $(ROOT_DIR)/tests/test_cvt_codec.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_HDMX_CODEC_OBJ): $(ROOT_DIR)/tests/test_hdmx_codec.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_GLYF_CODEC_OBJ): $(ROOT_DIR)/tests/test_glyf_codec.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_SFNT_WRITER_OBJ): $(ROOT_DIR)/tests/test_sfnt_writer.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_ENCODE_PIPELINE_OBJ): $(ROOT_DIR)/tests/test_encode_pipeline.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_TABLE_POLICY_OBJ): $(ROOT_DIR)/tests/test_table_policy.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_SUBSET_ARGS_OBJ): $(ROOT_DIR)/tests/test_subset_args.c $(ROOT_DIR)/src/subset_args.h \
	$(ROOT_DIR)/src/sfnt_subset.h | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_SFNT_SUBSET_OBJ): $(ROOT_DIR)/tests/test_sfnt_subset.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_OTF_CONVERT_OBJ): $(ROOT_DIR)/tests/test_otf_convert.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_OTF_PARITY_OBJ): $(ROOT_DIR)/tests/test_otf_parity.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_CU2QU_OBJ): $(ROOT_DIR)/tests/test_cu2qu.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_CFF_READER_OBJ): $(ROOT_DIR)/tests/test_cff_reader.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_CFF_VARIATION_OBJ): $(ROOT_DIR)/tests/test_cff_variation.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_TTF_REBUILDER_OBJ): $(ROOT_DIR)/tests/test_ttf_rebuilder.cc | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_TTF_REBUILDER_HEADER_OBJ): $(ROOT_DIR)/tests/test_ttf_rebuilder_header.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_WASM_API_OBJ): $(ROOT_DIR)/tests/test_wasm_api.cc $(ROOT_DIR)/src/wasm_api.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(TEST_CORETEXT_ACCEPTANCE_OBJ): $(ROOT_DIR)/tests/test_coretext_acceptance.c | $(OBJ_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TEST_PARALLEL_RUNTIME_OBJ): $(ROOT_DIR)/tests/test_parallel_runtime.cc $(ROOT_DIR)/src/parallel_runtime.h | $(OBJ_DIR)
	$(CXX) $(CXXFLAGS) -c $< -o $@

$(FONTTOOL_BIN): $(FONTTOOL_OBJ) $(COMMON_OBJ) $(SUBSET_TEST_OBJ) | $(BIN_DIR)
	$(CXX) $(CXXFLAGS) $(LDFLAGS) $^ $(HB_LIBS) -o $@

$(TEST_BIN): $(TEST_MAIN_OBJ) $(TEST_CLI_OBJ) $(TEST_EOT_HEADER_OBJ) $(TEST_LZCOMP_OBJ) $(TEST_MTX_CONTAINER_OBJ) $(TEST_DECODE_PIPELINE_OBJ) $(TEST_CVT_CODEC_OBJ) $(TEST_HDMX_CODEC_OBJ) $(TEST_GLYF_CODEC_OBJ) $(TEST_SFNT_WRITER_OBJ) $(TEST_ENCODE_PIPELINE_OBJ) $(TEST_TABLE_POLICY_OBJ) $(TEST_SUBSET_ARGS_OBJ) $(TEST_SFNT_SUBSET_OBJ) $(TEST_OTF_CONVERT_OBJ) $(TEST_OTF_PARITY_OBJ) $(TEST_CU2QU_OBJ) $(TEST_CFF_READER_OBJ) $(TEST_CFF_VARIATION_OBJ) $(TEST_TTF_REBUILDER_OBJ) $(TEST_TTF_REBUILDER_HEADER_OBJ) $(TEST_WASM_API_OBJ) $(TEST_CORETEXT_ACCEPTANCE_OBJ) $(TEST_PARALLEL_RUNTIME_OBJ) $(TEST_MAIN_SRC_OBJ) $(TEST_COMMON_OBJ) $(SUBSET_TEST_OBJ) | $(BIN_DIR) check-harfbuzz
	$(CXX) $(CXXFLAGS) $(LDFLAGS) $^ $(HB_LIBS) $(TEST_CORETEXT_LIBS) -o $@

WASM_CXX ?= em++
WASM_CXXFLAGS ?= -O3 -std=c++17
WASM_BASE_FLAGS := $(WASM_CXXFLAGS) $(HB_CFLAGS) -I$(ROOT_DIR)/src \
	-s MODULARIZE=1 -s EXPORT_ES6=1 -s ALLOW_MEMORY_GROWTH=1 \
	-s EXPORTED_FUNCTIONS='["_wasm_convert_otf_to_embedded_font","_wasm_buffer_destroy","_wasm_runtime_thread_mode"]' \
	-s EXPORTED_RUNTIME_METHODS='["cwrap","HEAPU8"]'
WASM_SOURCES := $(ROOT_DIR)/src/wasm_api.cc $(ROOT_DIR)/src/otf_convert.cc $(ROOT_DIR)/src/cff_reader.cc \
	$(ROOT_DIR)/src/cff_variation.cc $(ROOT_DIR)/src/cu2qu.cc $(ROOT_DIR)/src/tt_rebuilder.cc \
	$(ROOT_DIR)/src/parallel_runtime.cc $(ROOT_DIR)/src/subset_backend_harfbuzz.cc \
	$(ROOT_DIR)/src/byte_io.c $(ROOT_DIR)/src/file_io.c $(ROOT_DIR)/src/eot_header.c \
	$(ROOT_DIR)/src/mtx_container.c $(ROOT_DIR)/src/lzcomp.c $(ROOT_DIR)/src/sfnt_font.c \
	$(ROOT_DIR)/src/sfnt_writer.c $(ROOT_DIR)/src/cvt_codec.c $(ROOT_DIR)/src/hdmx_codec.c \
	$(ROOT_DIR)/src/glyf_codec.c $(ROOT_DIR)/src/mtx_decode.c $(ROOT_DIR)/src/sfnt_reader.c \
	$(ROOT_DIR)/src/mtx_encode.c $(ROOT_DIR)/src/table_policy.c $(ROOT_DIR)/src/subset_args.c \
	$(ROOT_DIR)/src/sfnt_subset.c

wasm: wasm-single wasm-pthreads

wasm-single: check-harfbuzz | $(BUILD_DIR)
	$(WASM_CXX) $(WASM_BASE_FLAGS) $(WASM_SOURCES) $(HB_LIBS) \
		-o $(BUILD_DIR)/fonttool-wasm.js

wasm-pthreads: check-harfbuzz | $(BUILD_DIR)
	$(WASM_CXX) $(WASM_BASE_FLAGS) -pthread -s USE_PTHREADS=1 $(WASM_SOURCES) $(HB_LIBS) \
		-o $(BUILD_DIR)/fonttool-wasm-pthreads.js

verify-wasm-artifacts: wasm
	bash $(ROOT_DIR)/tests/verify_wasm_artifacts.sh

test: check-harfbuzz $(TEST_BIN) $(FONTTOOL_BIN)
	TESTCASE="$(TESTCASE)" $(TEST_BIN)

fixtures:
	mkdir -p $(ROOT_DIR)/testdata
	cp -f $(FIXTURE_SOURCE) $(FIXTURE_DEST)
	chmod 0644 $(FIXTURE_DEST)

clean:
	rm -rf $(BUILD_DIR)

python-venv:
	@if [ ! -x "$(VENV_PYTHON)" ]; then python3 -m venv $(VENV_DIR); fi
	$(VENV_PYTHON) -m pip install -r $(ROOT_DIR)/tests/requirements.txt

verify-decode: $(FONTTOOL_BIN) python-venv
	mkdir -p $(BUILD_DIR)/out
	$(FONTTOOL_BIN) decode $(ROOT_DIR)/testdata/wingdings3.eot $(BUILD_DIR)/out/wingdings3.ttf
	$(VENV_PYTHON) $(ROOT_DIR)/tests/verify_font.py $(BUILD_DIR)/out/wingdings3.ttf

verify-roundtrip: $(FONTTOOL_BIN) python-venv
	mkdir -p $(BUILD_DIR)/out
	$(FONTTOOL_BIN) encode $(ROOT_DIR)/testdata/OpenSans-Regular.ttf $(BUILD_DIR)/out/OpenSans-Regular.eot
	$(FONTTOOL_BIN) decode $(BUILD_DIR)/out/OpenSans-Regular.eot $(BUILD_DIR)/out/OpenSans-Regular.roundtrip.ttf
	$(VENV_PYTHON) $(ROOT_DIR)/tests/compare_fonts.py \
		$(ROOT_DIR)/testdata/OpenSans-Regular.ttf \
		$(BUILD_DIR)/out/OpenSans-Regular.roundtrip.ttf
