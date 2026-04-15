# Legacy Native Build Notes

This directory records the native C/C++ build and test surface that remains in
the tree for reference and compatibility.

## What Is Retained

- `src/` contains the original native implementation.
- `Makefile` still builds and tests the native harness for historical coverage.
- Some Rust crates call into legacy-native adapters while the remaining slices
  are migrated.

## How To Use It

- Prefer the Rust workspace for day-to-day build and test work:
  `cargo test --workspace`
- Use the native Makefile only when you need compatibility checks, parity
  confirmation, or to compare behavior with the archived implementation.

## What This Means

The native implementation is still useful as reference coverage, but it is no
longer the primary entrypoint for the repository. New work should target the
Rust workspace unless a change explicitly depends on the legacy surface.
