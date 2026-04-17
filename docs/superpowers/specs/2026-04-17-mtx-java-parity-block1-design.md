# MTX Java Parity Block1 Design

## Goal

Upgrade the Rust MTX encoder so the PowerPoint sample's regenerated `font1.fntdata`
and final `pptx` outputs move materially closer to the original producer, with
`block1` compression as the first hard target and full Java-style MTX parity as
an opportunistic extension when the same implementation path supports it safely.

## Problem

Recent analysis established that the current size regression is concentrated in
`block1`, not the outer `pptx` packaging and not `block2` or `block3`.

For the tracked PowerPoint sample:

- the original and regenerated `block1` decompressed sizes are nearly identical
- `block2` and `block3` are effectively empty in both cases
- the regenerated `block1` compressed size is dramatically larger than the
  original producer's output

This means the main gap is the quality of the Rust `block1` MTX/LZ compression
decisions. The current Rust encoder in
[`crates/fonttool-mtx/src/lz.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-mtx/src/lz.rs)
uses a simpler heuristic than the Java reference implementation in
[`LzcompCompress.java`](/Users/ekxs/Codes/sfntly_web/sfntly/java/src/com/google/typography/font/tools/conversion/eot/LzcompCompress.java),
which uses hash-based match lookup, local cost estimation, and small look-ahead
adjustments.

## Success Criteria

This design uses a dual success model. Both tracks matter:

### 1. Correctness and compatibility

- Existing Rust encode/decode/subset regression coverage remains green.
- Regenerated embedded fonts still roundtrip through the Rust decode path.
- The PPT sample continues to reconstruct a valid, roundtrip-ready TrueType
  font.

### 2. Output-size target

- Regenerated `测试用例 7 (文本组件主题)-test.pptx` and
  `测试用例 7 (文本组件主题)-xor.pptx` must land within `+128 KiB` of the
  original `测试用例 7 (文本组件主题).pptx`.
- The `block1` compressed size must materially improve from the current Rust
  baseline and be the primary source of that win.

### 3. Explainability if the hard target is missed

If the first implementation wave does not reach the `+128 KiB` target, the work
is still incomplete unless it produces a block-level gap report that explains
the remaining difference in terms of:

- `font1.fntdata` header differences
- `block1/2/3` compressed and decompressed lengths
- remaining divergence in compression decisions versus Java
- any remaining block-construction differences

## Scope

### In scope

- Upgrade Rust MTX/LZ compression decisions so they better match the Java
  `LzcompCompress` behavior for `block1`
- Add analysis and regression coverage that measures block-level and final-file
  size outcomes on the tracked PPT sample
- Keep the implementation path reusable for `block2` and `block3`
- Allow the same work to expand into broader Java MTX parity if verification
  shows the extension is low-risk

### Out of scope

- Changing the MTX container format
- Adding new CLI flags or user-facing compression knobs
- Reopening `.fntdata` output support in the Rust Phase 1 CLI boundary
- Requiring bit-for-bit parity with PowerPoint or legacy Java output in this
  iteration

## Recommended Approach

Use a staged parity strategy:

1. Lock in a reliable parity baseline and sample analysis harness
2. Upgrade the Rust compression decision model in
   [`crates/fonttool-mtx/src/lz.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-mtx/src/lz.rs)
   so it more closely follows the Java `findMatch(...)` and
   `makeCopyDecision(...)` heuristics
3. First prove the improvement on `block1`
4. If the same implementation also stabilizes `block2/3` without introducing
   regressions, extend the iteration scope to partial or full Java MTX parity

This keeps the first delivery tightly focused on the known bottleneck while
preserving a clean path to a broader parity effort.

## Architecture

### 1. Compression decision layer

Keep `compress_lz(...)` as the single public entrypoint in
[`crates/fonttool-mtx/src/lz.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-mtx/src/lz.rs),
but change its internal strategy from a simple longest-match greedy search to a
cost-aware local decision model.

The upgraded model should support:

- hash-based match lookup instead of only brute-force scanning
- cached literal-cost estimation
- explicit copy-cost estimation
- `dup2`, `dup4`, and `dup6` treated as costed alternatives, not only fallback
  shortcuts
- one-step look-ahead when deciding whether to emit a copy now or a literal now
  and a better copy immediately after
- a local "shorten current match by one byte" correction when that produces a
  lower total cost, matching the Java strategy closely enough to matter on real
  data

The literal-only fallback remains in place as a safety net. If the upgraded
encoder fails or produces larger output than the literal-only path, the Rust
implementation still returns the smaller safe result.

### 2. Block strategy layer

Do not add new strategy branching at the CLI layer.

[`crates/fonttool-cli/src/main.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-cli/src/main.rs)
should continue to call the same `compress_lz(...)` entrypoint for all three
blocks.

The iteration policy is:

- `block1` improvement is mandatory
- `block2/3` remain on the same compressor but are not allowed to widen the
  scope unless verification shows they are naturally improved or at least not
  regressed

This keeps the "what to compress" policy stable and confines the main logic
changes to the compression engine itself.

### 3. Verification and analysis layer

Add a repeatable parity harness around the tracked PowerPoint sample.

The harness must be able to report:

- final regenerated `pptx` sizes
- extracted `font1.fntdata` sizes
- parsed EOT header values
- MTX `block1/2/3` compressed and decompressed lengths
- size delta versus the original sample

This reporting is part of the implementation, not an ad hoc debugging step. It
provides both the hard acceptance signal and the failure-explanation path if the
target is missed.

## File Boundaries

### Primary implementation

- Modify:
  [`crates/fonttool-mtx/src/lz.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-mtx/src/lz.rs)

This is the main implementation surface. If the file becomes too difficult to
reason about, it is acceptable to split internal helpers into focused files
inside the same crate, such as match-search or cost-model helpers, but only if
the split clarifies responsibility.

### Minimal wiring

- Modify:
  [`crates/fonttool-cli/src/main.rs`](/Users/ekxs/Codes/eot_tool/crates/fonttool-cli/src/main.rs)

Only minimal wiring or comments should be needed here. This layer should not
gain complex compression-policy branching.

### Verification surfaces

- Modify existing encode/decode integration tests under
  [`tests/rust_integration`](/Users/ekxs/Codes/eot_tool/tests/rust_integration)
- Add focused block-analysis support in existing Rust integration test helpers
  or a dedicated sample-analysis script under the repository `build` workflow

The testing layout should separate:

- compression behavior tests
- PPT sample block-level size tests
- final-file acceptance checks

so failures are easy to diagnose.

## Testing Strategy

### 1. LZ unit tests

In `fonttool-mtx`, cover:

- roundtrip safety
- repeated-data wins over literal-only output
- `dup2/dup4/dup6` behavior
- longer general backreferences
- non-regression on incompressible input

### 2. Java-strategy regression tests

Add focused tests that validate the upgraded Rust heuristic behaves more like
the Java compressor on deliberately chosen inputs. These tests do not need
bit-for-bit parity; they need to prove that the new decision model produces the
expected direction of improvement.

### 3. PPT sample block-level tests

For the tracked sample, assert:

- `block1` remains the main content block
- `block2/3` stay empty or non-regressing for this sample
- `block1` compressed size improves materially over the current Rust baseline

### 4. Final output acceptance checks

Regenerate:

- `测试用例 7 (文本组件主题)-test.pptx`
- `测试用例 7 (文本组件主题)-xor.pptx`

and compare them against the original sample.

Acceptance gate:

- both regenerated files must land within `+128 KiB` of the original

If the gate fails, the analysis output must clearly show where the remaining
difference comes from.

## Risks and Mitigations

### Risk: Java-style heuristics help the PPT sample but hurt other fonts

Mitigation:

- keep literal-only fallback
- retain broad encode/decode integration coverage
- use targeted synthetic tests plus the real PPT sample

### Risk: the compressor logic becomes too hard to maintain

Mitigation:

- keep one public entrypoint
- isolate helper responsibilities if the file grows too dense
- copy Java strategy only where it materially affects output, not line for line

### Risk: the size gap is not only compression quality

Mitigation:

- require compressed and decompressed block reports
- keep block-construction differences visible in the parity harness

### Risk: the work expands into full Java parity before delivering a result

Mitigation:

- treat scheme 1 as the hard delivery target
- allow scheme 2 only when the same implementation path proves stable and does
  not require a large second refactor

## Delivery Model

This design commits to the following rule:

- Scheme 1 must be completed
- Scheme 2 may be completed in the same iteration if verification shows it is a
  safe extension of the same work

In other words, the implementation must first solve the known `block1`
compression bottleneck, and only then may broaden itself into a fuller Java MTX
parity effort.
