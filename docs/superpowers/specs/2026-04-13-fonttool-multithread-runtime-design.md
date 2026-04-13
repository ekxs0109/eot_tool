# Fonttool Multithread Runtime, Packaging, and Benchmark Design

## Context

This repository already contains:

- native font-processing code under `src/`
- an existing parallel runtime abstraction in `src/parallel_runtime.h` and `src/parallel_runtime.cc`
- a partially completed publishable wrapper in `packages/fonttool-wasm/`

The goal of this design is to turn the current codebase into a consistent multithread-capable system, with the native conversion pipeline as the primary concern. Packaging and browser verification follow from that core runtime design.

User priorities for this design:

1. Native capability first
2. As much of the conversion pipeline as possible should use the same parallel runtime model
3. Browser support must automatically fall back to single-thread mode when pthread-capable WASM is unavailable
4. The npm package should expose both high-level APIs and lower-level runtime access
5. A benchmark-style web page is required for verification
6. Publish a single npm package, not a split default/parallel package set
7. Use a lightweight Turborepo workspace to coordinate packages and apps
8. The benchmark UI must be designed with `shadcn` and reviewed with `web-design-guidelines`

## Goals

- Introduce a unified parallel execution model across the full conversion pipeline where work is safely parallelizable
- Keep non-parallel-safe stages explicitly serial
- Expose deterministic runtime diagnostics across native, WASM, Node, and browser entry points
- Publish one npm package that works in both Node and browsers
- Provide a benchmark web app that verifies runtime mode selection, fallback behavior, and performance differences
- Keep the repository structure maintainable and explicit

## Non-Goals

- Rewriting all native modules into a new architecture
- Forcing every line of the conversion pipeline to become parallel
- Shipping the benchmark web app inside the npm package
- Designing multiple public npm packages in the first iteration
- Building a polished marketing site instead of an engineering benchmark tool

## Proposed Approach

Use a unified runtime strategy:

- define parallel behavior once in the native core
- expose the resulting runtime decisions through the WASM API
- keep Node and browser adaptation in `packages/fonttool-wasm`
- keep the benchmark UI in a separate app

This avoids split behavior between entry points and keeps performance, fallback, and diagnostics consistent.

## Architecture

The system is split into four layers.

### 1. Native Core Layer

Location:

- `src/`

Responsibilities:

- own the full conversion pipeline
- decide which stages can be represented as parallel task sets
- use `parallel_runtime_run_indexed_tasks()` or `parallel_runtime_run_task_list()` as the single scheduling mechanism
- preserve deterministic output regardless of requested thread count
- preserve existing error code semantics

### 2. Native/WASM Boundary Layer

Location:

- `src/wasm_api.h`
- `src/wasm_api.cc`

Responsibilities:

- expose conversion entry points used by Emscripten builds
- expose runtime diagnostics for mode selection and fallback
- surface requested and effective thread counts
- avoid making the JS/TS layer infer runtime behavior

### 3. Package Runtime Layer

Location:

- `packages/fonttool-wasm/`

Responsibilities:

- load the correct WASM variant for Node or browser
- expose a stable high-level API and an opt-in lower-level API
- stage and publish the required runtime artifacts
- normalize runtime diagnostics into JS-visible types

### 4. Verification and Benchmark Layer

Location:

- `apps/benchmark-web/`

Responsibilities:

- verify browser runtime capability detection
- verify single-thread and pthread-capable runs
- compare elapsed times and display diagnostics
- allow result download and inspection

## Parallelization Model

The conversion pipeline should be normalized into stage execution patterns rather than ad hoc thread usage inside each module.

### Stage Template

Each parallel-capable stage should follow a common structure:

1. serial pre-scan
2. task partitioning
3. parallel task execution
4. serial aggregation and finalization

This gives a consistent place to:

- allocate task-local buffers
- avoid shared mutable state during hot loops
- collect per-task status
- merge results into final output order

### Parallel-Capable Work

Candidate categories for parallel execution include:

- glyph-level transformations
- contour conversion and outline processing
- subset computations that can operate on independent glyph or table slices
- encoding and rebuilding tasks that can be partitioned by index
- independent metric or table-entry processing where output ordering can be reassembled deterministically

These categories should be audited stage by stage during implementation and converted only when they satisfy:

- no unsafe shared mutable state
- deterministic merge behavior
- no ordering-dependent binary layout side effects during the hot loop

### Serial Work

The following work should remain serial unless a later redesign proves otherwise:

- file header parsing
- top-level table directory assembly
- final binary buffer layout and offset fixing
- shared output stream writes
- any stage where output offsets depend on preceding writes

The design goal is not “everything runs in parallel”. The design goal is “the full pipeline uses one consistent runtime strategy, with explicit serial exceptions”.

## Runtime Modes and Fallback

All entry points should support the same abstract execution mode concept:

- `auto`
- `single`
- `multi-preferred`

### Requested vs Effective Threads

Public APIs should accept a requested thread count. The runtime must derive:

- `requestedThreads`
- `effectiveThreads`

If the runtime cannot satisfy the requested value, it must still run when possible and report why.

### Fallback Rules

#### Native and Node

- prefer the pthread-capable runtime when available
- if multithread support is unavailable or disabled, fall back to single-thread execution
- if the caller requests `single`, force single-thread execution even when parallel support exists

#### Browser

- detect `SharedArrayBuffer` availability
- detect worker support required by the pthread-capable Emscripten runtime
- detect whether the page is running in a cross-origin isolated context
- if any requirement is missing, load the single-thread runtime automatically

### Diagnostic Contract

Every conversion result should expose diagnostics containing at least:

- `requestedThreads`
- `effectiveThreads`
- `resolvedMode`
- `runtimeKind` (`node` or `browser`)
- `variant` (`single` or `pthread`)
- `fallbackReason` when a fallback occurred

This contract is required for both automated tests and the benchmark UI.

## Error Handling

Parallel execution must preserve existing native error behavior as closely as possible.

Rules:

- a task failure returns the first non-`EOT_OK` error encountered
- the stage must stop consuming further work as soon as practical
- task-local temporary allocations are always cleaned up in failure paths
- serial and parallel execution of the same input must not diverge in error semantics

If implementation details require a more explicit error aggregation structure, that structure should stay internal to the native layer and not leak into the JS API.

## Package Design

Package:

- `packages/fonttool-wasm`

### Public API Shape

High-level API:

- `convert(input, options)`
- `loadFonttool(options)`

Lower-level API:

- runtime loading primitives
- capability detection
- runtime diagnostics types
- explicit runtime lifecycle control for advanced consumers

The default usage should remain high-level. The lower-level API exists to support controlled lifecycle management and advanced embedding scenarios.

### Runtime Artifacts

The single published npm package includes both runtime variants:

- single-thread WASM artifacts
- pthread-capable WASM artifacts and any required JS glue/worker files

The benchmark app is not included in the npm package.

### Loader Policy

Node:

- try pthread-capable runtime first
- fall back to single-thread runtime if required

Browser:

- probe runtime capability first
- load pthread-capable runtime only when all prerequisites are met
- otherwise load the single-thread runtime

If the single-thread runtime cannot load, initialization fails. If the pthread runtime cannot load, the loader should fall back rather than fail when single-thread remains viable.

### Package Size Control

Because both runtime variants ship in one npm package, package size must be managed explicitly.

Required controls:

- whitelist published files
- do not publish benchmark static assets
- do not publish large example fonts or development fixtures
- add a pack inspection step to verify contents and size before release

If the single-package strategy becomes unacceptably large after implementation, splitting parallel artifacts into a second package may be reconsidered later, but that is out of scope for this design.

## Turborepo Adoption

Use a lightweight Turborepo setup for orchestration only.

### Why Turborepo

The repository now spans:

- native source and tests
- a publishable npm package
- a benchmark web app
- shared build and verification steps

Without orchestration, dependency order and CI commands will become script-heavy and fragile. Turborepo provides a minimal way to encode:

- build ordering
- dev workflows
- test fan-out
- pack validation

### Scope of Turborepo

Turborepo is only the workspace task orchestrator. It does not redefine the native build system.

The existing native build process remains authoritative for:

- core native builds
- WASM builds
- native test execution

Turbo coordinates those tasks with package and app tasks.

### Repository Layout

Recommended structure:

- `src/`
- `tests/`
- `packages/fonttool-wasm/`
- `apps/benchmark-web/`
- `scripts/` or `tools/`
- `turbo.json`

The native core remains at repository root. It should not be forced into a JS package just to fit workspace conventions.

## Benchmark Web App Design

App:

- `apps/benchmark-web/`

Purpose:

- engineering verification and benchmarking, not marketing

### Functional Requirements

The benchmark app must support:

- runtime capability detection display
- file upload for font inputs
- output mode selection
- thread count selection
- execution mode selection
- optional single-thread vs multi-thread benchmark runs
- elapsed time comparison
- display of runtime diagnostics
- output download
- compact structured logs for inspection

### Information Architecture

Recommended layout:

- top runtime status strip
- left control panel
- right result and comparison area
- lower diagnostics/log section

This should feel like a deliberate tool console, not a generic form page.

### UI Component Policy

Implementation of the benchmark UI must use the `shadcn` skill and the `shadcn/ui` component system.

Expected component categories:

- `Card`
- `Tabs`
- `Table`
- `Alert`
- `Badge`
- `ToggleGroup`
- `Progress`
- input and field primitives appropriate for file upload and runtime selection

### UI Review Gate

Before calling the benchmark UI complete, its key UI files must be reviewed with the `web-design-guidelines` skill against the current guideline source.

This review is a required verification step, not an optional polish pass.

## Proposed Web App Structure

The benchmark app should be structured with clear boundaries between runtime orchestration and presentation.

Recommended internal shape:

- `apps/benchmark-web/src/app/` or equivalent router entry
- `apps/benchmark-web/src/components/benchmark/`
- `apps/benchmark-web/src/components/runtime/`
- `apps/benchmark-web/src/lib/fonttool/`
- `apps/benchmark-web/src/lib/benchmark/`
- `apps/benchmark-web/src/lib/formatting/`
- `apps/benchmark-web/src/types/`

Responsibilities:

- `components/benchmark/`: benchmark panels, results cards, comparison tables, upload controls
- `components/runtime/`: capability badges, runtime status summaries, fallback indicators
- `lib/fonttool/`: package integration and runtime loading glue
- `lib/benchmark/`: run orchestration, elapsed time measurement, structured comparison records
- `lib/formatting/`: formatting helpers for bytes, durations, and diagnostics

The app should avoid embedding runtime loading logic directly in UI components.

## Testing Strategy

Testing is required at three layers.

### 1. Native/Core Tests

Add or extend tests to verify:

- serial vs parallel output equivalence
- requested thread count handling
- effective thread count reporting
- stage-level failure propagation
- cleanup on task failure
- deterministic output across repeated parallel runs

### 2. Package Tests

Add or extend tests to verify:

- Node runtime variant selection
- browser capability detection logic
- fallback to single-thread runtime
- diagnostics contract shape
- artifact staging correctness
- `npm pack` contents and size checks

### 3. Web App Verification

Verify:

- capability detection visibility
- mode selection behavior
- benchmark comparison output
- download flow
- structured diagnostics presentation
- key UI code against `web-design-guidelines`

## Delivery Strategy

Implementation should be phased so that native correctness leads packaging and UI.

Suggested order:

1. audit and mark serial vs parallel-capable native stages
2. normalize stage execution through the shared parallel runtime
3. extend WASM API diagnostics
4. complete `packages/fonttool-wasm` artifact loading and variant selection
5. add pack validation and release checks
6. build `apps/benchmark-web`
7. review benchmark UI against `web-design-guidelines`

## Risks

### 1. Limited Parallel Yield

Some stages may not parallelize cleanly enough to justify complexity. The design remains valid as long as the pipeline still uses one runtime model and documents serial exceptions explicitly.

### 2. Browser Runtime Constraints

Pthread-capable WASM in browsers depends on environmental constraints that many deployments do not satisfy. Fallback behavior and diagnostics therefore matter as much as raw multithread support.

### 3. Package Size

A single npm package that ships both runtime variants may become larger than expected. Pack validation and publish whitelisting are mandatory.

### 4. Drift Between Runtime Layers

If diagnostics and mode resolution are not defined centrally, the native, WASM, and JS layers may report different behavior. The diagnostic contract is intended to prevent this drift.

## Open Implementation Notes

These items are intentionally deferred to the implementation plan:

- exact Emscripten build outputs and naming conventions
- exact benchmark framework choice for `apps/benchmark-web`
- exact public TypeScript type names beyond current package patterns
- stage-by-stage native refactor sequence
- CI matrix details

## Acceptance Criteria

The work is successful when all of the following are true:

- the conversion pipeline uses a shared parallel runtime strategy throughout the codebase
- serial exceptions are explicit and justified
- Node and browser consumers use the same high-level package API
- browser runtime selection falls back automatically without API breakage
- runtime diagnostics are visible programmatically and in the benchmark UI
- a single npm package can be packed without benchmark assets
- the benchmark app verifies runtime behavior and benchmark comparisons
- benchmark UI implementation uses `shadcn` and is reviewed with `web-design-guidelines`
- Turborepo coordinates builds and tests without replacing the native build system
