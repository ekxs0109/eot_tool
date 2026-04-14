# OTF/CFF EOT Fidelity Design

## Context

This repository already supports:

- decoding EOT and `.fntdata` containers into SFNT output
- encoding `TTF` input into MTX-compressed EOT output
- parsing and serializing both TrueType-flavored (`0x00010000`) and OpenType/CFF-flavored (`OTTO`) SFNTs
- converting `OTF(CFF/CFF2)` input into TrueType-flavored intermediate SFNT data

Recent investigation in this repository showed an important mismatch between current encode behavior and real-world fixture behavior:

- [`testdata/font1.fntdata`](/Users/ekxs/Codes/eot_tool/testdata/font1.fntdata) decodes to an `OTTO/CFF` SFNT
- the current `encode` path converts `OTF(CFF)` input into `TTF(glyf)` before MTX/EOT encoding
- the current encoder always emits a fixed 120-byte EOT header with empty variable strings
- the current MTX container packer collapses empty block2/block3 output into a 1-block container

The user wants the project to move toward the structure observed in the historical `font1.fntdata` fixture:

1. `OTF(CFF)` should preserve `OTTO/CFF` flavor by default when encoded to EOT
2. output should be structurally closer to historical samples, including a fuller EOT header and 3-block MTX layout with empty placeholder blocks when appropriate
3. the existing `otf -> ttf` conversion logic must remain available behind an explicit parameter

Because this project is still pre-release, backward compatibility with the current default `OTF -> TTF -> EOT` behavior is not required. Simpler default product behavior is preferred.

## Goals

- Make `fonttool encode input.otf output.eot` preserve `OTTO/CFF` flavor by default for static `OTF(CFF)` input
- Emit EOT headers that carry richer metadata than the current fixed empty-string header
- Emit MTX containers that more closely resemble the historical 3-block layout even when block2 and block3 are empty placeholders
- Preserve the existing explicit `otf -> truetype sfnt` conversion capability behind a CLI/API option
- Keep the current `TTF(glyf)` encode path unchanged
- Keep decode behavior compatible with both `OTTO/CFF` and `TTF(glyf)` block1 payloads

## Non-Goals

- Reproducing historical EOT files byte-for-byte
- Implementing a new CFF-specific equivalent to the existing `glyf` compression format
- Changing the behavior of `TTF` input encoding
- Expanding this design to cover `CFF2` fidelity preservation
- Reworking subset architecture in the same change

## Decision Summary

The project should adopt a dual encoding strategy based on input flavor:

- `TTF` input continues to use the existing `glyf/loca` MTX encoding path
- `OTF(CFF)` input uses a new default `cff pass-through` path that preserves `OTTO/CFF`
- users may explicitly request the old behavior with a new `--force-otf-to-ttf` parameter

This keeps the product model simple:

- default behavior follows the real-world historical fixture direction
- the legacy conversion path still exists when desired
- implementation remains incremental rather than architectural

## Proposed CLI and API Behavior

### CLI

Current CLI shape:

- `fonttool encode <input> <output>`

New behavior:

- `fonttool encode <input.ttf|input.otf> <output.eot|output.fntdata>`
- optional encode flag: `--force-otf-to-ttf`

Semantics:

- `TTF` input ignores `--force-otf-to-ttf`
- `OTF(CFF)` input:
  - default: preserve `OTTO/CFF`
  - with `--force-otf-to-ttf`: convert to `TTF(glyf)` before MTX/EOT encode
- `OTF(CFF2)` input remains on the existing conversion/instancing path and is not treated as a fidelity-preserving pass-through candidate

### WASM / Programmatic Surface

Any browser-facing or lower-level entry point that currently accepts `.otf` input should gain the same explicit policy choice, even if it is initially represented as a boolean or enum rather than a CLI-style string flag.

The contract should be the same:

- default `OTF(CFF)` encode preserves `OTTO/CFF`
- callers can opt into forced TrueType conversion

## Architecture

The implementation should stay centered in the existing encode flow rather than introducing a second top-level pipeline.

### 1. Encode Policy Selection

Location:

- [`src/main.c`](/Users/ekxs/Codes/eot_tool/src/main.c)
- [`src/mtx_encode.c`](/Users/ekxs/Codes/eot_tool/src/mtx_encode.c)
- [`src/mtx_encode.h`](/Users/ekxs/Codes/eot_tool/src/mtx_encode.h)

Responsibilities:

- parse the new encode option
- propagate an explicit encode policy into the native encode layer
- choose between:
  - existing `glyf` MTX path
  - new `cff pass-through` path
  - explicit forced conversion path

This policy boundary is the most important decomposition line in the change.

### 2. CFF Pass-Through Block Builder

Location:

- primarily [`src/mtx_encode.c`](/Users/ekxs/Codes/eot_tool/src/mtx_encode.c)
- possibly a small helper module if the file becomes too dense

Responsibilities:

- detect `OTTO + CFF` input
- serialize the current `sfnt_font_t` while preserving `font->version == OTTO`
- store the serialized SFNT in block1
- emit empty placeholder block2 and block3 buffers
- avoid any dependency on `glyf`, `loca`, `glyf_encode()`, or push/code streams

This path is intentionally simple. It does not invent a CFF-specific MTX transform. It uses the MTX container as a transport wrapper around a serialized `OTTO/CFF` SFNT.

### 3. EOT Header Builder

Location:

- new small module, recommended:
  - `src/eot_header_builder.h`
  - `src/eot_header_builder.c`

Responsibilities:

- build EOT variable strings from the input font’s metadata
- calculate header length dynamically
- centralize EOT header serialization for both `TTF` and `OTF` encode flows
- keep header generation out of the already-large `mtx_encode.c`

This module should replace the current hard-coded “fixed 120-byte empty-string header” logic with a reusable builder used by all encode paths.

### 4. MTX Container Packing Compatibility

Location:

- [`src/mtx_container.c`](/Users/ekxs/Codes/eot_tool/src/mtx_container.c)

Responsibilities:

- support a compatibility mode that emits `num_blocks = 3` even when block2 and block3 are empty
- preserve legal offsets for empty placeholder blocks
- avoid breaking current 1-block behavior for paths that still want minimal packing

The historical fixture direction suggests that empty placeholder blocks are part of the observable shape of these files and are worth preserving.

## Data Flow

### `TTF(glyf)` Input

This remains unchanged:

1. parse SFNT
2. build MTX-style `glyf` block1 plus push/code block2/block3
3. LZ-compress blocks
4. build EOT header
5. pack output
6. apply `.fntdata` XOR when requested

### `OTF(CFF)` Input, Default Behavior

New default flow:

1. parse SFNT as `OTTO/CFF`
2. select `cff pass-through` policy
3. serialize `sfnt_font_t` directly into block1, preserving `OTTO`
4. create empty block2 and block3 placeholders
5. LZ-compress the three blocks
6. build dynamic EOT header from font metadata
7. pack EOT/`.fntdata`
8. apply `.fntdata` XOR when requested

### `OTF(CFF)` Input with `--force-otf-to-ttf`

Explicit fallback flow:

1. parse SFNT
2. convert with `otf_convert_to_truetype_sfnt(...)`
3. continue through the existing `glyf` MTX encode path

## EOT Header Design

The current encoder writes:

- fixed-size header
- empty `FamilyName`
- empty `StyleName`
- empty `VersionName`
- empty `FullName`
- empty `RootString`
- zero `RootStringChecksum`

That is legal enough for current roundtrips, but it does not resemble the observed historical files.

The new builder should populate:

- `FamilyName`
- `StyleName`
- `VersionName`
- `FullName`

Preferred source:

- `name` table records, using a deterministic mapping strategy

Fallback:

- empty string when the desired name entry is absent

The builder should also preserve current numeric metadata handling sourced from:

- `OS/2`
- `head`

### Root String Fields

The project does not currently have a verified, authoritative generation rule for `RootString` and `RootStringChecksum` that is required for compatibility. For the first iteration:

- keep `RootString` empty by default
- keep `RootStringChecksum` at zero unless a trustworthy rule is introduced later

This is an explicit fidelity boundary: richer name/header metadata is in scope, but reverse-engineering undocumented root-string behavior is not.

## MTX Container Design

For the `cff pass-through` path, the container should aim to be structurally closer to the historical sample:

- `num_blocks = 3`
- block1 contains the compressed serialized `OTTO/CFF` SFNT
- block2 is an empty placeholder block
- block3 is an empty placeholder block

The decode side already tolerates `OTTO/CFF` in block1 because it accepts standard SFNT versions including `OTTO` during block1 table extraction.

The implementation should ensure:

- offset ordering remains valid
- empty placeholder blocks remain legal under the packer/parser invariants
- decode of newly produced files succeeds without special-case logic

## Error Handling

- If an input `.otf` is not a static `OTTO + CFF` font and not a supported `CFF2` conversion case, return the existing structured error behavior
- Missing `name` records should not fail encoding; they should degrade to empty strings
- `--force-otf-to-ttf` should only affect `OTF(CFF)` inputs
- `.fntdata` XOR behavior remains unchanged and is orthogonal to flavor-preservation policy
- failures in the new header builder should return existing `eot_status_t` codes rather than introducing a parallel error taxonomy

## Testing Strategy

### Unit Tests

Add focused tests for:

- dynamic EOT header serialization from synthetic font metadata
- header length and size metadata self-consistency
- MTX container packing with empty block2/block3 placeholders
- encode policy selection for:
  - `TTF`
  - `OTF(CFF)` default
  - `OTF(CFF)` with `--force-otf-to-ttf`

### Integration / CLI Tests

Add or update tests covering:

- `fonttool encode input.otf output.eot` followed by `decode` yields an `OTTO/CFF` SFNT
- `fonttool encode --force-otf-to-ttf input.otf output.eot` followed by `decode` yields a TrueType-flavored SFNT
- `fonttool encode input.ttf output.eot` remains unchanged
- help/usage text documents the new encode parameter

### Fixture-Oriented Structural Tests

Use [`testdata/font1.fntdata`](/Users/ekxs/Codes/eot_tool/testdata/font1.fntdata) as a structure reference, not a byte-identical golden file.

Assertions should cover:

- produced `OTF(CFF)`-default EOT header length is no longer hard-coded to 120
- variable strings are populated when name data exists
- MTX container uses 3 blocks
- decode roundtrip preserves `OTTO/CFF`

## Acceptance Criteria

The change is complete when all of the following are true:

- default `encode` of static `OTF(CFF)` input preserves `OTTO/CFF` after `decode`
- `--force-otf-to-ttf` makes the same input roundtrip back to TrueType flavor
- `TTF` encode behavior continues to pass current tests
- encoded `OTF(CFF)` output uses a dynamic EOT header with populated variable strings when available
- encoded `OTF(CFF)` output uses a 3-block MTX structure with empty placeholder blocks
- no decode regressions are introduced for existing fixtures

## Implementation Notes for Planning

The most important decomposition boundaries for the implementation plan are:

1. add encode policy plumbing
2. add EOT header builder module
3. add `cff pass-through` block builder path
4. extend MTX container packing for 3-block empty placeholder output
5. add CLI and integration coverage for the new default and forced fallback parameter

Those tasks can be implemented incrementally and verified separately without rewriting the repository architecture.
