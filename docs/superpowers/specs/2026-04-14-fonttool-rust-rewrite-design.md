# Fonttool Rust Rewrite Design

## Context

This repository currently contains:

- a native `C/C++` core under `src/`
- a CLI binary built around `decode`, `encode`, and `subset`
- `OTF(CFF/CFF2)` conversion code, including variable-font instancing behavior
- a WASM/browser-facing layer
- native tests in `C/C++`
- verification scripts in Python using `fonttools`
- macOS-specific validation needs around `CoreText`

The user wants to discuss whether the project should move to Rust, with these clarified priorities:

1. The long-term direction should be a Rust-led CLI/native core
2. The top risk to reduce is handling untrusted font input on servers and in automation
3. The project is still pre-release, so a full rewrite is acceptable if it is disciplined
4. The success criterion is not “feature-complete enough”, but “passes the existing behavior checks”
5. `C/C++` tests should be migrated into Rust tests
6. Python verification should remain in place because it uses `fonttools` as an external validator
7. macOS validation should include a Swift/CoreText layer
8. HarfBuzz is allowed and should be treated as a trusted long-term dependency rather than something that must be replaced

## Goals

- Rewrite the project into a Rust-centered architecture focused on memory safety for untrusted font input
- Preserve current CLI/native behavior closely enough that the existing behavior can be re-expressed as passing Rust-led tests
- Keep HarfBuzz as a stable dependency for the OpenType behaviors it already handles well
- Replace the current native test harness with Rust tests as the primary internal validation layer
- Preserve Python/fonttools validation as an external semantic verifier
- Add a formal Swift/CoreText validation layer for macOS acceptance checks
- End with a repository that feels like a Rust project with a few explicit validation/tooling sidecars, not a `C/C++` project with a Rust wrapper

## Non-Goals

- Replacing HarfBuzz simply for purity
- Using FontForge as a runtime replacement for HarfBuzz
- Rewriting Python/fonttools verification into Rust
- Keeping `C/C++` as a first-class long-term implementation language
- Doing a one-shot rewrite with no phased acceptance criteria

## Decision Summary

The rewrite should follow a Rust-first architecture with a phased implementation sequence and a Rust-only internal test strategy, while keeping:

- HarfBuzz as a long-term dependency behind a thin Rust adapter layer
- Python/fonttools verification as an external correctness oracle
- Swift/CoreText validation as a macOS platform acceptance layer

This is not a “translate the existing `C/C++` into Rust syntax” effort. It is a controlled architectural rewrite that uses Rust to move binary parsing, bounds checking, serialization, and high-risk input handling into memory-safe code.

## Why Rust Is Justified Here

The strongest justification is not stylistic preference. It is the deployment model:

- the tool processes untrusted binary font input
- the tool is intended for server and automation use
- the codebase contains dense binary parsing, table offset handling, serialization, and cross-module data flow

Those are exactly the conditions where Rust’s safety model provides the most leverage. The project does not need Rust because the current code is “bad”. It benefits from Rust because the problem domain is high-risk for parser and memory bugs.

## Proposed Architecture

Use a Rust workspace with explicit crates for format boundaries and algorithm boundaries.

Suggested workspace structure:

- `crates/fonttool-bytes`
  Safe binary readers/writers, endian helpers, bounded slice access, checked offsets, and shared parsing primitives.
- `crates/fonttool-sfnt`
  SFNT model, table directory logic, table readers/writers, and common table abstractions.
- `crates/fonttool-eot`
  EOT header parsing, metadata handling, PowerPoint XOR behavior, and EOT-specific container semantics.
- `crates/fonttool-mtx`
  MTX container parsing/packing and MTX-specific encoding/decoding stages.
- `crates/fonttool-glyf`
  TrueType core data handling for `glyf`, `loca`, `head`, `maxp`, `hhea`, `hmtx`, and related codecs.
- `crates/fonttool-subset`
  Subset planning, glyph retention rules, output table rebuild logic, and subset warnings/policies.
- `crates/fonttool-cff`
  `CFF/CFF2`, `cu2qu`, OTF conversion logic, and variation-location support not delegated to HarfBuzz.
- `crates/fonttool-harfbuzz`
  The only crate allowed to touch HarfBuzz bindings directly.
- `crates/fonttool-runtime`
  Runtime execution strategy, thread handling, and deterministic task orchestration.
- `crates/fonttool-cli`
  CLI entry point, command parsing, exit-code mapping, and command orchestration.
- `crates/fonttool-wasm`
  WASM/browser entry point and runtime packaging concerns.

This layout makes the dangerous binary input edges explicit, keeps dependency boundaries narrow, and prevents HarfBuzz from becoming an implicit architecture center.

## HarfBuzz Strategy

HarfBuzz should be treated as a trusted long-term dependency.

It should remain responsible for:

- subsetting support that already depends on HarfBuzz semantics
- variable-font instancing support
- OpenType behaviors where HarfBuzz is the mature, standards-oriented implementation

It should not be allowed to leak throughout the workspace. Instead:

- only `fonttool-harfbuzz` links to or binds HarfBuzz
- all higher-level crates consume project-defined Rust types
- external inputs are parsed and validated by Rust before HarfBuzz is invoked
- HarfBuzz-specific output is converted immediately back into Rust-owned types

This keeps HarfBuzz as a dependency, not as the dominant abstraction for the whole codebase.

## FontForge Decision

FontForge should not be used as a runtime replacement for HarfBuzz.

Reasoning:

- HarfBuzz is a shaping/subsetting/variation engine with mature runtime APIs
- FontForge is primarily a font editor/conversion tool with scripting workflows
- FontForge may be useful as a tooling reference or offline utility, but it is not a good architectural replacement for the runtime behaviors currently delegated to HarfBuzz

## Rewrite Strategy

The rewrite should remain “all-Rust destination”, but the implementation sequence must still be phased.

Recommended phase order:

1. `fonttool-bytes + fonttool-sfnt + fonttool-eot + fonttool-mtx`
   First remove the riskiest binary parsing surface from `C/C++`.
2. `fonttool-glyf + encode/decode CLI parity`
   Establish safe end-to-end core operations for the common TrueType path.
3. `fonttool-subset`
   Build subset planning and table regeneration on top of stable Rust data models.
4. `fonttool-cff`
   Tackle `CFF/CFF2`, `cu2qu`, OTF conversion, and related complexity once the main model is stable.
5. `fonttool-runtime + fonttool-wasm`
   Reintroduce runtime and browser-facing concerns after native core behavior is stable.

This is still a full rewrite direction, but not an unstructured rewrite.

## Acceptance Model

The rewrite is complete when the Rust implementation becomes the primary implementation and passes the project’s agreed validation stack.

The acceptance stack should be:

1. Rust unit tests
2. Rust integration tests
3. Rust structural/golden tests
4. Swift/CoreText macOS validation
5. Python/fonttools verification

The old `C/C++` tests are not meant to survive forever. Their role is to be migrated into Rust tests.

## Testing Design

### 1. Rust Unit Tests

Each crate should own fine-grained tests for:

- range and offset validation
- parser error paths
- individual table handling
- serialization invariants
- runtime bookkeeping
- subset plan decisions
- variation normalization logic

These tests replace most of the current fine-grained `C/C++` unit coverage.

### 2. Rust Integration Tests

Workspace-level Rust integration tests should cover:

- `decode`
- `encode`
- `subset`
- `OTF(CFF/CFF2)` conversion
- variation-related flows
- CLI behavior

These tests should run against real fixtures and assert end-to-end semantics.

### 3. Rust Structural / Golden Tests

These tests should validate output structure and key semantics without requiring every artifact to be byte-identical:

- required table presence
- `head/maxp/hhea/hmtx/glyf/loca` coherence
- glyph counts
- subset consistency
- SFNT version correctness
- warning behavior where current behavior explicitly matters

This layer protects against behavior drift that simple parser success tests will miss.

### 4. Swift macOS Validation

Add and preserve a formal Swift validation layer that exercises:

- `CoreText`
- `CGFont`
- system acceptance of produced fonts

This should be a first-class test target, not an ad hoc debugging script.

### 5. Python/fonttools Verification

Python verification stays.

It should remain because it serves a distinct purpose:

- it is an external verifier using `fonttools`
- it helps validate semantic correctness independently of the Rust implementation
- it should not be rewritten just for language purity

Python does not remain as implementation. It remains as an external oracle.

## Additional Test Coverage To Add

The rewrite should strengthen test coverage where the current project is most exposed.

### Fuzzing

Prioritize fuzz targets for:

- EOT header parsing
- MTX container parsing
- SFNT table directory parsing
- CFF/CFF2 parsing entry points

### Property Tests

Add property-style tests for:

- parse -> serialize -> parse stability where appropriate
- offset/range rejection guarantees
- subset invariants (`numGlyphs`, `loca/glyf`, `maxp`)
- serialization invariants for table directory and key table relationships

### Crash-Resistance Tests

Explicitly test malformed input handling to ensure:

- invalid input returns structured errors
- invalid input does not panic
- invalid input does not crash worker processes

### Fixture Tiering

Organize fixtures into:

- smoke
- regression
- corpus

This keeps fast iteration practical while still preserving deep verification coverage.

## Safety Rules For The Rust Rewrite

The rewrite should adopt the following hard constraints:

1. Old `C/C++` implementation becomes reference/oracle code, not a continuing feature-development path
2. Each completed phase must expose a real Rust-executed target, not just library fragments
3. External input parsing defaults to reject-invalid behavior
4. `panic!` is never the normal mechanism for invalid input handling
5. Rust data models should be designed for clarity and safety, not as one-to-one copies of old `struct` layouts
6. Passing tests alone is not enough; corpus and platform validation must also pass

## Build and Delivery Model

The repository should converge on Rust as the primary build entry.

Target end state:

- root `Cargo.toml` defines the workspace
- `fonttool-cli` builds the main `fonttool` binary
- `fonttool-wasm` builds browser/WASM-facing deliverables
- Swift and Python validation remain as repository-level sidecar validation tools
- `Makefile` is either removed or reduced to a thin compatibility wrapper

The desired repository identity is:

- a Rust project
- with a small number of explicit platform/tooling sidecars
- and a narrowly scoped HarfBuzz dependency

## Open Questions

These should be answered during planning, not implementation:

- which Rust crates should be internal-only versus publishable
- which current fixtures become mandatory CI fixtures versus optional corpus
- whether the runtime/WASM layer should wait until native parity is complete
- which exact HarfBuzz binding strategy is preferred (`bindgen`, handwritten wrapper, or existing crate)

## Recommendation

Proceed with a Rust-led full rewrite strategy, but structure it as phased Rust acceptance rather than a chaotic one-shot port.

The key architectural decisions are:

- Rust owns binary parsing, serialization, and core orchestration
- HarfBuzz remains a trusted long-term dependency behind a thin adapter crate
- Rust tests replace `C/C++` tests as the primary internal validation layer
- Python/fonttools stays as an external verifier
- Swift/CoreText becomes a formal macOS platform validation layer

This gives the project the largest safety gain in the area the user cares most about: processing untrusted font input on servers and in automation.
