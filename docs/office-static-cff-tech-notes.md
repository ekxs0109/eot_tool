# Office Static CFF Technical Notes

Last updated: 2026-04-21

## Scope

This note captures the stable reverse-engineering findings for the embedded CFF fonts inside:

- `/Users/ekxs/Codes/eot_tool/Presentation1.pptx`

It focuses on the two Source Han Sans SC embedded fonts:

- `font1.fntdata` -> `思源黑体 Regular`
- `font2.fntdata` -> `思源黑体 Bold`

The Calibri `font3.fntdata` issue is separate and is not covered here.

## Short Version

- `font1` compatibility on Windows PowerPoint was primarily an MTX container / LZ window problem, not a decoded-SFNT problem.
- `font2` decode failure was primarily a wrapped MTX decompressed-length problem.
- After MTX/LZ decode succeeds, both `font1` and `font2` do **not** decode to a standard static OTF.
- They decode to an Office-specific static CFF intermediate representation that keeps much of the original CFF payload but rewrites SFNT directory framing and early CFF INDEX framing.
- `fontTools.cffLib.CFFFontSet.decompile()` is useful as a quick probe, but it is too lax to prove the CFF bytes are already standard.

## Artifacts

### Embedded font fixtures

- `/Users/ekxs/Codes/eot_tool/build/presentation1_more_experiments/orig/ppt/fonts/font1.fntdata`
- `/Users/ekxs/Codes/eot_tool/build/presentation1_more_experiments/orig/ppt/fonts/font2.fntdata`
- `/Users/ekxs/Codes/eot_tool/testdata/otto-cff-office.fntdata`
- `/Users/ekxs/Codes/eot_tool/testdata/presentation1-font2-bold.fntdata`

### Current decoded outputs

- `/tmp/otto-cff-office-decoded.otf`
- `/tmp/presentation1-font2-decoded.otf`

### Local Adobe OTF references

- `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
- `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`

These local OTFs are useful for CFF structure analysis, table boundaries, and size references. They should **not** be treated as guaranteed byte-for-byte decode truth for the Office intermediate.

## Font1 Baseline

These findings are already confirmed with Windows PowerPoint and should be treated as solved baseline behavior:

- Original `font1` uses `copy_dist=9000`.
- Original `font1` `block1` real `max_copy_distance=8998`.
- Re-encoded payloads using modern long-distance back-references were rejected by PowerPoint.
- `font1` must keep the Office MTX 3-block shape:
  - `block1`
  - empty `block2`
  - empty `block3`
- Header-only fixes were insufficient until the trailing UTF-16 `NUL` in `family/style` was preserved.

Known-good compatibility package:

- `/Users/ekxs/Codes/eot_tool/Presentation1-exp-font1-newHeaderOrigPayload-plusNul.pptx`

## Font2 Decode Breakthrough

The original `font2` MTX block1 prefix is:

- `01 6b b2 4d`

Under the original public MTX interpretation:

- 1-bit flag + 24-bit decompressed length
- raw length = `0x02D764 = 186212`

That value is obviously implausible for a `14.5 MB` compressed block.

The key observation is:

- local `SourceHanSansSC-Bold.otf` size = `16963428 = 0x0102D764`
- raw 24-bit read returns exactly `0x02D764`

So the strong decode-side fix is:

- keep the normal MTX stream start at bit `25`
- reconstruct wrapped 24-bit decompressed lengths by adding `0x01000000` until the result is no longer smaller than the compressed block

This decode-side fix is now implemented in Rust and the regression test passes.

## Decoded Office Static CFF Intermediate

After MTX/LZ decode succeeds, both Source Han Sans outputs begin with the same nonstandard SFNT prefix:

```text
4f54544f0001020000040010
```

This differs from a legal standard OTF header such as:

```text
4f54544f0011010000040010
```

The decoded outputs still preserve recognizable table order:

- `BASE`
- `CFF `
- `DSIG`
- `GDEF`
- `GPOS`
- `GSUB`
- `OS/2`
- `VORG`
- `cmap`

So the bytes are not random corruption. Office has rewritten the wrapper.

## CFF Alignment

For both fonts, the strongest current alignment for the Office intermediate `CFF ` region is:

- decoded bytes starting at `0x20e`

Useful prefix examples:

### `font1` decoded at `0x20e`

```text
04030001010218536f7572636548616e5361537353432d526567756c6172...
```

### `font2` decoded at `0x20e`

```text
04030001010215536f7572636548616e5361537353432d426f6c64...
```

These do not start with a standard CFF header `01 00 04 03`, but they still line up strongly with the source `CFF ` tables after the first few bytes.

## Implementation Boundary After Task 2

The Rust extractor surface now exists in `fonttool_cff::office_static` and is intentionally narrow:

- `extract_office_static_cff()` recognizes the known Office static-CFF intermediate shape on the tracked regular and bold fixtures.
- It accepts either:
  - `OTTO...`
  - or a single leading `0x00` followed by `OTTO...`
- It normalizes the returned `sfnt_bytes` to begin with `OTTO`.
- It exposes:
  - `cff_offset = 0x20e`
  - `office_cff_suffix = &sfnt_bytes[0x20e..]`

Important contract note:

- `office_cff_suffix` is **not** a bounded standard `CFF ` table slice.
- It is only the Office intermediate suffix view beginning at the known `CFF `-like region.
- Current direct parsing evidence says a standard CFF `Name INDEX` parse at `office_cff_suffix[4..]` fails immediately on both fixtures, so Task 3 must reconstruct framing rather than treat the Office bytes as already-standard INDEX blocks.

## Source CFF Logical Boundaries

### `SourceHanSansSC-Regular.otf`

- CFF header: `0..3`
- Name INDEX: `4..31`
- Top DICT INDEX: `32..106`
- String INDEX: `107..936`
- Global Subr INDEX begins at `937`
- Global Subr data begins at `2806`

### `SourceHanSansSC-Bold.otf`

- CFF header: `0..3`
- Name INDEX: `4..28`
- Top DICT INDEX: `29..103`
- String INDEX: `104..876`
- Global Subr INDEX begins at `877`
- Global Subr data begins at `3362`

## What `fontTools` Thresholds Actually Mean

Using `fontTools.cffLib.CFFFontSet.decompile()` as a quick probe:

- `font1`: patching the first `940` CFF bytes from the source OTF makes `decompile()` succeed
- `font2`: patching the first `880` CFF bytes from the source OTF makes `decompile()` succeed

These numbers are useful, but they do **not** mean the CFF has been restored to a standard form.

Important nuance:

- `decompile()` is a shallow parser for this investigation
- it does not immediately validate every later INDEX offset or subroutine body
- therefore `940` and `880` should be read as:
  - "fontTools can now step through the prefix without bailing"
  - not:
  - "the CFF table is now standard and complete"

## Non-Monotonic Prefix Repair Trap

`font2` showed an especially important trap during prefix patching.

Using `fontTools.cffLib.CFFFontSet.decompile()`:

- patching first `100..106` bytes from the source OTF succeeds
- patching first `107..879` bytes fails again
- patching first `880+` bytes succeeds again

This means prefix repair is **non-monotonic**.

Reason:

- around source CFF offset `104`, standard `String INDEX` framing begins
- partially mixing standard `count/offSize` bytes with Office-transformed offset-array bytes produces inconsistent hybrids
- some shorter prefixes accidentally parse only because they collapse later INDEX counts to zero or otherwise dodge the failing path

So "more source prefix bytes" is not automatically "closer to a standard font".

## Strict Standard CFF INDEX Findings

To avoid over-trusting `fontTools`, a small strict CFF prefix checker was used. It validates, in standard CFF terms:

- header major version
- Name INDEX
- Top DICT INDEX
- String INDEX
- Global Subr INDEX

Specifically it requires:

- `offSize` in `1..=4`
- first INDEX offset = `1`
- monotonic offsets

### Exact prefix needed to restore the first four INDEX blocks

To make those four INDEX structures exactly match the source OTF framing:

- `font1` needs the first `2806` CFF bytes restored
- `font2` needs the first `3362` CFF bytes restored

These numbers are not arbitrary. They equal the start of source Global Subr data:

- `font1`: Global Subr data starts at `2806`
- `font2`: Global Subr data starts at `3362`

Interpretation:

- repairing only the CFF header, Name INDEX, Top DICT INDEX, or String INDEX header is not enough
- Office rewrite reaches through the entire Global Subr offset array
- for a standard rebuild, we will likely need to reconstruct at least:
  - standard CFF header
  - Name INDEX
  - Top DICT INDEX
  - String INDEX
  - Global Subr INDEX count/offSize/offset array

### Why the earlier `880/940` numbers are still useful

They still tell us where the earliest parse-critical incompatibility lives for a shallow parser:

- `font1`: around Global Subr INDEX header start
- `font2`: around Global Subr INDEX header start

But they do **not** bound the full rewrite region.

## Current Interpretation

The strongest current model is:

1. Office keeps a large amount of original static CFF payload content.
2. Office rewrites the outer SFNT directory into a nonstandard static-CFF wrapper.
3. Office rewrites early CFF INDEX framing, at least through the Global Subr offset array.
4. `OS/2` and `head` are also structurally transformed, but CFF framing is currently the most productive reconstruction path.

## Post-GlobalSubr Tail Reuse Signal

There is now a second important signal after the strict INDEX findings.

When comparing source `CFF ` bytes against the decoded Office intermediate **after** Global Subr data begins:

- direct same-offset equality is poor
- but `src[i] == office[i - 2]` becomes much stronger over larger windows

Over the first `20 KB` after Global Subr data starts:

- `font1`: same-offset matches = `2249 / 20000`, but `-2` shifted matches = `11586 / 20000`
- `font2`: same-offset matches = `2177 / 20000`, but `-2` shifted matches = `10780 / 20000`

This is too large to be coincidence.

### Where the `-2` tail alignment gets strong

Using sliding windows over source `CFF ` tails:

- `font1`
  - first `1024`-byte window with `>= 85%` `-2` alignment: source offsets `8182..9206`
  - best sampled `512`-byte window: `9078..9590`, ratio `0.914`
- `font2`
  - no `1024`-byte window reached `0.85` in the first scanned range
  - best sampled `512`-byte window: `8802..9314`, ratio `0.869`

There are also many exact small-window hits at that same `-2` delta:

- `font1`: 32-byte exact-window matches repeatedly appear with `bad = src - 2`
- `font2`: same pattern appears later, again with `bad = src - 2`

Interpretation:

- the post-INDEX region is not uniformly random or freshly re-encoded
- at least in later Global Subr / CharString-related regions, Office seems to preserve substantial original payload content with a stable `-2` positional skew

This does **not** yet prove that every later byte can be recovered by a simple global shift.

But it strongly suggests the eventual reconstruction strategy may need three zones instead of one:

1. standard CFF prefix rebuilt from source-style framing
2. early Global Subr data region with heavier Office rewriting
3. later tail region where source payload reappears with a strong `-2` positional skew

## Important Boundary After GlobalSubrs: CharStrings Still Broken

A later probe narrowed the next major failure point more precisely.

Using source-wrapper hybrid experiments:

- source CFF prefix was kept intact
- later CFF tail was replaced with Office-intermediate bytes, optionally using the observed `-2` positional skew

Result:

- sampled `GlobalSubrs` can be accessed successfully in both fonts
- sampled `CharStrings` fail immediately from glyph index `0` onward

This means the current dominant breakage is **not** simply "Global Subr data is unreadable".

It is more specifically that the standard `CharStrings` machinery is still not reconstructed:

- `CharStrings INDEX`
- and/or structures it depends on:
  - `charset`
  - `FDSelect`
  - `FDArray` / `Private DICT` relationships

### Source Top DICT offsets

For the local Source Han Sans SC references:

#### `font1` / Regular

- `charset` offset = `10732`
- `FDSelect` offset = `10737`
- `CharStrings` offset = `11081`
- `FDArray` offset = `14267431`

#### `font2` / Bold

- `charset` offset = `12692`
- `FDSelect` offset = `12697`
- `CharStrings` offset = `13041`
- `FDArray` offset = `14784666`

### CharStrings INDEX probe

At the source `CharStrings` offset:

#### `font1`

- source bytes begin with a valid CharStrings INDEX header:
  - count=`65535`
  - offSize=`3`
  - first offsets=`[1, 77, 80, 114, 163, 257, 368, 504, ...]`
- Office intermediate at the **same** offset collapses to nonsense:
  - count=`768`
  - offSize=`0`
- Office intermediate at `offset - 2` restores only part of the shape:
  - count=`65535`
  - offSize=`3`
  - but offsets are still wrong very early:
    - `[1, 333, 19792, 20594, 29347, 1, 368, 28920, ...]`

#### `font2`

- source bytes begin with:
  - count=`65535`
  - offSize=`3`
  - first offsets=`[1, 77, 80, 129, 149, 247, 370, 507, ...]`
- Office intermediate at the **same** offset again collapses:
  - count=`768`
  - offSize=`0`
- Office intermediate at `offset - 2` is still invalid even at the first offset:
  - count=`65535`
  - offSize=`3`
  - first offsets=`[129, 16449613, 16469328, ...]`

Interpretation:

- the CharStrings region is not recoverable by a simple whole-tail `-2` shift
- at least the CharStrings offset array itself is structurally transformed
- this is consistent with the observed behavior:
  - `GlobalSubrs` are readable
  - `CharStrings` explode immediately

### Practical implication

The next focused reverse-engineering target should move from:

- "maybe the remaining problem is still early Global Subr data"

to:

- "how Office rewrites `charset` / `FDSelect` / `CharStrings INDEX` framing and offsets"

So the best current staged model is now:

1. standard prefix rebuilt through Global Subr offset array
2. later tail reuse explored under `-2` skew where it holds
3. separate targeted reconstruction for `charset` / `FDSelect` / `CharStrings INDEX`

## Isolation Experiments: CharStrings INDEX Is The Primary Breaker

More isolated source-wrapper experiments were run by replacing one structure at a time with Office-intermediate bytes under the observed `-2` alignment.

### Replacements that still work

For both `font1` and `font2`, the following isolated replacements still allowed sampled glyph access:

- `charset` only
- `charset + FDSelect`
- `Global Subr data` (from Global Subr data start up to `charset`)
- `CharStrings data zone` (from CharStrings data start up to `FDArray`)

The most important result is:

- replacing the **CharStrings data zone** alone with Office `-2` bytes still allows broad sampled glyph access

In one probe, the following glyph IDs all loaded successfully after replacing CharStrings data only:

- `0..511`
- `1024`
- `2048`
- `4096`
- `8192`
- `16384`
- `32768`
- `50000`
- `65534`

This means the glyph program bodies themselves are **not** the first-order incompatibility.

### Replacement that fails immediately

For both fonts:

- replacing **only** `CharStrings INDEX` header + offset array with Office bytes fails immediately with `AssertionError`

That is the cleanest isolation result so far.

Interpretation:

- `charset` is not the blocker
- `FDSelect` is not the blocker
- Global Subr data is not the blocker by itself
- CharStrings data bytes are not the blocker by themselves
- the primary structural incompatibility sits in the **CharStrings INDEX framing / offset array**

### Practical consequence

The next decode/rebuild target should narrow further from:

- `charset / FDSelect / CharStrings INDEX`

to:

- **CharStrings INDEX reconstruction first**

with `charset` / `FDSelect` treated as secondary follow-up checks rather than equal-priority unknowns.

## CoreText Registration Is Too Shallow To Validate Static-CFF Rebuilds

A new `font2` source-wrapper experiment was run using:

- `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
- `/tmp/presentation1-font2-decoded.otf`

Three temporary hybrids were built under the observed Office `-2` skew:

- `/tmp/font2-source-wrapper-office-charstrings-data.otf`
  - source wrapper with Office `CharStrings INDEX + data`
- `/tmp/font2-source-wrapper-office-charstrings-index-only.otf`
  - source wrapper with Office `CharStrings INDEX` only
- `/tmp/font2-source-wrapper-office-charstrings-data-only.otf`
  - source wrapper with Office `CharStrings data` only

All three still pass the current Swift/CoreText registration probe:

- `descriptorCount=1`
- `registerFontsForURL=true`
- `cgFontLoad=true`

So `CTFontManager` / `CGFont` acceptance is **not** a strong enough oracle here. A shallow loader can still accept fonts whose `CharStrings` internals remain structurally wrong.

### Stronger check with real CFF outline traversal

The repo's `fonttool convert --to ttf` path is much more informative because it forces allsorts to traverse CFF outlines.

Results on the same three hybrids:

- Office `INDEX + data` replacement:
  - fails with:
  - `range end index 16449612 out of range for slice of length 435622`
- Office `INDEX only` replacement:
  - fails with the **same** large bogus range:
  - `range end index 16449612 out of range for slice of length 435622`
- Office `data only` replacement:
  - gets past the offset-array structure
  - later fails during outline interpretation with:
  - `failed to visit glyph outline: an invalid amount of items are in an arguments stack`

### Practical interpretation

This materially sharpens the earlier isolation result:

- `CharStrings INDEX` is still the first-order structural blocker.
  - Once Office `INDEX` bytes are introduced, allsorts immediately chases a bogus huge range.
  - The failure signature is stable whether or not Office charstring bodies are also present.
- `CharStrings data` is a **secondary** problem.
  - Replacing only the data zone no longer triggers the giant-offset failure.
  - But later glyph-program semantics are still not fully standard under the current source-wrapper experiment.

Do **not** treat any of these as proof of successful standard-CFF rebuild:

- `CTFontManagerRegisterFontsForURL`
- `CGFont` creation
- shallow loader success

## Heuristic Decoder And Local Mode Blocks

The current best lightweight decoder for the Office `CharStrings INDEX` is now more explicit.

After skipping the first special Office triplet (`group0`) and keeping the source first offset `1`:

- read each later Office triplet as `[a, b, c]`
- keep a rolling `page`
- keep the previous **encoded** low byte, not the previous decoded low byte
- if `b == previous_encoded_low`, decode:
  - `offset = page << 8 | c`
- otherwise decode:
  - `page = b`
  - `offset = page << 8 | c`

This simple heuristic is surprisingly strong:

- `font1`: `595 / 600` exact matches against the source offsets
- `font2`: `572 / 600` exact matches against the source offsets

So the Office format is not globally alien. Most entries still follow a very regular rolling form.

### `font2` mismatch runs in the first 600 offsets

The current mismatch runs are:

- `(25,27)`
- `(33,37)`
- `(49,50)`
- `(52,61)`
- `(206,206)`
- `(222,222)`
- `(319,319)`
- `(334,337)`
- `(538,538)`

Interpretation:

- Office uses the normal rolling form for most of the `CharStrings INDEX`
- but switches to localized alternative control forms in a small number of regions
- those regions are sparse enough that a future decoder can likely be state-machine based rather than full-table brute force

## Stronger Evidence For A `low=00` Surrogate Form

The earlier `font1` suspicion about `...00` offsets is now better supported by `font2`.

Confirmed examples:

- `font1`: source `0x4500` -> Office triplet `31 45 31`
- `font1`: source `0x4800` -> Office triplet `44 48 44`
- `font2`: source `0x2a00` -> Office triplet `27 2a 27`

Interpretation:

- Office does **not** emit the real low byte `00` directly in these entries
- instead it emits a surrogate anchor byte
- the following normal-form entries in that local region then reuse that surrogate as the "previous encoded low" byte

This explains why the naive heuristic can be numerically wrong while the surrounding run still stays internally coherent.

### Important nuance

This surrogate form is **not** yet fully globalized into one clean rule.

Counterexamples show that there are at least two closely related special cases:

- a straightforward surrogate form like `27 2a 27` for source `0x2a00`
- a more compressed page-transition form such as:
  - source `0x4400` -> Office `00 1d 1d`
  - later entries in that run continue using the surrogate chain until the stream realigns

So `low=00` handling is definitely real, but it is mode-dependent.

## Patch Experiments: These Are Multi-Entry Blocks, Not Single Markers

More focused `font2` patch experiments were run against the heuristic-rebuilt standard `CharStrings INDEX`, while still reusing Office charstring bodies under the known-good source-wrapper setup.

The clean result is:

- patching only the start item of a bad run does **not** move the failure point
- patching a whole local bad run **does** move the failure point

Concrete results:

- heuristic-only index:
  - first failing glyph = `27`
- patching only glyph `25`:
  - first failing glyph = `27`
- patching only glyphs `26..27`:
  - first failing glyph = `25`
- patching glyphs `25..27`:
  - first failing glyph = `37`
- patching `25..27` and `33..37`:
  - first failing glyph = `50`
- patching `25..27`, `33..37`, and `49..50`:
  - first failing glyph = `53`

That is strong evidence that the early `font2` failures are genuine local control blocks, not just isolated bad page markers.

### Finer result for the `52..61` run

After patching the earlier runs:

- patching `52..54` moves the first failing glyph to `55`
- patching `52..60` moves the first failing glyph to `60`
- patching `53..61` is already enough to move the first failing glyph to `222`

Interpretation:

- most of `52..61` behaves like a contiguous local mode block
- but entry `52` itself is numerically mismatched under the heuristic without being parse-fatal
- later isolated mismatches such as `206`, `319`, and `538` also currently appear to be non-fatal under parsing

This matters for decoder design:

- "exact numeric match to source offset" and "first parse-fatal mismatch" are related, but not identical
- some Office local forms preserve enough structural validity that a wrong naive boundary can still parse
- therefore future validation should track both:
  - exact offset agreement
  - first semantic / parse failure point

## `font1` Looks Like The Same Mechanism, But Simpler

The same patch method was also applied to `font1`.

Results:

- heuristic-only `font1` rebuild fails very early
- patching the early mismatch block `5..7` moves the first failing glyph to `373`
- patching `359` alone does **not** move that failure point
- patching `373` is enough to restore the sampled parse path

Interpretation:

- `font1` appears to use the same style of localized control blocks as `font2`
- but it has far fewer of them
- not every exact numeric mismatch is equally important
- again, there is a difference between:
  - exact offset equality
  - parse-fatal structural mismatch

## The `a` Byte Behaves Like An Anchor Range Selector

The first Office triplet byte (`a`) is now clearly meaningful.

For `font2`, many consecutive `mid`-byte ranges share a constant `a` value on page-change entries.

Examples:

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

Within a normal anchor range, the triplet shape is:

- page-change entry:
  - `[anchor, current_mid, current_low]`
- same-page entry:
  - `[anchor, previous_encoded_low, current_low]`

So the current simple decoder is already getting the `b/c` rolling chain mostly right.

What remains is:

- how Office chooses / updates the anchor byte
- how local special blocks temporarily break the normal `previous_encoded_low` flow

This is a much narrower problem than "decode the whole index from scratch".

## Sparse-Block Model Gets Stronger

A stronger `font2` sanity check was run using the heuristic-rebuilt index plus only the currently known bad local blocks patched back to source offsets:

- `25..27`
- `33..37`
- `49..50`
- `53..61`
- `206`
- `222`
- `319`
- `334..337`
- `538`

Result:

- the first failing glyph moved from `27` to `618`

Interpretation:

- the dominant decoding model is already broadly correct
- the remaining incompatibilities really do behave like sparse local control blocks
- the task is no longer "find a totally different index format"
- it is "characterize the few local block forms that perturb an otherwise regular rolling encoding"

## High-Byte Transitions Narrow The Problem Further

A very useful probe was run around the first `0x00ffff -> 0x010000` rollover in `font2`.

At the first high-byte transition:

- source `0x010021` becomes Office `01 f6 21`
- then:
  - `0x010097` -> `01 21 97`
  - `0x0100aa` -> `01 97 aa`
  - `0x010112` -> `01 01 12`

This matters because it shows:

- the Office triplet `a` byte is not merely opaque control noise
- for `high > 0`, `a` is often the **real high byte**
- in those regions, the normal form often looks like:
  - `[high, current_mid, current_low]` for page-change entries
  - `[high, previous_encoded_low, current_low]` for same-page entries

Additional rollover checkpoints:

- `0x020004` -> `02 00 04`
- `0x030014` -> `03 00 14`
- `0x040112` -> `04 01 12`
- `0x050116` -> `05 01 16`

Interpretation:

- the difficult part is **not** a totally unknown 24-bit encoding on every entry
- the high-byte channel becomes relatively transparent once `high > 0`
- the remaining complexity is concentrated in:
  - the early `high == 0` region
  - local surrogate forms for `mid=00` / `low=00`
  - sparse later control blocks such as the single-entry failures at `619`, `812`, and `832`

## A Provisional State-Machine Decoder Now Covers Most Of The `high==0` Region

The `font2` `CharStrings INDEX` work has now moved beyond a plain heuristic.

The current provisional decoder keeps the normal rolling rule:

- page-change:
  - `[anchor, mid, low]`
- same-page:
  - `[anchor, previous_encoded_low, low]`

and adds three special forms.

### 1. Low-zero surrogate

Detect:

- `[anchor, mid, anchor]`

Decode as:

- `mid00`

This cleanly explains examples like:

- `00a300` from `94 a3 94`
- `00a800` from `94 a8 94`

### 2. Swapped page-change form

Detect:

- `[page, old_anchor, low]`
- with `old_anchor != 0`

Decode as:

- `page << 8 | low`

This explains examples like:

- `00dcfb` from `dc 7b fb`
- `00f6ec` from `f6 f2 ec`
- `00f8fd` from `f8 f6 fd`

### 3. Repeated-page low surrogate

Detect:

- `b == prev1`
- `c == prev2`
- `c < b`

Decode as:

- `page << 8 | page`

But importantly:

- keep the encoded-low chain on `c`
- do **not** advance the chain to the emitted repeated low byte

This explains examples like:

- `006c6c`
- `007575`
- `00d6d6`

and avoids the earlier false positive at `00e8c9`.

## What This New Decoder Fixes

Important positive hits under the provisional state machine:

- `538`: `006c6c`
- `619`: `007575`
- `812`: `00a300`
- `832`: `00a800`
- `996`: `00d6d6`
- `1028`: `00dcfb`
- `1207`: `00f6ec`
- `1208`: `00f6f9`
- `1231`: `00f8fd`

The normal entry:

- `1063`: `00e8c9`

now also stays correct.

### Match-rate improvement

Compared against source offsets:

- first `600` offsets:
  - plain heuristic: `572/600`
  - provisional state machine: `574/600`
- first `1300` offsets:
  - plain heuristic: `1227/1300`
  - provisional state machine: `1240/1300`

So the raw count improvement is modest, but the structural improvement is much larger:

- sparse later single-point failures are being absorbed into principled decoder rules instead of ad hoc patches

## Clean Boundary Before The High-Byte Rollover

A very important new boundary emerged.

After applying the provisional decoder, the remaining mismatches in the range `600..1399` collapse to:

- `(1266..1399)`

That is, there are no more isolated later mismatches in `600..1265`.

This is significant because:

- `1266` is exactly the first `0x00ffff -> 0x010000` rollover region

Interpretation:

- the `high==0` state machine is now substantially understood
- the next dominant unresolved zone begins where the true high byte becomes `0x01`
- this is a much cleaner handoff point for the next reverse-engineering pass

## Parse Progress Improves Again

Using:

- the new provisional decoder
- plus the already-known early hard-block source patches:
  - `25..27`
  - `33..37`
  - `49..50`
  - `53..61`
  - `206`
  - `222`
  - `319`
  - `334..337`

the first failing sampled glyph for `font2` now moves to:

- `1265`

Earlier, the heuristic-only route stalled around:

- `812`

So this is another real forward step, not just a cosmetic reclassification of the same failures.

## The `0x010000` Rollover Starts With A Small Bootstrap Block

The first `high=0x01` region now looks less mysterious than before.

Using:

- the provisional special decoder
- the already-known early hard-block source patches

the exact mismatch picture originally stayed continuous from:

- `1266..1399`

However, a very small bootstrap correction at:

- `1266..1269`

changes that picture dramatically.

After patching only those four lower-16 offsets to the source values, the next exact mismatch runs become:

- `(1396,1396)`
- `(1434,1436)`
- `(1472..)` in the scanned window

Interpretation:

- `1266..1269` is a dedicated rollover-start control block
- it is **not** evidence that all of `0x01xxxx` uses a fundamentally different lower-16 encoding
- once that startup block is neutralized, most of the `high=0x01` region falls back onto the same rolling logic that already worked before

## `high=0x01` Reframes The Meaning Of Some Low-Byte Surrogates

The low-byte behavior in the `high=0x01` region is now clearer.

### `low=0x00` stays explicit

Examples:

- `011400 -> 21 14 00`
- `011e00 -> 17 1e 00`
- `019400 -> 91 94 00`
- `01cb00 -> c5 cb 00`
- `01fe00 -> e0 fe 00`

So the `high=0x00` style `mid00` surrogate is **not** simply reused for every low zero in `high=0x01`.

### `low=0x01` often becomes an anchor-surrogate

Examples:

- `014401 -> 23 44 23`
- `018c01 -> 7a 8c 7a`
- `01b001 -> a7 b0 a7`
- `01d101 -> c5 d1 c5`
- `01db01 -> d6 db d6`
- `01e701 -> e0 e7 e0`

This is a real family, not an isolated coincidence.

### Important exception: bootstrap / anchor-change entry

One `low=0x01` entry clearly does **not** belong to that family:

- `015b01 -> 12 01 7f`

This one behaves like a bootstrap / anchor-change control item rather than a plain low-byte surrogate.

## `[anchor, mid, anchor]` Is Now Confirmed To Be Overloaded

This is one of the most important corrections in the current notes.

The byte shape:

- `[anchor, mid, anchor]`

does **not** have one universal meaning.

Confirmed meanings now include:

- `mid00` surrogate in parts of the early `high=0x00` region
- `mid01` surrogate in parts of the `high=0x01` region
- literal `mid + anchor` later on

Concrete literal counterexample:

- `017c66 -> 66 7c 66`

The correct lower-16 value there is:

- `0x7c66`

not:

- `0x7c00`
- or `0x7c01`

Practical implication:

- do **not** globalize any rule like `if a == c then low = 0`
- or `if a == c then low = 1`

That shape is family-dependent.

## A New Ambiguity Class: Page-Change Collides With `prev_low`

The exact mismatch run:

- `1434..1436`

looks like a different control family again.

Example:

- `014fae -> 46 4f ae`

Here:

- `b == prev_low`

So the naive rolling decoder interprets it as same-page data.
But the correct source value is actually a page change to:

- `0x4f`

This means there is a genuine ambiguity class where:

- the intended page byte
- and the previous encoded low byte

are numerically equal.

So the current rule:

- `if b == prev1 => same page`

is no longer sufficient in every context.

## Probe Outcome After Bootstrap

Using:

- the provisional special decoder
- the early hard-block source patches
- the `1266..1269` rollover bootstrap patch
- and a temporary patch for the old `52` blocker so it stops masking later behavior

sampled glyph access now reaches:

- `4095`

and the first sampled failure moves to:

- `8192`

This is another significant forward step. The work is still incomplete, but the reverse-engineering target is narrower and much more structured than it was before.

## Later `high=0x01/0x02` Work Separates Into Two Clear Categories

The next round of analysis after the `1472` bootstrap patch split the remaining exact mismatches into two distinct families.

### 1. Literal-anchor singletons

Examples:

- `017c66 -> 66 7c 66`
- `022411 -> 11 24 11`
- `022511 -> 11 25 11`
- `02524d -> 4d 52 4d`

These all share the same byte shape:

- `[anchor, mid, anchor]`

But unlike the surrogate cases, the correct lower-16 value is the literal:

- `mid + anchor`

So they are **not** surrogate encodings at all.

This confirms again that `[anchor, mid, anchor]` is fundamentally overloaded.

### 2. Bootstrap / anchor-change entries

After patching those literal-anchor singletons, the next dominant exact mismatch became:

- `2227..`

Patching only:

- `2227`

moves that frontier forward again to:

- `2313..`

This is strong evidence that `2227` is another bootstrap / anchor-change entry rather than a generic page-chain error.

## `high=0x02 low=0x02` Surrogate

A new clean singleton then appears at:

- `2313`

Example:

- source `02df02`
- Office triplet `76 df 76`

This is the direct `high=0x02` analogue of the earlier `high=0x01 low=0x01` surrogate family.

So `2313` is best interpreted as:

- a singleton `low == high` surrogate
- not a new broad family transition

Patching only that one entry removes the first mismatch there and exposes:

- `2319..`

## Bootstrap At `2319`

The next dominant entry is:

- source `02e200`
- Office triplet `a7 95 1b`

This behaves like another bootstrap / anchor-change block.

After patching:

- `2313`
- and `2319`

the first exact mismatch frontier moves again to:

- `2395..`

So the remaining unsolved region continues to decompose into:

- sparse singleton surrogate / literal ambiguities
- occasional bootstrap entries that retarget the active anchor family

## Practical Interpretation

At this stage the problem is no longer "there is one big unknown encoding after the rollover".

It is much more structured:

1. a small number of bootstrap entries that switch anchor regimes
2. several overloaded singleton forms such as:
   - surrogate `low == high`
   - literal `mid + anchor`
3. otherwise a stable rolling lower-16 reconstruction

## Exactness vs Parse Survival

One more important practical note:

Even with many later exact mismatches still present, the current reconstructed table has become much more parser-tolerant than before.

In the latest probe with:

- the provisional special decoder
- the known early hard-block patches
- the known bootstrap and singleton patches through `2319`

sampled glyph access through:

- `32768`

still succeeded in that probe.

So the remaining work from here is mainly:

- exact offset-table recovery

rather than:

- immediate parser survival

## The Next Layer Starts At `2395`: `0x02ffff -> 0x030000`

After patching the currently known families through:

- the `2313` singleton surrogate
- and the `2319` bootstrap

the next exact mismatch frontier becomes:

- `2395..`

This is not random late-table drift. It lines up with the next high-byte transition layer around:

- `0x02ffff -> 0x030000`

### The first failing entry

- source `0302fb`
- Office triplet `d5 00 fb`

The current decoder reconstructs this as:

- `0400fb`

So the failure is best read as:

- a wrong high-byte / anchor regime
- with a still-recognizable lower-byte payload

The next few entries keep the same flavor:

- `0303a5 <- 03 d5 a5`
- `03040e <- d5 04 0e`
- `030468 <- d5 0e 68`

This is very similar in spirit to the earlier rollover bootstrap blocks:

- the rolling lower-16 chain is still visible
- but the active high-byte / anchor assignment has switched incorrectly

## The Region After `2395` Is Still Mostly Rolling

Even inside the new mismatch zone, many later entries are structurally transparent:

- `030633 <- 03 06 33`
- `030799 <- 03 07 99`
- `030837 <- 03 08 37`
- `030a80 <- 03 0a 80`
- `031017 <- 03 10 17`

So the pattern remains consistent with the broader interpretation:

1. stable rolling lower-16 reconstruction
2. sparse bootstrap / anchor-change blocks
3. overloaded singleton forms on top of that rolling stream

## More Overloaded Singleton Forms Continue Inside `0x03xxxx`

The `0x03xxxx` region immediately shows more of the same structural style rather than a brand-new compression method.

Examples:

- `032599 <- 88 25 99`
- `032768 <- 88 27 68`
- `033384 <- 32 33 84`
- `033650 <- 32 36 50`
- `033872 <- 32 38 72`
- `033c30 <- 32 3c 30`

These look like the same broad family behavior already seen earlier:

- some are literal-anchor singletons
- some are singleton surrogates
- all of them sit on top of a mostly regular rolling sequence

## Clean Frontier Up To `2395`

With all currently known patches applied through `2319`, the exact mismatch frontier in the current `2600`-entry probe becomes:

- `(2395..2599)`

That means:

- no new exact mismatches remain before `2395`

So the next focused target is now very clean:

- characterize the `0x03xxxx` bootstrap around `2395`

rather than continuing to chase mixed earlier leftovers.

## What Not To Do

- Do not assume the Office static CFF output is "almost a legal OTF" with only a few wrong header bytes.
- Do not treat `fontTools.cffLib.CFFFontSet.decompile()` success as proof of standard CFF validity.
- Do not go back to generic XOR theories for `font2`.
- Do not keep digging in `sfntly_web` looking for a static CFF Office decoder; that tree is effectively TrueType-oriented for this problem.

## Best Next Step

The most promising next implementation path is:

1. Rebuild a standard CFF prefix from the source-style logical structure.
2. Treat the rebuild boundary as at least:
   - `2806` bytes for `font1`
   - `3362` bytes for `font2`
3. Only then evaluate how much of the later payload can be reused verbatim from the Office intermediate and how much still needs table-aware reconstruction.

## Sparse Lower-16 Frontier After The `2550..2555` Bootstrap

The next round of exact lower-16 comparison kept the same methodology:

- use the rolling heuristic as the baseline decoder
- keep previously established bootstrap / singleton fixes through:
  - `1266..1269`
  - `2395`
  - `2400..2404`
  - `2550..2555`
- then inspect the next exact lower-16 mismatch frontier

The important change is that the frontier is no longer a continuous zone.

The next sparse exact runs are:

- `2866..2867`
- `2902..2903`
- `2956`
- `3183`
- `3282`
- `3336`
- `3372`

Minimal patch progression is very clean:

- patch `2866` only -> frontier moves to `2867`
- patch `2866..2867` -> frontier moves to `2902`
- patch `2866..2867` plus `2902..2903` -> frontier moves to `2956`
- then patching `2956`, `3183`, `3282`, `3336`, `3372` advances the frontier one sparse singleton at a time
- after patching through `3372`, no lower-16 exact mismatch remains through `3399`

This is strong evidence that the post-`2555` region is not another long bootstrap block.
It is a sparse overlay of a few recurring local ambiguity families.

## One Dominant Family Persists: Page-Change Collides With `prev_encoded_low`

The largest recurring family in the new sparse layer is the same ambiguity already seen earlier around `1434..1436`.

Shape:

- the current triplet satisfies `b == prev_encoded_low`
- the rolling heuristic therefore interprets it as a same-page entry
- but the correct source lower-16 actually needs a new page change to `b`

Representative later examples:

- `2866`: source `04847a`, Office `5e 84 7a`
  - heuristic lower-16: `837a`
  - correct lower-16: `847a`
- `2902`: source `04a05c`, Office `5e a0 5c`
  - heuristic lower-16: `9f5c`
  - correct lower-16: `a05c`
- `2903`: source `04a0b0`, Office `5e 5c b0`
  - heuristic lower-16: `9fb0`
  - correct lower-16: `a0b0`
- `2956`: source `04cbbe`, Office `5e cb be`
  - heuristic lower-16: `cabe`
  - correct lower-16: `cbbe`
- `3183`: source `05b69c`, Office `a9 b6 9c`
  - heuristic lower-16: `b59c`
  - correct lower-16: `b69c`
- `3282`: source `061b60`, Office `06 1b 60`
  - heuristic lower-16: `1a60`
  - correct lower-16: `1b60`
- `3336`: source `065f22`, Office `24 5f 22`
  - heuristic lower-16: `5d22`
  - correct lower-16: `5f22`
- `3372`: source `0693d6`, Office `6e 93 d6`
  - heuristic lower-16: `92d6`
  - correct lower-16: `93d6`

Interpretation:

- the ambiguity is not limited to the early `0x01xxxx` transition layer
- it survives well into later high-byte regimes
- therefore the real decoder rule cannot remain:
  - "if `b == prev_encoded_low`, then same-page"

There must be an additional discriminator that sometimes upgrades that exact byte shape into a page change.

## `2867` Confirms The `high==low` Surrogate Family Continues

The paired entry at `2867` is structurally different from the surrounding page-collision items.

Example:

- source `048504`
- Office triplet `5e 85 5e`

Lower-16 behavior:

- heuristic gives `855e`
- correct lower-16 is `8504`

This is the same overloaded `[anchor, mid, anchor]` family already seen earlier,
but now the low byte equals the full 24-bit high byte `0x04`.

So this extends the already-confirmed surrogate ladder:

- `01xxxx` cases where low byte `== 0x01`
- `02df02`
- now `048504`

## Next Sparse Layer After `3372`

After patching the sparse set through `3372`, the next lower-16 exact singleton runs through `5000` begin at:

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

Already-stable classifications from that later layer:

- `3437`: source `06d306`, Office `b5 d3 b5`
  - another confirmed `high==low` surrogate, now for high byte `0x06`
- `3589`: source `075452`, Office `16 54 52`
  - page-collision family
- `3614`: source `076f50`, Office `16 6f 50`
  - page-collision family
- `3756`: source `07fbd4`, Office `a0 fb d4`
  - page-collision family
- `4284`: source `0a260a`, Office `05 26 05`
  - confirmed `high==low` surrogate for high byte `0x0a`
- `4294`: source `0a328e`, Office `27 32 8e`
  - page-collision family
- `4318`: source `0a494d`, Office `05 49 4d`
  - page-collision family
- `4369`: source `0a715b`, Office `5f 71 5b`
  - page-collision family
- `4663`: source `0ba50f`, Office `32 a5 0f`
  - page-collision family
- `4689`: source `0bca0b`, Office `bb ca bb`
  - confirmed `high==low` surrogate for high byte `0x0b`

Still unresolved / not yet safely classified:

- `3496`
- `4253`
- `4256`
- `4289`

## The `3437..4689` Layer Advances Strictly One Singleton At A Time

Using the same lower-16 exact-match method, once all known mismatches through `3372` are patched, the next frontier advances in a very clean singleton chain:

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

Interpretation:

- this region is not hiding another contiguous bootstrap block
- it is a sparse chain of overloaded singleton forms

## `3496` Looks Like A Tiny `c==00` Boundary Exception

This entry now looks more like a very small boundary family than a fresh bootstrap type.

Example:

- source `070403`
- Office `07 04 00`
- heuristic lower-16 `0400`
- correct lower-16 `0403`

A useful scan across the table found:

- entries with `c==00` and `a==high` are common
- almost all of them correspond to source `...00`
- only a tiny outlier set has source low byte != `00`

Current clear outliers:

- `3496`: `070403 <- 07 04 00`
- `10091`: `230022 <- 23 00 00`

Practical interpretation:

- do not assume `c==00` with `a==high` always means a literal `...00`
- there is at least a tiny boundary family where the real low byte is omitted and must be recovered from additional state

## `4253` And `4256` Behave Like `a==high` Bootstrap / Page-Injection Singletons

These two entries now look like a separate singleton family.

They are distinct from:

- the page-collision family (`b == prev_encoded_low`, but real result is page change)
- the symmetric low-equals-high surrogate family

They also patch independently:

- patch `4253` -> frontier moves to `4256`
- patch `4256` -> frontier moves to `4284`

Examples:

- `4253`: source `0a0687`, Office `0a 87 6e`
  - relation: `a == high`, `b == source_low`
  - heuristic lower-16 `876e`
  - correct lower-16 `0687`
- `4256`: source `0a09f3`, Office `0a e9 cd`
  - relation: `a == high`
  - heuristic lower-16 `e9cd`
  - correct lower-16 `09f3`

The local neighborhood is especially informative:

- normal rolling entries still surround them:
  - `0a04bf`
  - `0a05ad`
  - `0a07da`
  - `0a08dc`
  - `0a0afa`
- only the missing page entries (`06`, `09`) collapse into these singleton forms

So the strongest current interpretation is:

- these are `a==high` bootstrap / page-injection singletons
- not ordinary same-page ambiguity

## Low-Equals-High Surrogates Continue, And Symmetry Is No Longer Required

The later scan also strengthens the `low == high` family.

Important correction:

- this family is no longer limited to the symmetric `[anchor, mid, anchor]` shape

Later confirmed or very strong examples:

- `4284`: `0a260a <- 05 26 05`
- `4289`: `0a2c0a <- 0e 2c 05`
- `4689`: `0bca0b <- bb ca bb`
- `5296`: `0e570e <- 54 57 54`
- `5346`: `0e8c0e <- 5c 8c 5c`
- `5575`: `0f8b0f <- 3f 8b 3f`
- `5802`: `106b10 <- 42 6b 42`
- `6495`: `136813 <- 67 68 67`
- `6899`: `152a15 <- 1a 2a 1a`

So the upgraded family model is:

- sometimes low-equals-high is encoded symmetrically as `[anchor, mid, anchor]`
- sometimes the surrogate byte is asymmetric or tied to a local anchor family

This is enough to safely reclassify:

- `4289`

as part of the broader low-equals-high surrogate family rather than an unrelated bootstrap.

## New Rule Candidate: `a == high && c == low`

The strongest new reusable rule candidate from this round is:

- the entry is still mismatching under the rolling heuristic
- `a == source_high`
- `c == source_low`
- then the correct lower-16 is:
  - `mid << 8 | low`

This is now backed by real evidence rather than analogy.

In the current scan window:

- it fixes `23` mismatching entries immediately
- it produced **zero** observed false positives on already-correct entries in that same check

Representative fixed examples:

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

The wider scan is also positive:

- applying this rule after the earlier known patches fixes `30` mismatches in the broader scan window
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

This is the clearest current sign that the reverse-engineering work is turning into reusable decoder logic rather than a pure pile of singleton observations.

## New Frontier After Applying The `a == high && c == low` Rule

After:

- the previously known exact singleton / bootstrap fixes through `4689`
- plus the new `a == high && c == low` rule

the next lower-16 exact mismatch frontier jumps to:

- `5002`

The earliest remaining sparse runs become:

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

The first two are representative of the previously observed `c == low` family:

- `5002`: `0d06d0 <- 6e d3 d0`
- `5009`: `0d0db0 <- 6e 6e b0`

The next visible block continues the broadened low-equals-high surrogate family:

- `5296`
- `5346`
- `5575`
- `5802`
- `6495`
- `6899`

So the unresolved late tail is now compressing toward two main buckets:

1. `c == low` rotation-style entries
2. broadened `low == high` surrogate entries

## New Local Rule Candidate: `b == prevC && a != prevA`

The next strong result is a genuinely local rule candidate, based only on the triplet stream.

Candidate rule:

- the entry is still mismatching under the rolling heuristic
- `b == previous_triplet.c`
- `a != previous_triplet.a`
- then decode the lower-16 as:
  - `b << 8 | c`

This is much stronger than a loose heuristic.

In the current scan window:

- it fixes `45` exact remaining mismatches
- observed precision is `100%` in that check
- no false positives were observed for that predicate in the scanned range

Representative fixed examples:

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

Interpretation:

- this is a large “page collision with surrogate anchor switch” family
- even though `b == prevC` would normally imply same-page under the rolling heuristic,
  these entries must instead be reinterpreted as a fresh page change to `b`

## Remaining Same-Anchor `b << 8 | c` Subset

After applying:

- the earlier known exact singleton / bootstrap fixes
- the source-assisted `a == high && c == low` family
- and the new local rule `b == prevC && a != prevA`

the broader `b << 8 | c` family does not disappear completely.

Current counts in the scan window:

- total `b << 8 | c` family size: `66`
- covered by the new local rule: `45`
- unresolved remainder: `21`

Those remaining entries are the same-anchor subset, for example:

- `5849`: `1090a2 <- 76 90 a2`
- `5919`: `10d850 <- a0 d8 50`
- `8583`: `1cb029 <- 29 b0 29`
- `9358`: `20cd65 <- a0 cd 65`
- `9965`: `22b01b <- 85 b0 1b`
- `10026`: `22d433 <- c7 d4 33`
- `16178`: `37ebed <- 5e eb ed`
- `16978`: `3ab2a5 <- 84 b2 a5`

Important negative result:

- a brute-force sweep over simple local predicates using:
  - previous triplet
  - current triplet
  - next triplet
  - and a few `next+1` relations
- did **not** find another comparably clean high-precision rule for this remaining same-anchor subset

Current interpretation:

- the unresolved same-anchor subset likely needs extra decoder state
- it should not be forced into the new `a != prevA` local rule

## Stronger State Signal For The Remaining Same-Anchor Subset

The unresolved remainder of the `b << 8 | c` family is now pinned down to `21` entries in the current scan window:

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

These are all still:

- correctly explained by `b << 8 | c`
- but **not** covered by the new local rule:
  - `b == prevC && a != prevA`

The new stable observation is about decoder state rather than byte permutation.

For this same-anchor remainder:

- the rolling decoder's `page_before` is usually stale by only a very small amount
- most of the cluster has:
  - `delta = b - page_before = 1`
  - or `delta = 2`

Current strongest subclusters:

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
  - plus one more very similar case in the same scan
- `(delta=2, nextB=b+1, same anchor across prev/current/next)`:
  - `8583`
  - `9358`

Interpretation:

- this looks much more like a **page-state lag** problem
- not another missing triplet permutation rule
- the decoder is often carrying `page_before = b-1` (or occasionally `b-2`)
- but the correct interpretation at the current entry must force a new page of `b`

## Negative Result: No Third Clean Rule Yet

Several stateful candidate predicates were tested on that remaining same-anchor subset, combining:

- `b == prevC`
- same anchor across prev/current/next
- `delta = b - page_before in {1,2}`
- `nextB = b+1`

None of those candidates yet reaches the same quality as the first two strong rules.

The best narrower subset currently only reaches about:

- precision `~0.79`
- recall `~0.71`

So the current conclusion is:

- do **not** freeze a third decoder rule yet
- but do treat the remaining blocker as primarily a page-state advancement problem rather than a new broad remapping family

## Stronger Same-Anchor State Fact

The same-anchor remainder now has a stronger invariant than just “page_before is stale by 1 or 2”.

For **all 21** unresolved same-anchor `b << 8 | c` targets, the immediately previous solved lower-16 value is exactly:

- `(b-1)<<8 | b`
- or, in the two rarer exceptions:
- `(b-2)<<8 | b`

So the previous decoded entry already ends with the current entry's intended page byte `b`.

Typical `b-1` ladder examples:

- `5849`: previous solved `8f90`, current wants `90a2`
- `5919`: previous solved `d7d8`, current wants `d850`
- `10026`: previous solved `d3d4`, current wants `d433`
- `11306`: previous solved `f8f9`, current wants `f9a1`
- `12463`: previous solved `8d8e`, current wants `8e6a`
- `16178`: previous solved `eaeb`, current wants `ebed`
- `21233`: previous solved `2324`, current wants `2439`

The two clear `b-2` exceptions remain:

- `8583`: previous solved `aeb0`, current wants `b029`
- `9358`: previous solved `cbcd`, current wants `cd65`

Interpretation:

- this strongly supports the hidden-state model
- some earlier step effectively arms:
  - “the previous low becomes the next page”
- and the current same-anchor collision is where that armed promotion is consumed

Important limitation:

- this is still not a safe implementation rule by itself
- because the broader predicate:
  - previous solved in `{(b-1)<<8|b, (b-2)<<8|b}`
- has:
  - recall `21/21`
  - but precision only about `0.208`

The narrower version:

- previous solved `(b-1)<<8|b`
- same anchor with previous entry
- and `nextB = b+1`

reaches only about:

- precision `~0.765`
- recall `~0.619`

So this should currently be treated as:

- a strong explanatory state invariant
- not yet a frozen decoder rule

## Independent 5.4 Review: Same-Anchor Tail Agrees

An independent `gpt-5.4` explorer re-ran the same-anchor tail analysis directly against:

- `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Bold.otf`
- `/tmp/presentation1-font2-decoded.otf`

Its conclusion matches the main-thread work:

- no clean third local rule emerged
- the strongest signal is still page-state lag
- the missing variable behaves like a hidden per-anchor pending-page / deferred-promote state

The most useful predictor candidates from that independent pass were:

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

So the independent check strengthens the current interpretation:

- the unresolved same-anchor remainder is a state-lag problem
- not another byte-permutation problem

## Independent 5.4 Review: Low-Equals-High Tail Also Wants State

An independent `gpt-5.4` explorer also re-ran the remaining `low == high` tail after the known baseline fixes.

It reached the same broad conclusion:

- no clean local third rule like `b<<8|a` emerged
- the tail still needs a stronger high-byte / state model
- the remaining family is structurally split, not uniform

Useful structural split from that pass:

- symmetric “mid already correct, low omitted” cases
- same-anchor carry cases with `b == prevC`
- rare asymmetric gap singletons, including:
  - `7034`
  - `18807`

Additional useful counts from that broader reproduction:

- `150/215` had `source_mid == current b`
- `119/215` had symmetric `a == c`
- `62/215` had `b == prevC`
- `107/215` had `source_mid == prevB + 1`
- `166/215` had `source_mid == nextB - 1`

Interpretation:

- this independently reinforces that the unsolved `low==high` tail is not waiting on one more tiny local remap
- it also points back to a stronger state model as the likely missing piece

## Same-Anchor 21: New Macro Split

A fresh local replay against the full `font2` `CharStrings INDEX` now gives a stable `same-anchor + b << 8 | c` candidate pool of:

- `77`

Within that pool, the known unresolved `21` same-anchor cases split cleanly by immediate neighborhood shape into five macro families:

- `prev_raw_samepage + same_anchor_next + next_raw_pagechange`
  - `5919`
  - `11306`
  - `12463`
  - `16198`
- `prev_raw_samepage + same_anchor_next + next_raw_samepage`
  - `9965`
  - `11707`
  - `13269`
  - `23163`
- `prev_raw_pagechange + same_anchor_next + nextB = b + 1`
  - `5849`
  - `8583`
  - `9358`
  - `10026`
  - `13511`
  - `16178`
  - `16978`
  - `21233`
- `prev_raw_pagechange + same_anchor_next + next_raw_samepage`
  - `16580`
- `next anchor switches at the end of the current anchor run`
  - `12794`
  - `14376`
  - `23118`
  - `26075`

This is important because the `21` are no longer one opaque remainder. They can now be treated as a handful of repeatable local transition shapes.

## Same-Anchor 21: Exact Local Subfamilies

Several smaller zero-false-positive subsets now exist inside that `21`-entry remainder.

### Exact singleton

- `8583`
  - predicate:
    - `c == a`

### Exact triple

- `9358`
- `16178`
- `16978`
  - predicate:
    - `prev_raw_pagechange`
    - `nextB = b + 1`
    - current entry is within the first `3` positions of its anchor run

### Exact triple

- `16178`
- `16978`
- `26075`
  - predicate:
    - `prev_raw_pagechange`
    - `nextB = b + 1`
    - anchor run length `<= 5`

### Exact pair

- `12463`
- `16198`
  - predicate:
    - `prev_raw_samepage`
    - `same_anchor_next`
    - `next_raw_pagechange`
    - `nextB = b + 1`
    - anchor run length `<= 10`
    - current entry is within `3` steps of run end

### Exact pair

- `14376`
- `26075`
  - predicate:
    - `next anchor switches`
    - `prev_raw_pagechange`
    - `nextB = b + 1`
    - `nextA == c`
    - `nextA > a`

### Exact singleton

- `12794`
  - predicate:
    - `next anchor switches`
    - `nextB = b + 2`
    - `nextA < a`

### Exact singleton

- `23118`
  - predicate:
    - `next anchor switches`
    - `nextA == c`
    - anchor run length `== 3`

### Exact singleton

- `16580`
  - predicate:
    - `prev_raw_pagechange`
    - `next_raw_samepage`
    - current entry is at run end or one step before run end

## Same-Anchor 21: Remaining Core After New Split

After factoring out the exact local subfamilies above, the still-hard core is now:

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

These `10` look like the real “prototype” deferred-promote cases:

- same anchor on previous, current, and next entry
- previous solved lower-16 already ends in current `b`
- current still wants `b << 8 | c`
- no extra singleton marker like `c == a`
- no terminal anchor switch

So the reverse-engineering target is now narrower:

- less about hunting more tiny exceptional markers
- more about explaining this `10`-entry core pending-page state directly

## Font1 Cross-Check: Same Mechanism Family

A new control pass against `font1` (`SourceHanSansSC-Regular`) shows the same style of ladder-state candidates also exists there.

In the first `20000` entries, `font1` produces at least:

- `29` ladder-state candidates under the same broad invariant
  - previous solved lower-16 in `{(b-1)<<8|b, (b-2)<<8|b}`
  - current source lower-16 equals `b << 8 | c`
  - same anchor with previous entry

More importantly, `font1` candidates also fall into the same macro neighborhood families used for `font2`:

- `A = prev_raw_samepage + same_anchor_next + next_raw_pagechange`
  - `8` examples
- `B = prev_raw_samepage + same_anchor_next + next_raw_samepage`
  - `6` examples
- `C = prev_raw_pagechange + same_anchor_next + nextB = b + 1`
  - `13` examples
- `D = prev_raw_pagechange + same_anchor_next + next_raw_samepage`
  - `7` examples
- `E = next anchor switches at run end`
  - `5` examples

This is strong support that the current findings are not just a `font2` / Bold quirk.

Practical interpretation:

- the unresolved state behavior looks like an `Office static CFF` family mechanism
- not a one-off transform tied only to this single Bold file

Important nuance:

- this still does **not** prove “all arbitrary CFF fonts” are solved
- it is evidence that this Office static-CFF subfamily is internally consistent across at least:
  - `font1` Regular
  - `font2` Bold

## Remaining 10 Core: Further Reduction To 2 Prototypes

The previous hard core of `10` same-anchor cases:

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

is now reduced much further using exact local rules.

### Exact class-A pair

- `5919`
- `11306`
  - exact predicate:
    - `prev_raw_samepage`
    - `same_anchor_next`
    - `next_raw_pagechange`
    - `nextB = b + 1`
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
    - `nextB = b + 1`
    - `run_pos >= 6`
    - `to_end in {4, 5}`
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

After the new exact splits above, the same-anchor hard core is now only:

- `13269`
- `23163`

Shared shape of this remaining prototype pair:

- `prev_raw_samepage`
- `same_anchor_next`
- `next_raw_samepage`
- very short anchor runs:
  - `run_len` in `{4, 5}`
  - `run_pos = 2`
  - `to_end in {1, 2}`
- moderate local deltas:
  - `d0` in `133..159`
  - `d1` in `73..143`
  - `d2` in `118..191`

Important current negative result:

- several near-miss false positives still share almost the same short-run profile, including:
  - `32886`
  - `48472`
  - `51354`
  - `63388`
- so there is still no safe purely local threshold rule for this final pair

Practical interpretation:

- the deferred-promote family is now almost fully decomposed
- the remaining unknown state seems to collapse to one tiny short-run prototype
- this is the best current candidate for the true irreducible hidden decoder state

## Final Same-Anchor Closure

The last two short-run prototypes are now also explained by exact local predicates.

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

- The `21`-entry unresolved same-anchor `b << 8 | c` family is now fully covered by exact local predicates.
- Current verification status:
  - all `21/21` positives are covered
  - no extra positives were covered in the current ambiguous pool
- Practical interpretation:
  - the deferred-promote same-anchor family is now no longer the blocker
  - future decode work should shift back to the remaining unsolved families outside this specific `21`-entry tail

## Low-Equals-High Tail: Stable 9-Bucket Taxonomy

A new direct replay against `font2` shows the remaining non-explicit `source_high == source_low` mismatches are now best treated as a structural taxonomy rather than one undifferentiated tail.

Counting entries where:

- source `high == low`
- and Office did **not** simply emit `c == high`

yields:

- `240` entries in the current `font2` table

Those `240` entries split into the following stable buckets:

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

Practical interpretation:

- the low-equals-high tail is now no longer “one family that still needs a rule”
- it is at least:
  - two large `b == source_mid` families
  - two large `b == prevC` carry families
  - several small residual singleton / micro-families

## Low-Equals-High Tail: Reproduction Alignment

To reproduce the documented `240`-entry taxonomy correctly, the `CharStrings INDEX` replay must use the shifted Office alignment.

- source standard offset array:
  - `source_rel + 3 + i*3`
- Office triplet stream:
  - `source_rel + 1 + i*3`

So the Office static-CFF `CharStrings` triplets are effectively shifted by `-2` bytes relative to the standard `ffff 03` INDEX framing.

Important warning:

- if the two streams are compared from the same absolute relative offset
- the replay produces a bogus larger population and misleading bucket counts
- the corrected `-2` alignment is required to recover the stable `240`-entry split above

## Low-Equals-High Tail: New Local Macro Split

Inside the already-isolated `240`-entry `low==high` pool, the earlier `9` source-derived buckets now compress into a much simpler purely local triplet-shape split.

New exact local results on `font2`:

- `a == c` and `nextB == b+1`
  - selects exactly the `53`-entry bucket
  - no contamination from the other `8` buckets inside the `240`-entry pool
- `a != c` and `nextB == b+1`
  - selects exactly the `59`-entry bucket
  - no contamination from the other `8` buckets inside the `240`-entry pool
- `a != c` and `prevC == b` and `nextB != b+1`
  - selects exactly the `25`-entry bucket

The remaining local macro pools are then:

- `a == c` and `prevC == b` and `nextB != b+1`
  - `37`
  - equals:
    - `29 + 8`
- `a == c` and `prevC != b` and `nextB != b+1`
  - `49`
  - equals:
    - `48 + 1`
- `a != c` and `prevC != b` and `nextB != b+1`
  - `17`
  - equals:
    - `11 + 4 + 2`

Interpretation:

- the two biggest `b == source_mid` families are now locally recognizable without directly consulting `source_mid`
- one of the asymmetric carry families is also now locally exact
- the active blocker is no longer the full `240` tail
- it is the three residual macro pools above

## Low-Equals-High Tail: Cross-Font Confirmation

The same local macro split was checked against `font1` (`SourceHanSansSC-Regular`) using:

- `/Users/ekxs/Downloads/09_SourceHanSansSC/OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`
- `/tmp/otto-cff-office-decoded.otf`

`font1` reproduces the same strong local families in the first `65535` offsets:

- `a == c` and `nextB == b+1`
  - exact `53`-entry family
- `a != c` and `nextB == b+1`
  - exact `50`-entry family
  - the `font1` analogue of the `font2` `59` bucket
- `a != c` and `prevC == b` and `nextB != b+1`
  - exact `27`-entry family
  - the `font1` analogue of the `font2` `25` bucket

`font1` residual macro counts in the same taxonomy space are:

- `a == c` and `prevC == b` and `nextB != b+1`
  - `37`
  - equals:
    - `26 + 11`
- `a == c` and `prevC != b` and `nextB != b+1`
  - `49`
- `a != c` and `prevC != b` and `nextB != b+1`
  - `12`

Interpretation:

- this new macro split is not just a Bold-only quirk
- it is another stable Office static-CFF family mechanism shared by at least:
  - `font1` Regular
  - `font2` Bold

## Low-Equals-High Tail: Residual Pool Follow-Up

New stable follow-up facts inside the three residual macro pools:

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

Practical interpretation:

- these are not yet full decoder rules for the remaining residual pools
- but they are stable subfamily cuts that materially reduce the search surface
- the next highest-leverage targets are now:
  - the missing `1/11` singleton in the `11` pool (`26455`)
  - the remaining `2/48` exceptions in the `48` pool
  - the unsplit remainder of the `29 + 8` carry pool

## Low-Equals-High Tail: Full Exact Stepwise Decomposition

A new local stepwise classifier now covers the entire documented `font2` `low==high` tail exactly.

Verification result on the `240`-entry `font2` pool:

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
- total disagreements between the stepwise classifier and the documented taxonomy:
  - `0`

This is the first full end-to-end local decomposition of the `240` entries.

### Stage 1: Exact macro families

The top-level exact local predicates are:

- `a != c` and `nextB == b+1`
  - exact `59`
- `a == c` and `nextB == b+1`
  - exact `53`
- `a != c` and `prevC == b` and `nextB != b+1`
  - exact `25`

### Stage 2: Asymmetric residual `11 + 4 + 2`

Remaining local macro:

- `a != c`
- `prevC != b`
- `nextB != b+1`
- size:
  - `17`

Exact stepwise split inside that residual:

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
    - `n2A == b`

The `4` family is now also fully decomposed:

- `319`
  - exact by:
    - `a == b`
- `334`
  - exact by either:
    - `c == b`
    - `nextB == b`
- `1472`
  - exact by:
    - `a > b`
- `33`
  - exact inside the `4` family by either:
    - `run_len >= 16` and `not(c == b)`
    - `a < b` and `not(nextB == b)`

### Stage 3: Symmetric carry residual `29 + 8`

Remaining local macro:

- `a == c`
- `prevC == b`
- `nextB != b+1`
- size:
  - `37`

Exact stepwise split inside that residual:

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
      - `b < 128`
  - `62644`
    - exact inside the residual trio by:
      - `a == n3A`

### Stage 4: Symmetric residual `48 + 1`

Remaining local macro:

- `a == c`
- `prevC != b`
- `nextB != b+1`
- size:
  - `49`

Exact stepwise split inside that residual:

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
      - `not(prevB + 1 == b)`

## Low-Equals-High Tail: Status Shift

Practical implication:

- the `low==high` tail is no longer the main reverse-engineering blocker at the analysis level
- at least for `font2`, it is now captured by an exact local decision tree

Important nuance:

- the coarse macro split remains cross-font stable between `font1` and `font2`
- the finer singleton and residual-trio cuts above are currently verified only on `font2`
- they should not yet be generalized to arbitrary Office static-CFF fonts without implementation-time validation

## Low-Equals-High Tail: Tiny Asymmetric Gap Family

The rare asymmetric gap family previously suspected from:

- `7034`
- `18807`

is now confirmed as an exact `2`-entry structural bucket inside the broader `source_high == source_low` set.

Shared source-derived signature:

- asymmetric (`a != c`)
- `b != source_mid`
- `b != prevC`
- `source_mid == nextB - 1`

This confirms those two are a real micro-family, not unrelated noise.

Important limitation:

- a safe purely local triplet-only rule for that `2`-entry family is **not** established yet
- but they are now isolated cleanly for targeted follow-up

In other words:

- the next decode/rebuild attempt should not be "patch a few early bytes"
- it should be "reconstruct standard CFF framing through the end of the Global Subr offset array"

After that boundary is rebuilt, the most promising follow-up is:

- map the early Global Subr data region separately
- test whether the later `-2`-aligned tail can be reused more directly
