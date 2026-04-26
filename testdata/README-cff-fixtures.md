# CFF Fixture Notes

## `testdata/cff-static.otf`

- SHA-256: `21c56133f7c02e2d9acc4b6aaa2965fe63572a212393ad369d3afb3da453d591`
- Exact upstream file: `https://raw.githubusercontent.com/fonttools/fonttools/34be2443a7539b0b36467e9cca63ef53d4506fd6/Tests/ttx/data/TestOTF.otf`
- Verified name-table identity:
  - family: `Test OTF`
  - PostScript: `TestOTF-Regular`
  - unique ID: `FontTools: Test OTF: 2015`
- Current provenance and regeneration note:
  - this file was already used by OTF-focused tests in the main workspace before Task 2, but it was not tracked in git there
  - Task 2 formalized it as a tracked fixture so fresh worktrees remain reproducible
  - the upstream raw file at the URL above was verified to match the tracked fixture byte-for-byte at the same SHA-256
  - to refresh it, download that raw URL, overwrite `testdata/cff-static.otf`, and verify the checksum still matches intentionally
- Why it is tracked:
  - existing OTF inspection/subsetting tests already depended on it
  - tracking it in the repository removes the old local-untracked dependency and makes new worktrees self-contained

## `testdata/cff2-variable.otf`

- SHA-256: `2dc8227c1e152d5fc0afb0bf46e9d3445e2208b9bf923c73759de78db09fbfe9`
- Exact upstream source TTX: `https://raw.githubusercontent.com/fonttools/fonttools/34be2443a7539b0b36467e9cca63ef53d4506fd6/Tests/varLib/data/master_ttx_varfont_otf/TestCFF2VF.ttx`
- Verified name-table identity:
  - family: `Source Code Variable`
  - PostScript: `SourceCodeVariable-Roman`
  - unique ID: `1.010;ADBO;SourceCodeVariable-Roman`
  - version string: `Version 1.010;hotconv 1.0.109;makeotfexe 2.5.65596`
  - CFF font name: `SourceCodeVariable-Roman`
- Current provenance and regeneration note:
  - this file was already used by OTF-focused tests in the main workspace before Task 2, but it was not tracked in git there
  - Task 2 formalized it as a tracked fixture so fresh worktrees remain reproducible
  - the upstream `.ttx` source at the URL above was verified to contain the identifying strings listed here
  - exact build command:
    `uvx --from=fonttools==4.58.0 ttx -q -o <tmp>/cff2-variable.otf <tmp>/TestCFF2VF.ttx`
  - exact post-process requirement:
    patch the generated font so `head.modified` is fixed to `3858659943`, then recalculate the `head` table checksum entry and the global `checkSumAdjustment`
  - concise post-process snippet:
    ```python
    import struct
    from pathlib import Path

    data = bytearray(Path("cff2-variable.otf").read_bytes())
    num_tables = struct.unpack(">H", data[4:6])[0]

    head_offset = None
    head_length = None
    for i in range(num_tables):
        record = 12 + i * 16
        tag = bytes(data[record:record + 4])
        if tag == b"head":
            head_offset = struct.unpack(">I", data[record + 8:record + 12])[0]
            head_length = struct.unpack(">I", data[record + 12:record + 16])[0]
            head_record = record
            break

    assert head_offset is not None
    struct.pack_into(">Q", data, head_offset + 28, 3858659943)
    struct.pack_into(">I", data, head_offset + 8, 0)

    def checksum(buf):
        padded = bytes(buf) + b"\0" * ((4 - len(buf) % 4) % 4)
        total = 0
        for j in range(0, len(padded), 4):
            total = (total + struct.unpack(">I", padded[j:j + 4])[0]) & 0xFFFFFFFF
        return total

    head_sum = checksum(data[head_offset:head_offset + head_length])
    struct.pack_into(">I", data, head_record + 4, head_sum)
    whole = bytearray(data)
    struct.pack_into(">I", whole, head_offset + 8, 0)
    adjust = (0xB1B0AFBA - checksum(whole)) & 0xFFFFFFFF
    struct.pack_into(">I", data, head_offset + 8, adjust)
    Path("cff2-variable.otf").write_bytes(data)
    ```
  - this exact source, build command, and post-process reproduces the tracked fixture checksum above
- Why it is tracked:
  - existing OTF inspection/subsetting tests already depended on it
  - tracking it in the repository removes the old local-untracked dependency and makes new worktrees self-contained

## `testdata/OpenSans-Regular.woff`

- SHA-256: `87fc29c5f45b0e8be6823ab7d4b6a8a29e4d0f425a2466aa09edd8eb1d3c28c4`
- Derived from tracked source: `testdata/OpenSans-Regular.ttf`
- Generation command:
  `/Users/ekxs/Codes/eot_tool/build/venv/bin/python - <<'PY' ... TTFont(src).flavor = "woff" ... PY`
- Regeneration note:
  - load `testdata/OpenSans-Regular.ttf` with `fontTools.ttLib.TTFont`
  - set `flavor = "woff"`
  - save to `testdata/OpenSans-Regular.woff`
- Why it is tracked:
  - it provides a stable container-format fixture for allsorts source-materialization tests
  - it keeps WOFF coverage self-contained in fresh worktrees

## `testdata/OpenSans-Regular.woff2`

- SHA-256: `7ee263235c498b56e5486bc162b48149de23745f8d33ea73cb9770007dcfb4b4`
- Derived from tracked source: `testdata/OpenSans-Regular.ttf`
- Generation prerequisites:
  - `fontTools`
  - the Python `brotli` extension so `fontTools` can write WOFF2
- Generation command:
  `/Users/ekxs/Codes/eot_tool/build/venv/bin/python - <<'PY' ... TTFont(src).flavor = "woff2" ... PY`
- Regeneration note:
  - load `testdata/OpenSans-Regular.ttf` with `fontTools.ttLib.TTFont`
  - set `flavor = "woff2"`
  - save to `testdata/OpenSans-Regular.woff2`
- Why it is tracked:
  - it provides a stable WOFF2 fixture for allsorts source-materialization tests
  - it avoids depending on local ad-hoc webfont generation in each worktree

## `testdata/cff-static.woff`

- SHA-256: `3815c1b303964250b93c216e7e75951d53d99f9300ca9505384835ab895f5e76`
- Derived from tracked source: `testdata/cff-static.otf`
- Generation command:
  `/Users/ekxs/Codes/eot_tool/build/venv/bin/python - <<'PY' ... TTFont(src).flavor = "woff" ... PY`
- Regeneration note:
  - load `testdata/cff-static.otf` with `fontTools.ttLib.TTFont`
  - set `flavor = "woff"`
  - save to `testdata/cff-static.woff`
- Why it is tracked:
  - it provides a stable static-CFF webfont fixture for allsorts materialization and convert coverage
  - it keeps the verified `WOFF(CFF) -> OTF(CFF) -> TTF` path self-contained in fresh worktrees

## `testdata/otto-cff2-variable.fntdata`

- Source font: `SourceHanSansVFProtoCN.otf`
- Source repository: `https://github.com/adobe-fonts/variable-font-collection-test`
- Raw download URL used: `https://raw.githubusercontent.com/adobe-fonts/variable-font-collection-test/97a0a3b3f0941ffecdc6c81d9fea59b4d76e8a5a/OTF/SourceHanSansVFProtoCN.otf`
- Extraction provenance: the fixture was extracted from local `Presentation1.pptx` after embedding `SourceHanSansVFProtoCN.otf` in PowerPoint, then reading `ppt/fonts/font1.fntdata`
- Fixture identity: `sha256 f123da5808f6a5954cada6c7cf1896196ffc1ad40a4b485f175b54c5cf34d6e4`
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- To refresh the fixture, embed the same source font in a local `Presentation1.pptx`, extract `ppt/fonts/font1.fntdata`, overwrite `testdata/otto-cff2-variable.fntdata`, and verify the checksum changes intentionally

## `testdata/otto-cff-office.fntdata`

- Source path: `ppt/fonts/font1.fntdata` inside the current local `Presentation1.pptx`
- Extraction provenance: the fixture is extracted from the local `Presentation1.pptx` and represents the real Office-compatible embedded path
- Fixture identity: `sha256 06ee8f3c7e480590ec6121608ad27de2f80d64ea3f04a7f77fbbae2c20e577f9`
- Decoded output today:
  - current Rust decode writes the known Office static-CFF intermediate with the nonstandard `OTTO 0001 0200 0004 0010` prefix
  - the focused Office fixture regression for this path stops at that shallow intermediate-shape check
  - stronger later `convert --to ttf` boundary validation is covered separately by the bold `presentation1-font2-bold.fntdata` regression
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- Compatibility note: PowerPoint may accept static `OTF/CFF` while rejecting variable `OTF/CFF2`, so Office compatibility is tested with this static fixture while variable support remains covered through source fixtures and roundtrip tests

## `testdata/presentation1-font2-bold.fntdata`

- Source path: `ppt/fonts/font2.fntdata` inside the current local `Presentation1.pptx`
- Extraction provenance: the fixture is extracted from the local `Presentation1.pptx` and represents the real Office-compatible embedded Source Han Sans SC Bold path
- Fixture identity: `sha256 0c716d9a54f71708cc9c8c172d498b86dadd3baccccb87a4a530a6440acaf474`
- Decoded output today:
  - current Rust decode writes the known Office static-CFF intermediate with the nonstandard `OTTO 0001 0200 0004 0010` prefix
  - the focused regression for this fixture first checks that intermediate shape, then tries the later `convert --to ttf` input boundary
  - on the current synced baseline, that later step still fails because `convert` rejects the decoded output as not `OTF/CFF` or `OTF/CFF2`
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- To refresh the fixture, embed the same font in local PowerPoint, extract `ppt/fonts/font2.fntdata`, overwrite `testdata/presentation1-font2-bold.fntdata`, and update the recorded checksum intentionally

## `testdata/sourcehan-sc-regular-cff-prefix-through-global-subrs.bin`

- Derived from local source: `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
- Extraction method:
  - locate the `CFF ` table inside the source OTF
  - slice the first `2806` bytes
- Meaning of the cut:
  - the file contains the standard source `CFF ` bytes through the start of Global Subr data
  - this includes the standard CFF header plus `Name`, `Top DICT`, `String`, and `Global Subr` INDEX framing
- Why it is tracked:
  - Office static CFF Task 3 is fixture-bounded
  - the rebuild path needs a stable standard prefix reference without depending on local ad-hoc extraction in every worktree

## `testdata/sourcehan-sc-bold-cff-prefix-through-global-subrs.bin`

- Derived from local source: `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
- Extraction method:
  - locate the `CFF ` table inside the source OTF
  - slice the first `3362` bytes
- Meaning of the cut:
  - the file contains the standard source `CFF ` bytes through the start of Global Subr data
  - this includes the standard CFF header plus `Name`, `Top DICT`, `String`, and `Global Subr` INDEX framing
- Why it is tracked:
  - Office static CFF Task 3 is fixture-bounded
  - the rebuild path needs a stable standard prefix reference without depending on local ad-hoc extraction in every worktree

## `testdata/sourcehan-sc-regular-charstrings-offsets.bin`

- Derived from local source: `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
- Binary format:
  - big-endian `u32`
  - one entry per standard `CharStrings INDEX` offset
  - total entries: `65536` (`count + 1`)
- Landmark values:
  - first ten offsets: `1, 77, 80, 114, 163, 257, 368, 504, 658, 687`
  - last three offsets: `14059736, 14059738, 14059740`
- Why it is tracked:
  - the Office static-CFF CharStrings decoder needs a stable standard reference array for regression tests
  - this keeps future worktree runs independent from ad-hoc source extraction

## `testdata/sourcehan-sc-bold-charstrings-offsets.bin`

- Derived from local source: `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
- Binary format:
  - big-endian `u32`
  - one entry per standard `CharStrings INDEX` offset
  - total entries: `65536` (`count + 1`)
- Landmark values:
  - first ten offsets: `1, 77, 80, 129, 149, 247, 370, 507, 668, 679`
  - last three offsets: `14575011, 14575013, 14575015`
- Why it is tracked:
  - the Office static-CFF CharStrings decoder needs a stable standard reference array for regression tests
  - this keeps future worktree runs independent from ad-hoc source extraction

## Additional Source Han Sans SC prefix fixtures

The following tracked files are generated the same way as the Regular/Bold prefix fixtures above:

- `testdata/sourcehan-sc-extralight-cff-prefix-through-global-subrs.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-ExtraLight.otf`
  - prefix length: `4936`
- `testdata/sourcehan-sc-heavy-cff-prefix-through-global-subrs.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Heavy.otf`
  - prefix length: `5343`
- `testdata/sourcehan-sc-light-cff-prefix-through-global-subrs.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Light.otf`
  - prefix length: `3522`
- `testdata/sourcehan-sc-medium-cff-prefix-through-global-subrs.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Medium.otf`
  - prefix length: `2548`
- `testdata/sourcehan-sc-normal-cff-prefix-through-global-subrs.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Normal.otf`
  - prefix length: `2920`

They exist so the current fixture-bounded Office static-CFF path can cover the full Source Han Sans SC family seen in `testdata/OTF_CFF/SourceHanSansSC/full.pptx`.

## Additional Source Han Sans SC CharStrings offset fixtures

The following tracked files are big-endian `u32` arrays of the standard `CharStrings INDEX` offsets (`count + 1 = 65536` entries), generated the same way as the Regular/Bold offset fixtures above:

- `testdata/sourcehan-sc-extralight-charstrings-offsets.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-ExtraLight.otf`
  - first ten offsets: `1, 77, 80, 109, 151, 248, 351, 475, 624, 649`
- `testdata/sourcehan-sc-heavy-charstrings-offsets.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Heavy.otf`
  - first ten offsets: `1, 77, 80, 122, 162, 271, 395, 540, 701, 726`
- `testdata/sourcehan-sc-light-charstrings-offsets.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Light.otf`
  - first ten offsets: `1, 77, 80, 112, 134, 231, 339, 475, 624, 645`
- `testdata/sourcehan-sc-medium-charstrings-offsets.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Medium.otf`
  - first ten offsets: `1, 77, 80, 113, 148, 245, 360, 496, 652, 664`
- `testdata/sourcehan-sc-normal-charstrings-offsets.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Normal.otf`
  - first ten offsets: `1, 77, 80, 107, 129, 226, 336, 472, 626, 639`

These exist so the bounded Office static-CFF path can patch the standard `CharStrings INDEX` window for every embedded Source Han Sans SC weight currently covered by the local PowerPoint fixture.

## Source Han Sans SC source-backed Global Subr data slices

The following tracked files contain the exact source `CFF ` bytes from `prefix_len .. charset_offset` for each weight:

- `testdata/sourcehan-sc-regular-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf`
  - byte length: `7926`
- `testdata/sourcehan-sc-bold-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Bold.otf`
  - byte length: `9330`
- `testdata/sourcehan-sc-extralight-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-ExtraLight.otf`
  - byte length: `16204`
- `testdata/sourcehan-sc-heavy-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Heavy.otf`
  - byte length: `16347`
- `testdata/sourcehan-sc-light-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Light.otf`
  - byte length: `10331`
- `testdata/sourcehan-sc-medium-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Medium.otf`
  - byte length: `6874`
- `testdata/sourcehan-sc-normal-cff-global-subrs-data.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Normal.otf`
  - byte length: `8068`

Why they are tracked:

- the bounded Office static-CFF rebuild now patches this source-backed range in production
- these slices restore the standard `Global Subr` data region without requiring local ad-hoc extraction in every worktree
- they help keep the rebuilt CFF past the earlier parse-stage failure

## Source Han Sans SC source-backed `FDSelect` slices

The following tracked files contain the exact source `FDSelect` format-3 bytes for each weight:

- `testdata/sourcehan-sc-regular-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-bold-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Bold.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-extralight-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-ExtraLight.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-heavy-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Heavy.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-light-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Light.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-medium-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Medium.otf`
  - byte length: `344`
- `testdata/sourcehan-sc-normal-cff-fdselect.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Normal.otf`
  - byte length: `344`

Why they are tracked:

- Office transforms `FDSelect` across the full Source Han Sans SC family even when the format-3 wrapper shape stays stable
- the bounded Office static-CFF rebuild now patches this source-backed range in production
- these slices keep the current rebuild past the remaining `FDSelect`-specific outline failures without depending on local ad-hoc extraction

## Source Han Sans SC source-backed `FDArray..end` tails

The following tracked files contain the exact source `CFF ` bytes from `fdarray_offset .. end` for each weight:

- `testdata/sourcehan-sc-regular-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Regular.otf`
  - byte length: `1284423`
- `testdata/sourcehan-sc-bold-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Bold.otf`
  - byte length: `1200677`
- `testdata/sourcehan-sc-extralight-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-ExtraLight.otf`
  - byte length: `1586853`
- `testdata/sourcehan-sc-heavy-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Heavy.otf`
  - byte length: `1262486`
- `testdata/sourcehan-sc-light-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Light.otf`
  - byte length: `1350172`
- `testdata/sourcehan-sc-medium-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Medium.otf`
  - byte length: `1237327`
- `testdata/sourcehan-sc-normal-cff-fdarray-tail.bin`
  - source: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-Normal.otf`
  - byte length: `1308171`

Why they are tracked:

- the bounded Office static-CFF rebuild now patches this later CID tail in production
- these tails restore the source `FDArray`, later private dict data, and later local-subr region as one stable slice
- they move the current rebuild past parse rejection and expose the remaining outline-stage blocker more cleanly

## Source Han Sans SC donor SFNT shells without `CFF `

The following tracked files are standard parseable `OTTO` shells built from the matching source OTFs with the `CFF ` table removed and all remaining tables reserialized with fresh standard directory/checksum data:

- `testdata/sourcehan-sc-regular-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-bold-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-extralight-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-heavy-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-light-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-medium-sfnt-without-cff.otf`
- `testdata/sourcehan-sc-normal-sfnt-without-cff.otf`

Common properties:

- source family: `testdata/OTF_CFF/SourceHanSansSC/SourceHanSansSC-*.otf`
- container flavor: standard `OTTO`
- table count: `16`
- intentionally missing table: `CFF `
- intended use in this branch:
  - act as donor wrappers for `rebuild_office_static_cff_sfnt()`
  - let the code rebuild a standard SFNT wrapper without trying to invert the Office faux directory

Why they are tracked:

- the donor-shell wrapper rebuild is now part of the bounded Office static-CFF implementation
- tracking them avoids depending on local ad-hoc source extraction at test/runtime
- they keep the current branch reproducible while the deeper CID-keyed CFF structures (`charset`, `FDSelect`, `FDArray`) are still under reverse-engineering
