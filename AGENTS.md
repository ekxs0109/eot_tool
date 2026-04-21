# Office Static CFF Working Notes

Last updated: 2026-04-21

## Current Scope

- This branch is only for the bounded Office static CFF rebuild milestone.
- The active fixtures are:
  - `testdata/otto-cff-office.fntdata`
  - `testdata/presentation1-font2-bold.fntdata`
- The tracked standard prefix fixtures are:
  - `testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin`
  - `testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin`

## Stable Facts

- `extract_office_static_cff()` is the only supported parser entrypoint for the Office intermediate wrapper.
- The extractor contract is:
  - accepts either plain `OTTO...`
  - or one leading `0x00` followed by `OTTO...`
  - normalizes to `sfnt_bytes` beginning with `OTTO`
  - exposes `cff_offset = 0x20e`
  - exposes `office_cff_suffix = &sfnt_bytes[0x20e..]`
- `office_cff_suffix` is not a bounded standard `CFF ` table slice.

## Do Not Repeat

- Do not try to parse `office_cff_suffix[4..]` as a standard raw `Name INDEX`.
- Do not treat `2806` or `3362` as Office-side `Global Subr INDEX` starts.
- Do not treat `935` (regular) or `875` (bold) as valid raw standard INDEX starts either.
- Do not broaden this branch into `CharStrings INDEX`, `charset`, `FDSelect`, or CLI decode wiring yet.

## Current Task 3 Boundary

- The bounded Task 3 strategy is fixture-coupled prefix grafting.
- Regular fixture:
  - tracked standard prefix length = `2806`
  - Office tail splice offset = `2804`
- Bold fixture:
  - tracked standard prefix length = `3362`
  - Office tail splice offset = `3360`
- `rebuild_office_static_cff_table()` now rebuilds:
  - `tracked_standard_prefix + office_cff_suffix[office_tail_start..]`
- `rebuild_office_static_cff_sfnt()` remains intentionally unimplemented.

## Validation State

- `cargo test -p fonttool-cff --test office_static -- --nocapture` passes on this branch.
- The focused rebuild tests now prove:
  - the rebuilt bytes start with the tracked standard prefix
  - the post-prefix bytes splice into the expected Office tail
  - the rebuilt prefix exposes standard `Name`, `Top DICT`, `String`, and `Global Subr` framing

