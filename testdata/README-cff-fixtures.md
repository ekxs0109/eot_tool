# CFF Fixture Notes

- Fixture: `testdata/otto-cff2-variable.fntdata`
- Source font: `SourceHanSansVFProtoCN.otf`
- Source repository: `https://github.com/adobe-fonts/variable-font-collection-test`
- Raw download URL used: `https://raw.githubusercontent.com/adobe-fonts/variable-font-collection-test/master/OTF/SourceHanSansVFProtoCN.otf`
- Extraction provenance: the fixture was extracted from local `Presentation1.pptx` after embedding `SourceHanSansVFProtoCN.otf` in PowerPoint, then reading `ppt/fonts/font1.fntdata`.
- Fixture identity: `sha256 f123da5808f6a5954cada6c7cf1896196ffc1ad40a4b485f175b54c5cf34d6e4`
- The repository keeps the extracted `.fntdata` only; it does not keep the `.pptx`.
- To refresh the fixture, embed the same source font in a local `Presentation1.pptx`, extract `ppt/fonts/font1.fntdata`, overwrite `testdata/otto-cff2-variable.fntdata`, and verify the checksum changes intentionally.
