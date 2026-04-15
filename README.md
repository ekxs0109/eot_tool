# eot_tool

Standalone Rust-first font toolchain for MTX-compressed EOT decode/encode,
with archived native compatibility paths for subset execution and
`OTF(CFF/CFF2)` conversion.

## Rust-First Verification

Use the Rust workspace first. On macOS, `cargo test --workspace` includes the
Swift/CoreText validation path through `tests/rust_integration/validation.rs`.
The focused validation target remains useful when you only want that slice.

```bash
cargo build --workspace
cargo test --workspace
cargo test -p fonttool-cli --test validation
cargo run -p fonttool-cli --bin fonttool -- --help
cargo run -p fonttool-cli --bin fonttool -- decode testdata/font1.fntdata build/out/font1.rust-smoke.ttf
build/venv/bin/python tests/verify_font.py testdata/OpenSans-Regular.ttf
```

Fuzz smoke build (`fuzz/rust-toolchain.toml` pins nightly; when Homebrew's Rust
toolchain is ahead of `rustup` on `PATH`, prepend the rustup shims first):

```bash
cd fuzz
PATH="$(brew --prefix rustup)/bin:$PATH" rustup run nightly cargo fuzz build
```

Rust is the primary build and test path. The legacy native harness remains in
tree for compatibility and reference coverage, but it is no longer the default
entrypoint.

See [legacy/README.md](legacy/README.md) for the archived native-harness
notes.

Migration tracking lives in `tests/rust-test-inventory.md`.
The file-by-file Phase 1 ownership map for `parallel_runtime`,
`table_policy`, and the remaining shellout/deferred adapter callsites lives in
`docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`.

## Python Verification

```bash
python3 -m venv build/venv
build/venv/bin/python -m pip install -r tests/requirements.txt
```

Python tooling is verification/reference-only. The Rust workspace primarily
owns the migrated decode, TrueType encode, and Rust-facing runtime/WASM
diagnostics and contract slices. Subset execution and `OTF(CFF/CFF2)`
conversion remain archived native compatibility paths until later rewrite
phases.

Stable verifier entrypoints:

```bash
build/venv/bin/python tests/verify_font.py testdata/OpenSans-Regular.ttf
build/venv/bin/python tests/compare_fonts.py \
  testdata/OpenSans-Regular.ttf \
  build/out/OpenSans-Regular.roundtrip.ttf
```

If you run the scripts through another Python interpreter, they will exit with a
clear dependency error when `fontTools` is missing instead of crashing on import.

Legacy compatibility note: the sections below still include `./build/fonttool`,
`make verify-*`, and a few `make test TESTCASE=...` examples because those
paths remain useful for parity checks. Treat them as archived compatibility
commands unless a section explicitly says otherwise.

## Decode

`fonttool decode <input.eot|input.fntdata> <output.ttf>` parses the EOT header,
decodes the MTX payload, transparently removes PowerPoint-style XOR obfuscation
when the `0x10000000` flag is set, rebuilds an SFNT, and writes a TTF.

Reproducible manual check recorded on 2026-04-08:

```bash
./build/fonttool decode testdata/wingdings3.eot build/out/wingdings3.ttf
Decoded testdata/wingdings3.eot -> build/out/wingdings3.ttf (32120 bytes)

ls -lh build/out/wingdings3.ttf
-rw-r--r--  1 ... 31K ... build/out/wingdings3.ttf

build/venv/bin/python tests/verify_font.py build/out/wingdings3.ttf
font structure verified
```

Make target:

```bash
make verify-decode
```

## Encode / Roundtrip

`fonttool encode <input.ttf> <output.eot>` emits an MTX-compressed EOT.

The Rust CLI currently treats PowerPoint-compatible `.fntdata` output as
Phase 2-owned work. The archived native compatibility binary `./build/fonttool`
still provides that path; the Rust-owned encode boundary does not currently set
the PowerPoint XOR flag or apply the `0x50` obfuscation layer.

### Runtime Thread Control

`EOT_TOOL_THREADS` controls Rust-owned encode/runtime parallelism.

- default: unset (or invalid) uses the platform hardware concurrency
- `EOT_TOOL_THREADS=1`: strict serial mode for debugging/regression checks
- `EOT_TOOL_THREADS=<N>`: requests `N` worker threads

The Rust CLI currently supports TrueType input in the forward-supported
boundary. `OTF(CFF/CFF2)` encode and subset commands shown below are archived
compatibility flows that still require `./build/fonttool`.

OTF/CFF parity check against a reproducible local `fonttools` save (fixture:
`testdata/aipptfonts/香蕉Plus__20220301185701917366.otf`):

```bash
mkdir -p build/out
./build/fonttool encode testdata/aipptfonts/香蕉Plus__20220301185701917366.otf build/out/0213-parity.eot
./build/fonttool decode build/out/0213-parity.eot build/out/0213-fixed.ttf
build/venv/bin/python -c "from fontTools.ttLib import TTFont; f=TTFont('build/out/0213-fixed.ttf'); f.save('build/out/0213-fonttools-saved.ttf'); f.close()"
build/venv/bin/python tests/test_fonttools_parity.py \
  build/out/0213-fixed.ttf \
  build/out/0213-fonttools-saved.ttf
```

Current expected residual difference for this comparison is:

- `head`: checksum/timestamp serialization bytes differ

Converged runtime behavior:

- `cvt`: preserved on encode/decode
- `hdmx`: preserved on encode/decode, including shared trailing advance widths
- `VDMX`: dropped from the current Rust TrueType encode path

Archived native compatibility behavior still covers warning parity for dropped
`VDMX`/`HDMX` tables during the old subset/encode paths; those warnings are not
part of the current Rust-owned Phase 1 contract.

Subset architecture for the archived native compatibility flow is:

- `.eot` / `.fntdata`: `decode -> sfnt subset -> encode`
- `.ttf`: `sfnt load -> subset -> encode`
- `.otf`: `native CFF/CFF2 conversion -> subset -> encode`

The archived subset rebuild is HarfBuzz-backed, then re-serialized through the
existing C runtime. Extra-table behavior across the merged paths is:

- `cvt`: retained when present in the decoded/subset SFNT
- `hdmx`: preserved on encode/decode, but dropped during subset with a warning
- `VDMX`: dropped during encode/subset with a warning

`--keep-gids` depends on HarfBuzz retain-gids support. The native test suite
covers that behavior explicitly so unsupported builds fail instead of silently
renumbering glyphs.

Roundtrip verification example:

```bash
./build/fonttool encode testdata/OpenSans-Regular.ttf build/out/OpenSans-Regular.eot
./build/fonttool decode build/out/OpenSans-Regular.eot build/out/OpenSans-Regular.roundtrip.ttf
build/venv/bin/python tests/compare_fonts.py \
  testdata/OpenSans-Regular.ttf \
  build/out/OpenSans-Regular.roundtrip.ttf
```

Expected verifier output:

```text
required tables match exactly
```

When `cvt` or `hdmx` exist on both fonts, the same verifier also checks that
those preserved tables still match byte-for-byte.

PowerPoint-compatible `.fntdata` example:

```bash
./build/fonttool encode testdata/OpenSans-Regular.ttf build/out/OpenSans-Regular.fntdata
./build/fonttool decode build/out/OpenSans-Regular.fntdata build/out/OpenSans-Regular.fntdata.roundtrip.ttf
```

Make target:

```bash
make verify-roundtrip
```

Archived native-compatibility subset verification example:

```bash
./build/fonttool subset testdata/wingdings3.eot build/out/wingdings3-subset.eot --glyph-ids 0,1,2
./build/fonttool decode build/out/wingdings3-subset.eot build/out/wingdings3-subset.ttf
build/venv/bin/python tests/verify_font.py build/out/wingdings3-subset.ttf
build/venv/bin/python tests/compare_fonts.py \
  --require-subset-core-tables \
  build/out/wingdings3-subset.ttf
```

Static CFF subset example:

```bash
./build/fonttool subset testdata/cff-static.otf build/out/cff-static-subset.eot --text ABC
```

CFF2 instance subset example:

```bash
./build/fonttool subset testdata/cff2-variable.otf build/out/cff2-bold-subset.fntdata \
  --text ABC --variation wght=700
```

If `--variation` is passed for a non-variable input, the command fails instead
of silently ignoring the request.

## CFF2 Instancing

`CFF2` instance export remains an archived/native compatibility path during the
current rewrite phase. The historical conversion pipeline is:

1. validates the user axis-tag map
2. clamps to `fvar`
3. applies `avar`
4. resolves a full ordered axis location
5. instantiates outlines/metrics before `cu2qu` and TT rebuild

Variation tables such as `CFF2`, `fvar`, `avar`, `HVAR`, `MVAR`, `VVAR`,
`cvar`, and `gvar` were historically dropped from the rebuilt embedded-font
output in that archived compatibility flow.

## Conversion Tuning

The native cubic-to-quadratic conversion uses a conservative default tolerance.
`cu2quMaxError` is treated as an advanced option in the conversion modules and
is intentionally not auto-relaxed on failure.

## Browser / WASM API

The browser-oriented buffer API is exported from `src/wasm_api.{h,cc}`:

```c
const char *wasm_runtime_thread_mode(void);

eot_status_t wasm_convert_otf_to_embedded_font(const uint8_t *input,
                                               size_t input_size,
                                               const char *output_kind,
                                               const char *variation_axes,
                                               wasm_buffer_t *out);
```

- `wasm_runtime_thread_mode()`: compile-time build metadata string, either
  `"single-thread"` or `"pthreads"`
- `input` / `input_size`: memory buffer containing `.ttf` or `.otf`
- `output_kind`: `"eot"` or `"fntdata"`
- `variation_axes`: optional axis-tag map such as `"wght=700"`
- `out`: owned output buffer; release it with `wasm_buffer_destroy(...)`

Focused native coverage:

```bash
make test TESTCASE=test_wasm_runtime_mode_constant_is_exposed
make test TESTCASE=test_browser_wasm_api_converts_cff2_instance
```

Current Rust-facing bridge coverage for the staged rewrite:

```bash
cargo test -p fonttool-runtime
cargo test -p fonttool-wasm
cargo test --test runtime_wasm
```

These Rust tests cover the staged `fonttool-runtime` / `fonttool-wasm` API
surface, Rust-owned scheduling semantics, and the explicit unsupported boundary
for deferred `OTF(CFF/CFF2)` conversion. They do not yet replace the legacy
native WASM buffer ABI checks or the native-only variable-font conversion
success path.

The Makefile exposes explicit Emscripten build variants:

```bash
make wasm
make wasm-single
make wasm-pthreads
make verify-wasm-artifacts
```

Expected artifacts under `build/`:

- `fonttool-wasm.js` and `fonttool-wasm.wasm`: baseline single-thread build
- `fonttool-wasm-pthreads.js` and `fonttool-wasm-pthreads.wasm`: pthreads build
- optional `fonttool-wasm-pthreads*.worker.js`: toolchain-emitted pthread worker
  helper, depending on the Emscripten version and flags in use

These are separate outputs on purpose. Host code should pick the artifact that
matches the deployment environment instead of expecting one binary to toggle
thread support at runtime.

Artifact verification is wired into the build flow:

```bash
make verify-wasm-artifacts
```

That target builds both WASM outputs first, then runs
`tests/verify_wasm_artifacts.sh` to check required files and, when `node` is
available, load each generated JS module and verify that
`wasm_runtime_thread_mode()` reports the expected exact mode string.

Browser deployment notes for `make wasm-pthreads`:

- requires `SharedArrayBuffer`, which in browsers usually means
  cross-origin-isolated delivery

## Benchmark Web App

The benchmark scaffold lives in `apps/benchmark-web` and uses Vite + React +
TypeScript with a shadcn-compatible project surface. It is wired to the
workspace `fonttool-wasm` package, with runtime loading isolated under
`src/lib/fonttool/` instead of UI components, and keeps the full shadcn UI
composition work for a later task.

Install workspace dependencies once from the repo root:

```bash
npm install
```

Run the benchmark app in development:

```bash
npm run dev:benchmark-web
```

Build only the benchmark app:

```bash
npm run build:benchmark-web
```

Run the root workspace build and validation entrypoints:

```bash
npm run build
npm run test
npm run pack-check
```

Package-level validation for `fonttool-wasm`:

```bash
npm run build --workspace fonttool-wasm
npm test --workspace fonttool-wasm
npm run test:node-smoke --workspace fonttool-wasm
node packages/fonttool-wasm/scripts/pack-check.mjs
```

The current scaffold is intentionally minimal. It establishes the app package,
shadcn-compatible aliases/utilities, runtime boundary, benchmark-oriented
component structure, and a clean build target without starting the full UI
composition or benchmark polish work.
- typical headers are `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`
- the single-thread build remains the compatibility fallback when those
  constraints are not available

## Swift CoreText Validation

On macOS, the formal Rust-first CoreText acceptance check is:

```bash
cargo test -p fonttool-cli --test validation
```

That test decodes `testdata/font1.fntdata` through the current Rust CLI decode
path and invokes the Swift probe in `tests/macos-swift` on the produced TTF.

The repository-level Swift probe remains directly runnable for manual smoke
checks:

```bash
swift run --package-path tests/macos-swift FonttoolCoreTextProbe testdata/OpenSans-Regular.ttf
```

Expected output:

```text
coretext font accepted
```

This complements the Python verifier entrypoint while keeping the Swift
CoreText probe as a standalone manual diagnostic when needed.

## Fixtures

```bash
make fixtures
```

By default this copies the workspace-root `font2.fntdata` into
`eot_tool/testdata/wingdings3.eot`, with paths resolved relative to this
Makefile, and normalizes the fixture mode to non-executable.

Override the source path when needed:

```bash
make fixtures FIXTURE_SOURCE=/path/to/font2.fntdata
```
