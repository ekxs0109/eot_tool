# CFF Fixture Notes

## `testdata/cff-static.otf`

- SHA-256: `21c56133f7c02e2d9acc4b6aaa2965fe63572a212393ad369d3afb3da453d591`
- Exact upstream file: `https://raw.githubusercontent.com/fonttools/fonttools/main/Tests/ttx/data/TestOTF.otf`
- Verified name-table identity:
  - family: `Test OTF`
  - PostScript: `TestOTF-Regular`
  - unique ID: `FontTools: Test OTF: 2015`
- Current provenance and regeneration note:
  - this file was already used by OTF-focused tests in the main workspace before Task 2, but it was not tracked in git there
  - Task 2 formalized it as a tracked fixture so fresh worktrees remain reproducible
  - the upstream raw file at the URL above was verified to match the tracked fixture exactly at the same SHA-256
  - to refresh it, download that raw URL, overwrite `testdata/cff-static.otf`, and verify the checksum still matches intentionally
- Why it is tracked:
  - existing OTF inspection/subsetting tests already depended on it
  - tracking it in the repository removes the old local-untracked dependency and makes new worktrees self-contained

## `testdata/cff2-variable.otf`

- SHA-256: `2dc8227c1e152d5fc0afb0bf46e9d3445e2208b9bf923c73759de78db09fbfe9`
- Exact upstream source file: `https://raw.githubusercontent.com/fonttools/fonttools/main/Tests/varLib/data/master_cff2/TestCFF2_Regular.ttx`
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
  - to refresh it, download that `.ttx`, compile it to OTF with FontTools `ttx`, overwrite `testdata/cff2-variable.otf`, and verify or intentionally update the tracked checksum
- Why it is tracked:
  - existing OTF inspection/subsetting tests already depended on it
  - tracking it in the repository removes the old local-untracked dependency and makes new worktrees self-contained

## `testdata/otto-cff2-variable.fntdata`

- Source font: `SourceHanSansVFProtoCN.otf`
- Source repository: `https://github.com/adobe-fonts/variable-font-collection-test`
- Raw download URL used: `https://raw.githubusercontent.com/adobe-fonts/variable-font-collection-test/master/OTF/SourceHanSansVFProtoCN.otf`
- Extraction provenance: the fixture was extracted from local `Presentation1.pptx` after embedding `SourceHanSansVFProtoCN.otf` in PowerPoint, then reading `ppt/fonts/font1.fntdata`
- Fixture identity: `sha256 f123da5808f6a5954cada6c7cf1896196ffc1ad40a4b485f175b54c5cf34d6e4`
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`
- To refresh the fixture, embed the same source font in a local `Presentation1.pptx`, extract `ppt/fonts/font1.fntdata`, overwrite `testdata/otto-cff2-variable.fntdata`, and verify the checksum changes intentionally
