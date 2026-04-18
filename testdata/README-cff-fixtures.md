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

## `testdata/otto-cff2-variable.fntdata`

- Source font: `SourceHanSansVFProtoCN.otf`
- Source repository: `https://github.com/adobe-fonts/variable-font-collection-test`
- Raw download URL used: `https://raw.githubusercontent.com/adobe-fonts/variable-font-collection-test/97a0a3b3f0941ffecdc6c81d9fea59b4d76e8a5a/OTF/SourceHanSansVFProtoCN.otf`
- Extraction provenance: the fixture was extracted from local `Presentation1.pptx` after embedding `SourceHanSansVFProtoCN.otf` in PowerPoint, then reading `ppt/fonts/font1.fntdata`
- Fixture identity: `sha256 f123da5808f6a5954cada6c7cf1896196ffc1ad40a4b485f175b54c5cf34d6e4`
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- To refresh the fixture, embed the same source font in a local `Presentation1.pptx`, extract `ppt/fonts/font1.fntdata`, overwrite `testdata/otto-cff2-variable.fntdata`, and verify the checksum changes intentionally
