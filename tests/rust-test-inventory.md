# Rust Test Migration Inventory

This inventory records how the retired native C/C++ harness mapped into the
current Rust-first test surface.

The native `src/` tree, Makefile, and `tests/test_*.c` / `tests/test_*.cc`
harness were removed on 2026-04-15 after the supported repository contract was
moved onto Rust-owned code and package scripts.

Use the repository `README.md` as the current support-boundary summary. This
file is the historical test-migration map.

Status legend:

- `covered`: the supported behavior is represented by current Rust tests
- `partial`: Rust covers the supported boundary, while some historical
  success-path or parity detail remains out of scope
- `archive-only`: historical coverage kept only as documentation/reference

## Historical Native Tests And Their Rust Destinations

| Retired native test file | Status | Rust/current destination | Notes |
| --- | --- | --- | --- |
| `tests/test_decode_pipeline.c` | covered | `tests/rust_integration/decode.rs` | Rust decode CLI path is the supported coverage now. |
| `tests/test_encode_pipeline.c` | covered | `tests/rust_integration/encode.rs` | Rust covers current TrueType encode structure, decode-side roundtrip, `cvt` retention, `hdmx` behavior, and `VDMX` omission. |
| `tests/test_eot_header.c` | covered | `crates/fonttool-eot/tests/eot_header.rs` | Header parsing and rejection behavior migrated. |
| `tests/test_lzcomp.c` | partial | `crates/fonttool-mtx/tests/lz_decode.rs` | Rust covers the supported decode slice; extra legacy parity vectors are archive-only. |
| `tests/test_mtx_container.c` | covered | `crates/fonttool-mtx/tests/mtx_container.rs` | Container parsing and reject-invalid behavior migrated. |
| `tests/test_sfnt_writer.c` | covered | `crates/fonttool-sfnt/tests/sfnt_serialize.rs` | Rust covers directory structure, alignment, sorting, checksums, and `head` checksum-adjustment behavior. |
| `tests/test_otf_convert.cc` | covered | `tests/rust_integration/otf_convert.rs` + `tests/rust_integration/woff.rs` + `crates/fonttool-cff/tests/convert.rs` | Rust covers static CFF convert, variable CFF2 instancing-to-TTF, and WOFF/WOFF2 source-materialization routing for the supported convert surface. |
| `tests/test_coretext_acceptance.c` | covered | `tests/rust_integration/validation.rs` + `tests/macos-swift/...` | Rust validation decodes through the current CLI path and invokes the Swift probe on the produced SFNT. |
| `tests/test_table_policy.c` | covered | `crates/fonttool-subset` tests | Rust owns the supported table-policy matrix. |
| `tests/test_wasm_api.cc` | covered | `tests/rust_integration/runtime_wasm.rs` + `packages/fonttool-wasm/test/*.test.ts` | Rust/package tests cover the supported runtime/WASM convert contract for `CFF/CFF2/WOFF` embedded output; the retired native C buffer ABI is no longer part of the repository surface. |
| `tests/test_cff_reader.cc` | covered | `crates/fonttool-cff/tests/otf_support.rs` + `crates/fonttool-cff/tests/source.rs` | Rust covers CFF/CFF2 inspection plus WOFF/WOFF2 source materialization for the supported surface. |
| `tests/test_cff_variation.cc` | covered | `tests/rust_integration/otf_convert.rs` + `crates/fonttool-cff/tests/instance.rs` + `crates/fonttool-cff/tests/otf_support.rs` | Rust covers variable CFF2 instancing, convert success paths, and subset materialization on the supported boundary. |
| `tests/test_cli.c` | covered | `crates/fonttool-cli/tests/cli_contract.rs` + integration tests | Rust covers the current CLI contract and supported success paths. |
| `tests/test_cu2qu.cc` | archive-only | future `fonttool-cff` or `fonttool-glyf` tests | Historical conversion-internals coverage only. |
| `tests/test_cvt_codec.c` | covered | `crates/fonttool-mtx/tests/codecs.rs` | Rust owns the supported `cvt` codec helper surface. |
| `tests/test_glyf_codec.c` | covered | `crates/fonttool-glyf` tests | Rust covers detailed `glyf` codec vectors and corruption rejection for the supported path. |
| `tests/test_hdmx_codec.c` | covered | `crates/fonttool-mtx/tests/codecs.rs` + `tests/rust_integration/subset.rs` | Rust owns the supported `hdmx` codec helper surface and subset-time warning/drop behavior. |
| `tests/test_otf_parity.cc` | archive-only | `tests/rust_integration/otf_convert.rs` + `tests/test_fonttools_parity.py` | Historical parity/reference coverage only. |
| `tests/test_parallel_runtime.cc` | covered | `crates/fonttool-runtime` tests + `tests/rust_integration/runtime_wasm.rs` | Rust owns the supported runtime scheduling semantics plus the embedded OTF bridge used by the WASM surface. |
| `tests/test_sfnt_subset.c` | covered | `tests/rust_integration/subset.rs` + future `fonttool-subset` tests | Rust covers the supported non-OTF subset execution slice. |
| `tests/test_subset_args.c` | covered | `crates/fonttool-cli/tests/cli_contract.rs` + integration tests | Rust covers the supported parser/dispatch contract and explicit rejection of unsupported flags/paths. |
| `tests/test_ttf_rebuilder.cc` | archive-only | future `fonttool-glyf` / `fonttool-cff` tests | Historical rebuild-internals coverage only. |
| `tests/test_ttf_rebuilder_header.c` | archive-only | future `fonttool-glyf` tests | Historical rebuild-internals coverage only. |

## Retired Native Harness Infrastructure

| Retired file | Status | Notes |
| --- | --- | --- |
| `tests/test_main.c` | archive-only | Native test registration harness removed with the C/C++ suite. |
| `Makefile` | archive-only | Replaced by Rust workspace commands and package scripts. |
| `src/` | archive-only | Removed after all supported surfaces were moved or explicitly deferred. |

## Current Verification Entry Points

Use these instead of the retired native harness:

- `cargo test --workspace`
- `cargo test -p fonttool-cli --test decode`
- `cargo test -p fonttool-cli --test encode`
- `cargo test -p fonttool-cli --test subset`
- `cargo test -p fonttool-cli --test otf_convert`
- `cargo test -p fonttool-cli --test woff`
- `cargo test -p fonttool-cli --test runtime_wasm`
- `cargo test -p fonttool-cli --test validation`
- `pnpm --filter fonttool-wasm test -- --runInBand`
- `pnpm verify:wasm`
