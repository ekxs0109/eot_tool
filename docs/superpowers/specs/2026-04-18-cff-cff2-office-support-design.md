# CFF/CFF2 Office Support Design

## Goal

Add pure-Rust support for `OTF(CFF)` and `OTF(CFF2/variable)` across the Rust
CLI's `decode`, `encode`, and `subset` commands, and make Office/PowerPoint
compatibility a first-class acceptance target rather than an accidental side
effect.

The highest-priority acceptance case is successful decode of the current
PowerPoint-exported embedded font sample extracted from `Presentation1.pptx`,
which will be stored as a permanent test fixture under `testdata/`.

## Scope

This design covers:

- `fonttool decode` support for embedded `CFF` and `CFF2/variable` fonts in
  `.eot` / `.fntdata` containers
- `fonttool encode` support for static `CFF` and `CFF2/variable` OTF input
- `fonttool subset` support for static `CFF` and `CFF2/variable` OTF input
- Office/PowerPoint compatibility as a formal verification target
- Permanent regression fixtures for embedded OTF samples

This design does not include:

- `.otc` / TTC / OTC collection input support
- byte-for-byte reproduction of historical native output
- restoring a legacy/native backend bridge
- storing Office `.pptx` documents in the repository

## User-Confirmed Constraints

- Implementation must stay pure Rust end-to-end
- Third-party libraries are allowed, but must be discussed before being added
- Pure Rust crates are preferred over Rust+FFI
- The repository should keep only extracted `.fntdata` fixtures, not the source
  `.pptx`
- This phase must include `CFF2/variable` `encode + decode + subset`, not just
  static `CFF`
- Office/PowerPoint re-consumption compatibility is part of the formal success
  criteria

## Current State

The current Rust workspace already has:

- working `decode` for the supported TrueType/MTX path
- working `encode` / `subset` for the supported TrueType path
- `OTTO` SFNT parsing support in `fonttool-sfnt`
- `fonttool-cff` as a boundary crate that currently only inspects OTF input and
  emits deferred-boundary errors

The current Rust CLI does not yet provide:

- successful `encode` for `OTF(CFF)` input
- successful `encode` for `OTF(CFF2/variable)` input
- successful `subset` for `OTF(CFF)` input
- successful `subset` for `OTF(CFF2/variable)` input
- robust decode of all PowerPoint-exported OTF embedded font shapes

The current PowerPoint-derived `Presentation1.pptx` sample contains an embedded
font that fails today's Rust `decode` path and therefore acts as the lead
regression case for this work.

## Chosen Approach

Use the existing Rust CLI and EOT/MTX architecture as the outer shell, and add
an OTF-specific processing path under `fonttool-cff`.

At a high level:

- keep `fonttool-cli` responsible for argument parsing, input classification,
  and output dispatch
- keep `fonttool-eot` and `embedded_output` responsible for EOT/fntdata
  wrapping
- expand `decode` to support OTF-oriented embedded reconstruction in addition to
  the current TrueType-oriented path
- add a dedicated `encode_otf_file()` path in `fonttool-cli`
- move real `CFF/CFF2` subsetting and variation handling into `fonttool-cff`
- use a pure-Rust third-party crate only inside `fonttool-cff` for
  `CFF/CFF2` subset and serialization

## Alternatives Considered

### 1. Rust-native mainline plus targeted third-party CFF/CFF2 support

This keeps the current repository architecture intact and introduces a
third-party pure-Rust crate only in the `fonttool-cff` implementation layer for
the hardest `CFF/CFF2` subset/serialize logic.

Pros:

- smallest architecture change
- preserves current CLI/runtime layering
- isolates third-party dependencies
- best fit for the requested pure-Rust direction

Cons:

- requires a careful adapter layer between the chosen crate and the repository's
  existing SFNT/EOT/MTX pipeline

This is the chosen approach.

### 2. Migrate more aggressively to a third-party font stack

This would move much more of the font processing core onto a third-party
library ecosystem.

Pros:

- potentially more complete long-term font feature support

Cons:

- much larger refactor
- higher integration risk
- harder to keep focused on this project's Office compatibility target

This is not selected.

### 3. Build all `CFF/CFF2` encode/subset/write logic from scratch in-repo

Pros:

- maximum local control
- no new dependency risk

Cons:

- highest schedule risk
- weakest option for reaching `CFF2/variable + Office compatibility` in one
  iteration

This is not selected.

## Architecture

### Fixture Layer

Add permanent fixtures for OTF-focused embedded regression coverage:

- `testdata/otto-cff2-variable.fntdata`
  - extracted from the current `Presentation1.pptx`
  - renamed to a generic fixture name
  - stored without the `.pptx`
- keep the existing OTF source fixtures:
  - `testdata/cff-static.otf`
  - `testdata/cff2-variable.otf`

Add a fixture note file:

- `testdata/README-cff-fixtures.md`

It should record:

- the embedded fixture came from a PowerPoint export of an open-source source
  font
- the repository keeps only the extracted `.fntdata`
- how to regenerate the fixture when it is intentionally replaced

### Decode Layer

Refactor embedded decode into explicit stages:

1. `prepare_embedded_font_payload()`
   - parse header
   - strip XOR when present
   - expose payload bytes and metadata
2. `decode_embedded_payload()`
   - distinguish raw-SFNT payloads from MTX containers
   - dispatch reconstruction by font flavor and block shape
3. reconstruction helpers
   - `reconstruct_truetype_from_mtx_blocks()`
   - `reconstruct_cff_from_mtx_blocks()`
   - `reconstruct_cff2_from_mtx_blocks()`

The current TrueType path remains valid, but OTF/CFF/CFF2 must no longer be
forced through the `glyf/loca` reconstruction assumptions that exist today.

The decode result must be a legal SFNT accepted by `parse_sfnt()` /
`load_sfnt()`, with OTF flavor preserved:

- static CFF output remains `OTTO + CFF`
- variable CFF2 output remains `OTTO + CFF2 + fvar`

### Encode Layer

Split the current encode implementation into two explicit branches:

- `encode_truetype_file()`
- `encode_otf_file()`

`encode_otf_file()` handles both static CFF and variable CFF2 input.

Default behavior for OTF input:

- output target: `.eot`
- payload mode: raw SFNT payload by default
- minimal structural rewriting of the original OTF
- no reuse of the TrueType `glyf/loca`-specific MTX block generation logic

The default Office-compatible strategy is to preserve as much of the original
OTF byte structure as practical and only change the embedding container layer.

Support for MTX payload mode on OTF input is not the primary acceptance target
for the first implementation. The mainline success path is Office-compatible
raw-SFNT embedding.

### Subset Layer

Move OTF-specific subset logic into `fonttool-cff`.

Add dedicated operations such as:

- `subset_static_cff(...)`
- `instantiate_variable_cff2(...)`
- `subset_variable_cff2(...)`
- `serialize_subset_otf(...)`

Static CFF subset rules:

- require `--text`
- produce a valid subset OTF

Variable CFF2 subset rules:

- require `--text`
- accept `--variation`
- apply variation instance selection before or during subsetting
- produce a valid subset OTF that follows the chosen instance semantics

The CLI remains responsible for parsing and validation; `fonttool-cff` becomes
responsible for OTF/CFF/CFF2 processing internals.

## Third-Party Library Policy

The design allows a new pure-Rust third-party crate only if it is contained
inside `fonttool-cff`.

Adoption rules:

- prefer a pure-Rust crate over FFI
- do not leak third-party core types across crate boundaries
- require explicit user confirmation before adding the dependency
- keep `fonttool-cli`, runtime, and WASM layers insulated from the dependency's
  public API

Preferred candidate:

- `allsorts`

Reason:

- it already contains `CFF/CFF2` subset logic
- it is closer to the needed `subset + serialize` responsibilities than the
  current in-repo code

Fallback candidates may be evaluated later, but the implementation plan should
assume `allsorts` first and keep the integration boundary narrow enough to swap
it cleanly if the chosen dependency proves unsuitable during implementation.

## Functional Boundaries

### Decode

Must support:

- `.eot` / `.fntdata` input containing:
  - TrueType `glyf`
  - static `CFF`
  - `CFF2/variable`

Success conditions:

- CLI command succeeds
- output parses as legal SFNT
- output flavor is preserved

Lead acceptance case:

- `testdata/otto-cff2-variable.fntdata` decodes successfully

### Encode

Must support:

- static `CFF` OTF input
- `CFF2/variable` OTF input

Primary output:

- `.eot`

Success conditions:

- CLI command succeeds
- generated embedded font decodes back to a legal OTF
- roundtrip preserves the intended OTF flavor
- at least one Office/PowerPoint-oriented acceptance sample remains compatible

### Subset

Must support:

- static `CFF` with `--text`
- `CFF2/variable` with `--text` and `--variation`

Success conditions:

- subset result is a legal OTF
- subset result can be re-encoded
- encoded subset can be decoded back to a legal OTF
- variation behavior matches requested axes for the supported fixture set

## Acceptance Standards

This project uses Office compatibility as a first-class success criterion, but
the repository's default automated coverage should still rely on stable,
committable fixtures rather than live PowerPoint automation.

Formal acceptance coverage:

1. `decode` permanent regression for `otto-cff2-variable.fntdata`
2. `encode` success coverage for:
   - static `CFF`
   - variable `CFF2`
3. `subset` success coverage for:
   - static `CFF`
   - variable `CFF2`
4. `roundtrip` coverage:
   - `OTF -> EOT -> OTF`
   - `subset -> encode -> decode`
5. Office-oriented compatibility regression:
   - at least one PowerPoint-derived embedded sample must remain supported

Out of scope for formal completion:

- byte-identical parity with the retired native backend
- automated GUI Office save/reopen verification
- TTC/OTC collection support
- broad guarantees for every advanced OpenType layout behavior

## Testing Strategy

### 1. Unit Tests in `fonttool-cff`

Cover:

- static CFF detection
- variable CFF2 detection
- variation argument validation
- subset output validity
- CFF/CFF2 flavor preservation

### 2. CLI and Integration Tests

Add successful integration coverage for:

- `decode`:
  - `otto-cff2-variable.fntdata`
  - existing static CFF samples
- `encode`:
  - `cff-static.otf`
  - `cff2-variable.otf`
- `subset`:
  - static CFF with `--text`
  - variable CFF2 with `--text + --variation`

### 3. Roundtrip Tests

Add:

- static CFF `OTF -> EOT -> OTF`
- variable CFF2 `OTF -> EOT -> OTF`
- static CFF `subset -> encode -> decode`
- variable CFF2 `subset -> encode -> decode`

Assertions must include flavor-sensitive checks:

- static CFF roundtrip keeps `CFF`
- variable CFF2 roundtrip keeps `CFF2`
- variable support markers such as `fvar` remain or are removed according to the
  chosen instance semantics

### 4. Fixture Documentation

Document the origin and regeneration policy of the embedded fixture without
committing the `.pptx`.

## Implementation Sequence

Even though the desired outcome is full end-to-end support, implementation
should still proceed in this order:

1. fixture and decode stabilization
   - add `otto-cff2-variable.fntdata`
   - make decode succeed on it
2. OTF encode path
   - add `encode_otf_file()`
   - support static CFF and variable CFF2 encode to `.eot`
3. OTF roundtrip stabilization
   - verify Office-oriented raw-SFNT embedding path
4. OTF subset path
   - implement static CFF and variable CFF2 subset
5. contract and docs cleanup
   - replace current deferred-boundary tests with success-path coverage
   - update README and support docs

## Risk Controls

### Decode First

If `otto-cff2-variable.fntdata` cannot be decoded successfully, the project is
not considered complete regardless of encode/subset progress.

### OTF Encode Defaults to Raw-SFNT Payload

The first shipping OTF path should optimize for Office compatibility instead of
trying to mimic the TrueType MTX specialization.

### Third-Party Dependency Containment

If a third-party crate is used, it must remain inside `fonttool-cff` so the
rest of the workspace stays insulated from dependency churn.

## Completion Definition

This project is complete when the Rust CLI provides test-covered pure-Rust
support for static `CFF` and `CFF2/variable` `decode + encode + subset`, and
the default `.eot` raw-SFNT embedding path satisfies the current
PowerPoint-derived Office compatibility regression samples.
