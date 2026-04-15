# Rust Test Migration Inventory

This checklist tracks how the legacy native harness under `tests/test_*.c` and
`tests/test_*.cc` maps into the Rust-first test surface.

Deletion/archival source of truth:

- `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md` is the
  authoritative Phase 0 matrix for deciding whether a remaining native surface
  must be replaced, can stay unsupported, or should move to archive-only status
  before `src/` deletion.
- Use this inventory to track test migration progress, but do not use it by
  itself to decide the support boundary.

Status legend:

- `covered`: the primary behavior is already represented by Rust tests
- `partial`: some behavior is covered in Rust, but native-only coverage remains
- `deferred`: still depends on legacy native internals or tooling not migrated yet
- `next`: a good next migration target once the adjacent Rust crate is ready

## Already Covered By Rust Tests

| Legacy test file | Status | Rust destination | Notes |
| --- | --- | --- | --- |
| `tests/test_decode_pipeline.c` | covered | `tests/rust_integration/decode.rs` | Rust decode CLI path is primary coverage now. |
| `tests/test_encode_pipeline.c` | partial | `tests/rust_integration/encode.rs` | Rust now covers the current TrueType encode structure, `VDMX` omission from block1 and legacy-decoded roundtrip output, `cvt` retention in encoded block1, and a minimal synthetic `hdmx` roundtrip probe. Remaining native-only or not-yet-supported areas are runtime-thread parity, `VDMX` warning parity, PPT-XOR encode behavior, and full legacy-decodable `cvt` roundtrip for real TTF fixtures. |
| `tests/test_eot_header.c` | covered | `crates/fonttool-eot/tests/eot_header.rs` | Header parsing and rejection behavior migrated. |
| `tests/test_lzcomp.c` | partial | `crates/fonttool-mtx/tests/lz_decode.rs` | Rust now covers Java reference fixtures, additional truncated-stream shapes, and roundtrips through the current literal encoder; adaptive copy-encoding parity and additional legacy invalid-stream vectors still remain native-only. |
| `tests/test_mtx_container.c` | covered | `crates/fonttool-mtx/tests/mtx_container.rs` | Container parsing and reject-invalid behavior migrated. |
| `tests/test_sfnt_writer.c` | covered | `crates/fonttool-sfnt/tests/sfnt_serialize.rs` | Rust now covers directory structure, alignment/padding, sorting, search-range fields, OTTO preservation, checksum calculation, and `head` checksum-adjustment behavior. |
| `tests/test_otf_convert.cc` | partial | `tests/rust_integration/otf_convert.rs` | Rust now covers the explicit Phase 3 boundary for static CFF encode, parity-fixture OTF encode, and variable CFF2 subset requests. Successful OTF conversion/subset flows remain native-only compatibility paths. |
| `tests/test_coretext_acceptance.c` | covered | `tests/rust_integration/validation.rs` + `tests/macos-swift/...` | macOS Rust validation now decodes the supported `testdata/font1.fntdata` fixture through the Rust CLI path and invokes the Swift CoreText probe on the produced SFNT. Exact legacy fixture parity remains archived-only. |
| `tests/test_wasm_api.cc` | partial | `tests/rust_integration/runtime_wasm.rs` | Rust-facing runtime/WASM bridge shape is covered, including Rust-owned scheduling semantics and the explicit unsupported boundary for deferred OTF conversion. Native buffer ABI and variable-font conversion success remain native-only. |

## Partially Covered Or Deferred

| Legacy test file | Status | Planned Rust destination | Reason |
| --- | --- | --- | --- |
| `tests/test_cff_reader.cc` | partial | `crates/fonttool-cff` tests | Rust OTF inspection exists, but detailed reader parity is still native. |
| `tests/test_cff_variation.cc` | partial | `tests/rust_integration/otf_convert.rs` + future `fonttool-cff` tests | Variation rejection and the explicit Phase 3 boundary are represented today. Successful instance export remains native-only. |
| `tests/test_cli.c` | partial | `crates/fonttool-cli/tests/cli_contract.rs` + `crates/fonttool-cli/tests/workspace_cli_smoke.rs` + existing integration tests | Rust now covers top-level help/no-command behavior, unknown command status, decode/encode missing-arg contract errors, successful decode of the supported `font1.fntdata` fixture, decode/TrueType-encode success outside the workspace root without `build/fonttool`, decode of an XOR-obfuscated supported `.fntdata` copy, and the current subset/OTF deferred-boundary rejection matrix. Native-only gaps are now mostly archived success-output text, `wingdings3.eot` fixture-specific success paths, `.fntdata` obfuscation-flag encode assertions, warning-count/order checks, and thread/env legacy parity. |
| `tests/test_cu2qu.cc` | deferred | future `crates/fonttool-cff` or `fonttool-glyf` tests | Conversion internals not yet migrated into Rust ownership. |
| `tests/test_cvt_codec.c` | deferred | future `fonttool-mtx` tests | `cvt` codec surface is not yet a Rust-owned public module. |
| `tests/test_glyf_codec.c` | deferred | future `crates/fonttool-glyf` tests | Encode path exists, but detailed codec vectors are not yet ported. |
| `tests/test_hdmx_codec.c` | deferred | future `fonttool-mtx`/integration tests | HDMX preservation/drop semantics still partly native-owned. |
| `tests/test_otf_parity.cc` | partial | `tests/rust_integration/otf_convert.rs` + `tests/test_fonttools_parity.py` | Rust now covers the explicit Phase 3 boundary for the parity fixture. Actual OTF parity comparisons remain native-only compatibility coverage. |
| `tests/test_parallel_runtime.cc` | partial | `crates/fonttool-runtime` tests + `tests/rust_integration/runtime_wasm.rs` | Rust now covers requested/effective thread counts, mode resolution, idle diagnostics, and lowest-index failure ordering for the Rust-facing runtime/WASM crates. Native C ABI/export parity remains legacy-only. |
| `tests/test_sfnt_subset.c` | partial | `tests/rust_integration/subset.rs` + future `fonttool-subset` tests | Rust now covers subset planning on the supported embedded decode slice plus the explicit Phase 2 boundary for non-OTF subset execution. Native subset rebuild invariants remain legacy-only. |
| `tests/test_subset_args.c` | partial | `crates/fonttool-cli/tests/cli_contract.rs` + future `fonttool-cli` integration tests | Rust now covers the current parser and dispatch contract for missing values, duplicate or missing selection mode, unsupported flags, static OTF `--variation` rejection, OTF `--text` requirement, the non-OTF `--glyph-ids` contract, and the explicit deferred-boundary errors for Phase 2/3-owned execution paths. Remaining native-only cases are mostly legacy parser features that Rust does not support, including `--keep-gids`, `--unicodes`, and helper-level request initialization APIs. |
| `tests/test_table_policy.c` | deferred | future CLI/integration tests | Table retention policy is only indirectly covered right now. |
| `tests/test_ttf_rebuilder.cc` | deferred | future `fonttool-glyf`/`fonttool-cff` tests | Rebuilder internals still live behind the legacy backend. |
| `tests/test_ttf_rebuilder_header.c` | deferred | future `fonttool-glyf` tests | Same rebuild boundary as above. |

## Native Harness Infrastructure Or Legacy-Only Entry Points

| Legacy file | Status | Notes |
| --- | --- | --- |
| `tests/test_main.c` | deferred | Legacy test runner; remove only after Rust becomes the primary harness for the remaining native-only areas. |
| `tests/test_fonttools_parity.py` | deferred | Still useful as an external parity/reference tool, not an internal Rust harness replacement target. |
| `tests/verify_wasm_artifacts.sh` | deferred | Artifact verification remains tied to the legacy Emscripten outputs for now. |

## Recommended Next Migrations

1. Continue `tests/test_cli.c` migration only for still-relevant success/warning ordering checks and any supported fixture paths not yet represented by Rust tests.
2. Leave `tests/test_subset_args.c` legacy-only flags and helper APIs native unless the Rust CLI gains equivalent support.
3. Continue `tests/test_parallel_runtime.cc` migration only for the remaining native C ABI/export parity now that the Rust-facing scheduling semantics are owned by `fonttool-runtime`.
