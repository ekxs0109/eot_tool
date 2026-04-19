# eot_tool

Standalone Rust-first font toolchain for MTX-compressed EOT decode, TrueType
and CFF-aware encode/convert flows, non-OTF glyph-id subset plus OTF text
subset, WOFF/WOFF2 source materialization, and packaged WASM runtime delivery.

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

Historical retirement notes live in [legacy/README.md](legacy/README.md).

## Python Verification

```bash
python3 -m venv build/venv
build/venv/bin/python -m pip install -r tests/requirements.txt
```

Python tooling is verification/reference-only. The Rust workspace owns the
supported decode, TrueType/CFF encode and convert flows, non-OTF glyph-id
subset plus OTF text subset, and Rust-facing runtime/WASM contract surfaces.

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

`fonttool encode <input> <output.eot>` emits an MTX-compressed EOT for the
supported Rust-owned encode surface.

The Rust MTX encoder now applies Java-style local copy heuristics for MTX/LZ
compression across the shared compressor path, with a literal-only fallback
when that would be smaller. The tracked PowerPoint case 7 sample is used as an
acceptance check for output-size parity, but exact byte-for-byte parity with
legacy producers is still not guaranteed.

### Runtime Thread Control

`EOT_TOOL_THREADS` controls Rust-owned encode/runtime parallelism.

- default: unset (or invalid) uses the platform hardware concurrency
- `EOT_TOOL_THREADS=1`: strict serial mode for debugging/regression checks
- `EOT_TOOL_THREADS=<N>`: requests `N` worker threads

Current supported/unsupported encode boundaries:

- TrueType SFNT input to `.eot`: supported
- TrueType `WOFF/WOFF2` input to `.eot`: supported after source materialization
- static `OTF/CFF` input to `.eot`: supported
- variable `OTF/CFF2` input to `.eot`: supported, with optional `--variation`
- any input to `.fntdata`: unsupported

For `.eot` output, `fonttool encode` accepts embedded-output controls:

- `--payload-format mtx|sfnt`
- `--xor on|off`
- `--eot-version v1|v2`

Defaults remain `mtx + off + v2`. Passing these flags with `.ttf` output is a
contract error.

Current encode-output behavior and locked-in roundtrip baselines across the
Rust-owned TrueType path:

- `cvt`: preserved in the current Rust encode output and supported decode
  baselines
- `hdmx`: preserved in the current Rust encode output and Rust-owned roundtrip
  coverage, including shared trailing advance widths
- `VDMX`: dropped from the current Rust TrueType encode path

Public roundtrip caveat:

The public `fonttool decode` command still has the narrower supported boundary
described in the decode section above. A generic `fonttool encode <ttf>`
followed by `fonttool decode <eot>` is not yet a supported general-purpose
roundtrip for outputs whose MTX payload requires non-empty extra blocks.

The encode integration coverage currently locks in two narrower baselines:

- compression non-regression for general TrueType encode output, including a
  tracked sample with non-empty `block2` and `block3`
- a specific PPTX-derived sample that the current CLI `decode` command can
  decode into today's block1-shaped SFNT baseline

Public smoke checks:

```bash
cargo run -p fonttool-cli --bin fonttool -- \
  encode testdata/OpenSans-Regular.ttf build/out/OpenSans-Regular.eot
cargo run -p fonttool-cli --bin fonttool -- \
  decode testdata/font1.fntdata build/out/font1.rust-smoke.ttf
build/venv/bin/python tests/verify_font.py build/out/font1.rust-smoke.ttf
```

The PPTX-derived CLI roundtrip regression is covered in the Rust integration
tests; it does not imply general multi-block reconstruction parity for arbitrary
encode output yet.

## Convert

`fonttool convert <input> <output.ttf> --to ttf` is supported for the
Rust-owned conversion surface:

- static `OTF/CFF` input: supported
- variable `OTF/CFF2` input: supported, with optional `--variation`
- TrueType `WOFF/WOFF2` input: supported after source materialization
- static `WOFF(CFF)` input: supported after source materialization

Shared source loading materializes `WOFF/WOFF2` inputs into canonical SFNT
bytes before flavor detection, so `convert` and `encode` follow the same
content-based routing instead of branching on filename extensions.

## Subset

Rust-owned subset support currently covers two paths:

- non-OTF glyph-id path:
  - `.eot` / `.fntdata`: `decode -> sfnt subset -> encode`
  - `.ttf`: `sfnt load -> subset -> encode`
- OTF text-subset path:
  - static `OTF/CFF`: supported with `--text`
  - variable `OTF/CFF2`: supported with `--text`, plus optional `--variation`

The supported Rust subset path accepts `.eot`, `.fntdata`, and `.ttf` inputs
with `--glyph-ids`, and `OTF/CFF/CFF2` inputs with `--text`. It rebuilds the
supported subset tables (`cmap`, `glyf`, `loca`, `hhea`, `hmtx`, and `maxp`)
for the non-OTF path, and emits warnings when `HDMX` or `VDMX` are dropped
from subset output. `.fntdata` output stays wrapped with the PPT XOR flag so
the CLI can preserve the expected container shape.

For `.eot` / `.fntdata` input, the Rust subset path reconstructs a real SFNT
from the current Rust-encoded multi-block MTX payload (`block1` + `block2` +
`block3`) before handing the font to the Rust-owned glyph-id subsetter in
`fonttool-subset`. `OTF/CFF/CFF2` text subsetting stays in the Rust-owned
`fonttool-cff` path.

Extra-table behavior across the supported non-OTF subset path is:

- `cmap`: rebuilt for the selected glyph subset
- `cvt`: retained when present in the decoded/subset SFNT
- `glyf` / `loca`: rebuilt for the selected glyph subset
- `hhea` / `hmtx`: rebuilt for the selected glyph subset
- `hdmx`: dropped during subset with a warning
- `VDMX`: dropped during subset with a warning

The Rust CLI does not currently support `--text`, `--unicodes`, or
`--keep-gids` for non-OTF input, and it does not support `--glyph-ids` for
`OTF/CFF/CFF2` input.

For `.eot` / `.fntdata` output, `fonttool subset` accepts the same
embedded-output controls:

- `--payload-format mtx|sfnt`
- `--xor on|off`
- `--eot-version v1|v2`

Defaults remain `mtx + off + v2` for `.eot`. `.fntdata` output keeps the
PowerPoint-compatible XOR wrapper by default unless `--xor off` is passed
explicitly. Passing embedded-output flags with `.ttf` output is a contract
error.

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

## Remaining Deferred Boundary

The Rust contract still defers a few surfaces:

- `.fntdata` encode remains Phase 2-owned and is still rejected by the CLI
- non-OTF subset still only supports `--glyph-ids`
- `OTF/CFF/CFF2` subset still only supports `--text`
- the current non-OTF subsetter is limited to the Rust-owned TrueType glyph
  rebuild path and does not yet expose `--text`, `--unicodes`, or `--keep-gids`

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
