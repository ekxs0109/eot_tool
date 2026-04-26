# Presentation1 Font Debug Notes

Last updated: 2026-04-20

## Embedded Font Identity

- `font1.fntdata`: family=`思源黑体\0`, style=`Regular\0`, full=`Source Han Sans SC`
- `font2.fntdata`: family=`思源黑体\0`, style=`Bold\0`, full=`Source Han Sans SC Bold`
- `font3.fntdata`: family=`Calibri`, style=`Bold Italic`, full=`Calibri Bold Italic`

## Font1 Findings

These findings are already verified on Windows PowerPoint and should be treated as baseline facts to avoid repeating dead-end experiments.

- `font1` is the main solved path so far.
- `font1` header-only failure was caused by missing trailing UTF-16 `NUL` in `family/style`.
- `Presentation1-exp-font1-newHeaderOrigPayload-plusNul.pptx` is OK.
- `font1` MTX container must keep all 3 blocks present.
- Original payload with `block2/block3` both removed fails.
- Original payload with only `block2` kept fails.
- Original payload with only `block3` kept fails.
- Therefore Office requires the `block1 + empty block2 + empty block3` shape for this font.

## Font1 Payload Conclusions

- The main incompatibility for `font1` was not decoded SFNT content; it was MTX compression shape.
- Original `font1` uses `copy_dist=9000`.
- Original `font1` `block1` LZ analysis showed `max_copy_distance=8998`.
- Roundtrip payloads that used modern large-window backreferences were rejected by PowerPoint.
- Recompressing `font1` `block1` with bounded max distance fixed compatibility.
- Verified good experiment packages:
  - `Presentation1-exp-font1-boundedBlock1-maxDist9000.pptx`
  - `Presentation1-exp-font1-boundedBlock1-maxDist8998.pptx`

## Font1 Do Not Repeat

- Do not assume changing only EOT header metadata fixes `font1`.
- Do not remove empty `block2/block3` for `font1`.
- Do not use roundtrip `font1` payloads with oversized LZ window / oversized real backreferences.
- Keep `copy_dist=9000`, preserve 3-block structure, and preserve trailing UTF-16 `NUL` in `family/style`.

## Font2 Decode Findings

These are current Rust decode findings for the original `Presentation1.pptx` embedded `font2.fntdata`.

- `font2` is `思源黑体 Bold` and is the current decode failure case.
- Current CLI regression fixture:
  - `build/presentation1_more_experiments/orig/ppt/fonts/font2.fntdata`
- Current failing regression test:
  - `decode_presentation1_font2_fntdata_writes_otto_sfnt`
- Current failure is still:
  - `failed to decode MTX block1: lz stream contains an invalid back-reference`
- `font2` MTX container header is:
  - `num_blocks=3`
  - `copy_dist=9000`
  - `block1.len=14539322`
  - `block2.len=4`
  - `block3.len=4`
- `font2` `block2` and `block3` are both the empty 4-byte stream `00000000`.
- Hooking decode to respect MTX `copy_dist` is correct and does not break baseline `font1` decode, but it does **not** fix `font2`.
- After wiring `copy_dist` into decode, `font2` still fails inside block1 LZ decode, which means the remaining issue is **not** just the copy-limit/ring-buffer compatibility gap.
- `font2` block1 first 4 bytes are suspicious under the current LZ interpretation:
  - raw prefix: `016bb24d`
  - current decoder reads decompressed length `186212`, which is not plausible for a `14.5MB` compressed block
- `font1` block1 behaves normally under the same decoder:
  - raw prefix: `7e1cd449`
  - current decoder reads decompressed length `16529832`
  - actual decoded output size is `16529832`
- Strong current hypothesis:
  - `font2` uses a different or prefixed block1 header variant, and the mismatch likely happens at the very start of block1 interpretation rather than only in later back-reference validation.
- A wider temporary probe sweep was run against `font2` block1:
  - tested `flag24`, `flag31`, byte-aligned variants, `u24@1..4`, `u32@0..4`, and `tail_u32`
  - tested header bit offsets `0..31`
  - tested stream byte offsets up to `32`
  - scanned decoded prefix bytes for `OTTO`, `00010000`, `true`, `ttcf`, and `00020000`
- Result of that wider sweep:
  - `font1` is a positive control and the same probe finds `OTTO@0` for the normal `flag24` model
  - `font2` has **no magic hits at all** under those models
  - some `font2` candidate models decode a stable 64-byte prefix without immediate failure, but none look like an SFNT header
- Therefore the current evidence is stronger that `font2` is not just “32-bit length instead of 24-bit length”.
- More likely remaining possibilities are:
  - extra wrapper / preamble bytes before the real Huffman stream
  - another transform layer after LZ but before SFNT
  - an Office-specific block1 variant not covered by the current public MTX interpretation
- Important control result from the wider probe:
  - some wrong-model decodes produce a fake directory-looking prefix containing `BASE@11`
  - but the same fake `BASE@11` pattern also appears on `font1` under wrong header/alignment models
  - therefore `BASE@11` is **not** evidence of a `font2`-specific wrapper by itself
- Current probe evidence also does **not** support a simple ODTTF-style repeating XOR key on the decoded prefix:
  - XORing the strongest `font2` candidates against `font1`'s real `OTTO` head does not produce a stable 16-byte or 32-byte repeating pattern
  - several candidate prefixes are dominated by preload-like runs (`fc/fd/fe/ff`, ordered byte ramps, etc.), which is more consistent with wrong-stream decoding than with a cleanly XOR-obfuscated SFNT header

## Font2 Decode Breakthrough

These findings are stronger than the earlier header-variant speculation and should guide the next code change.

- Local static Adobe OTFs are useful as **size references**, but not byte-for-byte truth:
  - `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
  - `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
- Important correction:
  - downloaded static `Regular` has the same size as decoded `font1` (`16529832`) but only a 5-byte common prefix with the real `font1` decode output
  - therefore local Adobe OTFs should **not** be used as exact byte-level expected outputs
- However, the static file sizes are still highly informative:
  - `SourceHanSansSC-Regular.otf` size = `16529832` = `0x00FC39A8`
  - `SourceHanSansSC-Bold.otf` size = `16963428` = `0x0102D764`
  - current MSB-first `font2` header read returns `0x02D764 = 186212`
  - this is exactly `0x0102D764 mod 0x1000000`
- New hard-positive probe result:
  - forcing `font2` block1 decompressed length to `16963428` and starting the Huffman stream at the normal `25` bits (`1 + 24`) makes the full decode succeed
  - the probe wrote `/tmp/font2_knownsize25.otf`
  - `file /tmp/font2_knownsize25.otf` reports `OpenType font data`
  - `strings` inside that output show `SourceHanSaSsSC-Bold`
- Control result:
  - the same forced `known-size + bit25` path also succeeds for `font1`
  - probe wrote `/tmp/font1_knownsize25.otf`
- `font1_knownsize25.otf` and `font2_knownsize25.otf` are **not** the same file:
  - different SHA-256 hashes
  - only the first `32` bytes are identical
  - they diverge from byte `33` onward
- Therefore the remaining `font2` decode bug is now tightly scoped:
  - the MTX block1 payload itself is decodable with the normal stream start
  - the failure is caused by interpreting the decompressed-length field as plain 24-bit `186212`
  - `font2` needs length reconstruction above `0xFFFFFF`, at least for this overflow case

## Font2 Do Not Repeat

- Do not keep spending time on generic XOR theories for `font2` block1.
- Do not keep sweeping random stream starts as the primary theory.
- Do not assume the entire block format is different from `font1`.
- The strongest current evidence is that `font2` uses the normal MTX stream start (`bit 25`) and only the decompressed-length value is truncated / wrapped.

## Font2 Encode Follow-Up

- The decode-side overflow reconstruction fix is now implemented in Rust and the regression `decode_presentation1_font2_fntdata_writes_otto_sfnt` passes.
- However, `font2` encode/repack is still a separate unresolved problem.
- Current hard-negative results from temporary repack experiments:
  - re-encoding decoded `font2` through the current CLI `encode` path is blocked by an existing static-CFF boundary and does not produce `.eot/.fntdata`
  - direct MTX repack with the current shared compressor produces a new `font2.fntdata`, but the result still cannot be decoded back by the current CLI
  - a literal-only MTX repack for `font2` also still fails current CLI decode
- Therefore:
  - `font2` **decode is fixed**
  - `font2` **encode compatibility is not fixed yet**
  - any generated `Presentation1` package using a newly re-encoded `font2` should be treated as an experiment for Office-side behavior, not a verified good package

## Office Static CFF Intermediate Findings

These findings apply to the decoded `font1` / `font2` outputs after the MTX/LZ layer succeeds.

- Task 2 extractor surface is now landed in Rust under `fonttool_cff::office_static`.
  - Current extractor contract:
    - accepts either plain `OTTO...` or a single leading `0x00` followed by `OTTO...`
    - normalizes to `sfnt_bytes` beginning with `OTTO`
    - exposes `cff_offset = 0x20e`
    - exposes `office_cff_suffix`, which is the Office intermediate suffix from `0x20e` onward
- Important API boundary:
  - `office_cff_suffix` is **not** a bounded standard `CFF ` table slice.
  - It is intentionally named as a suffix/region view only, because slicing from `0x20e` to EOF includes more than a clean standard CFF table boundary.
- Task 2 extractor coverage now includes:
  - both tracked fixtures:
    - `testdata/otto-cff-office.fntdata`
    - `testdata/presentation1-font2-bold.fntdata`
  - exact-length acceptance at `cff_offset + 4`
  - single-leading-`NUL` normalization before `OTTO`
- Current decode output for both `font1` and `font2` is **not** a standard static OTF. It is better modeled as an Office-specific static CFF intermediate representation.
- `font1` and `font2` share the same nonstandard SFNT header shape:
  - `4f54544f0001020000040010`
  - This is not a legal standard OTF directory header like the original Source Han Sans OTFs.
- The decoded outputs still preserve the expected table tag order near the start of block1:
  - `BASE, CFF , DSIG, GDEF, GPOS, GSUB, OS/2, VORG, cmap, ...`
  - Many checksums still match the source OTF, so this is **not** random corruption.
- The main mismatch is that Office has rewritten directory offsets / lengths and also transformed several table payloads.
- `font1` and `font2` both show a CFF-like payload beginning near decoded offset `0x20c`.
  - `font1` decoded CFF prefix at `0x20c` starts with:
    - `6d7804030001010218536f7572636548616e5361537353432d526567756c6172...`
  - `font2` decoded CFF prefix at `0x20c` starts with:
    - `007804030001010215536f7572636548616e5361537353432d426f6c64...`
- That CFF region is close to the original source OTF CFF table, not unrelated data:
  - `font1`: first `256` bytes of source `CFF ` vs decoded `0x20c` region match in `221/256` byte positions
  - `font2`: first `256` bytes of source `CFF ` vs decoded `0x20c` region match in `220/256` byte positions
- Therefore the static CFF payload is **not** fully re-encoded; most of the CFF body survives and the transform is likely concentrated in the prefix / header area plus a small number of byte substitutions.
- Source `CFF ` structure for `SourceHanSansSC-Regular.otf` is now pinned down:
  - header: `01000403`
  - Name INDEX starts at source CFF offset `4`
    - count=`1`
    - offSize=`1`
    - offsets=`[1, 24]`
    - raw name=`SourceHanSansSC-Regular`
  - Top DICT INDEX starts at source CFF offset `32`
    - count=`1`
    - offSize=`1`
    - offsets=`[1, 71]`
    - DICT payload begins at source CFF offset `37`
  - String INDEX starts at source CFF offset `107`
    - count=`23`
    - offSize=`2`
    - string data begins at source CFF offset `158`
- The decoded Office intermediate does **not** destroy those structures uniformly; instead it appears to rewrite the INDEX headers while leaving much of the DICT body intact.
  - `font1` decoded bytes near `0x210` still resemble the source Name INDEX region.
  - `font1` decoded bytes near `0x230` line up closely with the source Top DICT payload itself.
  - In other words, the Top DICT body looks substantially preserved, but the surrounding CFF header / INDEX framing is altered.
- Useful alignment checkpoint for `font1`:
  - source `CFF[2..130]` vs decoded `font1[0x20e..0x28e]` matches `118/128` bytes in the leading comparison window
  - This is slightly better than naive `0x20c` alignment and suggests the Office intermediate may have displaced the first two CFF header bytes or otherwise shifted the table framing.
- Strong reconstruction checkpoint for `font1`:
  - if we overwrite decoded `font1[0x20e..]` with the source OTF `CFF ` bytes for the first `940` bytes only, `fontTools` can parse the resulting CFF table successfully.
  - patching only the first `937`, `938`, or `939` bytes is **not** enough; those still fail with `offSize too large: 6`.
  - This gives a hard upper bound: the parse-critical Office rewrite reaches through source CFF offset `939`.
- The `940`-byte repair boundary lines up with the start of the source CFF Global Subr INDEX header.
  - source `CFF[937..]` begins with:
    - `03 a4 02 00 01 00 05 00 09 ...`
  - This is consistent with a large Global Subr INDEX (`count=0x03a4`, `offSize=2`, offsets beginning `1, 5, 9, ...`).
  - The aligned decoded Office intermediate does **not** preserve that header verbatim.
- Practical implication:
  - the Office static CFF transform is not limited to the main CFF header / Name INDEX / Top DICT INDEX.
  - it also perturbs at least the early Global Subr INDEX header / offset array.
  - however, the corruption still appears bounded to the early framing zone rather than the full CFF body, because a `940`-byte prefix repair is already enough to make the whole CFF parse.
- Do **not** assume the Office CFF region starts cleanly at `0x20c` with an untouched standard CFF header.
- Do **not** assume `office_cff_suffix` becomes a valid standard Name INDEX stream at offset `4`.
  - A direct standard INDEX parse from `office_cff_suffix[4..]` is already known-bad and fails immediately on current fixtures.
- Do **not** assume the whole `CFF ` table must be rebuilt from scratch.
  - Current evidence is stronger that we will need to reconstruct standard CFF INDEX framing around largely preserved payload data.

## Office Static CFF Table Notes

- `OS/2` is only lightly transformed for `font1`.
  - Best recovered `font1` `OS/2` candidate currently lives near decoded offset `15766032`.
  - `73/96` bytes match the original `SourceHanSansSC-Regular.otf` `OS/2` table exactly.
- Important negative result:
  - the substituted bytes in `OS/2` are **not** explained by simple zero-run RLE counting.
  - markers such as `0x02`, `0x30`, `0x41`, and `0x1f` appear across multiple different zero-run lengths.
  - example: `0x02` appears in zero runs of length `1`, `2`, `3`, `4`, and `7`.
- `OS/2` also is not a pure “replace zero with marker” transform.
  - Example changed group from the original `OS/2` bytes:
    - original: `8a025800`
    - decoded candidate: `02000002`
  - This means some nonzero bytes are being moved, suppressed, or re-expressed structurally.
- Current best interpretation is that `OS/2` uses a field-aware structural encoding, not plain byte-stream RLE.
- `head` is more heavily transformed than `OS/2`.
  - The core head magic `5f0f3cf5` is still present in the decoded output.
  - But many zero/default fields around it are rewritten with marker-like bytes such as `a1`, `26`, `68`, and `01`.
  - This looks more like a dedicated small-table encoding than a broken decode.

## Office Static CFF Prefix Repair Notes

- Be careful not to over-trust `fontTools.cffLib.CFFFontSet.decompile()` as a validity oracle.
- Current shallow parse thresholds:
  - `font1`: patching first `940` bytes from the source `CFF ` makes `decompile()` succeed
  - `font2`: patching first `880` bytes from the source `CFF ` makes `decompile()` succeed
- Those thresholds are useful only as **shallow parse checkpoints**.
  - They do **not** mean the decoded Office CFF prefix has become a standard CFF.
- `font2` prefix repair is non-monotonic under that shallow parser:
  - patching first `100..106` bytes succeeds
  - patching first `107..879` bytes fails again
  - patching first `880+` bytes succeeds again
- Interpretation:
  - mixing a partially restored standard `String INDEX` header with Office-transformed downstream offset bytes creates inconsistent hybrids
  - some shorter prefixes only "work" because later INDEX counts collapse to `0` under the mixed byte stream
- A stricter local CFF INDEX checker was used to validate:
  - header major version
  - Name INDEX
  - Top DICT INDEX
  - String INDEX
  - Global Subr INDEX
- Under that stricter checker, the first prefix length that restores the first four INDEX structures exactly to source-style framing is:
  - `font1`: `2806` bytes
  - `font2`: `3362` bytes
- These values match the start of source Global Subr data:
  - `font1`: Global Subr data begins at `2806`
  - `font2`: Global Subr data begins at `3362`
- Therefore the Office rewrite is not limited to the CFF header / Name / Top DICT / String header area.
  - It extends through the entire Global Subr offset array.
- Practical guidance:
  - do **not** treat "patch first `880/940` bytes" as a real reconstruction strategy
  - the next serious CFF rebuild attempt should reconstruct standard framing through the end of the Global Subr offset array, not just the very early prefix

## Office Static CFF Tail Reuse Notes

- Comparing source `CFF ` tails against the decoded Office intermediate at the same offset is misleadingly pessimistic.
- After Global Subr data starts, both `font1` and `font2` show a strong later-tail reuse signal under a `-2` positional skew:
  - `font1`: over the first `20000` bytes after Global Subr data start, same-offset matches = `2249/20000`, but `src[i] == office[i-2]` matches = `11586/20000`
  - `font2`: same-offset matches = `2177/20000`, but `src[i] == office[i-2]` matches = `10780/20000`
- There are repeated exact `32`-byte window matches at this same `-2` delta:
  - `font1`: many hits beginning around source offset `6534 -> office 6532`
  - `font2`: many hits beginning around source offset `8114 -> office 8112`
- Stronger later windows:
  - `font1`: first sampled `1024`-byte window with `>=85%` `-2` alignment is `8182..9206`; best sampled `512`-byte window is `9078..9590` at `0.914`
  - `font2`: best sampled `512`-byte window is `8802..9314` at `0.869`
- Interpretation:
  - the post-INDEX / post-early-GlobalSubr region is not fully re-encoded from scratch
  - later payload bytes appear to be substantially preserved, but with at least a stable `-2` positional skew
- Practical guidance:
  - do **not** assume the whole CFF body after Global Subr offsets must be rebuilt from source
  - do **not** assume a single global shift fixes everything either
  - the current best model is a 3-zone reconstruction:
    - standard prefix rebuilt through Global Subr offset array
    - early Global Subr data region treated as heavily transformed
    - later tail investigated as a possible `-2`-aligned reusable payload zone

## Office Static CFF CharStrings Notes

- Later probing narrowed the next major breakage beyond Global Subrs:
  - sampled `GlobalSubrs` from hybrid source-wrapper experiments can be accessed successfully
  - sampled `CharStrings` fail immediately from glyph index `0` onward for both `font1` and `font2`
- Therefore the next dominant incompatibility is no longer best described as "Global Subr data still broken".
  - It is more specifically in the `CharStrings` machinery and/or the structures it depends on.
- Source Top DICT offsets from the local static OTFs:
  - `font1` / Regular:
    - `charset=10732`
    - `FDSelect=10737`
    - `CharStrings=11081`
    - `FDArray=14267431`
  - `font2` / Bold:
    - `charset=12692`
    - `FDSelect=12697`
    - `CharStrings=13041`
    - `FDArray=14784666`
- CharStrings INDEX probe at the source `CharStrings` offset:
  - `font1` source:
    - count=`65535`, offSize=`3`, first offsets=`[1,77,80,114,163,257,368,504,...]`
  - `font1` Office intermediate at same offset:
    - count=`768`, offSize=`0`
  - `font1` Office intermediate at `offset-2`:
    - count=`65535`, offSize=`3`, but offsets are still wrong very early:
    - `[1,333,19792,20594,29347,1,368,28920,...]`
  - `font2` source:
    - count=`65535`, offSize=`3`, first offsets=`[1,77,80,129,149,247,370,507,...]`
  - `font2` Office intermediate at same offset:
    - count=`768`, offSize=`0`
  - `font2` Office intermediate at `offset-2`:
    - count=`65535`, offSize=`3`, but even the first offset is wrong:
    - `[129,16449613,16469328,...]`
- Interpretation:
  - the later-tail `-2` reuse signal is real, but it is **not** sufficient to recover the `CharStrings INDEX` framing
  - the CharStrings offset array itself appears structurally transformed
- Practical guidance:
  - shift focus from "early Global Subr data" to:
    - `charset`
    - `FDSelect`
    - `CharStrings INDEX`
  - the next targeted reverse-engineering step should inspect how Office rewrites those structures, rather than assuming a whole-tail shift can make CharStrings standard again

## Office Static CFF Isolation Results

- More isolated source-wrapper swaps were run using Office bytes under the observed `-2` alignment.
- For both `font1` and `font2`, these isolated replacements still allow sampled glyph access:
  - `charset` only
  - `charset + FDSelect`
  - `Global Subr data` zone
  - `CharStrings data` zone from CharStrings data start up to `FDArray`
- Strong probe result for `CharStrings data` replacement:
  - sampled glyph IDs `0..511`, `1024`, `2048`, `4096`, `8192`, `16384`, `32768`, `50000`, and `65534` all loaded successfully in both fonts
- The clean hard-negative control:
  - replacing **only** the `CharStrings INDEX` header + offset array with Office bytes fails immediately with `AssertionError` for both fonts
- Therefore the current primary blocker is best described as:
  - the `CharStrings INDEX` framing / offset array
  - not `charset`
  - not `FDSelect`
  - not Global Subr data by itself
  - not CharStrings data bytes by themselves
- Practical guidance:
  - narrow the next reverse-engineering step from `charset / FDSelect / CharStrings INDEX`
  - to **CharStrings INDEX reconstruction first**
  - treat `charset` and `FDSelect` as secondary checks unless new contrary evidence appears

## CoreText Registration Is Too Shallow To Validate Static-CFF Rebuilds

- A new `font2` source-wrapper check was run using:
  - `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
  - `/tmp/presentation1-font2-decoded.otf`
- Three temporary hybrids were built under the observed Office `-2` skew:
  - `/tmp/font2-source-wrapper-office-charstrings-data.otf`
    - source wrapper with Office `CharStrings INDEX + data`
  - `/tmp/font2-source-wrapper-office-charstrings-index-only.otf`
    - source wrapper with Office `CharStrings INDEX` only
  - `/tmp/font2-source-wrapper-office-charstrings-data-only.otf`
    - source wrapper with Office `CharStrings data` only
- All three hybrids still pass the current Swift/CoreText registration probe:
  - `descriptorCount=1`
  - `registerFontsForURL=true`
  - `cgFontLoad=true`
- Therefore:
  - `CTFontManager` / `CGFont` success is **not** a strong enough oracle for this static-CFF reverse-engineering work
  - shallow loader success can still hide structurally bad `CharStrings`

### Stronger check with real CFF outline traversal

- The repo's `fonttool convert --to ttf` path is much more informative here because it forces allsorts to traverse CFF outlines.
- Results on the same three hybrids:
  - `INDEX + data` replacement:
    - fails with:
      - `range end index 16449612 out of range for slice of length 435622`
  - `INDEX only` replacement:
    - fails with the **same** large bogus range:
      - `range end index 16449612 out of range for slice of length 435622`
  - `data only` replacement:
    - gets past the offset-array structure
    - later fails during outline interpretation with:
      - `failed to visit glyph outline: an invalid amount of items are in an arguments stack`

### Practical interpretation

- This sharpens the earlier isolation result materially.
- `CharStrings INDEX` is still the first-order structural blocker:
  - once Office `INDEX` bytes are introduced, allsorts immediately chases a bogus huge range
  - and the failure signature is stable whether or not Office charstring bodies are also present
- `CharStrings data` is a **secondary** problem:
  - replacing only the data zone no longer triggers the giant-offset failure
  - but later glyph-program semantics are still not fully standard under the current source-wrapper experiment
- Do **not** use:
  - `CTFontManagerRegisterFontsForURL`
  - `CGFont` creation
  - or shallow loader success
  as proof that a standard static CFF rebuild is already correct

## Office Static CFF CharStrings Heuristic Notes

- A simple local heuristic works for most early `CharStrings INDEX` offsets after skipping the first special Office triplet (`group0`):
  - keep source offset `1` as the first entry
  - for each later Office triplet `[a, b, c]`
  - if `b == previous_encoded_low`, stay on the current page and decode `offset = page<<8 | c`
  - otherwise treat `b` as a page reset and decode `offset = b<<8 | c`
- Under that heuristic:
  - `font1`: `595/600` offsets match the source exactly
  - `font2`: `572/600` offsets match the source exactly
- Current `font2` mismatch runs in the first `600` offsets are:
  - `(25,27)`
  - `(33,37)`
  - `(49,50)`
  - `(52,61)`
  - `(206,206)`
  - `(222,222)`
  - `(319,319)`
  - `(334,337)`
  - `(538,538)`
- This is strong evidence that Office uses the normal rolling pattern for most entries, but switches to localized alternative control forms in a small number of blocks.

## Zero-Low Surrogate Form

- There is now stronger evidence for a dedicated `low=00` surrogate form in the Office `CharStrings INDEX`.
- Confirmed examples:
  - `font1`: source `0x4500` -> Office triplet `31 45 31`
  - `font1`: source `0x4800` -> Office triplet `44 48 44`
  - `font2`: source `0x2a00` -> Office triplet `27 2a 27`
- Interpretation:
  - Office does **not** write the real low byte `00`
  - instead it writes a surrogate anchor byte in its place
  - later normal-form entries in that local run then reuse the surrogate as the "previous low" byte
- This explains why some isolated mismatches are non-fatal:
  - the decoded offset is numerically wrong under the naive heuristic
  - but the surrounding Office byte stream still forms a locally consistent chain

## CharStrings Block Patch Results

- `font2` patch experiments against the heuristic-rebuilt `CharStrings INDEX` were refined.
- Important result:
  - patching only the start item of a bad run does **not** help
  - these are not single-entry page-reset markers
  - they behave like short local mode blocks
- Concrete `font2` evidence:
  - patching only glyph `25` still fails first at glyph `27`
  - patching only glyphs `26..27` fails first at glyph `25`
  - patching the full block `25..27` moves the first failing glyph to `37`
  - patching `25..27` plus `33..37` moves the first failing glyph to `50`
  - patching `25..27`, `33..37`, and `49..50` moves the first failing glyph to `53`
  - after that, patching `52..60` advances the first failing glyph one-by-one
  - patching `53..61` is enough to move the first failing glyph to `222`
- Practical interpretation:
  - `font2` bad regions are genuinely localized multi-entry control blocks
  - at least one entry inside `52..61` is numerically mismatched but not parse-fatal (`52`)
  - later isolated mismatches such as `206`, `319`, and `538` are also currently non-fatal under parsing
- `font1` patch probes show the same general pattern, but much simpler:
  - the early `5..7` mismatch block behaves as one local mode block
  - later isolated numeric mismatches are not equally important
  - after fixing `5..7`, the next parse-fatal point moves to `373`
  - patching `359` alone does not change that
  - patching `373` is sufficient to restore the sampled parse path

## CharStrings `a`-Byte Anchor Findings

- The Office triplet first byte (`a`) is not random padding.
- On `font2`, many consecutive `mid`-byte ranges share a constant `a` anchor on page-change entries:
  - `01..05 -> a=95`
  - `0c..0d -> a=86`
  - `0e..16 -> a=3d`
  - `17..29 -> a=15`
  - `2a..3b -> a=27`
  - `3c..41 -> a=d0`
  - `44..79 -> a=00`
  - `7a..96 -> a=9e`
  - `97..b9 -> a=94`
  - `ba..c6 -> a=c8`
  - `c7..cd -> a=97`
  - `ce..e1 -> a=7b`
  - `e2..e8 -> a=df`
  - `e9..f2 -> a=e8`
  - `f3..f6 -> a=f2`
- Within a normal anchor range, the triplet shape is:
  - page-change entry: `[anchor, current_mid, current_low]`
  - same-page entry: `[anchor, previous_encoded_low, current_low]`
- This means:
  - the current simple heuristic is correctly decoding the `b/c` rolling chain for most entries
  - the remaining hard part is how Office updates / chooses the anchor byte and how special local blocks perturb that chain

## Sparse-Block Model Evidence

- A stronger `font2` sanity check was run by patching only the currently known bad local blocks:
  - `25..27`
  - `33..37`
  - `49..50`
  - `53..61`
  - `206`
  - `222`
  - `319`
  - `334..337`
  - `538`
- Result:
  - the first failing glyph moved from `27` to `618`
- Interpretation:
  - the dominant decoding model is already broadly correct
  - the remaining incompatibilities behave like sparse local control blocks
  - we are not looking at a globally wrong `CharStrings INDEX` rule anymore

## High-Byte Boundary Findings

- A new probe around the first `0x00ffff -> 0x010000` transition shows that the triplet `a` byte is at least partly the real high byte, not just a control marker.
- Example at the first high-byte rollover in `font2`:
  - source `0x010021` -> Office triplet `01 f6 21`
  - subsequent entries in the same high-byte region continue with `a=01`
  - later ordinary page-change entries become fully transparent again, e.g.:
    - source `0x010112` -> Office `01 01 12`
- Additional high-byte rollovers:
  - `0x020004` -> `02 00 04`
  - `0x030014` -> `03 00 14`
  - `0x040112` -> `04 01 12`
  - `0x050116` -> `05 01 16`
- Interpretation:
  - for `high > 0`, the normal form often looks like `[high, mid_or_prev_low, low]`
  - the remaining weirdness near those boundaries is concentrated in the same kind of surrogate / local-control forms we already see in the low and mid channels
  - the hardest unresolved zone is still the `high == 0` early region plus sparse later control blocks, not the whole high-byte channel

## CharStrings Special Decoder Findings

- A stronger provisional state-machine decoder now exists for the `font2` `CharStrings INDEX` low/high-0 region.
- It keeps the earlier normal rolling rule and adds three special forms:
  - low-zero surrogate:
    - detect `[anchor, mid, anchor]`
    - decode as `mid00`
  - swapped page-change form:
    - detect `[page, old_anchor, low]` when `old_anchor != 0`
    - decode as `page<<8 | low`
  - repeated-page low surrogate:
    - detect `b == prev1 && c == prev2 && c < b`
    - decode as `page<<8 | page`
    - but keep the encoded-low chain on `c`
- Important positive hits under this provisional decoder:
  - `538`: `006c6c`
  - `619`: `007575`
  - `812`: `00a300`
  - `832`: `00a800`
  - `996`: `00d6d6`
  - `1028`: `00dcfb`
  - `1207`: `00f6ec`
  - `1208`: `00f6f9`
  - `1231`: `00f8fd`
- The decoder no longer falsely rewrites `1063`; that normal entry still decodes to `00e8c9`.
- Match quality:
  - first `600` offsets: `574/600`
  - first `1300` offsets: `1240/1300`
- Very important structural result:
  - after these new special rules, the remaining mismatches in `600..1399` collapse into one continuous unresolved zone:
    - `(1266..1399)`
  - `1266` is exactly the first `0x00ffff -> 0x010000` rollover region
- Interpretation:
  - the current low/high-0 state machine is now good enough that the next major unknown is the post-rollover high-byte region
  - we are no longer seeing random sparse errors all across the mid-table once those surrogate forms are modeled

## Parse Progress With Special Decoder

- Using the new special decoder plus the already-known early hard-block source patches:
  - `25..27`
  - `33..37`
  - `49..50`
  - `53..61`
  - `206`
  - `222`
  - `319`
  - `334..337`
- The first failing sampled glyph for `font2` now moves to:
  - `1265`
- This is a substantial improvement over the earlier heuristic-only path, which stalled around:
  - `812`

## High-Rollover Bootstrap Findings

- The `0x00ffff -> 0x010000` region is now better understood.
- A very small bootstrap patch at:
  - `1266..1269`
- is enough to realign the whole `high=0x01` startup zone.
- Concretely, patching those four lower-16 offsets to the source values changes the exact mismatch structure from:
  - `(1266..1399)` continuous
- to:
  - `(1396,1396)`
  - `(1434,1436)`
  - `(1472..)`
- Interpretation:
  - `1266..1269` is a dedicated rollover-start control block, not a broad new encoding over the whole `0x01xxxx` region
  - once that startup block is corrected, the rest of the `0x01xxxx` zone mostly follows the same lower-16 rolling logic as before

## New Exact-Mismatch Structure After Bootstrap

- After applying:
  - the provisional special decoder
  - early hard-block source patches
  - the `1266..1269` rollover bootstrap patch
  - and a temporary patch for the old `52` blocker
- the first exact mismatch runs in the first `2000` offsets become:
  - `(1396,1396)`
  - `(1434,1436)`
  - `(1472,1999)` in the scanned window
- This is a much cleaner picture than before and gives three focused next targets.

## `high=0x01` Low-Byte Findings

- `high=0x01` behaves differently from `high=0x00` for some low-byte surrogates.
- For `high=0x01`, `low=0x00` stays explicit:
  - `011400 -> 21 14 00`
  - `011e00 -> 17 1e 00`
  - `019400 -> 91 94 00`
  - `01cb00 -> c5 cb 00`
  - `01fe00 -> e0 fe 00`
- But `low=0x01` often uses an anchor-surrogate form:
  - `014401 -> 23 44 23`
  - `018c01 -> 7a 8c 7a`
  - `01b001 -> a7 b0 a7`
  - `01d101 -> c5 d1 c5`
  - `01db01 -> d6 db d6`
  - `01e701 -> e0 e7 e0`
- Important exception:
  - `015b01 -> 12 01 7f`
  - this is not the same family; it behaves like a bootstrap / anchor-change control entry

## Overloaded `[anchor, mid, anchor]` Form

- A very important correction:
  - `[anchor, mid, anchor]` is **not** a single semantic form
- Confirmed meanings now include:
  - `mid00` surrogate in the earlier `high=0x00` region
  - `mid01` surrogate in parts of the `high=0x01` region
  - literal `mid + anchor` in later regions
- Concrete literal counterexample:
  - `017c66 -> 66 7c 66`
  - here the correct lower-16 value is `0x7c66`, not `0x7c00` or `0x7c01`
- Practical consequence:
  - do **not** globalize `if a == c then low = 0/1`
  - the meaning depends on the current control family / anchor regime

## Ambiguous Page-Change Block

- The run:
  - `1434..1436`
- appears to be a separate ambiguity class.
- Example:
  - `014fae -> 46 4f ae`
- Here `b == prev_low`, so the naive rolling decoder treats it as same-page data, but the correct value is a page change to `0x4f`.
- Interpretation:
  - this is a real ambiguity where the encoded page byte collides with the previous encoded low byte
  - it needs another state-machine rule beyond the current `same-page if b == prev1`

## Parse Probe After Bootstrap

- With:
  - the provisional special decoder
  - early hard-block source patches
  - the `1266..1269` bootstrap patch
  - and the old `52` blocker patched out for the probe
- sampled glyph access now reaches:
  - `4095`
- and the first sampled failure moves to:
  - `8192`
- This is another substantial jump from the previous sampled first-failure point of:
  - `1265`

## Literal-Anchor Singletons

- After the `1472` bootstrap patch, the next exact singletons include:
  - `1528`
  - `1970`
  - `1971`
  - `2074`
- These all share the same pattern:
  - the triplet looks like `[anchor, mid, anchor]`
  - but the correct lower-16 value is the literal `mid + anchor`, not a low surrogate
- Concrete examples:
  - `017c66 -> 66 7c 66`
  - `022411 -> 11 24 11`
  - `022511 -> 11 25 11`
  - `02524d -> 4d 52 4d`
- Interpretation:
  - `[anchor, mid, anchor]` remains overloaded even after the high-rollover bootstrap
  - some families use it as a surrogate
  - some families use it literally as `mid + anchor`

## Next Bootstrap Layer

- After patching:
  - `1528`
  - `1970`
  - `1971`
  - `2074`
- the next dominant exact mismatch becomes:
  - `2227..`
- Patching only `2227` moves that exact mismatch frontier forward again to:
  - `2313..`
- This makes `2227` another clear bootstrap / anchor-change entry.

## `high=0x02` Low-Equals-High Family

- Inside the later `high=0x02` region, there is a direct analogue of the earlier `high=0x01 low=0x01` surrogate family.
- New confirmed example:
  - `02df02 -> 76 df 76`
- Patching only that singleton at `2313` removes the first mismatch there and leaves:
  - `2319..`
- So `2313` is best classified as:
  - a `high=0x02 low=0x02` surrogate singleton
  - not a new bootstrap block by itself

## Bootstrap At `2319`

- The entry:
  - `2319`
  - source `02e200`
  - Office triplet `a7 95 1b`
- behaves like another bootstrap / anchor-change entry.
- Patching:
  - `2313`
  - and `2319`
- moves the first exact mismatch frontier again to:
  - `2395..`
- This strongly suggests the remaining unsolved work is continuing as:
  - sparse singleton surrogate ambiguities
  - interleaved with occasional bootstrap entries that retarget the active anchor family

## Parse Vs Exactness Reminder

- Even with many later exact mismatches still present, the sampled parse probe is now much more tolerant than before.
- In the latest probe with all currently known bootstrap and singleton patches applied through `2319`:
  - sampled glyph access through `32768` still succeeded
  - no sampled parse failure was hit in that probe
- So the remaining work from here is primarily:
  - exact structural recovery of the offset table
  - not an immediate parser-survival blocker

## `2395+` High-Byte-3 Bootstrap Layer

- After patching:
  - the earlier hard blocks
  - the `1266..1269` rollover bootstrap
  - the `1396/1434..1436/1472` layer
  - the literal-anchor singletons (`1528`, `1970`, `1971`, `2074`)
  - the `2227` bootstrap
  - the `2313` singleton surrogate
  - the `2319` bootstrap
- the next exact mismatch frontier becomes:
  - `2395..`
- This region is not random noise. It is another clean high-byte bootstrap layer around the `0x02ffff -> 0x030000` transition.

## `2395` Bootstrap Start

- The first new failing entry is:
  - source `0302fb`
  - Office triplet `d5 00 fb`
- Current decoder reconstructs this as `0400fb`, so the failure is a high-byte overshoot with the lower byte payload still recognizable.
- The following entries continue the same pattern:
  - `0303a5 <- 03 d5 a5`
  - `03040e <- d5 04 0e`
  - `030468 <- d5 0e 68`
- Interpretation:
  - this is a bootstrap / anchor-switch zone for the `0x03xxxx` region
  - the lower-16 rolling structure is still visible under the wrong high-byte assignment

## Post-Bootstrap Rolling Continuity

- Even inside the `2395+` mismatch zone, many later entries clearly preserve the same rolling form:
  - `030633 <- 03 06 33`
  - `030799 <- 03 07 99`
  - `030837 <- 03 08 37`
  - `030a80 <- 03 0a 80`
  - `031017 <- 03 10 17`
- This is more evidence that the dominant mechanism is still:
  - stable rolling lower-16 reconstruction
  - plus sparse bootstrap / overloaded singleton entries

## New Singleton Families Inside The `0x03xxxx` Region

- New exact singleton at:
  - `2313`
  - source `02df02`
  - Office `76 df 76`
  - classified as `high=0x02 low=0x02` surrogate
- Similar later singletons / overloaded forms continue in the `0x03xxxx` region:
  - `2471`: `032599 <- 88 25 99`
  - `2473`: `032768 <- 88 27 68`
  - `2487`: `033384 <- 32 33 84`
  - `2489`: `033650 <- 32 36 50`
  - `2491`: `033872 <- 32 38 72`
  - `2493`: `033c30 <- 32 3c 30`
- These look like the same broad pattern as before:
  - some entries are literal anchor forms
  - some are singleton surrogates
  - they sit on top of an otherwise regular rolling sequence

## Current Exact-Mismatch Frontier

- With all currently known patches applied through `2319`, the first exact mismatch runs in the scanned window become:
  - `(2395..2599)` in the current `2600`-entry probe
- This means the earlier layers are holding:
  - no new exact mismatches remain before `2395`
- So the next focused reverse-engineering target is now:
  - the `0x03xxxx` bootstrap start around `2395`

## `0x03xxxx` Layer Is Also A Short Bootstrap, Not A Whole Bad Region

- A new minimal-patch experiment was run on the `2395+` layer.
- Results:
  - patching only `2395` moves the exact mismatch frontier to `2400`
  - patching `2395` plus the short block `2400..2404` moves the frontier all the way to `2550`
- Practical interpretation:
  - the `0x03xxxx` region begins with:
    - one bootstrap entry at `2395`
    - followed by a short transition block `2400..2404`
  - it is **not** a long uniformly broken region

## `2550` Starts Yet Another Short Bootstrap

- The next exact mismatch frontier after fixing the `2395/2400..2404` layer is:
  - `2550..`
- Another minimal-patch experiment showed:
  - patching only `2550` moves the frontier to `2551`
  - patching the short block `2550..2555` moves the frontier all the way to `2866`
- This is the same structural pattern again:
  - a short bootstrap / transition block
  - not a full-table rule break

## Strengthened Structural Model

- At this point the repeated pattern is very strong:
  1. long spans of regular rolling lower-16 reconstruction
  2. sparse overloaded singleton forms
  3. short bootstrap / transition blocks that retarget the active anchor/high-byte regime
- Confirmed short bootstrap blocks now include:
  - `1266..1269`
  - `2395` plus `2400..2404`
  - `2550..2555`
- This should guide the next decoder design step:
  - detect and classify short bootstrap blocks
  - do not model the late table as if every later mismatch is an independent singleton

## New Sparse Frontier After `2550..2555`

- Continuing the same lower-16 exact-match workflow, once the known layers through:
  - `1266..1269`
  - `2395`
  - `2400..2404`
  - `2550..2555`
- are neutralized, the next exact lower-16 mismatch runs are no longer continuous.
- The next sparse runs are:
  - `2866..2867`
  - `2902..2903`
  - `2956`
  - `3183`
  - `3282`
  - `3336`
  - `3372`
- Minimal patch progression is very clean:
  - patch `2866` -> frontier moves to `2867`
  - patch `2866..2867` -> frontier moves to `2902`
  - patch `2866..2867` plus `2902..2903` -> frontier moves to `2956`
  - then patching `2956`, `3183`, `3282`, `3336`, `3372` advances one sparse singleton at a time
  - after patching through `3372`, there is no remaining lower-16 exact mismatch through `3399`

## `2866..3372` Mostly Reuses The Old Page-Collision Family

- The dominant new family in this layer is the same ambiguity already seen at `1434..1436`.
- Shape:
  - `b == prev_encoded_low`
  - naive decoder treats the entry as same-page
  - but the correct source lower-16 actually requires a new page change to `b`
- Clean examples:
  - `2866`: `04847a <- 5e 84 7a`
    - heuristic lower-16: `837a`
    - correct lower-16: `847a`
  - `2902`: `04a05c <- 5e a0 5c`
    - heuristic lower-16: `9f5c`
    - correct lower-16: `a05c`
  - `2903`: `04a0b0 <- 5e 5c b0`
    - heuristic lower-16: `9fb0`
    - correct lower-16: `a0b0`
  - `2956`: `04cbbe <- 5e cb be`
    - heuristic lower-16: `cabe`
    - correct lower-16: `cbbe`
  - `3183`: `05b69c <- a9 b6 9c`
    - heuristic lower-16: `b59c`
    - correct lower-16: `b69c`
  - `3282`: `061b60 <- 06 1b 60`
    - heuristic lower-16: `1a60`
    - correct lower-16: `1b60`
  - `3336`: `065f22 <- 24 5f 22`
    - heuristic lower-16: `5d22`
    - correct lower-16: `5f22`
  - `3372`: `0693d6 <- 6e 93 d6`
    - heuristic lower-16: `92d6`
    - correct lower-16: `93d6`
- Interpretation:
  - this family survives well past the earlier `0x01/0x02/0x03` transition zones
  - it is not a one-off rollover artifact
  - the real unresolved rule is broader:
    - `b == prev_encoded_low` does **not** always mean same-page

## `2867` Adds A New Confirmed `high==low` Surrogate

- The paired entry at `2867` is **not** another page-collision.
- Example:
  - source `048504`
  - Office triplet `5e 85 5e`
- Lower-16 view:
  - heuristic gives `855e`
  - correct value is `8504`
- Structural interpretation:
  - this is the same overloaded `[anchor, mid, anchor]` family
  - but here the preserved low byte equals the **full 24-bit high byte** (`0x04`)
- This extends the earlier surrogate ladder:
  - `01xxxx` had `low=0x01`
  - `02df02`
  - now `048504`

## Next Layer After `3372`

- After patching the sparse set through `3372`, the next lower-16 exact singleton runs through `5000` begin at:
  - `3437`
  - `3496`
  - `3589`
  - `3614`
  - `3756`
  - `4253`
  - `4256`
  - `4284`
  - `4289`
  - `4294`
  - `4318`
  - `4369`
  - `4663`
  - `4689`
- Stable early classifications from that later layer:
  - `3437`: `06d306 <- b5 d3 b5`
    - another confirmed `high==low` surrogate, now for full high byte `0x06`
  - `3589`: `075452 <- 16 54 52`
    - page-collision family
  - `3614`: `076f50 <- 16 6f 50`
    - page-collision family
  - `3756`: `07fbd4 <- a0 fb d4`
    - page-collision family
  - `4284`: `0a260a <- 05 26 05`
    - confirmed `high==low` surrogate for full high byte `0x0a`
  - `4294`: `0a328e <- 27 32 8e`
    - page-collision family
  - `4318`: `0a494d <- 05 49 4d`
    - page-collision family
  - `4369`: `0a715b <- 5f 71 5b`
    - page-collision family
  - `4663`: `0ba50f <- 32 a5 0f`
    - page-collision family
  - `4689`: `0bca0b <- bb ca bb`
    - confirmed `high==low` surrogate for full high byte `0x0b`
- Still unresolved / not yet safely classified:
  - `3496`
  - `4253`
  - `4256`
  - `4289`

## Later Singleton Progression Is Strictly One-By-One

- Starting from the baseline where all known exact lower-16 mismatches through `3372` are patched, the next frontier advances one singleton at a time:
  - patch `3437` -> frontier moves to `3496`
  - patch `3496` -> frontier moves to `3589`
  - patch `3589` -> frontier moves to `3614`
  - patch `3614` -> frontier moves to `3756`
  - patch `3756` -> frontier moves to `4253`
  - patch `4253` -> frontier moves to `4256`
  - patch `4256` -> frontier moves to `4284`
  - patch `4284` -> frontier moves to `4289`
  - patch `4289` -> frontier moves to `4294`
  - patch `4294` -> frontier moves to `4318`
  - patch `4318` -> frontier moves to `4369`
  - patch `4369` -> frontier moves to `4663`
  - patch `4663` -> frontier moves to `4689`
  - patch `4689` -> no lower-16 exact mismatch remains through the scanned window
- Interpretation:
  - this later region is not hiding another multi-entry block
  - it is a sparse chain of individual overloaded entries

## `3496` Is A Tiny `c==00` Boundary Exception

- `3496` now looks less like a generic bootstrap and more like a very small page-boundary special case.
- Example:
  - source `070403`
  - Office triplet `07 04 00`
  - heuristic lower-16 `0400`
  - correct lower-16 `0403`
- Useful scan result:
  - entries with `c==00` and `a==high` are common
  - almost all of them correspond to source `...00`
  - only a tiny outlier set has source low byte != `00`
- In the current scan the clear outliers are:
  - `3496`: `070403 <- 07 04 00`
  - `10091`: `230022 <- 23 00 00`
- Practical takeaway:
  - do not assume `c==00` plus `a==high` always means a literal `...00`
  - there is at least a tiny boundary family where the real low byte is omitted and must come from extra state

## `4253` And `4256` Behave Like `a==high` Bootstrap Singletons

- These two entries now look like a separate singleton family, distinct from the page-collision and low-equals-high cases.
- Both patch independently and each patch advances the frontier by exactly one singleton.
- Examples:
  - `4253`: source `0a0687`, Office `0a 87 6e`
    - relation: `a == high`, `b == source_low`
    - heuristic lower-16 `876e`
    - correct lower-16 `0687`
  - `4256`: source `0a09f3`, Office `0a e9 cd`
    - relation: `a == high`
    - heuristic lower-16 `e9cd`
    - correct lower-16 `09f3`
- Neighborhood structure:
  - surrounding entries still use the normal rolling pages:
    - `0a04bf`
    - `0a05ad`
    - `0a07da`
    - `0a08dc`
    - `0a0afa`
  - only the missing page entries (`06`, `09`) collapse into these singleton forms
- Working interpretation:
  - these are `a==high` bootstrap / page-injection singletons
  - not ordinary same-page ambiguity
  - not the symmetric low-equals-high surrogate family

## Low-Equals-High Surrogates Continue, But Symmetry Breaks

- The later scan confirms that the `low == high` family continues far beyond the early `0x01/0x02/0x03` layers.
- Important correction:
  - this family is no longer limited to the symmetric `[anchor, mid, anchor]` shape
- Later confirmed or very strong examples:
  - `4284`: `0a260a <- 05 26 05`
  - `4289`: `0a2c0a <- 0e 2c 05`
  - `4689`: `0bca0b <- bb ca bb`
  - `5296`: `0e570e <- 54 57 54`
  - `5346`: `0e8c0e <- 5c 8c 5c`
  - `5575`: `0f8b0f <- 3f 8b 3f`
  - `5802`: `106b10 <- 42 6b 42`
  - `6495`: `136813 <- 67 68 67`
  - `6899`: `152a15 <- 1a 2a 1a`
- This upgrades the family model:
  - sometimes it is symmetric `[anchor, mid, anchor]`
  - sometimes the emitted surrogate byte for the low-equals-high case is asymmetric or tied to a local anchor family
- Therefore:
  - `4289` should now be treated as part of the broader `low == high` surrogate family
  - not as an unrelated singleton bootstrap

## New Rule Candidate: `a==high && c==low`

- A much stronger late-tail family is now identified.
- Candidate rule:
  - if the current entry is mismatching under the rolling heuristic
  - and `a == source_high`
  - and `c == source_low`
  - then the correct lower-16 is:
    - `mid << 8 | low`
- This rule is not a guess anymore.
  - In the current scan window it fixes `23` mismatching entries immediately.
  - It produces **zero** observed false positives on already-correct entries in that same check.
- Early confirmed examples:
  - `6222`: `12119c <- 12 ff 9c`
  - `7798`: `19197b <- 19 65 7b`
  - `8030`: `1a1ae3 <- 1a 11 e3`
  - `8460`: `1c1341 <- 1c 85 41`
  - `8465`: `1c1840 <- 1c 18 40`
  - `9561`: `21c5ca <- 21 ef ca`
  - `9562`: `21c5e0 <- 21 ca e0`
  - `10750`: `252591 <- 25 08 91`
  - `11033`: `262536 <- 26 d5 36`
  - `11034`: `2625a5 <- 26 36 a5`
  - `13010`: `2c8c38 <- 2c 26 38`
  - `13985`: `30047c <- 30 04 7c`
  - `14039`: `3030cb <- 30 b7 cb`
  - `14360`: `31353f <- 31 2f 3f`
- Wider positive signal:
  - applying this rule after the known earlier patches fixes `30` mismatches through the wider scan window
  - including later entries such as:
    - `18753`
    - `19074`
    - `19238`
    - `22317`
    - `23398`
    - `23945`
    - `24021`
    - `24201`
    - `24941`
    - `28710`
- Practical effect:
  - after applying the known earlier sparse fixes and then this new rule,
  - the next lower-16 exact mismatch frontier jumps to:
    - `5002`
- This is the strongest current sign that we are transitioning from one-off reverse-engineering
  - to actual reusable decoder rules.

## Post-Rule Frontier At `5002`

- After the new `a==high && c==low` rule is applied, the remaining first mismatch runs become sparse again.
- Earliest remaining runs:
  - `5002`
  - `5009`
  - `5296`
  - `5346`
  - `5575`
  - `5802`
  - `5849`
  - `5919`
  - `6373`
  - `6495`
  - `6899`
  - `7034`
- The first two are representative of the other still-open family:
  - `5002`: `0d06d0 <- 6e d3 d0`
  - `5009`: `0d0db0 <- 6e 6e b0`
- both are in the previously observed `c == low` family
- The next visible block (`5296`, `5346`, `5575`, `5802`, `6495`, `6899`) continues the broadened `low == high` surrogate family.

## New Local Rule Candidate: `b==prevC && a!=prevA`

- A second much stronger decoder rule now exists, and unlike the previous source-assisted family, this one is fully local to the triplet stream.
- Candidate rule:
  - if the current entry is still mismatching under the rolling heuristic
  - and `b == previous_triplet.c`
  - and `a != previous_triplet.a`
  - then decode the lower-16 as:
    - `b << 8 | c`
- In the current scan window this rule is extremely strong:
  - it hits `45` exact remaining mismatches
  - observed precision in that check is `100%`
  - no false positives were observed for that predicate in the scanned range
- This means a large part of the late `c == low` / page-collision family has now been converted into a real local rule instead of a manual patch list.
- Representative fixed examples:
  - `6373`: `12bf2b <- af bf 2b`
  - `7659`: `186f58 <- 57 6f 58`
  - `8145`: `1a93f9 <- 51 93 f9`
  - `8223`: `1ae9e2 <- 85 e9 e2`
  - `9323`: `209694 <- 6d 96 94`
  - `10542`: `24585c <- 3b 58 5c`
  - `11811`: `28bd2d <- a9 bd 2d`
  - `12802`: `2bb380 <- 96 b3 80`
  - `14047`: `30371b <- 26 37 1b`
  - `14417`: `31635a <- 39 63 5a`
  - `17276`: `3bbeee <- 47 be ee`
  - `18844`: `417e59 <- 27 7e 59`
- Structural interpretation:
  - this is a large “page collision with surrogate anchor switch” family
  - the ambiguity is resolved by treating the current triplet as a fresh page change despite `b == prevC`

## Remaining `b<<8|c` Family After The New Local Rule

- After applying:
  - the earlier known exact singleton / bootstrap fixes
  - the source-assisted `a==high && c==low` family
  - and the new local rule `b==prevC && a!=prevA`
- the `b<<8|c` family does **not** disappear completely.
- The original `b<<8|c` family size in the current scan window is:
  - `66`
- The new local rule covers:
  - `45`
- So the unresolved remainder is:
  - `21`
- Those remaining entries are the same-anchor subset such as:
  - `5849`: `1090a2 <- 76 90 a2`
  - `5919`: `10d850 <- a0 d8 50`
  - `8583`: `1cb029 <- 29 b0 29`
  - `9358`: `20cd65 <- a0 cd 65`
  - `9965`: `22b01b <- 85 b0 1b`
  - `10026`: `22d433 <- c7 d4 33`
  - `16178`: `37ebed <- 5e eb ed`
  - `16978`: `3ab2a5 <- 84 b2 a5`
- Important negative result:
  - a brute-force sweep over simple local predicates on:
    - previous triplet
    - current triplet
    - next triplet
    - and a few `next+1` relations
  - did **not** find another comparably clean high-precision rule for this remaining same-anchor subset
- Current interpretation:
  - that remainder likely needs extra decoder state beyond a simple one-triplet local predicate
  - so it should be treated separately from the already-clean `a!=prevA` subfamily

## Stronger State Signal For The Remaining Same-Anchor Subset

- The unresolved remainder of the `b<<8|c` family is now pinned down to `21` entries in the current scan window:
  - `5849`
  - `5919`
  - `8583`
  - `9358`
  - `9965`
  - `10026`
  - `11306`
  - `11707`
  - `12463`
  - `12794`
  - `13269`
  - `13511`
  - `14376`
  - `16178`
  - `16198`
  - `16580`
  - `16978`
  - `21233`
  - `23118`
  - `23163`
  - `26075`
- These are all still:
  - correctly decoded as `b<<8|c`
  - but **not** covered by the local rule:
    - `b == prevC && a != prevA`
- New stable observation:
  - for this remainder, the rolling decoder's `page_before` is usually stale by only a tiny amount
  - most of the cluster has:
    - `delta = b - page_before = 1`
    - or `delta = 2`
- Current strongest subclusters:
  - `(delta=1, nextB=b+1, same anchor across prev/current/next)`:
    - `5849`
    - `5919`
    - `10026`
    - `11306`
    - `12463`
    - `16178`
    - `16978`
    - `21233`
    - `23163`
    - and one more similar case in the same scan
  - `(delta=2, nextB=b+1, same anchor across prev/current/next)`:
    - `8583`
    - `9358`
- Practical interpretation:
  - this is now much more clearly a **page-state lag** problem
  - not a missing byte-permutation rule
  - the decoder often carries `page_before = b-1` (or occasionally `b-2`)
  - while the correct interpretation at the current entry must force a new page of `b`

## Important Negative Result On The Same-Anchor Remainder

- Several candidate predicates were tested on that remaining `21`-entry subset, including combinations of:
  - `b == prevC`
  - same anchor across prev/current/next
  - `delta = b - page_before in {1,2}`
  - `nextB = b+1`
- None of those stateful predicates yet reaches the same “safe to implement” quality as the earlier two strong rules.
- Best current narrower subset only reaches about:
  - precision `~0.79`
  - recall `~0.71`
- Therefore:
  - do **not** freeze a third decoder rule for this subset yet
  - but do keep the stronger conceptual conclusion:
    - the remaining blocker is primarily page-state advancement
    - not another broad triplet-remapping family

## Even Stronger Same-Anchor State Fact

- A more concrete state fact is now established for the remaining same-anchor `b<<8|c` subset.
- For **all 21** unresolved same-anchor targets, the immediately previous solved lower-16 value is exactly:
  - `(b-1)<<8 | b`
  - or the rarer:
  - `(b-2)<<8 | b`
- In other words, the previous decoded entry already ends with the current entry's intended page byte `b`.
- The most common case is the `b-1` ladder:
  - `5849`: previous solved `8f90`, current wants `90a2`
  - `5919`: previous solved `d7d8`, current wants `d850`
  - `10026`: previous solved `d3d4`, current wants `d433`
  - `11306`: previous solved `f8f9`, current wants `f9a1`
  - `12463`: previous solved `8d8e`, current wants `8e6a`
  - `16178`: previous solved `eaeb`, current wants `ebed`
  - `21233`: previous solved `2324`, current wants `2439`
- The two clean `b-2` exceptions remain:
  - `8583`: previous solved `aeb0`, current wants `b029`
  - `9358`: previous solved `cbcd`, current wants `cd65`
- This is very strong support for the hidden-state interpretation:
  - some earlier step is effectively arming “the previous low becomes the next page”
  - the current same-anchor collision is where that armed promotion is consumed
- Important limitation:
  - this fact is still not a safe implementation rule by itself
  - because the broader predicate:
    - previous solved in `{(b-1)<<8|b, (b-2)<<8|b}`
  - has:
    - recall `21/21`
    - but precision only about `0.208`
- The narrower version:
  - previous solved `(b-1)<<8|b`
  - same anchor with previous entry
  - and `nextB = b+1`
  - reaches about:
    - precision `~0.765`
    - recall `~0.619`
- So this is now best treated as:
  - a strong explanatory state invariant
  - not yet a frozen decoder rule

## 5.4 Subagent Confirmation: Same-Anchor Tail

- A separate `gpt-5.4` explorer independently reproduced the exact same `21` unresolved same-anchor cases from:
  - `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
  - `/tmp/presentation1-font2-decoded.otf`
- Its conclusion matches the local main-thread analysis:
  - no clean third local rule emerged
  - the strongest signal is still page-state lag
  - the missing variable behaves like a hidden per-anchor pending-page / deferred-promote state
- Best predictor candidates reported by the subagent on the ambiguous same-anchor pool:
  - `previous decoded page == b-1`
    - hits `19/21`
    - false positives `96`
  - `previous decoded page in {b-1, b-2}`
    - hits `21/21`
    - false positives `152`
  - `previous decoded page == b-1` and `nextB == b+1`
    - hits `13/21`
    - false positives `18`
  - `previous decoded page == b-1` and same anchor on next entry and `nextB == b+1`
    - hits `10/21`
    - false positives `12`
- This is still not strong enough to freeze as implementation logic, but it is now independently confirmed that:
  - the unresolved same-anchor remainder is a state-lag problem
  - not a byte-permutation problem

## 5.4 Subagent Confirmation: Low-Equals-High Tail

- A separate `gpt-5.4` explorer independently re-ran the remaining `low == high` tail after the known baseline fixes.
- Important agreement with the main thread:
  - no clean local third rule like `b<<8|a` emerged
  - the tail still needs a stronger high-byte / state model
  - the remaining family is structurally split, not uniform
- Its useful structural split:
  - symmetric “mid already correct, low omitted” cases
  - same-anchor carry cases with `b == prevC`
  - rare asymmetric gap singletons such as:
    - `7034`
    - `18807`
- Additional useful counts from that subagent's broader reproduction:
  - `150/215` had `source_mid == current b`
  - `119/215` had symmetric `a == c`
  - `62/215` had `b == prevC`
  - `107/215` had `source_mid == prevB + 1`
  - `166/215` had `source_mid == nextB - 1`
- Interpretation:
  - this independently reinforces that the unsolved `low==high` tail is not waiting on one more small remap
  - it also wants a stronger state model

## Same-Anchor 21: New Macro Split

- A fresh local replay against the full `font2` `CharStrings INDEX` now gives a stable `same-anchor + b<<8|c` candidate pool of:
  - `77`
- Within that pool, the known unresolved `21` same-anchor cases split cleanly by immediate neighborhood shape into five macro families:
  - `prev_raw_samepage + same_anchor_next + next_raw_pagechange`:
    - `5919`
    - `11306`
    - `12463`
    - `16198`
  - `prev_raw_samepage + same_anchor_next + next_raw_samepage`:
    - `9965`
    - `11707`
    - `13269`
    - `23163`
  - `prev_raw_pagechange + same_anchor_next + nextB=b+1`:
    - `5849`
    - `8583`
    - `9358`
    - `10026`
    - `13511`
    - `16178`
    - `16978`
    - `21233`
  - `prev_raw_pagechange + same_anchor_next + next_raw_samepage`:
    - `16580`
  - `next anchor switches at the end of the current anchor run`:
    - `12794`
    - `14376`
    - `23118`
    - `26075`
- This matters because:
  - the `21` are no longer one opaque bag
  - they can now be treated as a small number of repeatable local transition shapes

## Same-Anchor 21: Exact Local Subfamilies

- Several smaller zero-false-positive subsets now exist inside that `21`-entry remainder.
- Exact singleton:
  - `8583`
    - predicate:
      - `c == a`
- Exact triple:
  - `9358`
  - `16178`
  - `16978`
    - predicate:
      - `prev_raw_pagechange`
      - `nextB = b+1`
      - current entry is within the first `3` positions of its anchor run
- Exact triple:
  - `16178`
  - `16978`
  - `26075`
    - predicate:
      - `prev_raw_pagechange`
      - `nextB = b+1`
      - anchor run length `<= 5`
- Exact pair:
  - `12463`
  - `16198`
    - predicate:
      - `prev_raw_samepage`
      - `same_anchor_next`
      - `next_raw_pagechange`
      - `nextB = b+1`
      - anchor run length `<= 10`
      - current entry is within `3` steps of run end
- Exact pair:
  - `14376`
  - `26075`
    - predicate:
      - `next anchor switches`
      - `prev_raw_pagechange`
      - `nextB = b+1`
      - `nextA == c`
      - `nextA > a`
- Exact singleton:
  - `12794`
    - predicate:
      - `next anchor switches`
      - `nextB = b+2`
      - `nextA < a`
- Exact singleton:
  - `23118`
    - predicate:
      - `next anchor switches`
      - `nextA == c`
      - anchor run length `== 3`
- Exact singleton:
  - `16580`
    - predicate:
      - `prev_raw_pagechange`
      - `next_raw_samepage`
      - current entry is at run end or one step before run end

## Same-Anchor 21: Remaining Core After New Split

- After factoring out the exact local subfamilies above, the still-hard core is now:
  - `5849`
  - `5919`
  - `9965`
  - `10026`
  - `11306`
  - `11707`
  - `13269`
  - `13511`
  - `21233`
  - `23163`
- These `10` look like the real “prototype” deferred-promote cases:
  - same anchor on previous, current, and next entry
  - previous solved lower-16 already ends in current `b`
  - current still wants `b<<8|c`
  - no extra singleton marker like `c==a`
  - no terminal anchor switch
- Practical interpretation:
  - the reverse-engineering target is now narrower
  - the remaining work is less about finding more tiny exceptional markers
  - and more about explaining this `10`-entry core pending-page state directly

## Font1 Cross-Check: Same Mechanism Family

- A new control pass against `font1` (`SourceHanSansSC-Regular`) shows the same style of ladder-state candidates also exists there.
- In the first `20000` entries, `font1` produces at least:
  - `29` ladder-state candidates under the same broad invariant
    - previous solved lower-16 in `{(b-1)<<8|b, (b-2)<<8|b}`
    - current source lower-16 equals `b<<8|c`
    - same anchor with previous entry
- More importantly, `font1` candidates also fall into the same macro neighborhood families used for `font2`:
  - `A = prev_raw_samepage + same_anchor_next + next_raw_pagechange`
    - `8` examples
  - `B = prev_raw_samepage + same_anchor_next + next_raw_samepage`
    - `6` examples
  - `C = prev_raw_pagechange + same_anchor_next + nextB=b+1`
    - `13` examples
  - `D = prev_raw_pagechange + same_anchor_next + next_raw_samepage`
    - `7` examples
  - `E = next anchor switches at run end`
    - `5` examples
- This is strong support that the current findings are not just a `font2` / Bold quirk.
- Practical interpretation:
  - the unresolved state behavior looks like an `Office static CFF` family mechanism
  - not a one-off transform tied only to this single Bold file
- Important nuance:
  - this still does **not** prove “all arbitrary CFF fonts” are solved
  - it is evidence that this Office static-CFF subfamily is internally consistent across at least:
    - `font1` Regular
    - `font2` Bold

## Remaining 10 Core: Further Reduction To 2 Prototypes

- The previous hard core of `10` same-anchor cases:
  - `5849`
  - `5919`
  - `9965`
  - `10026`
  - `11306`
  - `11707`
  - `13269`
  - `13511`
  - `21233`
  - `23163`
- is now reduced much further using exact local rules.

### Exact class-A pair

- `5919`
- `11306`
  - exact predicate:
    - `prev_raw_samepage`
    - `same_anchor_next`
    - `next_raw_pagechange`
    - `nextB = b+1`
    - `run_len >= 17`
    - `d0 <= 168`

### Exact class-C quartet

- `5849`
- `10026`
- `13511`
- `21233`
  - exact predicate:
    - `prev_raw_pagechange`
    - `same_anchor_next`
    - `nextB = b+1`
    - `run_pos >= 6`
    - `to_end in {4,5}`
    - `d0 <= 307`

### Exact class-B medium pair

- `9965`
- `11707`
  - exact predicate:
    - `prev_raw_samepage`
    - `same_anchor_next`
    - `next_raw_samepage`
    - `run_len == 14`
    - `run_pos >= 8`

## Current Minimal Unresolved Core

- After the new exact splits above, the same-anchor hard core is now only:
  - `13269`
  - `23163`
- Shared shape of this remaining prototype pair:
  - `prev_raw_samepage`
  - `same_anchor_next`
  - `next_raw_samepage`
  - very short anchor runs:
    - `run_len` in `{4,5}`
    - `run_pos = 2`
    - `to_end in {1,2}`
  - moderate local deltas:
    - `d0` in `133..159`
    - `d1` in `73..143`
    - `d2` in `118..191`
- Important current negative result:
  - several near-miss false positives still share almost the same short-run profile, including:
    - `32886`
    - `48472`
    - `51354`
    - `63388`
  - so there is still no safe purely local threshold rule for this final pair
- Practical interpretation:
  - the deferred-promote family is now almost fully decomposed
  - the remaining unknown state seems to collapse to one tiny short-run prototype
  - this is the best current candidate for the true irreducible hidden decoder state

## Final Same-Anchor Closure

- The last two short-run prototypes are now also explained by exact local predicates.

### Exact singleton: `13269`

- predicate:
  - `prev_raw_samepage`
  - `same_anchor_next`
  - `next_raw_samepage`
  - `run_len == 4`
  - `run_pos == 2`
  - `to_end == 1`
  - `trip[i+2].b == current.b`
- structural reading:
  - unlike the nearby short-run false positives, the `i+2` step does **not** advance its `b` byte to `b+1`
  - it keeps the current promoted page byte alive for one more step before the surrounding anchor structure changes

### Exact singleton: `23163`

- predicate:
  - `prev_raw_samepage`
  - `same_anchor_next`
  - `next_raw_samepage`
  - `run_len == 5`
  - `run_pos == 2`
  - `to_end == 2`
  - `trip[i+2].a == current.a`
  - `trip[i+2].b == current.b + 1`
  - `trip[i+3].a != current.a`
- structural reading:
  - this is the short-run variant where the anchor stays stable through `i+2`
  - then the anchor flips immediately at `i+3`
  - nearby false positives either keep the same anchor one step longer or fail the exact run-shape

## Same-Anchor Tail Status

- The `21`-entry unresolved same-anchor `b<<8|c` family is now fully covered by exact local predicates.
- Current verification status:
  - all `21/21` positives are covered
  - no extra positives were covered in the current ambiguous pool
- Practical interpretation:
  - the deferred-promote same-anchor family is now no longer the blocker
  - future decode work should shift back to the remaining unsolved families outside this specific `21`-entry tail

## Low-Equals-High Tail: Stable 9-Bucket Taxonomy

- A new direct replay against `font2` shows the remaining non-explicit `source_high == source_low` mismatches are now best treated as a structural taxonomy rather than one undifferentiated tail.
- Counting entries where:
  - source `high == low`
  - and Office did **not** simply emit `c == high`
- yields:
  - `240` entries in the current `font2` table
- Those `240` entries split into the following stable buckets:
  - `59`
    - asymmetric
    - `b == source_mid`
    - `b != prevC`
    - `source_mid == nextB - 1`
  - `53`
    - symmetric (`a == c`)
    - `b == source_mid`
    - `b != prevC`
    - `source_mid == nextB - 1`
  - `48`
    - symmetric (`a == c`)
    - `b == source_mid`
    - `b != prevC`
    - `source_mid != nextB - 1`
  - `29`
    - symmetric
    - `b != source_mid`
    - `b == prevC`
    - `source_mid == nextB - 1`
  - `25`
    - asymmetric
    - `b != source_mid`
    - `b == prevC`
    - `source_mid == nextB - 1`
  - `11`
    - asymmetric
    - `b == source_mid`
    - `b != prevC`
    - `source_mid != nextB - 1`
  - `8`
    - symmetric
    - `b != source_mid`
    - `b == prevC`
    - `source_mid != nextB - 1`
  - `4`
    - asymmetric
    - `b != source_mid`
    - `b != prevC`
    - `source_mid != nextB - 1`
    - examples:
      - `33`
      - `319`
      - `334`
      - `1472`
  - `2`
    - asymmetric
    - `b != source_mid`
    - `b != prevC`
    - `source_mid == nextB - 1`
    - exact examples:
      - `7034`
      - `18807`
  - `1`
    - symmetric
    - `b != source_mid`
    - `b != prevC`
    - `source_mid == nextB - 1`
    - example:
      - `47528`
- Practical interpretation:
  - the low-equals-high tail is now no longer “one family that still needs a rule”
  - it is at least:
    - two large `b == source_mid` families
    - two large `b == prevC` carry families
    - several small residual singleton / micro-families

## Low-Equals-High Tail: Reproduction Alignment

- Be careful with the `CharStrings INDEX` alignment when reproducing the `240` count.
- The exact replay that matches the documented taxonomy uses:
  - source standard offset array from:
    - `source_rel + 3 + i*3`
  - Office triplet stream from:
    - `source_rel + 1 + i*3`
- In other words:
  - the Office static-CFF `CharStrings` triplets are shifted by `-2` bytes relative to the standard `ffff 03` INDEX framing
  - comparing the two streams at the same absolute relative offset produces the wrong population and misleading bucket counts
- Practical warning:
  - a naive same-origin replay can produce a bogus larger pool such as `272`
  - the corrected `-2` alignment is required to recover the stable `240`-entry taxonomy above

## Low-Equals-High Tail: New Local Macro Split

- Inside the already-isolated `240`-entry `low==high` pool, the earlier source-derived `9` buckets compress much further into a small number of purely local triplet-shape macros.
- New exact local results on `font2`:
  - `a == c` and `nextB == b+1`
    - selects exactly the `53`-entry bucket
    - no contamination from the other `8` buckets inside the `240`-entry pool
  - `a != c` and `nextB == b+1`
    - selects exactly the `59`-entry bucket
    - no contamination from the other `8` buckets inside the `240`-entry pool
  - `a != c` and `prevC == b` and `nextB != b+1`
    - selects exactly the `25`-entry bucket
- Remaining local macro families after those exact splits:
  - `a == c` and `prevC == b` and `nextB != b+1`
    - `37` entries
    - equals:
      - `29 + 8`
  - `a == c` and `prevC != b` and `nextB != b+1`
    - `49` entries
    - equals:
      - `48 + 1`
  - `a != c` and `prevC != b` and `nextB != b+1`
    - `17` entries
    - equals:
      - `11 + 4 + 2`
- Practical interpretation:
  - the two biggest `b == source_mid` families are now locally recognizable without directly consulting `source_mid`
  - and one of the `b == prevC` asymmetric families is also now locally exact
  - the remaining blocker is no longer the whole `240` tail, but the three residual local macro pools above

## Low-Equals-High Tail: Cross-Font Confirmation Of The Macro Split

- The same local macro split was checked against `font1` (`SourceHanSansSC-Regular`) using:
  - source `CharStrings` replay from `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
  - Office intermediate `/tmp/otto-cff-office-decoded.otf`
- `font1` reproduces the same strong local families in the first `65535` offsets:
  - `a == c` and `nextB == b+1`
    - exact `53`-entry family
  - `a != c` and `nextB == b+1`
    - exact `50`-entry family
    - the `font1` analogue of the `font2` `59` bucket
  - `a != c` and `prevC == b` and `nextB != b+1`
    - exact `27`-entry family
    - the `font1` analogue of the `font2` `25` bucket
- `font1` residual local macro counts in the same taxonomy space are:
  - `a == c` and `prevC == b` and `nextB != b+1`
    - `37`
    - equals:
      - `26 + 11`
  - `a == c` and `prevC != b` and `nextB != b+1`
    - `49`
  - `a != c` and `prevC != b` and `nextB != b+1`
    - `12`
- Interpretation:
  - this new macro split is not just a `font2` / Bold quirk
  - it appears to be another stable Office static-CFF family mechanism shared by at least:
    - `font1` Regular
    - `font2` Bold

## Low-Equals-High Tail: Residual Pool Follow-Up

- New stable follow-up facts inside the three residual local macro pools:
  - inside the `11 + 4 + 2` asymmetric residual pool:
    - `p2A == n2A`
    - selects exactly:
      - `7034`
      - `18807`
    - so the previously documented `2`-entry micro-family now also has an exact purely local selector once that residual pool is isolated
  - inside the same `11 + 4 + 2` pool:
    - `prevB + 1 == b`
    - selects `10/11` of the `11`-entry bucket
    - with no contamination from the `4`-entry or `2`-entry families
    - the remaining miss is:
      - `26455`
  - inside the `48 + 1` symmetric residual pool:
    - `a < b`
    - selects `46/48` of the `48`-entry bucket
    - and excludes the singleton `1`
  - inside the `29 + 8` symmetric carry pool:
    - `p3A == a`
    - `prevA == n2A`
    - `a > b`
    - selects an exact `9`-entry subset of the `29`-entry bucket
- Practical interpretation:
  - these are not yet full decoder rules for the remaining residual pools
  - but they are stable, reproducible subfamily cuts that materially reduce the search surface for the next round
  - the next highest-leverage targets are now:
    - the missing `1/11` singleton in the `11` pool (`26455`)
    - the remaining `2/48` exceptions in the `48` pool
    - the unsplit remainder of the `29 + 8` carry pool

## Low-Equals-High Tail: Full Exact Stepwise Decomposition

- A new local stepwise classifier now covers the full `font2` `low==high` tail exactly.
- Verification result on the documented `240`-entry `font2` pool:
  - predicted counts:
    - `59`
    - `53`
    - `48`
    - `29`
    - `25`
    - `11`
    - `8`
    - `4`
    - `2`
    - `1`
  - total mismatches between the stepwise classifier and the documented taxonomy:
    - `0`
- This is the first time the `240` entries have been covered end-to-end by a single local decision tree rather than a loose bucket inventory.

### Stage 1: Exact Macro Families

- The top-level exact local predicates are now:
  - `a != c` and `nextB == b+1`
    - exact `59`
  - `a == c` and `nextB == b+1`
    - exact `53`
  - `a != c` and `prevC == b` and `nextB != b+1`
    - exact `25`

### Stage 2: Asymmetric Residual `11 + 4 + 2`

- Remaining local macro:
  - `a != c`
  - `prevC != b`
  - `nextB != b+1`
  - size:
    - `17`
- Exact stepwise split inside that residual:
  - `p2A == n2A`
    - exact `2`
    - members:
      - `7034`
      - `18807`
  - `prevB + 1 == b`
    - exact `10/11` of the `11` family
    - members:
      - `13623`
      - `25354`
      - `26013`
      - `41154`
      - `42129`
      - `46155`
      - `59556`
      - `60777`
      - `60834`
      - `62239`
  - remaining `11` singleton:
    - `26455`
    - exact inside this residual by either:
      - `a == prevA`
      - or `n2A == b`
- The `4` family is now also fully decomposed:
  - `319`
    - exact by:
      - `a == b`
  - `334`
    - exact by either:
      - `c == b`
      - or `nextB == b`
  - `1472`
    - exact by:
      - `a > b`
  - `33`
    - exact inside the `4` family by either:
      - `run_len >= 16` and `not(c == b)`
      - or `a < b` and `not(nextB == b)`

### Stage 3: Symmetric Carry Residual `29 + 8`

- Remaining local macro:
  - `a == c`
  - `prevC == b`
  - `nextB != b+1`
  - size:
    - `37`
- Exact stepwise split inside that residual:
  - `nextB == a`
    - exact `8`
    - members:
      - `9555`
      - `52264`
      - `55489`
      - `57572`
      - `59016`
      - `59098`
      - `59289`
      - `59331`
  - remaining `29` family:
    - `n2A == a`
      - exact `26/29`
      - members:
        - `33831`
        - `41323`
        - `45285`
        - `50143`
        - `51209`
        - `51297`
        - `52052`
        - `52180`
        - `52639`
        - `52718`
        - `53359`
        - `53868`
        - `54567`
        - `55067`
        - `55824`
        - `56212`
        - `56725`
        - `56918`
        - `57343`
        - `57808`
        - `58383`
        - `59467`
        - `63199`
        - `64165`
        - `64632`
        - `65258`
  - final `29` residual singletons:
    - `55364`
      - exact inside the residual trio by any of:
        - `run_len == 6`
        - `a > b`
        - `a == p2A`
    - `61821`
      - exact inside the residual trio by:
        - `run_len == 3`
        - and `b < 128`
    - `62644`
      - exact inside the residual trio by:
        - `a == n3A`

### Stage 4: Symmetric Residual `48 + 1`

- Remaining local macro:
  - `a == c`
  - `prevC != b`
  - `nextB != b+1`
  - size:
    - `49`
- Exact stepwise split inside that residual:
  - `a < b`
    - exact `46/48` of the `48` family
  - remaining residual trio:
    - `2155`
      - exact inside the trio by:
        - `a == prevA`
    - `35189`
      - exact inside the trio by:
        - `b < 64`
    - `47528`
      - exact inside the trio by either:
        - `not(nextB == a)`
        - or `not(prevB + 1 == b)`

## Low-Equals-High Tail: Status Shift

- Practical implication of the full stepwise decomposition:
  - the `low==high` tail is no longer the main reverse-engineering blocker at the analysis level
  - at least for `font2`, it is now described by an exact local decision tree
- Important nuance:
  - the coarse macro split is cross-font stable between `font1` and `font2`
  - the finer singleton / residual-trio cuts above are currently verified only on `font2`
  - they should not yet be generalized to arbitrary Office static-CFF fonts without implementation-time validation

## Low-Equals-High Tail: Tiny Asymmetric Gap Family

- The rare asymmetric gap family previously suspected from:
  - `7034`
  - `18807`
- is now confirmed as an exact `2`-entry structural bucket inside the broader `source_high == source_low` set.
- Shared source-derived signature:
  - asymmetric (`a != c`)
  - `b != source_mid`
  - `b != prevC`
  - `source_mid == nextB - 1`
- This confirms those two are a real micro-family, not unrelated noise.
- Important limitation:
  - a safe purely local triplet-only rule for that `2`-entry family is **not** established yet
  - but they are now isolated cleanly for targeted follow-up

## sfntly_web Notes

- `/Users/ekxs/Codes/sfntly_web/sfntly/java/src/com/google/typography/font/tools/conversion/eot/` is useful as a reference for the public MTX / EOT TrueType path only.
- The shipped Java `EOTWriter` / `MtxWriter` path is fundamentally TrueType-oriented:
  - it removes `glyf/cvt/loca/hdmx/head`
  - it reconstructs MTX block1 via `GlyfEncoder`
  - `GlyfEncoder` only handles TrueType simple/composite glyphs
- There is **no** matching static CFF / CFF2 Office decode or encode path in that Java tree.
- Therefore:
  - do **not** keep digging in `sfntly_web` expecting an existing Office static CFF decoder
  - do use it only for public MTX / LZ / TrueType reference behavior
