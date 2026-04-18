# CFF Fixture Notes

## `testdata/cff-static.otf`

- SHA-256: `21c56133f7c02e2d9acc4b6aaa2965fe63572a212393ad369d3afb3da453d591`
- Verified name-table identity:
  - family: `Test OTF`
  - PostScript: `TestOTF-Regular`
  - unique ID: `FontTools: Test OTF: 2015`
- Current provenance and regeneration note:
  - this file was already used by OTF-focused tests in the main workspace before Task 2, but it was not tracked in git there
  - Task 2 formalized it as a tracked fixture so fresh worktrees remain reproducible
  - the exact original upstream file path is not currently proven, so this README records the current verified identity instead of inventing stronger provenance
  - if it is ever replaced, keep the verified identity story explicit and update the checksum and name-table strings intentionally
- Why it is tracked:
  - existing OTF inspection/subsetting tests already depended on it
  - tracking it in the repository removes the old local-untracked dependency and makes new worktrees self-contained

## `testdata/cff2-variable.otf`

- SHA-256: `2dc8227c1e152d5fc0afb0bf46e9d3445e2208b9bf923c73759de78db09fbfe9`
- Verified name-table identity:
  - family: `Source Code Variable`
  - PostScript: `SourceCodeVariable-Roman`
  - unique ID: `1.010;ADBO;SourceCodeVariable-Roman`
- Current provenance and regeneration note:
  - this file was already used by OTF-focused tests in the main workspace before Task 2, but it was not tracked in git there
  - Task 2 formalized it as a tracked fixture so fresh worktrees remain reproducible
  - its verified release context matches Adobe's Source Code Pro repository and releases page, including the `variable-fonts` / `1.010` release line: `https://github.com/adobe-fonts/source-code-pro`
  - if it is ever refreshed, replace it from the matching Adobe Source Code Pro variable-font release context, then update the checksum and name-table strings intentionally
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
