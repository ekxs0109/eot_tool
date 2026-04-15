# Fonttool Src Retirement Design

## Context

The repository has already moved to a Rust-first architecture for the primary
workspace, CLI, and most verification entrypoints. The `main` branch now builds
and tests through the Rust workspace, and a large part of the legacy native
test surface has already been migrated into Rust tests.

However, the repository still contains a native implementation under `src/`
that remains responsible for several capabilities either directly or through
legacy adapter boundaries:

- runtime scheduling and thread bookkeeping
- parts of `OTF(CFF/CFF2)` inspection and variation handling
- `cu2qu`, `tt_rebuilder`, and `otf_convert` internals
- some subset/backend glue
- some legacy-only verification paths

The user wants the repository to reach a stronger terminal state than
"Rust-first": the old native implementation under `src/` should be deleted,
but only after the Rust implementation fully replaces it without reducing
currently supported functionality or verification coverage.

This means `src/` retirement is not a cleanup task. It is the final acceptance
step of a complete Rust replacement program.

## Goal

Replace all remaining production-relevant native implementation in `src/` with
Rust-owned equivalents, preserve the currently supported behavior and
verification surface, and only then remove `src/` from the repository.

## Non-Goals

- Deleting `src/` while important runtime or format behavior still depends on it
- Accepting behavior loss in order to simplify the rewrite
- Keeping thin native adapters indefinitely once Rust-owned replacements exist
- Rewriting external Python or Swift validation tools into Rust
- Preserving every historical native-only helper API if it no longer maps to a
  supported product surface

## Decision Summary

The repository should move from "Rust-first with native fallback" to "fully
Rust-owned implementation with external validation sidecars". The work should
proceed in phases, but deletion of `src/` only happens after all remaining
runtime-relevant native capabilities have been reimplemented in Rust and proven
through tests and validation.

The replacement program should prioritize:

1. Rust ownership of shared low-level behavior that influences many call sites
2. Rust ownership of the full `OTF(CFF/CFF2)` conversion chain
3. Rust ownership of the remaining native-facing entrypoints and adapters
4. Final removal of `src/`, legacy Makefile build paths, and native harness
   dependencies that are no longer needed

## Replacement Standard

Deleting `src/` is allowed only when all of the following are true:

- production CLI behavior is implemented through Rust-owned code paths
- runtime/WASM-facing behavior is implemented through Rust-owned code paths
- remaining native-only tests have either:
  - been migrated into Rust/Swift/Python verification, or
  - been explicitly archived because they only test removed historical helpers
- `tests/rust-test-inventory.md` no longer lists production-relevant native
  implementation dependencies
- repository build/test docs do not require `src/`
- `cargo test --workspace` remains the primary verification command and passes

If any currently supported behavior still requires `src/`, then `src/` cannot
be deleted.

## Phase Plan

### Phase 1: Replace Shared Native Support Boundaries

Target the native code that currently shapes multiple higher-level paths:

- `parallel_runtime`
- `table_policy`
- remaining encode/decode support boundaries that still assume native helpers

Why first:

- these components influence many modules
- they reduce coordination risk for later replacement work
- they are easier to validate in isolation than the full OTF chain

Expected outcome:

- Rust owns runtime scheduling semantics that current tests care about
- Rust tests replace the remaining native runtime parity checks
- legacy adapter boundaries no longer need native runtime state

### Phase 2: Replace the OTF/CFF Conversion Chain

Target the remaining native `OTF(CFF/CFF2)` implementation path:

- `cff_reader`
- `cff_variation`
- `cu2qu`
- `tt_rebuilder`
- `otf_convert`

Why second:

- this is the densest and highest-risk remaining native implementation
- it currently blocks deletion of large parts of `src/`
- it underlies several remaining partial/deferred test areas

Expected outcome:

- Rust owns static CFF conversion
- Rust owns CFF2 variation-location resolution and instance export
- Rust owns cubic-to-quadratic conversion and TrueType rebuild internals
- Rust parity tests replace the remaining product-relevant native OTF tests

### Phase 3: Replace Native Entrypoints and Delete `src/`

Once Phases 1 and 2 are complete, replace the remaining entry and glue layers:

- `subset_backend_harfbuzz` native glue, if still present
- `wasm_api`
- `main.c`
- any remaining native bridge-only wrapper code

Then:

- remove `src/`
- remove obsolete Makefile/native build logic
- archive or delete leftover native harness files that are no longer needed
- reduce `tests/rust-test-inventory.md` to migration summary + archive notes

## Architecture Changes Required

### 1. Runtime Ownership

Rust must own task scheduling semantics rather than treating runtime
diagnostics as placeholders. The current runtime crate exposes static metadata
and limited conversion behavior; it must grow to represent:

- requested thread count
- effective thread count
- deterministic execution mode selection
- failure ordering semantics where tests currently care

This is the Rust replacement target for the old `parallel_runtime` surface.

### 2. CFF/CFF2 Ownership

Rust must grow from "inspection + adapter boundary" to a real owned model for:

- CFF table parsing
- CFF2 variation tables and axis resolution
- glyph outline extraction / intermediate geometry representation
- instance export behavior currently tested through native code

This requires clear Rust data models rather than direct translation of the
native file structure.

### 3. Conversion Pipeline Ownership

The conversion path should become:

- parse input in Rust
- resolve variation state in Rust
- convert outlines in Rust
- rebuild target SFNT in Rust
- encode/serialize in Rust

HarfBuzz may remain as a dependency where the architecture already allows it,
but it must remain behind a narrow Rust crate boundary and not be reintroduced
as a new native center.

### 4. Entry Surface Ownership

All repository entry surfaces that currently matter should eventually resolve to
Rust-owned orchestration:

- CLI commands
- runtime entrypoints
- WASM-facing conversion APIs
- repository verification commands

This does not require removing Python or Swift validation. It requires those
layers to validate Rust outputs instead of native outputs.

## Testing and Validation Strategy

### Rust Integration Becomes Canonical

Each remaining native test area should either become:

- a Rust unit test
- a Rust integration test
- an external validation test driven from Rust

The canonical acceptance stack should remain:

1. Rust unit tests
2. Rust integration tests
3. Python/fonttools verification
4. Swift/CoreText validation

### Migration Rules

When migrating remaining native-only areas:

- only claim coverage for behavior the Rust path actually owns
- do not retain ignored tests that merely document known failures as if they
  were coverage
- keep inventory wording precise about current boundaries
- prefer real fixture validation for product behavior
- prefer synthetic fixtures only for tightly-scoped structural invariants

### Deletion Gate for Native Tests

A native test file can be removed or archived when:

- its product-relevant behavior is covered elsewhere, and
- the replacement test executes a Rust-owned path, or
- the old test only checks a historical helper API that no longer maps to a
  supported product surface

## Risks

### 1. Partial Rust Ownership That Still Hides Native Dependency

Risk:
some Rust tests may appear to cover a behavior while still depending on native
adapters under the hood.

Mitigation:
track not only tests, but actual implementation ownership boundaries. Deletion
of `src/` requires implementation replacement, not merely test replacement.

### 2. OTF Chain Is Large and Tightly Coupled

Risk:
`cff_reader` / `cff_variation` / `cu2qu` / `tt_rebuilder` / `otf_convert`
form a coupled chain where incomplete replacement can stall progress.

Mitigation:
treat them as one architectural slice in planning, with internal decomposition
for execution but one acceptance boundary.

### 3. Runtime Semantics Drift

Risk:
threading/runtime behavior may subtly drift if Rust replaces the old runtime
without equivalent tests.

Mitigation:
migrate runtime parity checks before deleting native runtime code, and prefer
deterministic fixture-style tests for the semantics that matter.

### 4. Premature `src/` Deletion

Risk:
removing `src/` too early could strand still-supported workflows.

Mitigation:
make `src/` deletion the last step, gated by implementation ownership and full
workspace verification.

## Recommended Execution Strategy

The next implementation plan should be organized around the three phases above,
with the first plan targeting Phase 1 only unless the work is split into
multiple coordinated plans.

That plan should:

- identify exactly which current crates need to absorb runtime/table-policy
  ownership
- map each remaining native file in scope to a Rust destination
- define the tests that must pass before Phase 1 is considered complete
- explicitly list the still-blocked native files that prevent `src/` deletion

## Acceptance for This Design

This design is accepted when the implementation plan:

- treats `src/` deletion as the final result, not an immediate task
- preserves current functionality and validation surfaces
- phases the work so early wins unlock later deletion safely
- names the remaining native-owned slices explicitly instead of hiding them
