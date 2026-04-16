# eot_tool

Standalone Rust-first font toolchain for MTX-compressed EOT decode, TrueType
encode, non-OTF glyph-id subset, and packaged WASM runtime delivery.

The legacy native `src/` tree and Makefile pipeline were retired on
2026-04-15. The Rust workspace, package scripts, and vendored WASM artifacts
are now the only supported repository entrypoints.

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

Migration tracking lives in `tests/rust-test-inventory.md`.
The file-by-file ownership map for supported and explicitly deferred behavior
lives in `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`.

Historical retirement notes live in [legacy/README.md](legacy/README.md).

## Python Verification

```bash
python3 -m venv build/venv
build/venv/bin/python -m pip install -r tests/requirements.txt
```

Python tooling is verification/reference-only. The Rust workspace owns the
supported decode, TrueType encode, non-OTF subset, and Rust-facing runtime/WASM
contract surfaces.

Stable verifier entrypoints:

```bash
build/venv/bin/python tests/verify_font.py testdata/OpenSans-Regular.ttf
build/venv/bin/python tests/compare_fonts.py \
  testdata/OpenSans-Regular.ttf \
  build/out/OpenSans-Regular.roundtrip.ttf
```

If you run the scripts through another Python interpreter, they will exit with a
clear dependency error when `fontTools` is missing instead of crashing on
import.

## Decode

`fonttool decode <input.eot|input.fntdata> <output.ttf>` is currently supported
for the Rust-owned decode slice where the embedded MTX payload decodes through
`block1` without requiring non-empty extra MTX blocks. Within that supported
slice, it parses the EOT header, decodes the MTX payload, transparently removes
PowerPoint-style XOR obfuscation when the `0x10000000` flag is set, rebuilds an
SFNT, and writes a TTF.

Smoke check:

```bash
cargo run -p fonttool-cli --bin fonttool -- \
  decode testdata/font1.fntdata build/out/font1.rust-smoke.ttf
build/venv/bin/python tests/verify_font.py build/out/font1.rust-smoke.ttf
```

The standalone Rust `decode` command still keeps its narrower support boundary.
The wider multi-block MTX bridge currently exists for subset input
reconstruction, not for general decode.

## Encode / Roundtrip

`fonttool encode <input.ttf> <output.eot>` emits an MTX-compressed EOT for the
supported TrueType path.

The Rust MTX encoder now emits backreference-capable LZ streams for all output
blocks and falls back to literal-only output when that would be smaller. This
improves compression relative to the earlier literal-only implementation, but it
does not yet guarantee byte-for-byte or size parity with legacy or PowerPoint
producers.

### Runtime Thread Control

`EOT_TOOL_THREADS` controls Rust-owned encode/runtime parallelism.

- default: unset (or invalid) uses the platform hardware concurrency
- `EOT_TOOL_THREADS=1`: strict serial mode for debugging/regression checks
- `EOT_TOOL_THREADS=<N>`: requests `N` worker threads

Current supported/unsupported encode boundaries:

- TrueType input to `.eot`: supported
- TrueType input to `.fntdata`: unsupported
- `OTF(CFF/CFF2)` input: unsupported

Converged runtime behavior across the supported TrueType path:

- `cvt`: preserved on encode/decode
- `hdmx`: preserved on encode/decode, including shared trailing advance widths
- `VDMX`: dropped from the current Rust TrueType encode path

Roundtrip verification example:

```bash
cargo run -p fonttool-cli --bin fonttool -- \
  encode testdata/OpenSans-Regular.ttf build/out/OpenSans-Regular.eot
cargo run -p fonttool-cli --bin fonttool -- \
  decode build/out/OpenSans-Regular.eot build/out/OpenSans-Regular.roundtrip.ttf
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

## Subset

Rust-owned subset support currently covers the non-OTF glyph-id path:

- `.eot` / `.fntdata`: `decode -> sfnt subset -> encode`
- `.ttf`: `sfnt load -> subset -> encode`
- `.otf`: unsupported in the Rust contract

The supported Rust subset path accepts `.eot`, `.fntdata`, and `.ttf` inputs
with `--glyph-ids`, rebuilds the supported subset tables (`cmap`, `glyf`,
`loca`, `hhea`, `hmtx`, and `maxp`), and emits warnings when `HDMX` or `VDMX`
are dropped from the subset output. `.fntdata` output stays wrapped with the
PPT XOR flag so the CLI can preserve the expected container shape.

For `.eot` / `.fntdata` input, the Rust subset path reconstructs a real SFNT
from the current Rust-encoded multi-block MTX payload (`block1` + `block2` +
`block3`) before handing the font to HarfBuzz.

Extra-table behavior across the supported non-OTF subset path is:

- `cmap`: rebuilt for the selected glyph subset
- `cvt`: retained when present in the decoded/subset SFNT
- `glyf` / `loca`: rebuilt for the selected glyph subset
- `hhea` / `hmtx`: rebuilt for the selected glyph subset
- `hdmx`: dropped during subset with a warning
- `VDMX`: dropped during subset with a warning

The Rust CLI does not currently support `--text`, `--unicodes`, or
`--keep-gids` for non-OTF input.

Subset verification example:

```bash
cargo run -p fonttool-cli --bin fonttool -- \
  subset testdata/OpenSans-Regular.ttf build/out/OpenSans-Regular.subset.eot --glyph-ids 0,1,2
cargo run -p fonttool-cli --bin fonttool -- \
  decode build/out/OpenSans-Regular.subset.eot build/out/OpenSans-Regular.subset.ttf
build/venv/bin/python tests/compare_fonts.py \
  --require-subset-core-tables \
  build/out/OpenSans-Regular.subset.ttf
```

## OTF/CFF Deferred Boundary

`OTF(CFF/CFF2)` encode, subset, and variable-instance export remain explicitly
unsupported in the Rust contract. The CLI/runtime/WASM entrypoints surface
Phase 3-owned errors instead of silently routing through a hidden native
backend.

## Browser / WASM Runtime

The supported browser/WASM surface is the Rust-owned `fonttool-wasm` package
plus the vendored runtime artifacts under
`packages/fonttool-wasm/vendor/wasm`.

Canonical verification entrypoints:

```bash
pnpm --filter fonttool-wasm build
pnpm --filter fonttool-wasm test -- --runInBand
pnpm verify:wasm
```

`pnpm verify:wasm` runs `tests/verify_wasm_artifacts.sh`, which:

- verifies the required vendored single-thread and pthread artifacts exist
- stages them through the package contract
- builds the `fonttool-wasm` package entrypoint
- probes the staged runtimes through the package loader contract

Expected vendored artifacts:

- `fonttool-wasm.js` and `fonttool-wasm.wasm`: baseline single-thread runtime
- `fonttool-wasm-pthreads.js` and `fonttool-wasm-pthreads.wasm`: pthread
  runtime
- optional `fonttool-wasm-pthreads*.worker.js`: toolchain-emitted worker helper

These are separate outputs on purpose. Host code should pick the artifact that
matches the deployment environment instead of expecting one binary to toggle
thread support at runtime.

Browser deployment notes for the pthread variant:

- requires `SharedArrayBuffer`, which in browsers usually means
  cross-origin-isolated delivery
- typical headers are `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`
- the single-thread runtime remains the compatibility fallback when those
  constraints are not available

## Benchmark Web App

The benchmark scaffold lives in `apps/benchmark-web` and uses Vite + React +
TypeScript with a shadcn-compatible project surface. It is wired to the
workspace `fonttool-wasm` package, with runtime loading isolated under
`src/lib/fonttool/` instead of UI components.

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

## Fixtures

Recreate the `wingdings3.eot` fixture from a local `font2.fntdata` copy when
needed:

```bash
cp /path/to/font2.fntdata testdata/wingdings3.eot
chmod 0644 testdata/wingdings3.eot
```
