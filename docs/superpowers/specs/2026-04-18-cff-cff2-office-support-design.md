# CFF/CFF2 Office Support Design

## Goal

Add Rust support for `OTF(CFF)`, `OTF(CFF2/variable)`, and `WOFF/WOFF2`
sources across CLI, runtime, and WASM-facing `decode`, `encode`, `subset`,
and explicit conversion flows, while making Office/PowerPoint compatibility an
explicit product target.

This design started as a larger convergence plan. The earlier
HarfBuzz-transition idea has now been retired, and the current shipped
direction is the steady-state one:

- converge the supported font-processing path onto `allsorts` plus Rust-owned
  subset/rebuild code
- remove `fonttool-harfbuzz` from the final shipped `CFF/CFF2/WOFF` feature set

The compatibility baseline changes in one important way:

- Office compatibility does not require PowerPoint to natively emit
  `CFF2/variable` embedded samples
- the default Office-oriented path for variable input is
  `CFF2/variable -> instantiated static OTF/CFF -> embedded output`
- `OTF -> TTF` remains available as an explicit CLI option for compatibility
  workflows, but it is not the default Office path

Historical note: this spec was updated in place instead of being split into an
archival transition document. The current shipped workspace state is the
Rust-owned one described above, and the remaining work is runtime/WASM
alignment rather than HarfBuzz convergence.

## Scope

This design covers:

- `fonttool decode` support for embedded `CFF` fonts and structurally valid
  embedded `CFF2/variable` fonts in `.eot` / `.fntdata` containers
- `fonttool encode` support for static `CFF` and `CFF2/variable` OTF input
- `fonttool subset` support for static `CFF` and `CFF2/variable` OTF input
- `fonttool encode`, `fonttool subset`, and explicit conversion support for
  `WOFF/WOFF2` input after source materialization
- `fonttool-runtime` and `fonttool-wasm` support for the same embedded
  `EOT` / `.fntdata` success paths exposed by the shipped CLI boundary
- variable-instance selection for `CFF2`
- Office-compatible embedding through a static `OTF/CFF` default path
- an explicit CLI option to convert `OTF(CFF/CFF2)` input to `TTF`
- an explicit CLI path to convert `WOFF/WOFF2` sources into `TTF` or embedded
  output after flavor-aware source loading
- permanent regression fixtures for Office-derived embedded OTF samples

This design does not include:

- requiring PowerPoint to generate native `CFF2/variable` embedded fixtures
- `.otc` / TTC / OTC collection input support
- byte-for-byte reproduction of retired native output
- restoring the retired standalone native backend as a separate product path
- storing Office `.pptx` documents in the repository

## User-Confirmed Constraints

- The public CLI and crate surfaces must stay Rust-first
- Third-party libraries are allowed, but must be discussed before being added
- the long-term target is still an `allsorts`-first implementation for the
  supported `CFF/CFF2/WOFF` flows
- The repository should keep extracted fixtures, not source `.pptx` files
- This phase must still include `CFF2/variable` `encode + decode + subset`
- Office/PowerPoint compatibility remains a formal success criterion
- `OTF -> TTF` must remain available as a CLI-selectable compatibility option

## Current State

The current Rust workspace already has:

- working `decode` for the supported TrueType/MTX path
- working `encode` / `subset` for the supported TrueType path
- `OTTO` SFNT parsing support in `fonttool-sfnt`
- `fonttool-cff` as a boundary crate with basic OTF inspection helpers

The workspace now routes the shipped CLI paths through Rust-owned crates:

- `fonttool-cff` owns `OTF/CFF/CFF2/WOFF` inspection, materialization,
  instancing, and explicit `OTF/WOFF -> TTF` conversion
- `fonttool-subset` owns the non-OTF glyph-id subset path

The earlier HarfBuzz split has been removed from the shipped workspace.

The remaining gap is not font parsing or conversion capability in the mainline
CLI path. The remaining gap is that `fonttool-runtime` and the Rust-side
`fonttool-wasm` wrapper still expose an incomplete contract:

- runtime diagnostics and scheduler behavior are implemented and tested
- the runtime bridge still rejects `.fntdata` output
- the runtime bridge still rejects variable `CFF2` conversion
- the runtime bridge does not yet materialize `WOFF/WOFF2` before flavor-aware
  OTF processing

That gap is the final scope this updated plan needs to close.

Recent investigation established:

- the current local `Presentation1.pptx` contains embedded static `OTF/CFF`
  samples for `思源黑体`
- at least one embedded sample from that file decodes into a legal
  `OTTO + CFF` font
- the tested `SourceCodeVF-Upright.otf` sample is a legal
  `OTTO + CFF2 + fvar` variable font
- PowerPoint rejects that variable OTF with a generic "not TrueType" style
  error even though PowerPoint accepts static `OTF/CFF`

Therefore the repository must not treat "PowerPoint can directly emit variable
`CFF2` fixtures" as a prerequisite for supporting variable input.

## Chosen Approach

Use the existing Rust CLI and embedded-output architecture as the outer shell,
keep OTF/CFF-specific transform logic under `fonttool-cff`, and make
`fonttool-runtime` / `fonttool-wasm` thin wrappers over the same Rust-owned
success path.

At a high level:

- keep `fonttool-cli` responsible for argument parsing, input classification,
  and output dispatch
- keep `fonttool-eot` and shared embedded-output code responsible for
  `.eot` / `.fntdata` wrapping
- expand `decode` to preserve embedded `CFF` and embedded `CFF2` when the
  payload is structurally complete
- add an OTF-specific encode path that defaults to Office-compatible static
  `OTF/CFF` embedding
- add variable-instance selection so `CFF2` input can be instantiated before
  Office embedding
- add `WOFF/WOFF2` source loading that materializes canonical SFNT bytes before
  flavor inspection and output dispatch
- keep runtime/WASM conversion requests on that same materialize -> inspect ->
  instantiate -> embed path
- keep `OTF -> TTF` as an explicit CLI compatibility branch, not the default
  output flavor

## Alternatives Considered

### 1. Default variable input to static `OTF/CFF`, keep `TTF` as opt-in

Pros:

- matches the observed Office behavior more closely
- avoids making TrueType conversion the only compatibility story
- keeps PostScript/CFF outlines on the mainline path

Cons:

- still requires real variable-instance handling before embedding
- needs careful serialization of static `CFF` output

This is the chosen approach.

### 2. Default variable input to `TTF`

Pros:

- may maximize compatibility with environments that truly require TrueType

Cons:

- makes outline conversion the primary path even though static `OTF/CFF`
  already appears Office-compatible
- larger fidelity risk
- higher implementation risk for a pure-Rust-only stack

This is not the default, but it remains an explicit CLI option.

### 3. Require native PowerPoint-generated `CFF2` fixtures before proceeding

Pros:

- keeps Office acceptance strictly tied to PowerPoint-produced artifacts

Cons:

- conflicts with observed product behavior
- blocks variable support on a tool limitation outside the repository

This is not selected.

## Architecture

### Fixture Layer

Keep two acceptance tracks.

Office-derived fixtures:

- `testdata/otto-cff-office.fntdata`
  - extracted from `Presentation1.pptx`
  - represents the real static `OTF/CFF` Office-compatible path
- `testdata/README-cff-fixtures.md`
  - records provenance and regeneration policy

Source OTF fixtures:

- `testdata/cff-static.otf`
- `testdata/cff2-variable.otf`

If a structurally complete embedded `CFF2` sample becomes available later, it
can be added as an extra fixture, but it is no longer required to unblock the
main implementation path.

### Runtime/WASM Bridge Layer

`fonttool-runtime` and `fonttool-wasm` are compatibility wrappers, not separate
font engines.

Responsibilities:

- accept filesystem-backed or buffer-backed requests for embedded output
- reuse the same Rust-owned `load_font_source -> inspect -> instantiate ->
  embed` path already shipped by the CLI
- preserve runtime scheduling diagnostics and thread-mode reporting
- surface the same validation errors the CLI path already exposes, especially
  static-input `--variation` rejection

Non-responsibilities:

- they do not introduce a second font parser or subset implementation
- they do not own CFF/CFF2 instancing policy
- they do not diverge from the CLI support boundary for supported
  `CFF/CFF2/WOFF` inputs

### Steady-State Allsorts Core Layer

`fonttool-cff` becomes the long-term owner of:

- source loading for `OTF`, `WOFF`, and `WOFF2`
- OTF/WOFF flavor inspection
- variable-instance materialization
- CFF/CFF2 subset execution
- glyph outline extraction for explicit `OTF/WOFF(CFF) -> TTF`

The steady-state boundary keeps using the existing native `cu2qu`,
`tt_rebuilder`, and SFNT rebuild code where it is already stable, but all font
parsing, variation, subset selection, and outline visitation should come from
`allsorts`.

### WOFF Source Layer

`WOFF` and `WOFF2` are container formats, not distinct outline flavors.

Rules:

- `WOFF/WOFF2` input must first materialize to canonical SFNT bytes
- if the materialized flavor is TrueType, the input may flow directly into the
  existing TrueType encode path
- if the materialized flavor is `OTF/CFF` or `OTF/CFF2`, the input must flow
  through the same CFF/CFF2 logic used for raw OTF sources
- `WOFF(CFF/CFF2) -> TTF` remains an explicit conversion path, not a silent
  encode default

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

Decode rules:

- embedded static `CFF` must decode to legal `OTTO + CFF`
- embedded variable `CFF2` must decode to legal `OTTO + CFF2 + fvar` when the
  input payload is structurally complete
- Office acceptance requires successful decode of the static Office fixture
- variable decode remains supported for committable fixtures and synthetic
  roundtrip coverage; it is not blocked on PowerPoint producing such a fixture

### Encode Layer

Split the current encode implementation into explicit branches:

- `encode_truetype_file()`
- `encode_otf_file()`

`encode_otf_file()` handles both static `CFF` and variable `CFF2` input.

Default behavior for OTF input targeting embedded output:

- static `CFF` input: preserve static `OTF/CFF`
- variable `CFF2` input: instantiate to a static font, then emit static
  `OTF/CFF` by default
- payload mode: raw SFNT payload by default
- no reuse of TrueType-specific `glyf/loca` MTX synthesis logic

The default Office-compatible strategy is to preserve or materialize a static
`OTF/CFF` font first and only then wrap it in the embedding container.

### Variable Handling Layer

Move variable-instance handling into `fonttool-cff` and keep it there for the
steady-state implementation.

Key operations:

- `inspect_otf_font(...)`
- `parse_variation_axes(...)`
- `instantiate_variable_cff2(...)`
- `serialize_static_cff_instance(...)`
- `load_font_source(...)`

`fonttool-cff` remains the owner of:

- OTF flavor inspection policy
- instance-to-static-CFF serialization policy
- CFF/CFF2-specific error mapping

Variable rules:

- `CFF2` input accepts `--variation`
- when no explicit variation is supplied, the default instance is used
- the Office-compatible output of variable input is a static font
- static-instance semantics must be explicit in tests and docs

### Optional TTF Conversion Layer

Keep `OTF -> TTF` available as an explicit CLI path for compatibility
workflows that need TrueType outlines.

This path is secondary, not default.

CLI surface:

- add a dedicated conversion command or output-flavor option that makes the
  conversion explicit
- do not silently replace the default static `OTF/CFF` Office path with TTF
  conversion

Requirements:

- the conversion path must be visibly opt-in
- docs and tests must distinguish "default Office mode" from
  "explicit TTF compatibility mode"
- outline extraction for this path must come from the shipped Rust-owned
  `fonttool-cff` path
- `fonttool-cff` must keep only the conversion stages that are specific to its
  own domain:
  - `cu2qu`
  - `tt_rebuilder`
  - SFNT copy/rebuild policy

## Third-Party Library Policy

New dependencies are allowed only if they remain contained within
`fonttool-cff`, `fonttool-runtime`, or another narrowly scoped font-processing
boundary.

Adoption rules:

- do not leak third-party core types across crate boundaries
- require explicit user confirmation before adding the dependency
- keep `fonttool-cli`, runtime, and WASM layers insulated from dependency
  public APIs
- do not reintroduce a second native or non-Rust-owned path for the supported
  `CFF/CFF2/WOFF` features

Reason:

- it already helps with `CFF/CFF2` inspection, variation, subsetting, and
  serialization-adjacent work

If the optional `OTF -> TTF` branch needs additional pure-Rust support, that
dependency decision must be reviewed separately and kept just as isolated.

## Functional Boundaries

### Decode

Must support:

- `.eot` / `.fntdata` input containing:
  - TrueType `glyf`
  - static `CFF`
  - structurally complete `CFF2/variable`

Success conditions:

- CLI command succeeds
- output parses as legal SFNT
- output flavor is preserved for the decoded payload

Lead acceptance cases:

- `testdata/otto-cff-office.fntdata` decodes successfully
- committable `CFF2` fixtures and roundtrip outputs decode successfully

### Encode

Must support:

- static `CFF` OTF input
- `CFF2/variable` OTF input
- `WOFF` input whose materialized source flavor is `OTF/CFF`, `OTF/CFF2`, or
  TrueType
- `WOFF2` input whose materialized source flavor is `OTF/CFF`, `OTF/CFF2`, or
  TrueType

Primary embedded output:

- `.eot`
- `.fntdata` when the Office-compatible embedded mode is requested

Success conditions:

- CLI command succeeds
- generated embedded font decodes back to a legal static font
- default variable-input path yields static `OTF/CFF`
- at least one Office-derived fixture remains compatible with the supported
  decode path

### Subset

Must support:

- static `CFF` with `--text`
- `CFF2/variable` with `--text` and optional `--variation`
- `WOFF/WOFF2` input after source materialization, with behavior matching the
  materialized source flavor

Success conditions:

- subset result is a legal font
- subset result can be re-encoded through the embedded output layer
- variable subset semantics match the chosen instance

### Optional OTF to TTF Conversion

Must support:

- explicit CLI opt-in only
- static `CFF` input
- variable `CFF2` input after instance selection
- `WOFF/WOFF2` input after source materialization and flavor classification

Success conditions:

- CLI command succeeds
- output parses as legal `TTF`
- tests prove the path is optional and does not change default Office behavior

## Acceptance Standards

Formal acceptance coverage:

1. `decode` permanent regression for the static Office fixture
2. `encode` success coverage for:
   - static `CFF`
   - variable `CFF2` through static `OTF/CFF` output
3. `subset` success coverage for:
   - static `CFF`
   - variable `CFF2`
4. `roundtrip` coverage:
   - `OTF(CFF) -> embedded -> OTF(CFF)`
   - `OTF(CFF2 variable) -> instantiate -> embedded -> static OTF(CFF)`
   - `subset -> encode -> decode`
5. explicit CLI conversion coverage:
   - `OTF(CFF) -> TTF`
   - `OTF(CFF2 variable) -> instance -> TTF`
   - `WOFF(TTF) -> TTF` materialization
   - `WOFF(CFF/CFF2) -> instance/convert -> TTF`
6. Office-oriented compatibility regression:
   - at least one PowerPoint-derived static `OTF/CFF` embedded sample must
     remain supported
7. runtime/WASM regression:
   - `fonttool-runtime` and `fonttool-wasm` must support:
     - static `CFF -> .eot`
     - static `CFF/WOFF(CFF) -> .fntdata`
     - variable `CFF2 -> instance -> .eot`
     - variable `CFF2 -> instance -> .fntdata`
   - the runtime/WASM path must surface the same static-input variation
     validation errors as the CLI path

Out of scope for formal completion:

- PowerPoint GUI automation
- requiring PowerPoint to emit native variable `CFF2` fixtures
- byte-identical parity with retired native output
- TTC/OTC support

## Testing Strategy

### 1. Unit Tests in `fonttool-cff`

Cover:

- static CFF detection
- variable CFF2 detection
- variation argument validation
- variable-instance materialization
- static CFF serialization
- optional TTF conversion boundary selection

### 2. Unit Tests in `fonttool-runtime` And `fonttool-wasm`

Cover:

- static `CFF -> .eot`
- `WOFF(CFF) -> .fntdata`
- variable `CFF2 -> instance -> embedded output`
- shared validation-error surfacing for static `--variation`
- scheduler diagnostics remaining intact while conversion support expands

### 3. CLI and Integration Tests

Add successful integration coverage for:

- `decode`
  - `otto-cff-office.fntdata`
  - source OTF and roundtrip-derived variable fixtures as needed
- `encode`
  - `cff-static.otf`
  - `cff2-variable.otf`
- `subset`
  - static CFF with `--text`
  - variable CFF2 with `--text + --variation`
- optional conversion
  - explicit `OTF -> TTF`
  - explicit `WOFF/WOFF2 -> TTF`
  - flavor-aware `WOFF/WOFF2 -> embedded output`

### 4. Roundtrip Tests

Add:

- static CFF `OTF -> embedded -> OTF`
- variable CFF2 `OTF -> instantiate -> embedded -> static OTF`
- static CFF `subset -> encode -> decode`
- variable CFF2 `subset -> encode -> decode`
- explicit `OTF -> TTF` compatibility conversions

### 5. Fixture Documentation

Document the origin and regeneration policy of the embedded Office fixture
without committing the `.pptx`.

## Implementation Sequence

Implementation should proceed in this order:

1. replace the old blocked Office-fixture assumption
   - store the current static Office fixture
   - update regression coverage and docs
2. stabilize decode around the real Office static CFF path
3. implement variable-instance handling for `CFF2`
4. implement default OTF encode through static `OTF/CFF`
5. implement static and variable subset flows
6. expand `fonttool-runtime` to reuse the shipped Rust-owned embedded OTF path
7. rebase `fonttool-wasm` and runtime integration tests onto real success paths
8. update runtime/WASM docs and verification contracts

## Risk Controls

### Office Compatibility Uses the Real Supported Path

The project is blocked only on the real Office-compatible static `OTF/CFF`
fixture, not on PowerPoint producing a variable sample it does not appear to
support.

### Variable Support Still Must Be Real

`CFF2` support cannot be reduced to a deferred error. Variable input must be
accepted, instantiated, tested, and emitted through at least one supported path.

### TTF Conversion Stays Opt-In

The explicit `OTF -> TTF` branch is valuable for compatibility, but it must not
silently replace the default static `OTF/CFF` Office path.

### Runtime/WASM Must Reuse The Shipped Path

This plan is not complete if runtime/WASM stays on deferred or divergent logic
while the CLI path succeeds. The runtime/WASM surfaces must reuse the same
Rust-owned source materialization, variation instancing, and embedded-output
path that already defines the shipped CLI behavior.

## Completion Definition

This project is complete when the Rust CLI, runtime, and WASM surfaces provide
test-covered support for:

- embedded static `CFF` decode from a real Office fixture
- static `CFF` `encode + subset`
- variable `CFF2` instance selection plus `encode + subset`
- default Office-compatible embedded output through static `OTF/CFF`
- runtime/WASM `EOT` and `.fntdata` output for static `CFF`, variable `CFF2`,
  and materialized `WOFF(CFF)` input
- explicit CLI-selectable `OTF -> TTF` conversion
- explicit `WOFF/WOFF2` input support with flavor-aware dispatch
- a final allsorts-first supported path for `CFF/CFF2/WOFF` flows
