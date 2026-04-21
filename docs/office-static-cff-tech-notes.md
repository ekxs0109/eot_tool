# Office Static CFF Technical Notes

Last updated: 2026-04-21

## Scope

This note only tracks the bounded Office static CFF work in this branch. It does not claim generic Office static CFF support.

Tracked fixtures:

- `testdata/otto-cff-office.fntdata`
- `testdata/presentation1-font2-bold.fntdata`

Tracked source-derived standard prefixes:

- `testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin`
- `testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin`

## Office Intermediate Invariants

- After MTX/LZ decode, the Office static CFF intermediate still begins with `OTTO`.
- The known `CFF `-like region begins at decoded offset `0x20e`.
- The first four bytes at that region are `04 03 00 01`, not a standard CFF header.
- `office_cff_suffix[4..]` is not directly standard-parsable as `Name INDEX`.
  - regular begins `01 02 18 ...`
  - bold begins `01 02 15 ...`

## Corrected Task 3 Strategy

The earlier raw-INDEX Task 3 pseudocode was invalid on real Office bytes.

What is true:

- `2806` and `3362` are source-side Global Subr data-start landmarks.
- Office tail reuse begins two bytes earlier:
  - regular tail splice at `2804`
  - bold tail splice at `3360`
- `935` (regular) and `875` (bold) are useful Office-side landmarks for the transformed Global Subr header zone, but they are not valid standard raw INDEX starts.

The bounded implementation in this branch is therefore:

1. detect the known Office regular/bold prefix shape
2. select the tracked standard prefix through Global Subr data start
3. rebuild `CFF ` as `tracked_standard_prefix + office tail`

## Current Implementation State

- `fonttool_cff::office_static::extract_office_static_cff()` is implemented and tested.
- `fonttool_cff::office_static::rebuild_office_static_cff_table()` is implemented for the two tracked fixtures using bounded prefix grafting.
- `fonttool_cff::office_static::rebuild_office_static_cff_sfnt()` is still a stub.

## Verified Test State

The current proof point for this branch is:

```bash
cargo test -p fonttool-cff --test office_static -- --nocapture
```

Current expected result:

- all `office_static` crate tests pass

## Still Out Of Scope

- `CharStrings INDEX` reconstruction
- full standard SFNT rebuild from the Office intermediate
- CLI decode wiring to the rebuilt CFF path
- end-to-end `convert --to ttf` acceptance for the PowerPoint bold fixture

