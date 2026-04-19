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
- Decoded output: a static `OTTO + CFF` font
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- Compatibility note: PowerPoint may accept static `OTF/CFF` while rejecting variable `OTF/CFF2`, so Office compatibility is tested with this static fixture while variable support remains covered through source fixtures and roundtrip tests
