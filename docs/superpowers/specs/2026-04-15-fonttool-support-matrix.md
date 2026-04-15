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

## CLI And User-Facing Workflows

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| CLI `decode` for supported `.eot` / `.fntdata` fixtures with empty extra MTX blocks | supported | rust | no | Rust CLI/tests cover `testdata/font1.fntdata` plus XOR-obfuscated copies. This is the current narrow supported decode slice. |
| CLI `decode` of multi-block MTX payloads, including current Rust TrueType encode output | unsupported | rust | yes | `crates/fonttool-cli/src/main.rs` rejects non-empty block2/block3. Manual probe on 2026-04-15 confirmed `cargo run ... encode OpenSans-Regular.ttf` then Rust `decode` fails with `non-empty extra MTX blocks are not supported in this decode slice`. |
| CLI `encode` for TrueType input to `.eot` | supported | rust | no | Rust owns the TTF encode path and current Rust tests cover structure, `VDMX` omission, `cvt` retention in block1, and a synthetic `hdmx` roundtrip probe. |
| CLI `encode` PowerPoint-compatible `.fntdata` output and `0x10000000` flag semantics | unsupported | rust | yes | README still documents this workflow, but current Rust CLI always calls `build_eot_file(..., false)`. Manual header probe on 2026-04-15 saw flags `0x4`, not `0x10000000`. |
| CLI `encode` for `OTF(CFF/CFF2)` input | supported | mixed | yes | Rust CLI dispatch exists, but conversion still shells out through `fonttool-cff` to `build/fonttool`. Static CFF roundtrip is covered in Rust integration; CFF2 encode behavior still depends on native coverage. |
| CLI `subset` for non-OTF input with `--glyph-ids` | supported | mixed | yes | Rust owns arg parsing and subset planning; execution still goes through `fonttool-harfbuzz`'s legacy subset adapter. |
| CLI `subset` for `OTF(CFF/CFF2)` input with `--text` and optional `--variation` | supported | mixed | yes | Rust CLI validates the contract, then `fonttool-cff` shells out to the legacy backend. Static CFF and variable CFF2 subset success are still adapter-backed. |
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
| `HDMX` behavior during subset (`warning` + drop) | supported | mixed | yes | Current subset warnings come from the legacy subset backend and are surfaced through the Rust adapter. |
| `VDMX` behavior during encode/subset (`warning` + drop) | supported | mixed | yes | Rust encode tests cover table omission; warning parity and subset behavior still depend on adapter/native coverage. |
| Native `cvt_codec` unit API (`tests/test_cvt_codec.c`) | archive-only | legacy | no | No Rust-owned public codec module exists yet; keep only as native reference coverage. |
| Native `hdmx_codec` unit API (`tests/test_hdmx_codec.c`) | archive-only | legacy | no | Detailed codec vectors still live only in the native harness. |

## OTF, Runtime, And WASM Entry Points

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| Static `OTF(CFF)` conversion for CLI/runtime/WASM flows | supported | mixed | yes | The supported path still runs through legacy conversion code. Rust owns the surrounding API surface, not the conversion core. |
| Variable `OTF(CFF2)` subset instance export with `--variation` | supported | mixed | yes | Current success path still depends on the native backend. Rust integration covers the CLI contract and adapter-backed subset result. |
| Variation tables (`CFF2`, `fvar`, `avar`, `HVAR`, `MVAR`, `VVAR`, `cvar`, `gvar`) being dropped from rebuilt embedded output | supported | mixed | yes | README documents this as the current behavior. The implementation remains legacy-backed for the supported OTF paths. |
| Rust runtime entrypoint `fonttool_runtime::convert_otf_to_embedded_font` for static CFF input | supported | mixed | yes | The Rust API is current, but it calls `encode_otf_with_legacy_backend(...)` under the hood. |
| Rust runtime variable-font conversion via `ConvertRequest { variation_axes: ... }` | unsupported | mixed | yes | `tests/rust_integration/runtime_wasm.rs` asserts the explicit `runtime bridge does not yet support variable-font conversion` error. |
| Rust runtime diagnostics parity with native `parallel_runtime` | unsupported | mixed | yes | `default_runtime_diagnostics()` is still placeholder data and explicitly says scheduling diagnostics are not yet available in the Rust bridge. |
| Rust WASM crate bridge `fonttool_wasm::wasm_convert_otf_to_embedded_font` for static CFF input | supported | mixed | yes | Same ownership split as the runtime crate: Rust API surface, legacy conversion backend. |
| Native browser/WASM C buffer ABI in `src/wasm_api.{h,cc}` including `wasm_buffer_destroy(...)` | supported | legacy | yes | Still the only implementation for the documented C ABI and buffer ownership contract. No Rust-owned replacement exists yet. |
| Native WASM CFF2 instance conversion success path (`tests/test_wasm_api.cc`) | supported | legacy | yes | Rust runtime/WASM tests explicitly keep variable-font conversion unsupported for now; native WASM API still owns the successful CFF2 instance path. |
| Native WASM runtime diagnostics struct export (`wasm_runtime_get_diagnostics`) | supported | legacy | yes | Native WASM API still exposes real `parallel_runtime` diagnostics; Rust-facing crates only expose placeholder diagnostics today. |

## Validation, README Compatibility Paths, And Harness Infrastructure

| legacy/native surface | current user-visible/support status | current implementation owner | required replacement before `src/` deletion | replacement destination / notes |
| --- | --- | --- | --- | --- |
| Python verification entrypoints `tests/verify_font.py` and `tests/compare_fonts.py` | supported | legacy | no | Verification/reference-only tooling. Not a Rust product surface and not blocked on `src/` deletion. |
| Python parity entrypoint `tests/test_fonttools_parity.py` in the current OTF parity workflow | supported | mixed | yes | Still used by Rust integration and README parity recipes, but the roundtrip setup currently depends on legacy encode/decode behavior. |
| Swift CoreText validation entrypoint `cargo test -p fonttool-cli --test validation` | supported | mixed | yes | The Rust test harness invokes the Swift probe after a roundtrip that still uses the legacy decode adapter. |
| Standalone Swift probe `tests/macos-swift` (`swift run --package-path tests/macos-swift FonttoolCoreTextProbe ...`) | supported | legacy | no | Manual diagnostic tool; independent from `src/` retirement once it is handed a TTF file. |
| README compatibility commands using `./build/fonttool`, `make verify-*`, and `make test TESTCASE=...` | archive-only | legacy | no | README already says to treat these as archived compatibility commands unless a section says otherwise. |
| Native harness runner `tests/test_main.c` | archive-only | legacy | no | Legacy test registration/capture harness. Remove when the remaining native-only tests are archived or replaced. |
| WASM artifact verification script `tests/verify_wasm_artifacts.sh` | supported | legacy | yes | Still part of the documented build flow and tied to legacy Emscripten outputs plus `src/wasm_api` exports. Replace with a Rust/package-owned verifier before deleting `src/`. |
