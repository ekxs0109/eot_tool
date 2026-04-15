# Legacy Retirement Notes

The legacy native `src/` tree, Makefile, and native C/C++ harness were removed
from this repository on 2026-04-15.

What remains:

- historical migration context in `docs/superpowers/plans/`
- the support boundary and ownership map in
  `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`
- the historical test-to-Rust mapping in `tests/rust-test-inventory.md`

Day-to-day build, test, and packaging work should use the Rust workspace and
package scripts only.
