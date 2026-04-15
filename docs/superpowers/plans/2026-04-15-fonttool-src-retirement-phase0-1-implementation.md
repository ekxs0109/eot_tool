# Fonttool Src Retirement Phase 0-1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the support matrix and replace the remaining shared native support boundaries so the repository can continue toward full `src/` retirement without hidden legacy runtime ownership.

**Architecture:** This phase does not delete `src/`. It makes the deletion boundary explicit and moves the shared runtime/support layer into Rust ownership first. The key outputs are: (1) a committed support matrix that classifies every remaining legacy-tested behavior, (2) Rust-owned runtime semantics that replace `parallel_runtime`, and (3) removal of Rust production/runtime shellouts that still depend on `build/fonttool` for supported behavior in the Phase 0/1 scope.

**Tech Stack:** Rust workspace (`cargo`), existing Rust crates (`fonttool-runtime`, `fonttool-wasm`, `fonttool-cli`, `fonttool-harfbuzz`, `fonttool-cff`), Python/fonttools verification, Swift/CoreText validation

---

## File Structure Map

### Existing files likely to change

- `tests/rust-test-inventory.md`
  Migration ledger and current source of truth for remaining partial/deferred native areas.
- `README.md`
  May need small Phase 0/1 clarifications after the support matrix is introduced.
- `crates/fonttool-runtime/src/lib.rs`
  Must grow from placeholder diagnostics into Rust-owned runtime scheduling semantics.
- `crates/fonttool-wasm/src/lib.rs`
  Must remain aligned with `fonttool-runtime` once runtime semantics become real.
- `tests/rust_integration/runtime_wasm.rs`
  Will absorb migrated runtime parity expectations.
- `crates/fonttool-cli/src/main.rs`
  May need to stop routing supported behavior through legacy binary paths covered by this phase.
- `crates/fonttool-harfbuzz/src/lib.rs`
  May need to stop delegating runtime-relevant behavior through native shellouts in Phase 1 scope.
- `crates/fonttool-cff/src/lib.rs`
  May need interface changes only where Phase 1 removes shellout/runtime ownership ambiguity; full CFF replacement is explicitly out of scope for this plan.

### New files likely to be created

- `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`
  Phase 0 source-of-truth matrix for supported vs unsupported vs archival legacy behavior.

## Task 1: Commit the Support Matrix Source of Truth

**Files:**
- Create: `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`
- Modify: `tests/rust-test-inventory.md`

- [ ] **Step 1: Build the first support matrix draft from current sources**

Required inputs to classify:
- `tests/rust-test-inventory.md`
- current CLI/runtime/WASM integration tests
- Python and Swift validation entrypoints
- README-documented workflows

Matrix columns:
- legacy/native surface
- current user-visible/support status: `supported` | `unsupported` | `archive-only`
- current implementation owner: `rust` | `legacy` | `mixed`
- required replacement before `src/` deletion: `yes` | `no`
- replacement destination / notes

- [ ] **Step 2: Save the support matrix document**

Write `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md` with explicit rows for at least:
- CLI `decode`, `encode`, `subset`
- runtime/WASM conversion entrypoints
- MTX/LZ/VDMX/HDMX/CVT behavior
- OTF/CFF/CFF2 conversion and variation behavior
- legacy-only flags/helper APIs such as `--keep-gids`, `--unicodes`, native-only helper init APIs
- test harness infrastructure files (`test_main.c`, `verify_wasm_artifacts.sh`)

- [ ] **Step 3: Tighten `tests/rust-test-inventory.md` to reference the support matrix**

Update the inventory intro or a short note so future migration decisions point to the new support matrix as the deletion/archival source of truth.

- [ ] **Step 4: Verify the matrix is actionable**

Run:

```bash
rg -n "supported|unsupported|archive-only" docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md
```

Expected:
- every category appears
- the matrix exists and is non-empty

- [ ] **Step 5: Commit the support matrix**

```bash
git add -f docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md tests/rust-test-inventory.md
git commit -m "docs: add fonttool support matrix"
```

## Task 2: Replace Runtime Diagnostics Placeholders with Rust-Owned Scheduling Semantics

**Files:**
- Modify: `crates/fonttool-runtime/src/lib.rs`
- Modify: `crates/fonttool-wasm/src/lib.rs`
- Modify: `tests/rust_integration/runtime_wasm.rs`
- Reference: `tests/test_parallel_runtime.cc`
- Reference: `src/parallel_runtime.cc`
- Reference: `src/parallel_runtime.h`

- [ ] **Step 1: Port the next runtime parity expectations into Rust tests**

Add failing Rust tests for the native runtime semantics that Phase 1 must own:
- requested vs effective thread counts
- invalid override fallback behavior
- requested-mode transitions between `single` and `threaded`
- task-count clamp behavior
- deterministic failure-order behavior
- wait-for-all-started-tasks-before-error behavior
- lowest-failing-index behavior regardless of completion order

Example shape:

```rust
#[test]
fn runtime_reports_requested_and_effective_threads_after_task_clamp() {
    let diagnostics = run_runtime_probe(8, 3);
    assert_eq!(diagnostics.requested_threads, 8);
    assert_eq!(diagnostics.effective_threads, 3);
    assert_eq!(diagnostics.resolved_mode, "threaded");
}
```

- [ ] **Step 2: Run the focused runtime test target to confirm failure**

Run:

```bash
cargo test -p fonttool-cli --test runtime_wasm
```

Expected:
- FAIL on the newly added parity assertions because runtime diagnostics are still placeholder values

- [ ] **Step 3: Define a Rust-owned runtime execution model**

In `crates/fonttool-runtime/src/lib.rs`, add the minimal API needed to model:
- effective thread resolution from requested thread count + task count + explicit single/threaded mode
- deterministic execution metadata
- reusable indexed-task execution semantics that higher layers can call without touching native runtime code
- failure aggregation behavior that matches the migrated runtime parity tests

- [ ] **Step 4: Implement the runtime diagnostics and mode resolution**

Replace placeholder `default_runtime_diagnostics()` behavior with actual Rust-owned semantics matching the migrated tests.

- [ ] **Step 5: Mirror the runtime behavior into `fonttool-wasm`**

Ensure the WASM-facing diagnostics path uses the same runtime source of truth instead of a duplicated placeholder model.

- [ ] **Step 6: Re-run the runtime integration tests**

Run:

```bash
cargo test -p fonttool-cli --test runtime_wasm
```

Expected:
- PASS

- [ ] **Step 7: Commit the runtime ownership slice**

```bash
git add crates/fonttool-runtime/src/lib.rs crates/fonttool-wasm/src/lib.rs tests/rust_integration/runtime_wasm.rs
git commit -m "feat(rust): replace native runtime diagnostics"
```

## Task 3: Remove Phase 1 Shellout Dependence from Rust-Owned Runtime Surfaces

**Files:**
- Modify: `crates/fonttool-runtime/src/lib.rs`
- Modify: `crates/fonttool-harfbuzz/src/lib.rs`
- Modify: `crates/fonttool-cff/src/lib.rs`
- Modify: `crates/fonttool-cli/src/main.rs`
- Modify: `tests/rust_integration/runtime_wasm.rs`
- Modify: `tests/rust-test-inventory.md`

- [ ] **Step 1: Write failing tests that expose remaining Phase 1 shellouts**

Add or extend tests so supported runtime-facing behavior fails if `build/fonttool` is absent, for any surface that Phase 1 claims Rust should own.

Example shape:

```rust
#[test]
fn runtime_owned_path_does_not_require_legacy_binary() {
    let probe = run_without_legacy_binary(...);
    assert!(probe.status.success());
}
```

- [ ] **Step 2: Run the narrow test to confirm failure**

Run the smallest relevant target that demonstrates the shellout dependency.

Expected:
- FAIL because the current path still invokes `build/fonttool`

- [ ] **Step 3: Replace the shellout or narrow the support boundary explicitly**

For each remaining shellout encountered in Phase 1 scope, including shellouts currently reachable through:
- `crates/fonttool-runtime/src/lib.rs`
- `crates/fonttool-cff/src/lib.rs`
- `crates/fonttool-harfbuzz/src/lib.rs`
- `crates/fonttool-cli/src/main.rs`

For each one:
- either replace it with Rust-owned behavior, or
- move it out of the supported/runtime-owned boundary and record that in the support matrix for a later phase

Phase 1 must not leave ambiguous “Rust-owned but still shells out” behavior.

- [ ] **Step 4: Re-run the focused no-shellout tests**

Run the target(s) from Step 2 again.

Expected:
- PASS

- [ ] **Step 5: Update the inventory notes**

Tighten any inventory rows whose “partial” wording assumed runtime shellouts were still acceptable in supported behavior.

- [ ] **Step 6: Commit the shellout-boundary cleanup**

```bash
git add crates/fonttool-runtime/src/lib.rs crates/fonttool-harfbuzz/src/lib.rs crates/fonttool-cff/src/lib.rs crates/fonttool-cli/src/main.rs tests/rust_integration/runtime_wasm.rs tests/rust-test-inventory.md
git commit -m "refactor(rust): remove phase1 legacy shellouts"
```

## Task 4: Replace `table_policy` with Rust-Owned Behavior

**Files:**
- Modify: `crates/fonttool-subset/src/lib.rs`
- Modify if needed: `crates/fonttool-cli/src/main.rs`
- Create or modify: Rust tests replacing `tests/test_table_policy.c`
- Modify: `tests/rust-test-inventory.md`
- Reference: `src/table_policy.c`
- Reference: `src/table_policy.h`
- Reference: `tests/test_table_policy.c`

- [ ] **Step 1: Port the current `table_policy` expectations into Rust tests**

Write failing Rust tests for the still-supported table retention/drop decisions that currently live only in the native harness.

- [ ] **Step 2: Run the focused table-policy tests to confirm failure**

Run the smallest relevant Rust test target added in Step 1.

Expected:
- FAIL because the behavior is still native-owned or not yet expressed in Rust

- [ ] **Step 3: Implement the minimal Rust table-policy surface in `fonttool-subset`**

Move the currently supported table-retention decisions into `crates/fonttool-subset/src/lib.rs` with one clear API boundary that `fonttool-cli` and later subset/encode paths can call.

- [ ] **Step 4: Re-run the focused table-policy tests**

Run the target from Step 2 again.

Expected:
- PASS

- [ ] **Step 5: Update the migration inventory**

Tighten the `tests/test_table_policy.c` row to reflect the new Rust-owned state and any remaining native-only edges.

- [ ] **Step 6: Commit the table-policy replacement**

```bash
git add crates tests/rust-test-inventory.md
git commit -m "feat(rust): replace native table policy"
```

## Task 5: Make Remaining Shared-Support Ownership Explicit

**Files:**
- Modify: `tests/rust-test-inventory.md`
- Modify: `README.md`
- Create or modify: `docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md`

- [ ] **Step 1: Inventory the remaining shared-support native files in Phase 1 scope**

Explicitly map:
- `src/parallel_runtime.cc` -> exact Rust destination file(s)
- `src/parallel_runtime.h` -> exact Rust destination file(s)
- `src/table_policy.c` -> exact Rust destination file(s)
- `src/table_policy.h` -> exact Rust destination file(s)
- each remaining Phase 1 shellout callsite in:
  - `crates/fonttool-runtime/src/lib.rs`
  - `crates/fonttool-harfbuzz/src/lib.rs`
  - `crates/fonttool-cff/src/lib.rs` with an explicit note whether it is deferred to Phase 3 or still a Phase 1 blocker
  - `crates/fonttool-cli/src/main.rs`

For each, record:
- current Rust destination crate
- current Rust destination file
- still-missing behavior
- test target that must pass before Phase 1 is done

- [ ] **Step 2: Save the destination mapping into the support matrix or inventory notes**

This should leave no ambiguity about which Rust file owns each remaining Phase 1 replacement.

- [ ] **Step 3: Verify Phase 1 blockers are now explicit**

Run:

```bash
rg -n "parallel_runtime|table_policy|shellout|Phase 1" docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md tests/rust-test-inventory.md README.md
```

Expected:
- explicit mapping entries exist

- [ ] **Step 4: Commit the explicit Phase 1 ownership map**

```bash
git add -f docs/superpowers/specs/2026-04-15-fonttool-support-matrix.md tests/rust-test-inventory.md README.md
git commit -m "docs: map phase1 rust ownership boundaries"
```

## Task 6: Phase 0/1 Verification Gate

**Files:**
- Modify if needed: any files touched by prior tasks only

- [ ] **Step 1: Run focused verification for the newly migrated surfaces**

Run:

```bash
cargo test -p fonttool-cli --test runtime_wasm
```

Expected:
- PASS

- [ ] **Step 2: Run full workspace verification**

Run:

```bash
cargo test --workspace -q
```

Expected:
- PASS

- [ ] **Step 3: Verify the repository still has no new supported shellout ambiguity**

Run:

```bash
rg -n "build/fonttool" crates tests | sed -n '1,120p'
```

Expected:
- only lines that remain are either:
  - explicitly documented as later-phase blockers, or
  - tests covering still-unreplaced later-phase legacy behavior

- [ ] **Step 4: Summarize remaining Phase 2+ blockers in the inventory**

Update `tests/rust-test-inventory.md` only if verification reveals new precision is needed.

- [ ] **Step 5: Commit any final verification-only adjustments**

```bash
git add tests/rust-test-inventory.md
git commit -m "docs: finalize phase0-1 verification status"
```
