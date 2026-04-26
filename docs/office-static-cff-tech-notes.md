# Office Static CFF Technical Notes

Last updated: 2026-04-22

## Scope

This note only tracks the bounded Office static CFF work in this branch. It does not claim generic Office static CFF support.

Tracked fixtures:

- `testdata/otto-cff-office.fntdata`
- `testdata/presentation1-font2-bold.fntdata`

Tracked source-derived standard prefixes:

- `testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-extralight-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-heavy-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-light-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-medium-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-normal-cff-prefix-through-global-subrs.bin`

Tracked source-derived standard `CharStrings INDEX` offsets:

- `testdata/sourcehan-sc-regular-charstrings-offsets.bin`
- `testdata/sourcehan-sc-bold-charstrings-offsets.bin`
- `testdata/sourcehan-sc-extralight-charstrings-offsets.bin`
- `testdata/sourcehan-sc-heavy-charstrings-offsets.bin`
- `testdata/sourcehan-sc-light-charstrings-offsets.bin`
- `testdata/sourcehan-sc-medium-charstrings-offsets.bin`
- `testdata/sourcehan-sc-normal-charstrings-offsets.bin`

Tracked source-derived CID parse-range repair slices:

- `testdata/sourcehan-sc-regular-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-bold-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-extralight-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-heavy-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-light-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-medium-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-normal-cff-global-subrs-data.bin`
- `testdata/sourcehan-sc-regular-cff-fdselect.bin`
- `testdata/sourcehan-sc-bold-cff-fdselect.bin`
- `testdata/sourcehan-sc-extralight-cff-fdselect.bin`
- `testdata/sourcehan-sc-heavy-cff-fdselect.bin`
- `testdata/sourcehan-sc-light-cff-fdselect.bin`
- `testdata/sourcehan-sc-medium-cff-fdselect.bin`
- `testdata/sourcehan-sc-normal-cff-fdselect.bin`
- `testdata/sourcehan-sc-regular-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-bold-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-extralight-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-heavy-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-light-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-medium-cff-fdarray-tail.bin`
- `testdata/sourcehan-sc-normal-cff-fdarray-tail.bin`

Tracked source-derived donor SFNT shells without `CFF `:

- `testdata/sourcehan-sc-regular-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-bold-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-extralight-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-heavy-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-light-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-medium-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-normal-sfnt-without-cff.otf`

## Office Intermediate Invariants

- After MTX/LZ decode, the Office static CFF intermediate still begins with `OTTO`.
- The known `CFF `-like region begins at decoded offset `0x20e`.
- The first four bytes at that region are `04 03 00 01`, not a standard CFF header.
- `office_cff_suffix[4..]` is not directly standard-parsable as `Name INDEX`.
- The Source Han Sans SC family currently verified on this branch all share the same transformed wrapper:
  - decoded header starts `OTTO 0001 0200 0004 0010 BASE`
  - `office_cff_suffix[0..4] == 04 03 00 01`
  - the embedded Office font name is encoded as:
    - `01 02 <len> <name bytes...>`
  - real samples include:
    - `SourceHanSaSsSC-Regular`
    - `SourceHanSaSsSC-Bold`
    - `SourceHanSaSsSC-ExtraLight`
    - `SourceHanSaSsSC-Heavy`
    - `SourceHanSaSsSC-Light`
    - `SourceHanSaSsSC-Medium`
    - `SourceHanSaSsSC-Normal`
- `But Head` does not use this wrapper shape in the local corpus; its decoded payload already looks like a standard static `OTTO/CFF`.

## Corrected Task 3 Strategy

The earlier raw-INDEX Task 3 pseudocode was invalid on real Office bytes.

What is true:

- the Source Han Sans SC weights currently covered by this branch can be selected from the Office embedded font name
- the source-side Global Subr data start is still the right standard-prefix cutoff
- Office tail reuse begins two bytes earlier than that prefix cutoff
- the exact cutoff is no longer stored as hand-written literals in the layout
  - it is derived from the tracked standard prefix fixture length
- the `CharStrings INDEX` window is also no longer stored as hand-written literals in the layout
  - `charstrings_offset` is derived from the tracked standard prefix `Top DICT`
  - `charstrings_data_start` is derived from the tracked standard `CharStrings INDEX` offsets fixture
- the later CID parse ranges are now source-backed from tracked fixtures
  - `standard_prefix.len() .. charset_offset`
  - `fdselect_offset .. fdselect_offset + fdselect_len`
  - `fdarray_offset .. end`
- the older Office-side landmarks such as `935` (regular) and `875` (bold) remain useful for reverse-engineering context, but they are not valid raw standard INDEX starts
- full standard `OTTO` materialization does not need faux-directory inversion
  - it can be bounded as `tracked donor SFNT shell without CFF + rebuilt standard CFF table`

The bounded implementation in this branch is therefore:

1. parse the Office embedded font name from the transformed prefix
2. select the tracked Source Han Sans SC standard prefix, source-backed CID repair slices, and tracked `CharStrings INDEX` offsets for that weight
3. rebuild `CFF ` as `tracked_standard_prefix + office tail`
4. patch the source `Global Subr data .. charset` range back into the rebuilt `CFF `
5. patch the source `FDSelect` range back into the rebuilt `CFF `
6. patch the standard `CharStrings INDEX` window back into the rebuilt `CFF `
7. patch the source `FDArray..end` tail back into the rebuilt `CFF `

## Current Implementation State

- `fonttool_cff::office_static::extract_office_static_cff()` is implemented and tested.
- `fonttool_cff::office_static::rebuild_office_static_cff_table()` now does both:
  - prefix grafting through the start of `Global Subr` data
  - source-backed repair of the `Global Subr data .. charset` range
  - source-backed repair of the `FDSelect` range
  - in-place repair of the standard `CharStrings INDEX` window from tracked reference offsets
  - source-backed repair of the `FDArray..end` tail
- `fonttool_cff::office_static::rebuild_office_static_cff_sfnt()` is no longer a stub.
  - it now loads the tracked donor `OTTO` shell for the matched weight
  - reinserts the rebuilt standard `CFF ` table
  - reserializes a fresh standard SFNT via `fonttool_sfnt::serialize_sfnt()`
- The current tracked Source Han Sans SC family coverage is:
  - `Regular`
  - `Bold`
  - `ExtraLight`
  - `Heavy`
  - `Light`
  - `Medium`
  - `Normal`
- A failed experiment in this branch already established that a naive `load_sfnt()` / `serialize_sfnt()` wrapper rebuild is wrong for this Office intermediate:
  - the faux `OTTO` header advertises only `1` table
  - later faux-directory `offset` / `length` fields are transformed and often out of range if read as standard SFNT records
- The new donor-shell path replaces that failed approach:
  - do not read table payload locations from the faux directory
  - instead, use the tracked standard shell for the matching weight
  - this produces a standard parseable `OTTO` wrapper for the regular fixture today
- However, the bounded rebuild is still not fully standard-CFF complete for deep consumers:
  - `inspect_otf_font()` accepts the rebuilt SFNT as static `OTF/CFF`
  - `convert_otf_to_ttf()` now fails later with `failed to visit glyph outline: an invalid amount of items are in an arguments stack`
  - direct source-vs-rebuilt byte comparison narrows the next transformed CID structures to:
    - `charset`
    - `FDSelect`
    - `FDArray`
    - `Private DICT` / local subr data referenced from `FDArray`
  - more precise regular-case characterization now exists:
    - `charset` already matches source in the current bounded regular rebuild
    - `FDSelect` keeps the same `format 3 / nranges 113 / length 344` shape as source, but the range starts are corrupted
    - `FDArray` preserves object `0`, while objects `1..17` are no longer valid standard `Font DICT` byte streams
    - only private dict `0` still matches source; later private dicts diverge
  - cross-weight local characterization sharpens the priority:
    - `charset` is mostly stable across the seven Source Han Sans SC weights; the current observed anomaly is concentrated in bold
    - `FDSelect` is transformed in all seven weights while preserving a stable `format 3` container
    - `FDArray` is transformed in all seven weights while preserving its INDEX header shape
  - local source-backed control tests split the remaining blocker in two:
    - backfilling source bytes from `global_subrs_data_start` through the source `CFF ` end makes `convert_otf_to_ttf()` succeed
    - backfilling only source `Global Subr` data plus the `FDArray..end` tail changes the error to `failed to visit glyph outline: an invalid amount of items are in an arguments stack`
    - the current production rebuild's first outline failure is glyph `0` (`.notdef`)
    - that rebuilt `.notdef` `CharString` is still a bytewise mismatch against source even though its byte length matches
    - a local Type 2 token pass shows `.notdef` is only locally rewritten:
      - tokens `0..27` still match source
      - the first token-level divergence appears at token `28`
      - source `-313 403 313 403 rlineto -31 -846 rmoveto`
      - rebuilt `66 -100 369 147 rlineto -100 rlineto -31`
    - glyph-level source backfill shows the outline failure advancing in glyph order rather than staying tied to `.notdef`:
      - backfill glyph `0` only -> first outline failure becomes glyph `3` (`quotedbl`)
      - backfill glyphs `0` and `3` -> first outline failure becomes glyph `4` (`numbersign`)
      - backfill glyphs `0`, `3`, and `4` -> first outline failure becomes glyph `5` (`dollar`)
      - backfill glyphs `0`, `3`, `4`, and `5` -> first outline failure becomes glyph `6` (`percent`)
      - by glyph `6`, the error string also changes from stack-shape corruption to `an invalid operator occurred`
      - backfill glyphs `0` and `29` -> first outline failure still stays at glyph `3`
      - glyph `3` (`quotedbl`) now has a stronger localization result:
        - patching only the suffix beginning at the first draw op is already enough to advance the first failing glyph to `4`
        - patching only the prefix through that first draw boundary is not enough
      - glyph `4` (`numbersign`) behaves the same way:
        - suffix-only patching from its first draw boundary is enough to advance the first failing glyph to `5`
        - prefix-only patching is not enough
      - glyph `5` (`dollar`) is the first early glyph where this simple split fails:
        - prefix-only patching through the first draw boundary still fails at glyph `5`
        - suffix-only patching from that boundary still fails at glyph `5`
      - a tighter slice matrix now shows:
          - patching `glyph 5` prefix plus a later window `[70, 91)` is already enough to advance the first failing glyph to `6`
          - leaving only the middle diff window `[31, 48)` unpatched still advances to glyph `6`
          - leaving only the later window `[70, 91)` unpatched still fails at glyph `5`
        - splitting that late window sharpens it again:
          - patching `glyph 5` prefix plus bytes `[72, 74)` is already enough to advance to glyph `6`
          - patching `glyph 5` prefix plus bytes `[70, 72)` is not enough
          - leaving only bytes `[70, 72)` unpatched still advances to glyph `6`
          - leaving only bytes `[72, 74)` unpatched still fails at glyph `5`
          - splitting once more shows only one byte in that tiny window is currently causal:
            - source byte `72` is `0xf7`, rebuilt byte `72` is `0x07`
            - byte `73` already matches source in both versions
            - patching only glyph `5` byte `72` together with the glyph `5` prefix is already enough to advance to glyph `6`
            - leaving only glyph `5` byte `72` unpatched still fails at glyph `5`
          - a new diagnostic helper now shows the late glyph `5` fix really does belong to the broader kind-swapped-lead family:
            - once glyph `5`'s prefix is source-backed, applying only the local kind-swapped-lead fixes inside glyph `5` is already enough to advance the first failure to glyph `6`
        - practical reading: the current causal regions are the prefix plus the single post-hintmask byte `72`, not the middle diff cluster or the `hintmask/mask` bytes that immediately precede it
      - `glyph 6` (`percent`) is now coarsely characterized too:
        - patching only the prefix through the first draw boundary changes the failure from `an invalid operator occurred` to `an invalid amount of items are in an arguments stack`, but still fails at glyph `6`
        - patching everything except a coarse late window `[90, end)` still leaves `an invalid operator occurred`
        - patching everything except a coarse mid window `[30, 90)` leaves only the stack-shape failure
        - the prefix-side invalid-operator trigger is already somewhat localized:
          - patching glyph `6` byte `25` alone is not enough
          - patching glyph `6` byte `27` alone is not enough
          - patching glyph `6` bytes `25` and `27` together is already enough to change the failure class from `an invalid operator occurred` to the later stack-shape error
          - the broader kind-swapped-lead helper is also already sufficient to make that same glyph `6` transition from `an invalid operator occurred` to stack-shape error
        - the late-side invalid-operator trigger is now sharper too:
          - in an otherwise source-backed glyph `6`, leaving only byte `114` rebuilt is enough to retain `an invalid operator occurred`
          - leaving only byte `119` rebuilt is also enough to retain `an invalid operator occurred`
          - but leaving only bytes `[118, 121)` rebuilt no longer keeps glyph `6` stuck; the first failure advances to glyph `7` (`ampersand`) with a stack-shape error
        - a stronger token-aware diagnostic now goes one level above the pairwise lead-swap heuristic:
          - a source-token-span scan of glyph `6` shows the late `111..121` region is ordinary draw-program structure, not a local `hintmask` / `cntrmask` boundary:
            - byte `111` is `hvcurveto`
            - byte `113` is `vmoveto`
            - byte `114` is a `num1`
            - bytes `117` and `119` are `num2_pos` starts
          - with glyphs `0`, `3`, `4`, and `5` source-backed, patching glyph `6` only at source token starts / token-head bytes now advances the first failure to glyph `7` (`ampersand`) with a stack-shape error
          - practical reading:
            - the late glyph `6` blocker is not just the earlier `f7/fb` sign-bucket family
            - operator heads and `num1` token heads also participate
        - glyph `7` (`ampersand`) now shows the same abstraction remains useful one glyph later:
          - on top of the glyph `6` token-start-head baseline, patching glyph `7` token-start heads is already enough to advance the first failure to glyph `8` (`quotesingle`) with `an invalid operator occurred`
          - fully source-backing glyph `7` gives the same `(glyph 8, quotesingle, invalid operator)` result
          - the current glyph `7` diff windows are `[57,60)`, `[98,101)`, `[114,115)`, `[132,133)`, and `[153,154)`
          - only `[98,101)` is currently sufficient by itself to advance the failure frontier
          - practical reading:
            - glyph `7` is narrower than glyph `6`
            - the current causal window sits around two adjacent `num2_pos` tokens just before a `rmoveto`
        - glyph `8` (`quotesingle`) currently behaves the same way one step later:
          - on top of the glyph `6` and `7` token-start-head baseline, patching glyph `8` token-start heads is already enough to advance the first failure to glyph `9` (`parenleft`) with `missing moveto operator`
          - fully source-backing glyph `8` gives that same `(glyph 9, parenleft, missing moveto operator)` result
          - practical reading:
            - the current frontier is still moving forward with token-head normalization alone
            - deeper payload-localization can wait until glyph `9` is characterized
        - glyph `9` (`parenleft`) now extends that same chain:
          - on top of the glyph `6`, `7`, and `8` token-start-head baseline, patching glyph `9` token-start heads is already enough to advance the first failure to glyph `10` (`parenright`) with a stack-shape error
          - fully source-backing glyph `9` gives that same `(glyph 10, parenright, stack error)` result
          - practical reading:
            - through glyph `9`, the frontier is still moving via token-head normalization alone
            - glyph `10` is now the next place where we need fresh localization
        - practical reading: `glyph 6` already looks like at least a three-region problem rather than a simple suffix-local corruption
    - the current production rebuild now restores source `FDSelect`; for example regular glyph `103` (`dieresis`) matches source again (`14`)
    - backfilling only the source `CharStrings` data payload now makes `convert_otf_to_ttf()` succeed
    - the tracked bold fixture now shows the same behavior after the new `FDSelect` patch
  - practical interpretation:
    - the current production path is already past the earlier parse-stage blocker
    - the remaining blocker is now narrowed to the transformed `CharStrings` payload
  - new byte-level probes suggest more than one remaining payload transform:
    - regular glyph `42`: `fb d8 ed 0a -> 75 d8 ed 0a`
    - regular glyph `194`: `e8 ac 0a -> ac f5 0a`
    - bold glyph `42`: `fb e7 e4 0a -> 77 e7 e4 0a`
    - bold glyph `89`: `20 fb 6b 1d -> 20 fb 15 1d`
  - broader regular-family byte scans now show those probes are part of a larger repeated family:
    - all source/rebuilt `CharStrings` keep identical byte lengths
    - only a small minority of glyphs are byte-identical
    - the strongest recurring mismatch family is a first-byte category swap inside stable 2-byte slots:
      - source number-lead bytes such as `f7` / `fb` repeatedly become operator-range bytes such as `15`, `06`, `07`, or `0a`
      - and the reverse family where operator bytes such as `13`, `08`, `0a`, or `15` become number-lead bytes such as `f7` / `fb`
      - the current local regular top pairs are:
        - `f7 -> fb` (`12465`)
        - `fb -> f7` (`10811`)
        - `13 -> f7` (`7698`)
        - `f7 -> 15` (`6776`)
        - `f7 -> 06` (`6601`)
        - `f7 -> 07` (`6302`)
      - in a majority of those pairwise examples, the second byte in the 2-byte slot remains unchanged
    - practical reading:
      - one transform often rewrites the earliest operand bytes around `callsubr` / `callgsubr` / first-stem arguments
      - another transform can rewrite later outline numbers inside an otherwise recognizable program such as `.notdef`
      - more generally, this increasingly looks like a stable local state-machine or token-kind rewrite bug
      - glyph `5`'s `f7 04 -> 07 04` is not an isolated oddity but a member of the dominant global family
      - the current diagnostic kind-swapped-lead helper is already strong enough to explain:
        - glyph `5`'s late byte-72 repair, once its prefix has been normalized
        - glyph `6`'s prefix-side invalid-operator family, without source-backing the whole glyph prefix
      - a source-token-start / token-head patcher is a better next abstraction than raw byte-diff heuristics:
        - it preserves source tokenization without immediately source-backing whole payloads
        - it already advances the regular first-failure frontier one glyph further than the kind-swapped helper (`glyph 7` instead of `glyph 6`)
        - it also holds for glyph `7`, where token-start-head patching performs as well as full-source patching
        - it now also holds for glyph `8`, where token-start-head patching again matches full-source patching for the current frontier
        - it now also holds for glyph `9`, where token-start-head patching again matches full-source patching for the current frontier
  - `CharStrings INDEX` is therefore no longer the main blocker

## Verified Test State

The current proof point for this branch is:

```bash
cargo test -p fonttool-cff --test office_static -- --nocapture
```

Current expected result:

- all `office_static` crate tests pass
- this now includes a local optional regression that rebuilds all seven Source Han Sans SC `fntdata` entries from `testdata/OTF_CFF/SourceHanSansSC/full.pptx` when those local cases are present
- the focused regular-case donor-shell tests also prove:
  - `rebuild_office_static_cff_sfnt()` materializes a standard parseable `OTTO`
  - the deeper allsorts/CFF outline path is still blocked and is recorded as an expected current failure
  - optional local source-backed characterization tests also prove:
    - full source backfill from `global_subrs_data_start` onward is a successful conversion control
    - partial source backfill of `Global Subr` data plus the `FDArray` tail is not enough; it still fails during outline visitation
    - `.notdef` keeps its Type 2 token prefix through token `27` before diverging midstream
    - small regular/bold `callsubr` / `callgsubr` glyphs expose stable byte-level operand-prefix rewrites
    - extending the early-glyph backfill chain advances the first failure from `quotedbl` to `numbersign`, then `dollar`, then `percent`
    - fixing `.notdef` alone does not finish the font; it advances the first failing glyph to `quotedbl`, and fixing `quotedbl` advances it again to `numbersign`
    - glyph `3` and glyph `4` each localize to a post-first-draw suffix region
    - glyph `5` now localizes further to `prefix + byte 72`; the current middle diff cluster, byte `73`, and the preceding `hintmask/mask` bytes `[70,72)` do not need to be source-backed to move the failure onward
    - glyph `6` now has a coarse three-region characterization: prefix and late-tail both participate in the invalid-operator class, a mid region remains after those are repaired, the prefix-side invalid-operator trigger is already narrowed to bytes `25` and `27`, and the late side now has two strong single-byte suspects (`114` and `119`)
    - the current best local diagnostic for glyph `6` is now source token-start / token-head patching rather than the narrower kind-swapped-lead heuristic:
      - the kind-swapped helper lowers glyph `6` from `invalid operator` to stack-shape error
      - the token-start helper advances the first failure to glyph `7` (`ampersand`) while keeping the error class at stack-shape
    - glyph `7` is now the next characterized step in that same chain:
      - token-start-head patching advances the first failure to glyph `8` (`quotesingle`)
      - the current minimum causal window is `[98,101)`, while the other four local diff windows are not individually sufficient
    - glyph `8` is now characterized one step further:
      - token-start-head patching advances the first failure to glyph `9` (`parenleft`)
      - full-source patching of glyph `8` does not outperform token-head patching at the current frontier
    - glyph `9` is now characterized one step further:
      - token-start-head patching advances the first failure to glyph `10` (`parenright`)
      - full-source patching of glyph `9` does not outperform token-head patching at the current frontier

## Still Out Of Scope

- generic Office static-CFF support outside the currently tracked Source Han Sans SC family
- generic repair of the transformed CID-keyed structures inside the rebuilt `CFF ` table
- CLI decode wiring to the rebuilt CFF path
- end-to-end `convert --to ttf` acceptance for the PowerPoint bold fixture
