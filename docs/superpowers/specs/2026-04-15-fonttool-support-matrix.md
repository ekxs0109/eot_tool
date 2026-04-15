# Fonttool Support Matrix

Date: 2026-04-15

This document is the Phase 0 source of truth for what is still supported,
unsupported, or kept only for archival/reference while `src/` retirement is in
progress.

Interpretation:

- `supported`: a current workflow or API that the repository still presents as a
  valid path today, even if it still delegates to legacy code. This includes
  retained verification and validation utilities that are still part of the
  accepted repository workflow.
- `unsupported`: a surface that the current Rust-first contract rejects,
  documents as not yet available, or fails to honor with the legacy semantics.
- `archive-only`: retained for parity, reference, compatibility commands, or
  native-harness coverage only; not part of the forward supported boundary.

Implementation-owner labels:

- `rust`: the current behavior is implemented through Rust-owned code paths
- `legacy`: the current behavior is still implemented only by the native code or
  native-only tooling
- `mixed`: Rust owns part of the user-facing surface, but execution still
  depends on legacy implementation or legacy-backed verification

Inputs used for this matrix:

- `tests/rust-test-inventory.md`
- current Rust integration tests under `tests/rust_integration/`
- native coverage still defining behavior under `tests/test_*.c` and
  `tests/test_*.cc`
- Python and Swift validation entrypoints in `tests/`
- README workflows and compatibility notes in `README.md`

## Explicit Phase 1 Shared-Support Ownership Map

This file-level map replaces the old category-level shorthand for the remaining
Phase 1 shared-support native files and shellout/deferred adapter callsites.

| legacy/support item | current Rust destination crate | current Rust destination file | still-missing behavior | test target that must pass before Phase 1 is done |
| --- | --- | --- | --- | --- |
| `src/parallel_runtime.cc` | `fonttool-runtime` | `crates/fonttool-runtime/src/lib.rs` | Rust now owns requested/effective thread resolution, mode selection, task-count clamp behavior, and lowest-failing-index scheduling semantics. The native C ABI/export surface in `parallel_runtime_run_*`, `parallel_runtime_set_*`, and `parallel_runtime_last_run_*` is still legacy-only for archived/native callers. | `cargo test -p fonttool-cli --test runtime_wasm -q` |
| `src/parallel_runtime.h` | `fonttool-runtime`, `fonttool-wasm` | `crates/fonttool-runtime/src/lib.rs`; `crates/fonttool-wasm/src/lib.rs` | There is still no Rust-owned replacement for the native C header contract or the legacy WASM-facing C ABI exports that include process-wide diagnostics storage. Phase 1 only owns the Rust-facing runtime/WASM semantics, not the old C ABI. | `cargo test -p fonttool-cli --test runtime_wasm -q` |
| `src/table_policy.c` | `fonttool-subset` | `crates/fonttool-subset/src/lib.rs` | Rust now owns the supported table classification matrix, but archived native subset/encode flows still carry legacy warning/output behavior outside the Rust Phase 1 boundary. | `cargo test -p fonttool-subset table_policy -q` |
| `src/table_policy.h` | `fonttool-subset` | `crates/fonttool-subset/src/lib.rs` | There is still no Rust-owned C-ABI replacement for native callers that include this header. Phase 1 only requires the Rust table-policy API used by the rewrite crates. | `cargo test -p fonttool-subset table_policy -q` |
| `crates/fonttool-runtime/src/lib.rs::run_conversion_request(...)` deferred shellout adapter via `encode_otf_with_legacy_backend(...)` | `fonttool-runtime` -> `fonttool-cff` | `crates/fonttool-runtime/src/lib.rs` -> `crates/fonttool-cff/src/lib.rs` | Static `OTF(CFF/CFF2)` runtime/WASM conversion is still missing. For Phase 1 this row is acceptable only because the Rust runtime now fails explicitly with `CffError::EncodeDeferredToPhase3` instead of hiding a legacy shellout. | `cargo test -p fonttool-cli --test runtime_wasm -q` |
| `crates/fonttool-harfbuzz/src/lib.rs::run_subset_adapter(...)` deferred shellout adapter | `fonttool-harfbuzz` | `crates/fonttool-harfbuzz/src/lib.rs` | The legacy adapter is still retained for archived compatibility callers, but the supported Rust CLI subset path now owns non-OTF glyph-id execution, warnings, and output writing. | `cargo test -p fonttool-cli --test subset -q` |
| `crates/fonttool-cff/src/lib.rs::encode_otf_with_legacy_backend(...)` deferred shellout adapter | `fonttool-cff` | `crates/fonttool-cff/src/lib.rs` | Actual static `OTF(CFF/CFF2)` encode implementation is still missing. This is explicitly deferred to Phase 3 and is not a Phase 1 blocker as long as callers keep returning `EncodeDeferredToPhase3`. | `cargo test -p fonttool-cli --test otf_convert -q` |
| `crates/fonttool-cff/src/lib.rs::subset_otf_with_legacy_backend(...)` deferred shellout adapter | `fonttool-cff` | `crates/fonttool-cff/src/lib.rs` | Actual `OTF(CFF/CFF2)` subset execution and variable-instance export are still missing. This is explicitly deferred to Phase 3 and is not a Phase 1 blocker as long as callers keep returning `SubsetDeferredToPhase3`. | `cargo test -p fonttool-cli --test otf_convert -q` |
| `crates/fonttool-cli/src/main.rs::encode_file(...)` `.fntdata` deferred shellout boundary | `fonttool-cli` | `crates/fonttool-cli/src/main.rs` | PowerPoint-compatible `.fntdata` encode remains missing: Rust still does not emit the `0x10000000` flag or XOR-obfuscated payload. This is Phase 2-owned, so Phase 1 only requires an explicit rejection without reviving a shellout. | `cargo test -p fonttool-cli --test cli_contract -q` |
| `crates/fonttool-cli/src/main.rs::encode_file(...)` CFF/CFF2 deferred shellout boundary | `fonttool-cli` -> `fonttool-cff` | `crates/fonttool-cli/src/main.rs` -> `crates/fonttool-cff/src/lib.rs` | CLI-owned `OTF(CFF/CFF2)` encode execution is still missing. This remains Phase 3-owned and is not a Phase 1 blocker while the CLI keeps surfacing the explicit deferred error. | `cargo test -p fonttool-cli --test otf_convert -q` |
| `crates/fonttool-cli/src/main.rs::subset_file(...)` non-OTF subset execution | `fonttool-cli` -> `fonttool-subset` | `crates/fonttool-cli/src/main.rs` -> `crates/fonttool-subset/src/lib.rs` | CLI argument validation and supported non-OTF subset execution are now Rust-owned, including rebuilding `maxp`, `hhea`, `hmtx`, `glyf`, `loca`, and `cmap`, warning emission, and `.eot` / `.fntdata` output creation. OTF subset remains deferred. | `cargo test -p fonttool-cli --test subset -q` |
| `crates/fonttool-cli/src/main.rs::subset_otf_file(...)` OTF deferred shellout boundary | `fonttool-cli` -> `fonttool-cff` | `crates/fonttool-cli/src/main.rs` -> `crates/fonttool-cff/src/lib.rs` | CLI-owned `OTF(CFF/CFF2)` subset execution and variable-instance export are still missing. This remains Phase 3-owned and is not a Phase 1 blocker while the CLI keeps surfacing the explicit deferred error. | `cargo test -p fonttool-cli --test otf_convert -q` |

## CLI And User-Facing Workflows

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| CLI `decode` for supported `.eot` / `.fntdata` fixtures with empty extra MTX blocks | supported | rust | no | Rust CLI/tests cover `testdata/font1.fntdata` plus XOR-obfuscated copies. This is the current narrow supported decode slice. |
| CLI `decode` of multi-block MTX payloads, including current Rust TrueType encode output | unsupported | rust | yes | `crates/fonttool-cli/src/main.rs` rejects non-empty block2/block3. Manual probe on 2026-04-15 confirmed `cargo run ... encode OpenSans-Regular.ttf` then Rust `decode` fails with `non-empty extra MTX blocks are not supported in this decode slice`. |
| CLI `encode` for TrueType input to `.eot` | supported | rust | no | Rust owns the TTF encode path and current Rust tests cover structure, `VDMX` omission, `cvt` retention in block1, and a synthetic `hdmx` roundtrip probe. |
| CLI `encode` PowerPoint-compatible `.fntdata` output and `0x10000000` flag semantics | unsupported | rust | yes | The archived native compatibility path still documents this workflow, but the current Rust CLI always calls `build_eot_file(..., false)`. Manual header probe on 2026-04-15 saw flags `0x4`, not `0x10000000`. |
| CLI `encode` for `OTF(CFF/CFF2)` input | unsupported | rust | yes | Rust CLI now rejects this path with `OTF(CFF/CFF2) encode remains Phase 3-owned...` instead of shelling out through `fonttool-cff`. Archived native compatibility commands remain available via `./build/fonttool`. |
| CLI `subset` for non-OTF input with `--glyph-ids` | supported | rust | no | Rust now owns this supported subset slice directly, including `.eot` / `.fntdata` wrappers, `maxp.numGlyphs` updates, and HDMX/VDMX warning drops when those tables are present. |
| CLI `subset` for `OTF(CFF/CFF2)` input with `--text` and optional `--variation` | unsupported | rust | yes | Rust CLI validates the contract, then rejects the execution path with `OTF(CFF/CFF2) subset remains Phase 3-owned...` instead of shelling out through `fonttool-cff`. |
| CLI `subset` for non-OTF input with `--text` | unsupported | rust | no | Current Rust contract explicitly rejects this path with `subset currently only supports --glyph-ids for non-OTF input`. |
| CLI `subset --keep-gids` | unsupported | legacy | no | Still only covered in native tests (`tests/test_subset_args.c`, `tests/test_cli.c`). The Rust CLI does not accept the flag today. |
| CLI `subset --unicodes` | unsupported | legacy | no | Native parser behavior only. No Rust CLI equivalent exists today. |
| Native-only helper subset APIs such as `subset_request_init_for_glyph_ids_keep(...)` | archive-only | legacy | no | Keep only as native reference coverage unless a Rust public API is intentionally added later. |

## MTX, LZ, And Table Semantics

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| MTX container parsing plus LZ block1 decode for the supported Rust decode slice | supported | rust | no | Rust owns the current block1 decode path and rejects out-of-scope extra blocks. |
| LZ adaptive copy-encoding parity and additional native invalid-stream vectors from `tests/test_lzcomp.c` | archive-only | legacy | no | Rust `fonttool-mtx` covers the literal encoder and migrated fixture set; remaining parity vectors are still native-only reference coverage. |
| `cvt` preservation in current encode/decode flows | supported | mixed | yes | Rust encode preserves `cvt` bytes in block1, but full legacy-decodable roundtrip coverage for real fixtures still depends on native decode/parity paths. |
| `HDMX` preservation on encode/decode | supported | mixed | yes | Rust encode coverage uses a synthetic `hdmx` font and native decode/parity helpers for roundtrip verification. |
| `HDMX` behavior during subset (`warning` + drop) | supported | rust | no | Rust-owned non-OTF subset execution now emits the supported HDMX drop warning and removes the table from the subset output. |
| `VDMX` behavior during Rust TrueType encode (`drop`) | supported | rust | yes | Rust encode tests cover VDMX omission from the current TrueType encode slice. Legacy warning text parity is still not part of the Rust contract. |
| `VDMX` behavior during subset (`warning` + drop) | supported | rust | no | Rust-owned non-OTF subset execution now emits the supported VDMX drop warning and removes the table from the subset output. |
| Native `cvt_codec` unit API (`tests/test_cvt_codec.c`) | archive-only | legacy | no | No Rust-owned public codec module exists yet; keep only as native reference coverage. |
| Native `hdmx_codec` unit API (`tests/test_hdmx_codec.c`) | archive-only | legacy | no | Detailed codec vectors still live only in the native harness. |

## OTF, Runtime, And WASM Entry Points

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| Static `OTF(CFF)` conversion for CLI/runtime/WASM flows | unsupported | rust | yes | Rust-facing entrypoints now reject static CFF conversion with explicit Phase 3-owned errors instead of shelling out. Archived native compatibility commands remain available via `./build/fonttool`. |
| Rust runtime/WASM `.fntdata` output kind | unsupported | rust | yes | Rust runtime-facing APIs now reject PowerPoint-compatible `.fntdata` output with an explicit Phase 2-owned error instead of silently carrying that output kind forward. |
| Variable `OTF(CFF2)` subset instance export with `--variation` | unsupported | rust | yes | Rust CLI/runtime surfaces now keep this path outside the supported boundary. Successful instance export remains a native compatibility-only flow until Phase 3. |
| Variation tables (`CFF2`, `fvar`, `avar`, `HVAR`, `MVAR`, `VVAR`, `cvar`, `gvar`) being dropped from rebuilt embedded output | archive-only | legacy | yes | This behavior is currently observable only through archived native OTF compatibility flows because Rust no longer claims supported OTF export behavior in Phase 1. |
| Rust runtime entrypoint `fonttool_runtime::convert_otf_to_embedded_font` for static CFF input | unsupported | rust | yes | The Rust API is current, but for `.eot` output it now returns `CffError::EncodeDeferredToPhase3` instead of shelling out through `fonttool-cff`. |
| Rust runtime variable-font conversion via `ConvertRequest { variation_axes: ... }` | unsupported | rust | yes | `tests/rust_integration/runtime_wasm.rs` asserts the explicit `runtime bridge does not yet support variable-font conversion` error. |
| Rust runtime diagnostics parity with native `parallel_runtime` | supported | rust | no | Rust now owns requested/effective thread counts, mode resolution, and lowest-index failure ordering for the Rust-facing runtime/WASM crates. Native C ABI diagnostics remain a separate legacy surface. |
| Rust WASM crate bridge `fonttool_wasm::wasm_convert_otf_to_embedded_font` for static CFF input | unsupported | rust | yes | Same ownership split as the runtime crate: for `.eot` output the Rust API surface now rejects static CFF conversion with `CffError::EncodeDeferredToPhase3`, and `.fntdata` stays Phase 2-owned. |
| Native browser/WASM C buffer ABI in `src/wasm_api.{h,cc}` including `wasm_buffer_destroy(...)` | supported | legacy | yes | Still the only implementation for the documented C ABI and buffer ownership contract. No Rust-owned replacement exists yet. |
| Native WASM CFF2 instance conversion success path (`tests/test_wasm_api.cc`) | supported | legacy | yes | Rust runtime/WASM tests explicitly keep variable-font conversion unsupported for now; native WASM API still owns the successful CFF2 instance path. |
| Native WASM runtime diagnostics struct export (`wasm_runtime_get_diagnostics`) | supported | legacy | yes | Native WASM API still exposes the C ABI diagnostics struct. Rust-facing crates now own requested/effective thread resolution and failure-order semantics, but not the legacy C export surface. |

## Validation, README Compatibility Paths, And Harness Infrastructure

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| Python verification entrypoints `tests/verify_font.py` and `tests/compare_fonts.py` | supported | legacy | no | Verification/reference-only tooling. Not a Rust product surface and not blocked on `src/` deletion. |
| Python parity entrypoint `tests/test_fonttools_parity.py` in the current OTF parity workflow | archive-only | legacy | no | The helper remains as a compatibility/reference tool, but the current Rust integration suite no longer depends on it after Phase 1 shellout removal. |
| Swift CoreText validation entrypoint `cargo test -p fonttool-cli --test validation` | supported | rust | no | The Rust test harness now decodes `testdata/font1.fntdata` through the Rust CLI decode slice, then invokes the Swift probe on the produced TTF. |
| Standalone Swift probe `tests/macos-swift` (`swift run --package-path tests/macos-swift FonttoolCoreTextProbe ...`) | supported | legacy | no | Manual diagnostic tool; independent from `src/` retirement once it is handed a TTF file. |
| README compatibility commands using `./build/fonttool`, `make verify-*`, and `make test TESTCASE=...` | archive-only | legacy | no | README already says to treat these as archived compatibility commands unless a section says otherwise. |
| Native harness runner `tests/test_main.c` | archive-only | legacy | no | Legacy test registration/capture harness. Remove when the remaining native-only tests are archived or replaced. |
| WASM artifact verification script `tests/verify_wasm_artifacts.sh` | supported | legacy | yes | Still part of the documented build flow and tied to legacy Emscripten outputs plus `src/wasm_api` exports. Replace with a Rust/package-owned verifier before deleting `src/`. |
