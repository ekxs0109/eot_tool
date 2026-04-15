# Fonttool Support Matrix

Date: 2026-04-15

This document is the source of truth for the post-`src/` repository contract.
The legacy native tree, Makefile, and native C/C++ harness have been removed.

Interpretation:

- `supported`: current repository workflow or API that is expected to work
- `unsupported`: current repository workflow/API that intentionally returns an
  explicit error or is not implemented
- `archive-only`: historical context retained only in docs or external tools,
  not in the active repository surface

Implementation-owner labels:

- `rust`: implemented through Rust-owned code paths or package scripts
- `legacy`: external reference tooling only, not an in-repo product surface

Inputs used for this matrix:

- `tests/rust-test-inventory.md`
- current Rust integration tests under `tests/rust_integration/`
- Python and Swift validation entrypoints in `tests/`
- `README.md`

## Shared Ownership Map

| item | current owner | destination | current contract | verification |
| --- | --- | --- | --- | --- |
| Runtime thread selection and diagnostics semantics | rust | `crates/fonttool-runtime/src/lib.rs` | Supported for Rust-facing runtime/WASM crates, including requested/effective thread counts, mode resolution, and lowest-failing-index ordering | `cargo test -p fonttool-cli --test runtime_wasm -q` |
| Table policy for supported subset output | rust | `crates/fonttool-subset/src/lib.rs` | Supported for the Rust-owned non-OTF subset path | `cargo test -p fonttool-subset table_policy -q` |
| Non-OTF subset execution | rust | `crates/fonttool-cli/src/main.rs` + `crates/fonttool-subset/src/lib.rs` + `crates/fonttool-harfbuzz/src/lib.rs` | Supported for `.ttf`, `.eot`, and `.fntdata` input with `--glyph-ids` | `cargo test -p fonttool-cli --test subset -q` |
| TrueType encode | rust | `crates/fonttool-cli/src/main.rs` + `crates/fonttool-mtx` + `crates/fonttool-glyf` | Supported for `.ttf -> .eot` | `cargo test -p fonttool-cli --test encode -q` |
| Supported decode slice | rust | `crates/fonttool-cli/src/main.rs` + `crates/fonttool-eot` + `crates/fonttool-mtx` + `crates/fonttool-sfnt` | Supported for block1-backed `.eot` / `.fntdata` fixtures | `cargo test -p fonttool-cli --test decode -q` |
| WASM artifact verification | rust | `tests/verify_wasm_artifacts.sh` + `packages/fonttool-wasm/scripts/*.mjs` | Supported via vendored package artifacts and runtime probing through the package loader contract | `pnpm verify:wasm` |

## CLI And User-Facing Workflows

| workflow | status | owner | notes |
| --- | --- | --- | --- |
| CLI `decode` for supported `.eot` / `.fntdata` fixtures with empty extra MTX blocks | supported | rust | Current Rust decode slice includes PPT-XOR handling when the header flag is present. |
| CLI `decode` of multi-block MTX payloads, including current Rust TrueType encode output | unsupported | rust | The wider MTX bridge exists only for subset input reconstruction today. |
| CLI `encode` for TrueType input to `.eot` | supported | rust | Rust owns the supported forward encode path. |
| CLI `encode` PowerPoint-compatible `.fntdata` output and `0x10000000` flag semantics | unsupported | rust | Explicitly outside the current supported boundary. |
| CLI `encode` for `OTF(CFF/CFF2)` input | unsupported | rust | Returns the explicit Phase 3-owned deferred error. |
| CLI `subset` for non-OTF input with `--glyph-ids` | supported | rust | Rust owns `.ttf`, `.eot`, and `.fntdata` execution, table rebuilds, warnings, and output writing. |
| CLI `subset` for non-OTF input with `--text` | unsupported | rust | Explicitly rejected by the Rust CLI today. |
| CLI `subset --keep-gids` | unsupported | rust | No current Rust CLI support. |
| CLI `subset --unicodes` | unsupported | rust | No current Rust CLI support. |
| CLI `subset` for `OTF(CFF/CFF2)` input with `--text` and optional `--variation` | unsupported | rust | Returns the explicit Phase 3-owned deferred error. |

## MTX, SFNT, And Table Semantics

| surface | status | owner | notes |
| --- | --- | --- | --- |
| MTX container parsing plus LZ block1 decode for the supported Rust decode slice | supported | rust | Rust owns the current decode path and reject-invalid behavior. |
| `cvt` preservation in current encode/decode flows | supported | rust | Rust-owned helpers and tests cover the supported behavior. |
| `HDMX` preservation on encode/decode | supported | rust | Rust-owned helpers and tests cover shared-width behavior and roundtrips. |
| `HDMX` behavior during subset (`warning` + drop) | supported | rust | Rust-owned non-OTF subset emits the warning and drops the table. |
| `VDMX` behavior during Rust TrueType encode (`drop`) | supported | rust | Covered by current encode tests. |
| `VDMX` behavior during subset (`warning` + drop) | supported | rust | Covered by current subset tests. |
| Native unit-test-only APIs from the retired `src/` tree | archive-only | legacy | Historical only; no active repository dependency remains. |

## OTF, Runtime, And WASM Entry Points

| surface | status | owner | notes |
| --- | --- | --- | --- |
| Static `OTF(CFF)` conversion for CLI/runtime/WASM flows | unsupported | rust | Rust entrypoints explicitly reject this path instead of shelling out. |
| Rust runtime/WASM `.fntdata` output kind | unsupported | rust | Explicitly rejected. |
| Variable `OTF(CFF2)` subset instance export with `--variation` | unsupported | rust | Explicitly rejected. |
| Rust runtime entrypoint `fonttool_runtime::convert_otf_to_embedded_font` for static CFF input | unsupported | rust | Returns `CffError::EncodeDeferredToPhase3`. |
| Rust runtime variable-font conversion via `ConvertRequest { variation_axes: ... }` | unsupported | rust | `runtime_wasm` tests assert the explicit unsupported error. |
| Rust runtime diagnostics parity for Rust-facing APIs | supported | rust | Rust owns the current supported runtime semantics. |
| Rust WASM package runtime selection and staged artifact probing | supported | rust | Verified through vendored artifacts and package loader probing. |
| Retired native browser/WASM C buffer ABI and native runtime diagnostics struct | archive-only | legacy | Removed from the repository along with `src/`; historical references remain in plan docs only. |

## Validation And Tooling

| surface | status | owner | notes |
| --- | --- | --- | --- |
| Python verification entrypoints `tests/verify_font.py` and `tests/compare_fonts.py` | supported | legacy | Verification/reference-only tooling; independent of the retired native tree. |
| Python parity helper `tests/test_fonttools_parity.py` | archive-only | legacy | Historical parity/reference tool. |
| Swift CoreText validation entrypoint `cargo test -p fonttool-cli --test validation` | supported | rust | Rust test harness decodes through the Rust CLI and invokes the Swift probe on the produced TTF. |
| Standalone Swift probe `tests/macos-swift` | supported | legacy | Manual diagnostic tool; independent of `src/` retirement. |
| `tests/verify_wasm_artifacts.sh` | supported | rust | Validates vendored package artifacts, builds the package entrypoint, and probes staged runtimes. |

## Retirement Summary

There are no remaining `legacy | supported | required before src deletion = yes`
surfaces. Anything that used to depend directly on `src/` is now either:

- replaced by a Rust-owned implementation or package script
- explicitly unsupported in the Rust contract
- retained only as archive/reference notes outside the active build and test
  graph
