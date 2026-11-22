# Microtuning — verified facts for round 42 (2026-11-22)

Companion to `research/instruments/microtuning.md` (the design note; not repeated here). Every
claim is marked **[V 2026-11-22]** (verified today from the primary source named beside it) or
**[NOT VERIFIED]** (with the reason). No cargo, no build; web and file reading only. Downloads
are data only, under `F:\projects\daw-r42\research\` (nothing in them was executed).

Local copies used for this note (untracked, outside the repository):

- `F:\projects\daw-r42\research\pages\` — the Scala pages (`scl_format.html`, `help.htm`,
  `downloads.html`, `scala_index.html`) as fetched today, plus the other pages cited below.
- `F:\projects\daw-r42\research\scala-download\scales.zip` — the archive (version 94);
  `extracted\scl\` its 5 401 files; `wayback\scales.zip` the same bytes from the Wayback Machine.
- `F:\projects\daw-r42\research\tuning-library-tests\` — test files from Surge's MIT
  `tuning-library` (named in §1.4).
- `F:\projects\daw-r42\research\mts-esp\` (ODDSound wrappers, README, LICENSE — no binaries
  downloaded), `clap\` (`tuning.h`, `events.h`, `version.h`), `scale-workshop\` (its MTS and
  Image-Line exporters), `pages\` also holds `fluid_synth.c`, `ivstnoteexpression.h`, the
  archived MMA tuning page, the Logic, Pianoteq, Bitwig and Live pages.

---

## 1. Scala `.scl` and `.kbm` — the grammar

### 1.1 `.scl` — the rules, quoted from the spec

Source: <https://www.huygens-fokker.org/scala/scl_format.html> (© Manuel Op de Coul 2001–2026),
fetched 2026-11-22. **[V 2026-11-22]** for every point in this list — quoted or near-quoted:

1. "The files are human readable ASCII or 8-bit character text-files." Footnote: a file meant
   for the archive "should be in ISO 8859-1 'Latin-1' or the ASCII subset". → **decode as
   Latin-1, not UTF-8** (125 archive files hold bytes ≥ 0x80; see 1.3).
2. "The file type is .scl. There is one scale per file."
3. "Lines beginning with an exclamation mark are regarded as comments and are to be ignored."
   (The `!` must be the first character of the line; `!` later in a line is not a comment by
   this rule — on a pitch line it is simply "anything after a valid pitch value", ignored.)
4. "The first (non comment) line contains a short description of the scale, but long lines are
   possible and should not give a read error. The description is only one line. If there is no
   description, there should be an empty line."
5. "The second line contains the number of notes. This number indicates the number of lines
   with pitch values that follow. In principle there is no upper limit to this, but it is
   allowed to reject files exceeding a certain size. The lower limit is 0, which is possible
   since degree 0 of 1/1 is implicit. Spaces before or after the number are allowed."
6. "After that come the pitch values, each on a separate line, either as a ratio or as a value
   in cents. If the value contains a period, it is a cents value, otherwise a ratio. Ratios are
   written with a slash, and only one. Integer values with no period or slash should be
   regarded as such, for example "2" should be taken as "2/1"."
7. "Numerators and denominators should be supported to at least 2^31-1 = 2147483647."
   (The archive goes far beyond: see 1.3 — 25-digit integers.)
8. "Anything after a valid pitch value should be ignored. Space or horizontal tab characters
   are allowed and should be ignored."
9. "Negative ratios are meaningless and should give a read error." (Negative **cents** are
   valid: the spec's own list of valid lines includes `-5.0`.)
10. "The first note of 1/1 or 0.0 cents is implicit and not in the files." The **last** pitch
    line is the period (the "formal octave"); it need not be 2/1.
11. "Files for which Scala gives Error in file format are incorrectly formatted. They should
    give a read error and be rejected."

The spec's list of valid pitch lines (byte-exact test vectors; leading spaces are part of the
lines): `81/64`, `408.0`, `408.`, `5`, `-5.0`, `10/20`, `100.0 cents`, ` 100.0 C#`,
` 5/4   E\`. Note `10/20` (a ratio below 1, i.e. −1200 c, is valid) and `408.` (trailing
period, no fraction digits — still cents).

The spec's example file (verbatim, leading spaces as shown on the page):

```
! meanquar.scl
!
1/4-comma meantone scale. Pietro Aaron's temperament (1523)
 12
!
 76.04900
 193.15686
 310.26471
 5/4
 503.42157
 579.47057
 696.57843
 25/16
 889.73529
 1006.84314
 1082.89214
 2/1
```

(The archive's own `meanquar.scl` differs in its description line — see 2.3; the spec page's
copy is an older wording. Both are valid.)

Derived grammar (our reading; every production traces to a rule above):

```
file        := { comment } description-line { comment } count-line { comment | pitch-line }*
comment     := "!" <any bytes to end of line>
description := <any bytes to end of line, possibly empty>         ; NOT trimmed of meaning
count-line  := ws* digits ws* [<anything>]                         ; see note (a)
pitch-line  := ws* value <anything>                                ; rule 8
value       := cents | ratio | integer
cents       := ["-"] digits "." [digits] | ["-"] "." digits        ; contains a period
ratio       := digits "/" digits                                    ; one slash; num, den > 0
integer     := digits                                               ; = integer/1
ws          := " " | "\t"
line ends   := CRLF | LF (also tolerate lone CR)
```

Notes on what the spec leaves open (decisions for the parser, flagged as ours):

- (a) The spec allows spaces around the count but says nothing of trailing text on that line;
  no archive file has any (scan 1.3). Accept trailing text after the digits, as Scala does on
  pitch lines? **Our call**: accept `digits` then whitespace then anything — lenient on read.
- (b) Whether a comment may sit between the description and the count: the spec says "first
  (non comment) line" and "second line" — the archive does it constantly (`dekany_agni.scl`
  has a comment between them; the template has `!` before the first pitch). Comments are
  allowed anywhere. **[V 2026-11-22]** by the archive's files.
- (c) Lines after the last pitch: the archive has comment lines after the pitches
  (`blackwoo.scl`, 2.3) and never non-comment lines; ignore everything after `count` values.
- (d) Blank lines among the pitches: none in the archive. Treat as an error (Surge's
  `tuning-library` has `bad/blanknote.scl` as a must-fail case — §1.4).
- (e) A sign on a ratio (`-5/4`): rule 9, read error. A zero numerator or denominator: error.
- (f) Ratios must be parsed exactly (`u128` holds every archive ratio: the longest integer is
  25 digits, `1709671705179880612640625/1208925819614629174706176` in `atomschis.scl`); cents
  then computed as `1200·log2(n/d)` via `cantus_dsp::math` — or keep the ratio exact for
  display and JI arithmetic.
- (g) Cents with many digits: up to 18 fraction digits in the archive
  (`70.247930173690388400`); parse as f64.
- (h) Degrees are **not** monotonic in 115 archive files and can be negative or below 1/1
  (`mavila12.scl` starts at −30.99719 c; `dekany_agni.scl` holds 4/375). Never sort, never
  reject; the scale's order is the file's order.

### 1.2 `.kbm` — the keyboard mapping

Source: the Scala help, section "Mappings",
<https://www.huygens-fokker.org/scala/help.htm#mappings> (local copy lines 9424–9497),
fetched 2026-11-22. **[V 2026-11-22]**. The fields are positional, one per non-comment line,
in this order (comments are the same `!` lines as in `.scl`). The template, verbatim:

```
! Template for a keyboard mapping
!
! Size of map. The pattern repeats every so many keys:
12
! First MIDI note number to retune:
0
! Last MIDI note number to retune:
127
! Middle note where the first entry of the mapping is mapped to:
60
! Reference note for which frequency is given:
69
! Frequency to tune the above note to (floating point e.g. 440.0):
440.0
! Scale degree to consider as formal octave (determines difference in pitch
! between adjacent mapping patterns):
12
! Mapping.
! The numbers represent scale degrees mapped to keys. The first entry is for
! the given middle note, the next for subsequent higher keys.
! For an unmapped key, put in an "x". At the end, unmapped keys may be left out.
0
1
2
3
4
5
6
7
8
9
10
11
```

| # | Field | Type | Rules from the help text |
|---|---|---|---|
| 1 | map size | integer ≥ 0 | "The pattern repeats every so many keys." **0 = linear mapping** (key n − middle = degree; "or zero for a linear mapping"). May be smaller than the scale size when not every degree is mapped. |
| 2 | first MIDI note to retune | 0–127 | keys outside first..last are not retuned (what "not retuned" means is the instrument's choice — the help says behaviour of unmapped keys "is not defined"). |
| 3 | last MIDI note to retune | 0–127 | |
| 4 | middle note | 0–127 | the key where mapping entry 0 sits (and where degree 0 is, if entry 0 is 0). |
| 5 | reference note | 0–127 | the key whose frequency is given. "If [an x] is done with the frequency reference note it will be considered an error." |
| 6 | reference frequency | float (Hz) | "floating point e.g. 440.0" — but real files write integers (`452` in a Surge test file, 1.4). |
| 7 | formal octave degree | integer | the scale degree that is one pattern repeat: pitch of key `middle + k·size + i` = pitch(entry i) + k·pitch(degree *octave*). "There is no restriction to the degree numbers in the mapping or to the scale degree to consider as formal octave, they can be any number, also negative, also lie outside the scale range. It means pitches are always calculated based on octave extension." Two-octave patterns: set it to twice the scale size. |
| 8.. | mapping entries | integer or `x` | `size` entries; entry *i* is the degree for key `middle + i` (and every `+k·size`). `x` = unmapped key. "At the end, unmapped keys may be left out" → fewer than `size` entries is valid, the missing tail is `x`. A degree may appear on several keys or none. |

Multichannel mappings (Scala relay; Pianoteq reads them too): `name_1.kbm` … `name_16.kbm`
in one folder act per MIDI input channel 1–16. **[V 2026-11-22]** (same help section).

Frequency of a key (the help's "octave extension", our formula, to be test-vectored with
Surge's `tuning-library` files of 1.4):

```
offset  = key − middle
k, i    = floor_div(offset, size), offset mod size          (size > 0)
degree  = map[i] + k · octave_degree                       (map[i] ≠ x)
cents(d)= with N = scale.len(): q, r = floor_div(d, N), d mod N;
          q · period_cents + (r == 0 ? 0 : scale[r−1])
f(key)  = ref_freq · 2^((cents(degree(key)) − cents(degree(ref_note))) / 1200)
size = 0: degree = offset (linear), octave_degree ignored (often written 0).
```

### 1.3 What the real archive contains — scan of all 5 401 files

Scan script (data only, `python -I`, in the session scratchpad) over
`extracted\scl\*.scl`, version 94. **[V 2026-11-22]** (counts are exact for that version):

| Feature | Files | Example |
|---|---|---|
| CRLF line ends (Scala writes them; the download page warns Mac/Linux users to `unzip -aa`) | 5 330 | almost all |
| LF only | 71 | `chrys_diat-1st-ji.scl` |
| No newline after the last line | 33 | `blackwoo.scl`, `amogussussybaka.scl` |
| Bytes ≥ 0x80 (Latin-1: `ß` = 0xDF etc.) | 125 | `neidhardt3.scl` (description "'groß Stadt'") |
| Tab characters | 1 | `fj-31tet.scl` (tabs after values) |
| Text after the value (`cents`, note names, spaces) | 1 738 lines | `arist_chrominv.scl` (` 300.000 cents`) |
| Bare integers as ratios (`5`, `15 `) | 32 lines | `dekany_agni.scl`, `ariel1.scl` (` 2`) |
| Cents with a trailing period (`600.`) | 15 lines | `canou1d.scl`, `pepper_meantone-killer.scl` |
| Negative cents | 16 lines | `mavila12.scl`, `deanm_pi.scl` (−2320.15965) |
| Degrees not ascending | 115 | `mavila12.scl`, `arch_dor.scl` |
| Period ≠ 2/1 (non-octave or stretched) | 823 | `bohlen-p.scl` (3/1), `17wtn.scl` (9/4), `mavila12.scl` (1206.54826) |
| Ratio terms > 2^31−1 | 57 lines | `atomschis.scl` |
| Ratio terms > 2^53 (not exact in f64) | 8 lines | `atomschis.scl`, `chan34.scl` |
| Most degrees | 579 | `gann_wolfe.scl` |
| Spaces in the file name | 54 | `buselik tetrachord 13-limit.scl` |
| Blank description line | 0 | (Surge's `12-intune-nodesc.scl` covers it, 1.4) |
| BOM, zero-note scale, blank line inside pitches | 0 | — |

Not in the archive: **plain equal temperaments** (no 12-EDO, no 31-EDO file — Scala generates
them with its EQUAL command; the archive holds ET *approximations* such as `fj-12tet.scl` and
the 13-ED3 `bohlen-p_et.scl`). A 12-EDO and a 31-EDO fixture must come from elsewhere (1.4, 2.4).

### 1.4 Surge's `tuning-library` — a second, independent reader and its test vectors

Repository <https://github.com/surge-synthesizer/tuning-library> (head `d6fcc08f`,
2026-10-19), licence **MIT** ("Copyright 2019-2020, Paul Walker", `LICENSE.md`).
**[V 2026-11-22]**. It is the reader Surge XT and others use (the Scala page credits "The
Surge team created a free library with C++ source code to parse .scl and .kbm files"). It
reads `.ascl` too. Its `tests/data` (MIT, ~45 files) gives what the archive lacks: a 12-EDO
with a **blank description** (`12-intune-nodesc.scl`), a **31-EDO** in cents (`31edo.scl`,
and `31edo_dos_lineends.scl` with CRLF), a shuffled 12, 13 `.kbm` mappings (white keys at
C = 261.625565280 Hz, a 7-note scale on 12 keys at A442 with formal octave 7, `128.kbm`
key-for-key, `piano.kbm` 21–108, size-0 mappings with an integer frequency `452`), Ableton
`.ascl` files (`31-edo.ascl`, `rast.ascl`, `maqamat.ascl`), and a `bad/` folder whose
expected outcomes are in `tests/alltests.cpp` lines 639–662:

| File | Defect | `tuning-library` | Scala spec says |
|---|---|---|---|
| `bad/badnote.scl` | a pitch line `What is this` | error | error (rule 11) |
| `bad/blanknote.scl` | a blank line among 17 pitches (→ 16) | error ("fewer notes than count") | not addressed; error is right |
| `bad/missingnote.scl` | 16 pitches for count 17 | error | error |
| `bad/extraline.scl` | non-comment garbage after the 17 pitches | **accepted** ("Trailing data is OK") | not addressed |
| `bad/blank-line.kbm` | a blank line inside the mapping | error | not addressed |
| `bad/garbage-key.kbm` | `garbage` as a mapping entry | error | error (only integers or `x`) |
| `bad/missing-note.kbm` | 11 entries for size 12 (degree 7 dropped mid-list) | error | the spec allows only a short *tail* of omitted `x` — accepting it would shift keys silently; error is right here |
| `bad/empty-bad.kbm` | formal-octave line missing | error | error |
| `bad/empty-extra.kbm` | blank lines after `! Mapping.` | accepted | — |
| `bad/extraline-long.kbm` | trailing blank lines | accepted | — |

Where `tuning-library` is **stricter than the Scala spec** (do not copy): it rejects a count of
0 (`res.count < 1` → "Invalid SCL note count"; the spec allows 0), and it requires exactly
`size` mapping entries (the spec: "At the end, unmapped keys may be left out").
**[V 2026-11-22]** (`include/TuningsImpl.h`, lines 186–231 and 513).

SHA-256 of the files fetched today (raw.githubusercontent.com, branch `main`):

```
57df6703f8e7ac890f81952c25605a51f953ced955a6d3b839554d61b39f1c69  12-intune-nodesc.scl
70fbcf5ea1cb73ff3b76bbbc1164c51048159ac77d5c7f62f2769aae65164855  31edo.scl
4d89377e75bf7db9be8967204cd8beccb4e7d7898550f5eb3502f60780e06ad1  31edo_dos_lineends.scl
a2bb810e71ff468c13ddbc5003a29175dea2b770121ae46e2a0c258f7521fef2  ED3-17.scl
ca21c9d009ba741643e1f99a0ac7aa5f222d7c21a59d5d0bfafc10a94655a16e  6-exact.scl
e65a093d7f4fa5a8aea65434ae481426ffcd7edce0b64b4bb4b1d041e7e12dfb  128.kbm
5d64cf7d537b1e2cf34e8e0a0e18698fdd3b38bf4bdb7513b245a47fe528e31a  31-edo.kbm
de181813f063eeb62d58c6f063cd2c9338e7f5eb9b7f8ff473bbd1fedaf81967  empty-note69.kbm
78e13a070d2352696aa033e2032f64fb405dc4ed20a3be4a4efe35c8d9c769b1  mapping-a442-7-to-12.kbm
414e30fb7ce25bf415353641dc558b8529dbe71bd893be1c2e4f9d9261b6be3d  mapping-whitekeys-c261.kbm
e679156ddf423502f59dbecefee90199dd6c74cfa2b389da4175230d7423d235  piano.kbm
68e18a2a739ed8eb1832b8bcd691e17d707ea7cf98e5fe27fd342108c68df106  31-edo.ascl
fd85da7946841ec85fd3506caa2122d9026a1d9fbd417e9845349e84f35fcca6  rast.ascl
0691105fed29c36c6c3d37b537e461cedc5193f005d9bf4f1ede2cb0f83fed53  bad/badnote.scl
8575779d71c66e21467910220d85ec4a137a41d76dea26d4a4554aed46122cb2  bad/blanknote.scl
dcaf40b16816d52dd73f537adb50d3d5b9958da596ef8e8b30dd46ad44a20d5c  bad/missingnote.scl
99e00f325164ee4b9127700301c2b4d8f78fe394adfded9a9991ff673f00407a  bad/extraline.scl
2529029c334bb71402c24de17bc255c9f5977035c3e66daa6fc2bded17e27afd  bad/blank-line.kbm
8e1c427815adfb2466103ec281d69533b155f89dd9938613810250a1be667ad1  bad/garbage-key.kbm
c8a3e20396d5a20ecd90bdf51de7dd83488eb659aa644d80fabc24a45e6890d2  bad/missing-note.kbm
fea35f5a3ffce293633b6b91f51f9c32f52b2da6b98ab77421b1aac853b9ff96  bad/empty-bad.kbm
```

(`marvel12.scl` and `zeus22.scl` in that folder are identical, ignoring CR, to the Scala
archive's files of the same name — the MIT repository redistributes archive files; that is
its call, not a licence grant from the archive's author. Do not rely on it, §2.2.)

### 1.5 Ableton `.ascl` — the extension Live 12 reads (worth reading too)

Source: Ableton's "ASCL Specification", Help Center article 10998372840220, updated
2026-05-25, read through the Zendesk API
(`https://help.ableton.com/api/v2/help_center/en-us/articles/10998372840220.json`; the HTML
page sits behind a Cloudflare check). **[V 2026-11-22]**. The spec is itself an `.ascl` file.

- A valid `.scl` plus comment lines `! @ABL <DIRECTIVE> <args…>` **after the pitch list**;
  arguments space-separated, double quotes around an argument containing spaces; an extra
  `!` disables a directive. Any `.ascl` is readable as `.scl`.
- "All Scala files that implement Ableton extensions must be in UTF-8 format. A compliant
  parser for ABL extensions must reject any file that contains Ableton extensions but is not
  in UTF-8 format." (Plain `.scl` is Latin-1. Rule for us: if any `@ABL` line is present,
  decode as strict UTF-8; otherwise Latin-1.)
- `REFERENCE_PITCH <octave −2..8> <index 0..n−1> <Hz>` — e.g. `@ABL REFERENCE_PITCH 4 9 440.0`
  (index 0 = the implied 1/1).
- `NOTE_NAMES <name0> <name1> …` — one per pitch class; the first is the implied 1/1.
- `NOTE_RANGE_BY_FREQUENCY <min Hz 4..21000> [<max Hz>]` or
  `NOTE_RANGE_BY_INDEX <min oct −99..99> <min idx> [<max oct> <max idx>]` (the latter needs a
  REFERENCE_PITCH); more than one NOTE_RANGE directive is an error.
- Without a range: the notes are centred on A 440 Hz at MIDI note 69 (the spec calls it "A3",
  Live's octave naming) — for a 12-note period the note nearest 440 Hz becomes index 69;
  otherwise 64 notes below the centre and 63 above.
- `SOURCE <text>`, `LINK <url>` — documentation only.
- Live's defaults without REFERENCE_PITCH (from the tuning guide): 12 notes → `3 9 440.0`,
  other sizes → `3 0 440.0`. **[NOT VERIFIED]** verbatim — came through a search summary of
  tuning.ableton.com, not read on the page itself.

---

## 2. The Scala scale archive — download, licence, fixtures

### 2.1 The download **[V 2026-11-22]**

- Page: <https://www.huygens-fokker.org/scala/downloads.html#scales> — "Scale archive,
  version 94, Mar 2026: https://www.huygens-fokker.org/docs/scales.zip"; "over 5350 scale
  files"; "stored in Windows text format (CR-LF)"; Mac/Linux users told to `unzip -aa`.
- Fetched today: HTTP 200, `Content-Type: application/zip`, `Content-Length: 1688704`,
  `Last-Modified: Thu, 26 Mar 2026 16:06:33 GMT`; starts with `PK`; zip comment "Archive of
  scale files for Scala"; **5 401 entries, all `scl/*.scl`, 1 430 459 bytes uncompressed**;
  no README or licence inside.
- **SHA-256 `d2218f2fa6acd3b11c116e0626bd178af927636e2ed26d0ab7a4c27741b97b9a`**
  (SHA-1 `52e28206460962562a3fd0a61584f62e9fde2a5a`).
- **The URL is not versioned**: version 95 will replace the bytes at the same address and break
  a pinned hash. The Wayback Machine holds this exact version (CDX digest
  `KLRIEBSGBFRFMKR72CTBLBHWF2P54KS2` = base32 of the SHA-1 above; first captured 2026-04-04):
  **`https://web.archive.org/web/20260404005834id_/https://huygens-fokker.org/docs/scales.zip`**
  (the `id_` form returns the raw bytes) — downloaded today into `scala-download\wayback\`:
  1 688 704 bytes, the same SHA-256. Recommendation: tests fetch the Wayback URL
  (immutable), fall back to the official URL only if its hash matches, cache the zip, and skip
  with a clear message offline (as sample packs do, ADR 0030). The 2025 version (digest
  `IOFXQGVG…`) is on the Wayback too.

### 2.2 The licence — there is none **[V 2026-11-22]**

- The archive carries no licence file and the pages state no licence; they say only "The scale
  archive can be freely downloaded" (scl_format page) and "Download these for free from this
  website" (Scala home page).
- The only terms on the site are for the **program**: "Scala is freeware without warranty and
  may not be sold, modified, or distributed for sale in combination with commercial products.
  It may only be distributed as one package containing all the files mentioned here and for
  free." (<https://www.huygens-fokker.org/scala/>, "Distribution"). Whether it covers the
  separate archive is not said.
- The numbers of a historical temperament are facts, but the description lines and the
  collection are Manuel Op de Coul's work, and some files are recent work by named living
  authors (`deanm_pi.scl` 2025, `lehman1.scl` 2005).
- **Decision this enables:** do **not** commit archive files to the public GPL/MPL repository.
  Instead:
  1. **Our own fixtures**, written from the facts (the tables of §5, our own descriptions and
     names) — MPL-2.0 with the code. They can reproduce every *format* quirk of the archive
     (CRLF and LF, a Latin-1 byte, a tab, a `cents` suffix, bare integers, a trailing period,
     negative and non-monotonic degrees, 25-digit ratios, trailing comments, no final
     newline, a 3/1 period, 579 degrees) without copying a file.
  2. **Surge `tuning-library` test files** (MIT, 1.4) for the outside-bytes rule — a real
     second implementation's fixtures; list them in `THIRD-PARTY-NOTICES.md` (MIT is
     compatible with MPL-2.0 and GPL-3.0).
  3. **The whole archive by download** (2.1) for the sweep "every archive file parses, and
     agrees with the cents Scala prints" — the outside-source test the round wants.
  4. Optionally, a courtesy email to coul@huygens-fokker.org asking leave to commit ~20 archive
     files; nothing blocks on the answer.

### 2.3 Representative archive files — exact contents **[V 2026-11-22]**

SHA-256 / bytes / name (version 94):

```
7e7d41af241d88fb89e80de311d2f72402a9025974775f716373c67954208e1c  238  meanquar.scl
8d69e4d778636b5df2c10be512ca4bedd8dcee1cccc1a195a57ed61bc8d0b011  223  meansixth.scl
991ebb62e4767cb02257daa55ff4ba43b473a98149409457f9ba2964ae18f044  207  pyth_12.scl
f8d0c3cf7d396ae873391fc7f2f5f4a1f0196f8169f1693676bb4288e885475d  211  werck3.scl
a14135f9de77ada725fabc0a2e569f9f00cd8794914d74c7d1ca2d4c73ebad55  198  werck4.scl
b7358fe116cc72cd7d4b787317db03bc11f4cb2a8d3dabd2ea8b3dbcce93cf67  189  kirnberger2.scl
afe48d48170e030e826502b82748b4255fa4634f3cba597653c0cbaa6755f73e  172  kirnberger3.scl
5c0c4734c91e7b97a58fa4dbbce89fb6fae09df8a61bfcec5223f28373bf2015  247  vallotti.scl
6c63c7aa98608a485cd1eec646d7f34132dd30a8b8295b6036f8d09989dee53a  207  young2.scl
ccfa3b2b802117ac9fe37b322f53368992ffd05988d788a3308dcff9bb69a272  218  kellner.scl
b7bd2a54b0a0927bc240d5417dc6081036fc3f22bcc948bdafb3816bf33f8158  202  neidhardt1.scl
a6da2d4a9a880189536bb543c984cb6605c597cc400062fd89e8457ca151a38c  206  lehman1.scl
f408f43e6717b8b932f51f1589c78f074c73fcf2d5e5a7bf45b3dc11d8cdd486  153  ji_12.scl
235ec7dca241157eb07c3104e60f8c4d926bac0ef288f4eeaf3ab630ea18a222  268  bohlen-p_et.scl
946334f8e37f6c225c94ee0ff5d90a89916f0261afe315535d140eb367c9017d  199  bohlen-p.scl
2028b5524ece2860c3bb8e6b27c36cbf234f57d3dc979fcd1fe8c6d932138a41  292  fokker_31.scl
c301c1df6c170b53e152bd2569cd8c708a4639bbd6f7b5e21e4e10c665727e4c  218  arist_chrominv.scl
f6aa04f5e818deae454a40dbdbfc7ac54f932d4c00f183b62ab3b51cb4b7dfcc  259  dekany_agni.scl
d43ae069c1f56185f6ea07c32a08f5f23a156028b76d1671e55d0542ecca8c09  245  mavila12.scl
9742889e023544ddf1b4beb7d69706fdc22ee910c07b3792b10adc6ec9f805ac  334  fj-31tet.scl
7b4626ff7dbe03cd0d36fc878fb435a8cc8833a16e489e3a572bb459a1f449b0  209  neidhardt3.scl
9a2f50c44109ed23d54327f3490fb0851c60206ec53cc09c06e739cd340de565  131  chrys_diat-1st-ji.scl
b2ede88401aa7e0702a9ccf77dfebf66fa9f51124058bef7ec39f1ba3afe1baf  683  blackwoo.scl
a75deda96eba863291ab0bcf070e7eecb33e312ecd8f17c98591f85c7af51db7  389  atomschis.scl
16b0ae9a729ce375c10cc9101f5ad2757bc018ae05feb3e1bc5573740c924e60 4882  gann_wolfe.scl
38b590ee2c96fa5888e887a5f9f05a96709fc15cd46328447562cb6ace842adf  278  canou1d.scl
```

Exact contents of the most useful ones (CRLF in the files; leading spaces as shown; `⏎` marks
a description line that carries trailing spaces in the file):

```
! werck3.scl
!
Andreas Werckmeister's temperament III (the most famous one, 1681)
 12
!
 256/243
 192.18000
 32/27
 390.22500
 4/3
 1024/729
 696.09000
 128/81
 888.26999
 16/9
 1092.18000
 2/1
```

```
! meanquar.scl
!
1/4-comma meantone scale. Pietro Aaron's temp. (1523). 6/5 beats twice 3/2      ⏎
 12
!
 76.04900
 193.15686
 310.26471
 5/4
 503.42157
 579.47057
 696.57843
 25/16
 889.73529
 1006.84314
 1082.89214
 2/1
```

Non-octave, period 3/1, 13 steps:

```
! bohlen-p.scl
!
See Bohlen, H. 13-Tonstufen in der Duodezime, Acustica 39: 76-86 (1978)         ⏎
 13
!
 27/25
 25/21
 9/7
 7/5
 75/49
 5/3
 9/5
 49/25
 15/7
 7/3
 63/25
 25/9
 3/1
```

(`bohlen-p_et.scl`, "13-tone equal division of 3/1", is the tempered BP: 146.30423, 292.60846 …
1755.65077 in steps of 146.30423 c, period `3/1`.)

Negative first degree, not ascending, period 1206.54826 c (description has no trailing spaces):

```
! mavila12.scl
!
A 12-note mavila scale (for warping meantone-based music), 5-limit TOP
 12
!
-30.99719
 163.50770
 358.01258
 327.01540
 521.52028
 490.52310
 685.02798
 654.03080
 848.53568
 1043.04057
 1012.04338
 1206.54826
```

A comment between description and count; bare integers; ratios below 1/1:

```
! dekany_agni.scl
! 
Dekany agni {385/384, 1375/1372} oblique transversal convex closure
! valid val <284 83 146| generators 2.60/49.10/7
16
!
25/2
4/375
2/15
5/3
75/2
4/125
2/5
5
125/2
4/75
2/3
25/3
15
8/1125
4/25
2/1
```

(In `dekany_agni.scl` the second line is `! ` and the `! valid val …` line, the count line `16 `
and every pitch line except the last `2/1` end with one space before CRLF.)

No `!` line between the name comment and the description; ratio terms up to 25 digits:

```
! atomschis.scl
Atom Schisma Scale
12
!
156348578434374084375/147573952589676412928
134217728/119574225
1307544150375/1099511627776
18014398509481984/14297995284350625
10935/8192
1709671705179880612640625/1208925819614629174706176
16384/10935
14297995284350625/9007199254740992
2199023255552/1307544150375
119574225/67108864
295147905179352825856/156348578434374084375
2/1
```

Other edge files, by feature: `fj-31tet.scl` (tab characters after `7/5`, `97/58`, `65/38`,
`93/52`, plus runs of trailing spaces); `arist_chrominv.scl` (` 300.000 cents` — text after the
value); `canou1d.scl` (` 600.` — trailing period); `neidhardt3.scl` (byte 0xDF, Latin-1 `ß`,
in the description); `chrys_diat-1st-ji.scl` (LF only); `blackwoo.scl` (21 pitches, then 25
comment lines of an alternative scale, the last one `! 2/1` with no final newline);
`gann_wolfe.scl` (579 degrees, 4 882 bytes).

### 2.4 Other collections and their licences **[V 2026-11-22]** unless noted

| Collection | Licence | Use for Cantus |
|---|---|---|
| Surge `tuning-library` `tests/data` (~45 `.scl`/`.kbm`/`.ascl`, incl. a `bad/` set) | MIT (repo `LICENSE.md`) | **Commit as fixtures** with a notice; the outside-bytes source for the parser. |
| Surge XT factory tuning library (`resources/data/tuning_library`: `SCL/` 149 files incl. `12 Tone Equal Temperament.scl`, `Bohlen-Pierce.scl`, `ED2-05`…`ED2-31`, ED3, ED4, Carlos α/β/γ; `KBM Concert Pitch/`; `Documentation.txt`) | No separate licence in the folder; the repository is **GPL-3.0** (GitHub licence API: `GPL-3.0`) | Usable in the **app** (GPL-3.0-or-later) — e.g. ship it as presets — but **not** as fixtures of an MPL crate (ADR 0047: no GPL into MPL crates). |
| Scale Workshop (xenharmonic-devs) | MIT | Its exporter tests give hashes, not bytes (see §4.5) — little direct use. |
| Ableton Core Library tunings (`.ascl`, tuning.ableton.com) | **No licence stated** on the site ("Make your own tuning" page) | Do not redistribute. |
| Xenharmonic Wiki | Wiki says content is free to redistribute and modify unless marked; exact licence not found | **[NOT VERIFIED]** — not needed. |

Recommendation: the MPL tuning crate's fixtures = our own files + `tuning-library` (MIT); the
archive sweep by pinned download; Surge XT's GPL library may later seed the app's preset browser
(credited).

---

## 3. MTS-ESP and CLAP tuning

### 3.1 Licence and files **[V 2026-11-22]**

Repository <https://github.com/ODDSound/MTS-ESP>, head `f214739b` (2026-02-04). `LICENSE`:
"Copyright (C) 2021 by ODDSound Ltd. … Permission to use, copy, modify, and/or distribute this
software for any purpose with or without fee is hereby granted. THE SOFTWARE IS PROVIDED "AS
IS" …" — **0BSD** (GitHub reports SPDX `0BSD`). The same text heads `libMTSClient.h/.cpp` and
`libMTSMaster.h/.cpp`. Files:

- `Client/libMTSClient.h`, `Client/libMTSClient.cpp` (908 lines) — the client wrapper (source).
- `Master/libMTSMaster.h`, `Master/libMTSMaster.cpp` (227 lines) — the master wrapper (source).
- `libMTS/` — the **binary** dynamic library the two wrappers load, plus installers:
  `Win/64bit/LIBMTS.dll`, `Win/32bit/LIBMTS.dll`, `Win/libMTSWin_v1.03.exe`,
  `Mac/x86_64_ARM/libMTS.dylib` + `libMTSMac_v1.03.pkg`, `Mac/i386_x86_64/…`,
  `Linux/x86_64|arm64|arm32hf/libMTS.so`, `MTS-ESP.conf`, `CHANGELOG.md` (libMTS 1.03,
  2025-09-04). **The source of libMTS itself is not published** — only the binaries.

What the 0BSD covers: the repository as a whole, so the wrappers (source) and, by the README's
own words, the binaries ("Windows and OSX Installers are provided which you can bundle into
your own installer or, if you prefer, just include the library files and install to the above
locations"). A Rust port of the wrapper logic may be MPL-2.0 (0BSD asks nothing).

Shipping question for a GPL app: Cantus never links libMTS at build time — it is opened at run
time from a system folder, like a plug-in, and only if present. Recommendation: **do not bundle
libMTS** in Cantus's installers; load it if the user has it (any MTS-ESP product — MTS-ESP Mini,
which is free, Surge XT, Pianoteq … — installs it); show "MTS-ESP: not installed" with a link
otherwise. This avoids shipping a closed binary inside a GPL distribution and avoids two
installers fighting over one system file. (This licence reading is ours, not a lawyer's.)

### 3.2 Where libMTS lives — and how the wrappers find it **[V 2026-11-22]**

README / devs page: "Windows 64bit: Program Files\Common Files\MTS-ESP (64 bit library) and
Program Files (x86)\Common Files\MTS-ESP (32 bit library) … Mac OSX: /Library/Application
Support/MTS-ESP … Linux: /usr/local/lib". The code (`libMTSClient.cpp` lines 95–183, the
master the same):

- **Windows:** `SHGetKnownFolderPath(FOLDERID_ProgramFilesCommon)` (GUID
  `{F7F1ED05-9F6D-47A2-AAAE-29D317C6F066}`) + `\MTS-ESP\LIBMTS.dll`, `LoadLibraryW`; nothing
  else is searched.
- **macOS:** `dlopen("/Library/Application Support/MTS-ESP/libMTS.dylib", RTLD_NOW)`, then
  **Linux:** `dlopen("/usr/local/lib/libMTS.so", RTLD_NOW)` (the same code path tries both).
- macOS: "Standalone master or client apps on macOS must use the Disable Library Validation
  Entitlement to allow the library to load." → **Cantus's macOS build needs
  `com.apple.security.cs.disable-library-validation`** (it already hosts plug-ins, so it
  likely has it — check the entitlements file in round 42).
- IPC: libMTS shares state across processes (for hosts that sandbox plug-ins). Config
  `MTS-ESP.conf` (`ipc_support = 1` default) in the same folder on Windows/macOS,
  `/usr/local/etc` on Linux. IPC matters for Cantus only if it hosts plug-ins out of process.

Exported symbols (`GetProcAddress`/`dlsym` names) — what a Rust loader in `cantus-rt` binds:

Client side: `MTS_RegisterClient`, `MTS_DeregisterClient`, `MTS_HasMaster`,
`MTS_GetVersionNumber`, `MTS_ShouldFilterNote`, `MTS_ShouldFilterNoteMultiChannel`,
`MTS_GetTuningTable` (returns `const double*` to 128 frequencies — the client reads it
directly, no call per note), `MTS_GetMultiChannelTuningTable`, `MTS_UseMultiChannelTuning`,
`MTS_GetScaleName`, `MTS_GetPeriodRatio`, `MTS_GetMapSize`, `MTS_GetMapStartKey`,
`MTS_GetRefKey`.

Master side: `MTS_RegisterMaster` (takes one `void*`, the wrapper passes 0),
`MTS_DeregisterMaster`, `MTS_Reinitialize`, `MTS_HasMaster`, `MTS_HasIPC`,
`MTS_GetVersionNumber`, `MTS_GetNumClients`, `MTS_SetNoteTunings` (`const double[128]`),
`MTS_SetNoteTuning(double, char)`, `MTS_SetScaleName`, `MTS_SetPeriodRatio`,
`MTS_SetMapSize`, `MTS_SetMapStartKey`, `MTS_SetRefKey`, `MTS_FilterNote(bool, char, signed
char)`, `MTS_ClearNoteFilter`, `MTS_SetMultiChannel`, `MTS_SetMultiChannelNoteTunings`,
`MTS_SetMultiChannelNoteTuning`, `MTS_FilterNoteMultiChannel`,
`MTS_ClearNoteFilterMultiChannel`.

### 3.3 The API, as the headers state it **[V 2026-11-22]**

Client (public wrapper functions, `libMTSClient.h`):

```c
MTSClient *MTS_RegisterClient();               // plug-in constructor
void   MTS_DeregisterClient(MTSClient *c);     // destructor
bool   MTS_HasMaster(MTSClient *c);
bool   MTS_ShouldFilterNote(MTSClient *c, char midinote, signed char midichannel); // ch 0..15 or -1
double MTS_NoteToFrequency(MTSClient *c, char midinote, signed char midichannel);
double MTS_RetuningInSemitones(MTSClient *c, char midinote, signed char midichannel);
double MTS_RetuningAsRatio(MTSClient *c, char midinote, signed char midichannel);
char   MTS_FrequencyToNote(MTSClient *c, double freq, signed char midichannel);
char   MTS_FrequencyToNoteAndChannel(MTSClient *c, double freq, signed char *midichannel);
const char *MTS_GetScaleName(MTSClient *c);
double MTS_GetPeriodRatio(MTSClient *c);  double MTS_GetPeriodSemitones(MTSClient *c);
signed char MTS_GetMapSize(MTSClient *c); signed char MTS_GetMapStartKey(MTSClient *c);
signed char MTS_GetRefKey(MTSClient *c);  // -1 = not supplied by the master
void   MTS_ParseMIDIDataU(MTSClient *c, const unsigned char *buf, int len); // MTS SysEx
bool   MTS_HasReceivedMTSSysEx(MTSClient *c);
bool   MTS_Client_ShouldUpdateLibrary(MTSClient *c);
```

The header's 12 steps, in short: (1) register/deregister (required); (2) query at note-on
(required); (3) **recommended: query continuously while a note sounds** ("ideally at the same
time as processing any other pitch modulation sources"); (4) offer note-on-only vs continuous;
(5) check `MTS_ShouldFilterNote` at note-on (unmapped keys of a `.kbm`); (6) always pass the
MIDI channel — but "If your plug-in supports MPE … it is recommended to NOT supply a MIDI channel
if MPE is enabled"; (7) **a plug-in's own loaded tuning should override MTS-ESP, or offer a
switch** — exactly Cantus's per-instrument override; (8) parse MTS SysEx when no master is
connected; (9) show connection status; (10) scale name; (11) warn on an old libMTS.
"When not connected to a master plugin, a client will automatically revert to a local tuning
table, set to 12-TET by default."

Master (`libMTSMaster.h`): "Only one master plugin may connect via MTS-ESP at any one time."
The registration pattern the header prescribes:

```c
if (MTS_CanRegisterMaster())          // = !HasMaster()
    MTS_RegisterMaster();
else if (MTS_HasIPC())                // a crashed process may have left the flag set
    /* warn; offer "Reinitialize MTS-ESP" → MTS_Reinitialize(); MTS_RegisterMaster(); */
else
    /* warn: another master is connected; no reinitialize option */
```

Then `MTS_SetNoteTunings(freqs[128])` (or per note), `MTS_SetScaleName`, `MTS_SetPeriodRatio`,
`MTS_SetMapSize/StartKey/RefKey` (−1 = not supplied), `MTS_FilterNote(true, note, -1)` for
unmapped keys ("it is suggested to provide a frequency for all MIDI notes, even unmapped ones.
Ideally unmapped notes should use the frequency of the next lowest mapped note, or next highest
if there is none lower"), multi-channel tables for > 128-key layouts with a single-channel table
as fallback; `MTS_GetNumClients()` for the UI.

### 3.4 Can a host be the master without ODDSound's paid Suite? — **yes** **[V 2026-11-22]**

- The master API and libMTS are 0BSD with "any purpose with or without fee"; nothing in the
  repository, README or devs page reserves the master role. ODDSound's own README: "A free
  master plugin, ODDSound MTS-ESP Mini … provides a simple, no-cost way for anyone to start
  using MTS-ESP"; "Most developers will have no reason to read further than the Client section
  … unless you plan to build a master plugin" — third-party masters are anticipated.
- Third-party masters exist: Surge XT "can act as MTS-ESP source" (Surge tuning guide, since
  1.2); Infinitone DMT, Entonal Studio and Wilsonic appear in ODDSound's own compatibility list
  (<https://oddsound.com/usingmtsesp.php>).
- **No DAW is a native MTS-ESP master** as far as searches found (Live, Bitwig, Logic, FL,
  REAPER: none documented). Renoise 3.5 is a native **client** (its sampler instruments follow a
  master; KVR/CDM coverage of 3.5 — secondary sources **[NOT VERIFIED]** on renoise.com) and
  appears in ODDSound's list. Cantus as master *and* client would be first among DAWs.
- The paid MTS-ESP Suite (GBP 79.99 in 2021, gearnews) adds dynamic tuning tools and the "MIDI
  Client" that retunes non-MTS plug-ins/hardware by MIDI — features Cantus can build itself.
- **Trademark**: no trademark notice or usage terms for "MTS-ESP" were found on oddsound.com
  (pages fetched: index, mtsespmini, mtsespsuite, devs, usingmtsesp; footer "© 2021 ODDSOUND -
  All Rights Reserved"). **[NOT VERIFIED]** whether it is a registered mark. Safe practice:
  name it descriptively ("MTS-ESP (ODDSound)" support, "follows MTS-ESP"), no ODDSound logo, a
  credit line in the notices; optionally email tech@oddsound.com to be listed on their
  compatibility page (they invite developers to do so).

### 3.5 Real-time notes for Cantus (from the client code) **[V 2026-11-22]**

- The client reads the master's table through a pointer into libMTS's (possibly shared)
  memory: `MTS_NoteToFrequency` is a table read plus a branch — no lock, no allocation. Safe to
  call on the audio thread (ADR 0005) once registration (which loads the DLL) happened on the
  main thread. Registration/deregistration and `LoadLibraryW`/`dlopen` must stay off the audio
  thread.
- Changes are seen on the next read — the Surge guide warns "the clients don't retune instantly";
  a master should publish a tuning change about one audio block before the notes that need it.
  For Cantus as master, write the table at the start of the block (or ahead, for automated
  tuning changes) so hosted clients see it before rendering.
- Determinism: a hosted MTS-ESP client's pitch is only as deterministic as when it reads; for
  offline render = real time, Cantus must publish the table at the same block positions in both.

### 3.6 CLAP tuning **[V 2026-11-22]**

CLAP repository <https://github.com/free-audio/clap> (MIT), `version.h` 1.2.10 on `main`.
`include/clap/ext/draft/tuning.h` exists and is still **draft**: `CLAP_EXT_TUNING[] =
"clap.tuning/2"`. Its shape:

```c
// event (space id from clap_host_event_registry->query(host, CLAP_EXT_TUNING, &space_id))
typedef struct clap_event_tuning {
   clap_event_header_t header;
   int16_t port_index; // -1 global
   int16_t channel;    // 0..15, -1 global
   clap_id tunning_id; // (sic — spelled "tunning" in the header)
} clap_event_tuning_t;
typedef struct clap_tuning_info { clap_id tuning_id; char name[CLAP_NAME_SIZE]; bool is_dynamic; } clap_tuning_info_t;
typedef struct clap_plugin_tuning { void (*changed)(const clap_plugin_t *plugin); } clap_plugin_tuning_t; // [main-thread]
typedef struct clap_host_tuning {
   // relative tuning in semitones against 12-EDO A4=440; "at a rate that makes sense for *low*
   // frequency modulations"; should_play() checked first   [audio-thread & in-process]
   double   (*get_relative)(const clap_host_t*, clap_id tuning_id, int32_t channel, int32_t key, uint32_t sample_offset);
   bool     (*should_play)(const clap_host_t*, clap_id tuning_id, int32_t channel, int32_t key); // [audio-thread]
   uint32_t (*get_tuning_count)(const clap_host_t*);                                       // [main-thread]
   bool     (*get_info)(const clap_host_t*, uint32_t tuning_index, clap_tuning_info_t *info); // [main-thread]
} clap_host_tuning_t;
```

So a host offers a *pool* of tunings, tells the plug-in which applies per port/channel by an
event, and the plug-in pulls relative semitones per key. It is a draft: few plug-ins implement it
(none found named; **[NOT VERIFIED]** adoption). Cantus can offer `clap.tuning/2` cheaply on top
of its tuning core, but must not rely on it.

What works with every CLAP instrument today: **note expressions**. `events.h`:
`CLAP_NOTE_EXPRESSION_TUNING = 2` — "Relative tuning in semitones, from -120 to +120.
Semitones are in equal temperament and are doubles; the resulting note would be retuned by
`100 * evt->value` cents." A host sends a per-note tuning expression right after the note-on
(Bitwig does this from Micro-pitch; Surge's "Tuning Note CLAPs" turn MTS-ESP into note
expressions — <https://surge-synth-team.org/tuning-note-claps>). A CLAP plug-in can only say it speaks the CLAP note dialect
(`note-ports.h`: `CLAP_NOTE_DIALECT_CLAP` "Uses clap_event_note and clap_event_note_expression",
alongside `_MIDI`, `_MIDI_MPE`, `_MIDI2`); there is **no per-expression capability flag**, so
whether it honours TUNING is known only by testing it **[V 2026-11-22]**.

Delivery order for a hosted plug-in in Cantus (proposal): (1) plug-in is an MTS-ESP client →
Cantus as master covers it with no per-note work; (2) CLAP with tuning note expressions →
per-note expression; (3) VST3 → `kTuningTypeID` per note — `pluginterfaces/vst/ivstnoteexpression.h` (MIT):
"Tuning, plain range [0 = -120.0 (ten octaves down), 0.5 none, 1 = +120.0 (ten octaves up)] …
plain = 240 * (norm - 0.5)" semitones **[V 2026-11-22]**; a VST3 instrument lists the
expressions it supports through `INoteExpressionController`, so the host can know; (4) MPE-capable (VST3/CLAP/AU) → per-note pitch bend with a 48-semitone
range, as Live does; (5) otherwise MTS SysEx into the plug-in's MIDI input (if it reads it) or
nothing — and say so in the instrument's header ("not following the project tuning").

---

## 4. MIDI Tuning Standard — byte layouts

### 4.1 Sources

- The current spec, "MIDI Tuning Updated Specification" (MMA/AMEI), is behind a free MIDI
  Association login: <https://midi.org/midi-tuning-updated-specification> ("Join Us to
  Download"; files listed: "MIDI Tuning Updated Specification.pdf", "MIDI Tuning Bank/Dump
  Extensions (CA-020)", "MIDI Tuning Scale/Octave Extensions (CA-021/RP-020)", "Defaults for
  Scale/Octave Tuning (RP-020)"). **Not downloaded** (needs an account — the owner's).
- The MMA's own public page of the same text, archived: **`https://web.archive.org/web/20100414023103id_/http://www.midi.org:80/techspecs/midituning.php`**
  ("CA-020/CA-021/RP-020 Approved by MMA 02/99 / Approved by AMEI 05/99"). Everything in 4.2–4.4
  is from it **[V 2026-11-22]**. (Its text is © MMA, "no part … may be reproduced" — quote the
  layouts in our docs as facts, do not paste the page.)
- Two independent implementations read today, for the points where the spec is loose: ODDSound
  `libMTSClient.cpp` `parseMIDIData` (0BSD, lines 593–822) and FluidSynth
  `src/synth/fluid_synth.c` `fluid_synth_sysex_midi_tuning` (LGPL-2.1, read only — do not copy
  into MPL code).

### 4.2 The frequency word (3 bytes) **[V 2026-11-22]**

"The first byte of the frequency data word specifies the nearest equal-tempered semitone below
the frequency. The next two bytes (14 bits) specify the fraction of 100 cents above the semitone
… Effective resolution = 100 cents / 2^14 = .0061 cents."

```
0xxxxxxx 0abcdefg 0hijklmn     xxxxxxx = semitone (MIDI note of 12-EDO, A4 = 69 = 440 Hz)
                                abcdefghijklmn = fraction, units of 100/16384 cent
f = 440 · 2^((xxxxxxx + abcdefghijklmn/16384 − 69) / 12)
7F 7F 7F = "no change" (reserved): keep that key's stored frequency.
```

Range: MIDI note 0 = 8.1758 Hz up to `7F 7F 7E` = top. The spec's example table, checked against
the formula (our computation):

| bytes | spec Hz | formula Hz | |
|---|---:|---:|---|
| `00 00 00` | 8.1758 | 8.1758 | ✓ |
| `00 00 01` | 8.2104 | 8.1758 | **erratum** in the spec (one step is 0.00003 Hz here) |
| `01 00 00` | 8.6620 | 8.6620 | ✓ |
| `0C 00 00` | 16.3516 | 16.3516 | ✓ |
| `3C 00 00` | 261.6256 | 261.6256 | ✓ middle C |
| `3D 00 00` | 277.1827 | 277.1826 | ✓ (rounding) |
| `44 7F 7F` | 439.9984 | 439.9984 | ✓ |
| `45 00 00` | 440.0000 | 440.0000 | ✓ A-440 |
| `45 00 01` | 440.0016 | 440.0016 | ✓ |
| `78 00 00` | 8372.0190 | 8372.0181 | ✓ (rounding) |
| `78 00 01` | 8372.0630 | 8372.0476 | **erratum** (+0.0154 Hz) |
| `7F 00 00` | 12543.8800 | 12543.8540 | **erratum** (+0.026 Hz) |
| `7F 00 01` | 12543.9200 | 12543.8982 | **erratum** |
| `7F 7F 7E` | 13289.7300 | 13289.6566 | **erratum** (+0.073 Hz) |

→ The table is a usable **outside-source test vector** for the nine consistent rows (tolerance
0.001 Hz); the five inconsistent rows are the spec's own arithmetic slips and must not be
"fixed" in our decoder.

Our computed vectors (from the formula, for round-trip tests only — not outside bytes):
415 Hz → `43 7E 30`; 430 Hz → `44 4D 07`; 432 Hz → `44 57 2B`; 442 Hz → `45 0A 06`;
443 Hz → `45 0F 07`; 261.6255653 Hz → `3C 00 00`.

Implementations differ in the last bit: libMTSClient divides the 14-bit fraction by **16383**
(`(sysex_value & 16383) / 16383.0`), not 16384 — up to 0.006 c sharp at the top of a semitone.
FluidSynth encodes with `frac = ((cents − note·100)·16384 + 50)/100` (rounded) and clips to
0..16383. Our decoder: /16384 per the spec.

### 4.3 The messages **[V 2026-11-22]**

All MTS messages: `F0 <7E|7F> <device ID> 08 <sub-ID#2> …`. 7E = non-real-time, 7F = real-time;
device ID `7F` = all devices. ("Even though the first two messages are in the Universal Non-Real
Time area and the last in the Real Time area, they keep the same sub-IDs.")

| sub-ID#2 | Name | Layout | Bytes (with F0/F7) |
|---|---|---|---|
| `00` | Bulk tuning dump request | `F0 7E dev 08 00 tt F7` | 7 |
| `01` | **Bulk tuning dump** | `F0 7E dev 08 01 tt <name 16 ASCII> [xx yy zz]×128 chk F7` | **408** |
| `02` | **Single note tuning change (real-time)** | `F0 7F dev 08 02 tt ll [kk xx yy zz]×ll F7` | 8 + 4·ll |
| `03` | Bulk tuning dump request (bank) | `F0 7E dev 08 03 bb tt F7` | 8 |
| `04` | Key-based tuning dump (bank) | `F0 7E dev 08 04 bb tt <name 16> [xx yy zz]×128 chk F7` | 409 |
| `05` | Scale/octave tuning dump, 1-byte | `F0 7E dev 08 05 bb tt <name 16> [xx]×12 chk F7` | 37 |
| `06` | Scale/octave tuning dump, 2-byte | `F0 7E dev 08 06 bb tt <name 16> [xx yy]×12 chk F7` | 49 |
| `07` | Single note tuning change (bank) | `F0 7F dev 08 07 bb tt ll [kk xx yy zz]×ll F7` (real-time: affects sounding notes) **or** `F0 7E …` (non-real-time: does not) | 9 + 4·ll |
| `08` | **Scale/octave tuning, 1-byte form** | `F0 7F dev 08 08 ff gg hh [ss]×12 F7` (or `7E` non-real-time) | **21** |
| `09` | **Scale/octave tuning, 2-byte form** | `F0 7F dev 08 09 ff gg hh [ss tt]×12 F7` (or `7E`) | **33** |

Fields:
- `tt` tuning program 0–127; `bb` bank 0–127 ("described as 1-128 in MIDI Tuning
  Specification"); `ll` number of changes; `kk` MIDI key.
- `ff gg hh` — channel bitmap: `ff` bits 0–1 = channels 15–16 (bits 2–6 reserved, must be 0),
  `gg` bits 0–6 = channels 8–14, `hh` bits 0–6 = channels 1–7. All 16 channels = `03 7F 7F`.
- 1-byte offsets (`05`, `08`): 12 bytes C, C#, … B; `00` = −64 c, `40` = 0, `7F` = +63 c.
- 2-byte offsets (`06`, `09`): 14-bit, `00 00` = −100 c ("8,192 steps of .012207 cents"), `40 00`
  = 0, `7F 7F` = +100 c ("8,191 steps"); step = 200/16384 = 0.012207 c. So
  `cents = (v − 8192) · 200/16384` — FluidSynth decodes exactly so; libMTSClient uses
  `(v − 8192)/8192` semitone below and `/8191` above (makes `7F 7F` exactly +100 c).
- Scale/octave offsets are relative to equal temperament (or to the selected preset: "Each
  message that is received will offset from the original preset, not from the modified
  tuning").
- Dumps (`01`, `04`): "in order from note 0 (earliest sent) to note 127 (latest sent)"; keys an
  instrument cannot play may be sent as `7F 7F 7F`.
- Real-time vs not: real-time changes "should instantly be re-tuned … while it continues to
  sound … without glitching, forced Note-Offs, re-triggering"; non-real-time ones are setup
  messages and ignored for sounding notes.

Selecting tunings (RPN): Tuning Program Select = RPN 03, Tuning Bank Select = RPN 04, e.g.
`Bn 64 03 65 00 06 tt` (data entry), `Bn 64 03 65 00 60 7F` (increment), `… 61 7F` (decrement).
RP-020: with presets, bank 0 / preset 0 should be equal temperament.

### 4.4 The checksum — three conventions in the wild

- **Spec, CA-020** (applies to `04`, `05`, `06`; the original text for `01` is older):
  "calculated by successively XOR'ing the bytes in the message, excluding the F0, F7, and the
  checksum field. The resulting value is then AND'ed with 7F." **[V 2026-11-22]**
- The original `01` text: "chksum (XOR of 7E <device ID> nn tt <388 bytes>)" — "388" cannot be
  right (name 16 + data 384 = 400); the archived page has it so. **[V 2026-11-22]**
- **FluidSynth** writes the `01` checksum as `7E ^ 08 ^ 01 ^ prog ^ <384 data bytes>` — without
  the device ID and the name (comment: "Checksum is not as straight forward as the bank based
  messages"), and the `04` checksum over everything from `7E` through the data. **[V]** (code)
- **Scale Workshop** (MIT, `src/exporters/mts-sysex.ts`) seeds the XOR with `0xFF` over every
  byte < 0x80 (so `7E dev 08 01 tt name data`), then `& 0x7F` — i.e. the **inverted** spec value.
  **[V]** (code)
- libMTSClient **ignores** the checksum (`case eCheckSum: state = eIgnoring`). **[V]** (code)

For the 12-EDO dump `F0 7E 7F 08 01 00 "12-TET A440     " [kk 00 00]×128 chk F7` (408 bytes) the
three give `12` (CA-020 rule), `77` (FluidSynth), `6D` (Scale Workshop) — our computation.
**Decision:** write the CA-020 value; **read leniently** — accept any of the three, flag a
mismatch in the import report but load the tuning.

### 4.5 Real dumps from outside sources — what exists

| Source | What | Status |
|---|---|---|
| The spec's example table (4.2) | 14 frequency words with Hz | **[V]** — usable now (9 consistent rows). |
| libMTSClient and FluidSynth parsers | independent decoders to cross-check ours | **[V]** — port the libMTS logic's *behaviour* to a test oracle (0BSD allows copying); FluidSynth only as reference. |
| Scala (the program, free) | writes MTS tuning dumps into MIDI files (`EXAMPLE/MIDI/MTS`: "An extra track is created with the tuning dump, based on the current scale and keyboard mapping") and sends dumps to synths (`SEND`) | **[V]** that it can; generating `.syx`/`.mid` with it is an owner-side step on Windows (Scala needs a GTK runtime) → the bytes are tool output of our own scale, fine to commit. Put the exact steps in `docs/beta-checklist.md`. |
| Sequential OB-6 / Prophet X "16 alternative tunings" SysEx downloads | real synth-maker files | download links are behind JavaScript on sequential.com; **[NOT VERIFIED]** that they are MTS (they may be Sequential's own format) or their terms. |
| Scale Workshop's MTS exporter test | SHA-256 of an exported dump | hashes only, of a scale built by its own engine — not usable as bytes. |
| MTS-ESP Mini | loads "MTS SysEx files" | ships none that we found. |

**Recommendation:** outside-source tests = the spec table + Scala-generated dumps (owner step)
+ cross-decoding against a 0BSD port of libMTSClient's parser; hardware dumps (a real synth's
`F0 7E … 08 01`) go to the beta checklist for whoever owns MTS-capable hardware.

Examples of what Cantus will send (our computation, for the round's golden tests):

- Werckmeister III (C unchanged) to all channels, 1-byte real-time:
  `F0 7F 7F 08 08 03 7F 7F 40 36 38 3A 36 3E 34 3C 38 34 3C 38 F7` (21 bytes).
- The same, 2-byte real-time:
  `F0 7F 7F 08 09 03 7F 7F 40 00 39 5F 3A 7F 3C 20 39 5F 3E 60 38 3F 3D 40 3A 7F 38 3F 3D 40 3A 7F F7` (33 bytes).
- 12-EDO bulk dump: starts
  `F0 7E 7F 08 01 00 31 32 2D 54 45 54 20 41 34 34 30 20 20 20 20 20 00 00 00 01 00 00 02 00 00 …`,
  checksum `12`, then `F7`.

---

## 5. Historical temperaments and just intonation — 12 values in cents from C

### 5.1 Sources and method

- Values are computed (our script, `python -I`, exact ratios → `1200·log2`) from the **Scala
  archive v94** files named in the last column — Manuel Op de Coul's compilation, each file
  citing its historical source in its description. **[V 2026-11-22]** that the numbers below are
  those files' numbers.
- Independent cross-check: each temperament rebuilt from its **construction rule** (which
  fifths are narrowed by which fraction of the Pythagorean comma PC = 23.460010 c or the
  syntonic comma SC = 21.506290 c; schisma = 1.953721 c) agrees with the archive file to
  **0.0000 c** for: ¼- and ⅙-comma meantone (chain E♭–G♯), Werckmeister III (¼ PC on C–G, G–D,
  D–A, B–F♯), Kellner (⅕ PC on C–G, G–D, D–A, A–E, B–F♯), Kirnberger III (¼ SC on C–G, G–D, D–A,
  A–E, schisma on F♯–C♯), Vallotti (⅙ PC on F–C–G–D–A–E–B), Young II (⅙ PC on C–G–D–A–E–B–F♯).
- Second published source: Wikipedia's tables (secondary; they cite Barbour 1951, Jorgensen
  1991, Lindley 1987, Rasch's 1983 edition of Werckmeister, Donahue 2005) agree with the rows for
  Werckmeister III and IV, Kirnberger I/II/III, Vallotti (modern) and Young II — fetched
  2026-11-22. Barbour, Lindley and Jorgensen themselves were **not** consulted (books).
- Where the archive and the standard tables disagree, both are listed and flagged (5.3).

### 5.2 The tables

The default anchor of every row is C = 0. A temperament "in another key" is the same 12 values
rotated (5.4); where A = 440 Hz must hold, shift the whole row (5.4).

| Temperament | C | C# | D | Eb | E | F | F# | G | G# | A | Bb | B | Source |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Pythagorean (chain Eb–G#, wolf G#–Eb) | 0.000 | 113.685 | 203.910 | 294.135 | 407.820 | 498.045 | 611.730 | 701.955 | 815.640 | 905.865 | 996.090 | 1109.775 | `pyth_12.scl` |
| 1/4-comma meantone (Aaron 1523; chain Eb–G#) | 0.000 | 76.049 | 193.157 | 310.265 | 386.314 | 503.422 | 579.471 | 696.578 | 772.627 | 889.735 | 1006.843 | 1082.892 | `meanquar.scl` |
| 1/6-comma meantone (Salinas; chain Eb–G#) | 0.000 | 88.594 | 196.741 | 304.888 | 393.482 | 501.629 | 590.224 | 698.371 | 786.965 | 895.112 | 1003.259 | 1091.853 | `meansixth.scl` |
| Werckmeister III (1681/1691; Werckmeister's "I (III)") | 0.000 | 90.225 | 192.180 | 294.135 | 390.225 | 498.045 | 588.270 | 696.090 | 792.180 | 888.270 | 996.090 | 1092.180 | `werck3.scl` |
| Werckmeister IV (1691; Werckmeister's "II (IV)") | 0.000 | 82.405 | 196.090 | 294.135 | 392.180 | 498.045 | 588.270 | 694.135 | 784.360 | 890.225 | 1003.910 | 1086.315 | `werck4.scl` |
| Kirnberger II (1771/1774), Db/Ab spelling | 0.000 | 90.225 | 203.910 | 294.135 | 386.314 | 498.045 | 590.224 | 701.955 | 792.180 | 895.112 | 996.090 | 1088.269 | `construction; = Wikipedia table` |
| Kirnberger II, archive spelling (C# 135/128) | 0.000 | 92.179 | 203.910 | 294.135 | 386.314 | 498.045 | 590.224 | 701.955 | 794.134 | 895.078 | 996.090 | 1088.269 | `kirnberger2.scl` |
| Kirnberger III (letter to Forkel, 1779) | 0.000 | 90.225 | 193.157 | 294.135 | 386.314 | 498.045 | 590.224 | 696.578 | 792.180 | 889.735 | 996.090 | 1088.269 | `kirnberger.scl` |
| Vallotti (modern; 1/6 PC on F–C–G–D–A–E–B) | 0.000 | 94.135 | 196.090 | 298.045 | 392.180 | 501.955 | 592.180 | 698.045 | 796.090 | 894.135 | 1000.000 | 1090.225 | `vallotti.scl` |
| Young II (1799/1800; 1/6 PC on C–G–D–A–E–B–F#) | 0.000 | 90.225 | 196.090 | 294.135 | 392.180 | 498.045 | 588.270 | 698.045 | 792.180 | 894.135 | 996.090 | 1090.225 | `young.scl` |
| Kellner (1975; 1/5 PC on C–G–D–A–E and B–F#) | 0.000 | 90.225 | 194.526 | 294.135 | 389.052 | 498.045 | 588.270 | 697.263 | 792.180 | 891.789 | 996.090 | 1091.007 | `kellner.scl` |
| Neidhardt III, "große Stadt" (1724) | 0.000 | 96.090 | 196.090 | 298.045 | 394.135 | 498.045 | 596.090 | 698.045 | 796.090 | 894.135 | 998.045 | 1096.090 | `neidhardt3.scl` |
| Lehman, Bach temperament I (2005) | 0.000 | 99.347 | 196.741 | 299.512 | 393.482 | 501.629 | 597.392 | 698.371 | 799.510 | 895.112 | 999.674 | 1095.437 | `lehman1.scl` |
| 5-limit just, major and minor on C (Ellis duodene) | 0.000 | 111.731 | 203.910 | 315.641 | 386.314 | 498.045 | 590.224 | 701.955 | 813.686 | 884.359 | 1017.596 | 1088.269 | `duodene.scl` |
| 5-limit just, Malcolm 1721 (Bb = 16/9) | 0.000 | 111.731 | 203.910 | 315.641 | 386.314 | 498.045 | 590.224 | 701.955 | 813.686 | 884.359 | 996.090 | 1088.269 | `malcolm.scl` |
| 7-limit just 12 (basis for "barbershop") | 0.000 | 111.731 | 203.910 | 266.871 | 386.314 | 498.045 | 582.512 | 701.955 | 813.686 | 933.129 | 968.826 | 1088.269 | `ji_12a.scl` |

Deviation from 12-EDO (cents; what MTS scale/octave messages and per-note "cents" UIs carry):

| Temperament | C | C# | D | Eb | E | F | F# | G | G# | A | Bb | B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Pythagorean (chain Eb–G#, wolf G#–Eb) | +0.000 | +13.685 | +3.910 | -5.865 | +7.820 | -1.955 | +11.730 | +1.955 | +15.640 | +5.865 | -3.910 | +9.775 |
| 1/4-comma meantone (Aaron 1523; chain Eb–G#) | +0.000 | -23.951 | -6.843 | +10.265 | -13.686 | +3.422 | -20.529 | -3.422 | -27.373 | -10.265 | +6.843 | -17.108 |
| 1/6-comma meantone (Salinas; chain Eb–G#) | +0.000 | -11.406 | -3.259 | +4.888 | -6.518 | +1.629 | -9.776 | -1.629 | -13.035 | -4.888 | +3.259 | -8.147 |
| Werckmeister III (1681/1691; Werckmeister's "I (III)") | +0.000 | -9.775 | -7.820 | -5.865 | -9.775 | -1.955 | -11.730 | -3.910 | -7.820 | -11.730 | -3.910 | -7.820 |
| Werckmeister IV (1691; Werckmeister's "II (IV)") | +0.000 | -17.595 | -3.910 | -5.865 | -7.820 | -1.955 | -11.730 | -5.865 | -15.640 | -9.775 | +3.910 | -13.685 |
| Kirnberger II (1771/1774), Db/Ab spelling | +0.000 | -9.775 | +3.910 | -5.865 | -13.686 | -1.955 | -9.776 | +1.955 | -7.820 | -4.888 | -3.910 | -11.731 |
| Kirnberger II, archive spelling (C# 135/128) | +0.000 | -7.821 | +3.910 | -5.865 | -13.686 | -1.955 | -9.776 | +1.955 | -5.866 | -4.922 | -3.910 | -11.731 |
| Kirnberger III (letter to Forkel, 1779) | +0.000 | -9.775 | -6.843 | -5.865 | -13.686 | -1.955 | -9.776 | -3.422 | -7.820 | -10.265 | -3.910 | -11.731 |
| Vallotti (modern; 1/6 PC on F–C–G–D–A–E–B) | +0.000 | -5.865 | -3.910 | -1.955 | -7.820 | +1.955 | -7.820 | -1.955 | -3.910 | -5.865 | +0.000 | -9.775 |
| Young II (1799/1800; 1/6 PC on C–G–D–A–E–B–F#) | +0.000 | -9.775 | -3.910 | -5.865 | -7.820 | -1.955 | -11.730 | -1.955 | -7.820 | -5.865 | -3.910 | -9.775 |
| Kellner (1975; 1/5 PC on C–G–D–A–E and B–F#) | +0.000 | -9.775 | -5.474 | -5.865 | -10.948 | -1.955 | -11.730 | -2.737 | -7.820 | -8.211 | -3.910 | -8.993 |
| Neidhardt III, "große Stadt" (1724) | +0.000 | -3.910 | -3.910 | -1.955 | -5.865 | -1.955 | -3.910 | -1.955 | -3.910 | -5.865 | -1.955 | -3.910 |
| Lehman, Bach temperament I (2005) | +0.000 | -0.653 | -3.259 | -0.488 | -6.518 | +1.629 | -2.608 | -1.629 | -0.490 | -4.888 | -0.326 | -4.563 |
| 5-limit just, major and minor on C (Ellis duodene) | +0.000 | +11.731 | +3.910 | +15.641 | -13.686 | -1.955 | -9.776 | +1.955 | +13.686 | -15.641 | +17.596 | -11.731 |
| 5-limit just, Malcolm 1721 (Bb = 16/9) | +0.000 | +11.731 | +3.910 | +15.641 | -13.686 | -1.955 | -9.776 | +1.955 | +13.686 | -15.641 | -3.910 | -11.731 |
| 7-limit just 12 (basis for "barbershop") | +0.000 | +11.731 | +3.910 | -33.129 | -13.686 | -1.955 | -17.488 | +1.955 | +13.686 | +33.129 | -31.174 | -11.731 |

Construction cross-checks (max |archive − construction|, cents):
- Young II construction vs young.scl: 0.0000
- Vallotti construction vs vallotti.scl: 0.0000
- 1/4-comma chain Eb–G# vs meanquar.scl: 0.0000
- 1/6-comma chain Eb–G# vs meansixth.scl: 0.0000
- Werckmeister III (1/4 PC on C–G, G–D, D–A, B–F#) vs werck3.scl: 0.0000
- Kellner (1/5 PC on C–G–D–A–E, B–F#) vs kellner.scl: 0.0000
- Kirnberger III (1/4 SC on C–G–D–A–E, schisma on F#–C#) vs kirnberger.scl: 0.0000
- Pythagorean comma 23.460010 c, syntonic comma 21.506290 c, schisma 1.953721 c

### 5.3 Flags and disagreements

- **Kirnberger II** — the archive's `kirnberger2.scl` spells C♯ = 135/128 (92.179) and
  A♭ = 405/256 (794.134) and approximates A by 270/161 (895.078); the standard table
  (Wikipedia, and the construction "D–A and A–E each ½ SC narrow, schisma on F♯–C♯") has
  C♯ 90.225, G♯ 792.180, A 895.112. **Use the standard row**; keep the archive row as a test that
  the parser reads it, not as the preset. Year: archive says 1774 ("Die Kunst des reinen
  Satzes"); the treatise's first volume is 1771 — **[NOT VERIFIED]**.
- **Kirnberger III** — the archive has two files: `kirnberger.scl` ("letter to Forkel 1779",
  C♯ 90.225, G♯ 792.180) = the standard table; `kirnberger3.scl` ("1744", C♯ 92.179 = 135/128,
  G♯ 794.134) is a different spelling. **Use `kirnberger.scl`'s values.**
- **Young** — the request said "Young (1799)". The archive's `young2.scl` is titled "Thomas
  Young well temperament no.2 (1799)" but its fifths are four ⅙ PC, E–B 1/12 PC and B–F♯ ¼ PC
  (F = 500.000, B = 1092.180) — **not** the well-known Young II. The well-known Young II (⅙ PC on
  C–G–D–A–E–B–F♯, six pure; = "Vallotti shifted one fifth") is archive `young.scl`, whose
  title reads "(1807), also Luigi Malerbi nr.2 (1794)"; Wikipedia dates Young's two
  temperaments to his 1800 *Philosophical Transactions* paper. **Use the Young II row above**
  and name it "Young (1800)" or "Young II"; **[NOT VERIFIED]** which date is the first
  publication.
- **Vallotti** — the row is the *modern* version (⅙ PC). Wikipedia notes Vallotti's original
  used ⅙ **SC** with a schisma left on B♭–F (C differs by 1 c, A-fixed). Ship the modern one
  (what every tuner means), name it "Vallotti".
- **Werckmeister numbering** — "III" and "IV" follow the monochord labels (Werckmeister's
  presentation numbers are I and II); say so in the preset's info text.
- **Neidhardt** — the archive's `neidhardt1..4.scl` are titled I–IV (1724), IV = equal, III
  "'große Stadt'"; it also holds the 1732 samples (`neidhardt-s1..3.scl`) and many variants. The
  literature's naming by village / small town / large town / court ("Dorf", "kleine Stadt",
  "große Stadt", "Hof") is from memory, and Neidhardt's own assignment of names to numbers was
  not checked — **[NOT VERIFIED]**, no second source read. The row uses III "große Stadt".
- **Lehman (2005)** — `lehman1.scl` has five fifths F–C–G–D–A–E narrowed by ⅙ **SC**
  (3.584 c), E–B–F♯–C♯ pure, C♯–G♯ and E♭–B♭ narrowed 1.792 c, G♯–E♭ 1.955 c, B♭–F pure (closes
  to 23.460 c). Lehman's own article/site was not read; **[NOT VERIFIED]** whether he specified
  syntonic or Pythagorean sixths (the archive also has `lehman2.scl` "squiggle II (2005)" and
  `lehman3.scl` "III (2006)").
- **Kellner** — archive: "Herbert Anton Kellner's Bach tuning. 5 1/5 Pyth. comma and 7 pure
  fifths"; the year (1975 or 1977) **[NOT VERIFIED]**.
- **Pythagorean** — the row puts the wolf on G♯–E♭ (chain E♭…G♯). Pianoteq's manual
  (appendix "temperaments construction") builds it from A with exponents
  `[-3 -8 -1 -6 1 -4 3 -2 -7 0 -5 2]` for C…B, i.e. chain D♭…F♯, wolf F♯–D♭ **[V 2026-11-22]** —
  the chain's placement is a design choice; show the wolf's position in the UI.
- **Just intonation** — the duodene (Ellis) serves C major *and* C minor (5/4, 5/3, 15/8 for
  major; 6/5, 8/5, 9/5 for minor). The 7-note Ptolemaic major (`ptolemy.scl`: 9/8 5/4 4/3 3/2
  5/3 15/8) and the natural minor 1, 9/8, 6/5, 4/3, 3/2, 8/5, 9/5 are its subsets. Known
  limit: D = 9/8 makes D–A (5/3 ÷ 9/8 = 40/27, 680.4 c) a wolf in C major's ii chord — the reason
  fixed JI is "per key" and adaptive JI exists.
- **"Barbershop" 7-limit** — barbershop singers tune each dominant-seventh chord 4:5:6:7
  chord by chord; a fixed 12-note table can make only some of them pure. `ji_12a.scl`
  ("7-limit 12-tone scale": 7/6 on E♭, 7/5 on F♯, 12/7 on A, 7/4 on B♭) gives C7 = 1, 5/4, 3/2,
  7/4 pure. A table with G7 pure needs F = 21/16 (470.78 c) instead of 4/3 — our derivation, not
  a cited table. Recommendation: ship "7-limit (C7 pure)" from `ji_12a` as the static preset and
  treat real barbershop as **adaptive tuning** (a later feature; Logic's Hermode is the model,
  5.5).

### 5.4 Rotation to any key, and the anchor

Rotation (our formula): for a 12-value row `T[0..11]` (C = 0) and tonic `k` (0 = C … 11 = B),
the cents of pitch class `p` are `R_k(p) = T[(p − k) mod 12] + 100·k − (k > p ? 1200 : 0)` —
i.e. the temperament's "C" moves to `k`, preserving every interval. Deviations from 12-EDO
rotate the same way: `D_k(p) = D[(p − k) mod 12]`.

Anchor (which note keeps its pitch): Pianoteq leaves **A** unchanged (`ratio = ratio/ratio(10)
% leaving A unchanged`, manual appendix) **[V]**; Logic's Fixed tuning has a **Root Key** and
the Tune slider is "concert pitch A (440 Hz)" **[V]**; `.kbm` names a reference note and
frequency explicitly. Proposal: the project stores `reference_note` (default A4) + `reference_hz`
(default 440) + `tonic` (rotation); A4 sounds at exactly `reference_hz` whatever the
temperament, and the UI says so ("A4 = 415 Hz · Werckmeister III in E♭").

### 5.5 Stretch (piano), for the record

Pianoteq: at stretch 1 "the stretching follows the natural inharmonicity of the strings …
there still is a slight stretching. If you want no stretching at all, then use the flat
temperament"; "Harmonic stretching" ignores inharmonicity; "Transfer the stretching into
Detune note-edit" shows it in cents. Logic: Stretch Upper / Stretch Lower sliders. **[V
2026-11-22]**. (No Railsback data was gathered here — the design note's plan to measure it from
Salamander and real tunings stands.)

---

## 6. What the competitors do

**Ableton Live 12** **[V 2026-11-22]** (manual "Using Tuning Systems",
<https://www.ableton.com/en/live-manual/12/using-tuning-systems/>; Live 12 release notes,
<https://www.ableton.com/en/release-notes/live-12/>). One tuning **per Set**, loaded into the
browser's Tuning section ("As long as the file is loaded in the Tuning section, the tuning will
be saved with the Live Set"); `.scl` and Ableton's own `.ascl` (1.5); **no `.kbm`**, **no
MTS-ESP** mentioned anywhere. Reference pitch = octave + note + frequency ("only audibly affected
by the Ref. Pitch/Freq value"); lowest/highest note range. "All of Live's built-in instruments
are supported", plus "MPE-enabled plug-ins or external Max for Live instruments … provided that
their pitch bend range is set to 48 semitones" (switched on per plug-in from its header's
context menu); "instruments that are not MPE-enabled or use different pitch bend ranges are
likely to play out of tune". Per track: a **Bypass Tuning** toggle; Drum Racks bypass
automatically; a **Track Tuning MIDI Mapper** for controller layouts. The piano roll shows the
tuning's notes and names (NOTE_NAMES), Push's pads follow, 12-note tunings work with Live's
scales and MIDI Tools; existing notes are not re-pitched unless "Retune Set On Loading Tuning
Systems" is on. Strength: whole-workflow integration (editor, Push, scales). Weakness: no
`.kbm`, no MTS-ESP, no per-instrument override beyond bypass, plug-ins only via 48-st MPE.

**Bitwig Studio** **[V 2026-11-22]** (Bitwig "Microtuning in Bitwig Studio", 2020-07-08; Bitwig
6.1 release notes, <https://downloads.bitwig.com/6.1/Release-Notes-6.1.html>). Microtuning is a
**device**, Micro-pitch (Note FX), placed per track: per-pitch-class offsets, presets, Scala
import — "For 12-note scales, all note values will be directly applied", 7-note scales map to
the diatonic notes with the rest interpolated; 6.1's browser "will display any Scala (SCL) files
that describe either seven- or 12-note scales" and dragging one creates a Micro-pitch device.
Reference A3 ≈ 415–466 Hz. It acts through per-note pitch expressions, so Bitwig's own devices,
MPE/CLAP plug-ins and CV hardware follow. **No native MTS-ESP** (the Surge team's "Tuning Note
CLAPs" bridge MTS-ESP into note expressions for Bitwig). Strength: modulatable, per-note
expressions throughout. Weakness: per track, 12/7-note only, no `.kbm`, no project tuning.

**Logic Pro** **[V 2026-11-22]** ("Tuning project settings in Logic Pro for Mac",
<https://support.apple.com/guide/logicpro/tuning-project-settings-lgcp452f2693/mac>, Logic
13.0 at the top of its version list). Project-wide, for "the included software instruments": a **Tune** slider (A = 440 Hz,
cent steps); **Equal Tempered**; **Fixed** (a Type menu of "the most important historic tuning
scales", a **Root Key** C–B, **Copy to User**); **User** (per-semitone cents, root dimmed,
**Export** to `Music/Audio Music Apps/Tuning Tables`, re-listed in the Type menu); **Stretch
Upper / Stretch Lower**; **Hermode Tuning** — adaptive: Classic (3/5-all), Pop/Jazz (3/5/7-all),
Baroque (3/5-adaptive), Depth 0–100 %, and a per-track HMT menu. **No Scala, no MTS-ESP**
documented. Strength: the only DAW with **adaptive** tuning (Hermode) and a key-rotatable
historical set. Weakness: Apple's instruments only; 12-note only.

**FL Studio** — **no project/global microtuning** found in Image-Line's documentation; users
have asked for it on the Image-Line forum (threads "Feature request: Built-in microtuning",
"Microtuning in FL Sampler", 2025). Tuning lives in individual plug-ins: the Scala page lists
**Harmor** and **Sytrus** as reading `.scl` **[V]**, and Scale Workshop exports `.fnv` pitch maps
for them ("IL products can only retune from C0 to C10" — 121 notes; Harmor ±5 octaves, Sytrus ±4,
from its exporter code **[V]**). **[NOT VERIFIED]** that FL 2025's latest build has nothing new
(absence of documentation is not proof). FL is the reference Cantus beats here outright.

**Pianoteq 9** **[V 2026-11-22]** (Modartt user manual, "6. Tuning panel",
<https://www.modartt.com/user_manual?product=pianoteq&lang=en>). Diapason menu; temperaments
Equal, Pythagore, Zarlino, Meantone, Well temperament, Werckmeister III, **Flat** ("octaves ratio
is strictly 2"); "tuning does not follow a pre-computed frequency table (except for the flat
temperament), but takes into account the inharmonicity of the strings, in the same way a piano
tuner does"; Advanced Tuning: **import Scala `.scl` and keyboard mappings `.kbm`**, an extended
layout "for up to 16*128 notes" (channel = octave) for 31/53-EDO; **String tension vs Full
rebuild** when retuning; **Stretch** with Natural vs **Harmonic stretching**, "Transfer the
stretching into Detune note-edit"; **Unison width** and **Unison balance**; Direct sound duration;
a **Note Edit – Detune** per-key editor. MTS-ESP client (ODDSound's list). This is the bar for
Chorda: temperament computed against inharmonicity, per-key detune, unison width/balance,
stretch with a cents readout.

**Surge XT** **[V 2026-11-22]** (Surge tuning guide, <https://surge-synthesizer.github.io/tuning-guide>;
`tuning-library`). Per instance `.scl` + `.kbm` (MIT parser, 1.4), a factory tuning library
("182 SCL/KBM files"), a Tuning Editor (Scala text, Polar wheel, Interval matrix, To Equal,
Rotation, True Keys), non-monotonic scales; **MTS-ESP client** (note-on *and* continuous,
multi-channel) **and source** ("Surge itself can act as MTS-ESP source", since 1.2); "Apply
Tuning After Modulation" for microtonal step sequencing. Strength: the most complete free
implementation and the de-facto reference parser.

What is better than what, and what a professional expects (synthesis, ours):
1. **Scope**: Logic and Live have a project tuning; Bitwig has a track device; FL none. Cantus's
   plan (project tuning + per-instrument override + per-track bypass) is a superset of all four.
2. **Formats**: Pianoteq and Surge read `.scl` + `.kbm`; Live reads `.scl`/`.ascl` without `.kbm`;
   Bitwig only 7/12-note `.scl`; Logic none. Expect: `.scl`, `.kbm`, `.ascl` (names, range), `.tun`
   (AnaMark — MTS-ESP Mini reads it; not researched here), MTS SysEx in and out.
3. **Interop**: no DAW is an MTS-ESP master; Live reaches plug-ins only by 48-st MPE. Cantus as
   MTS-ESP master + CLAP/VST3 tuning expressions + MPE fallback + MTS SysEx would cover every
   plug-in class — a measurable first.
4. **Historical**: Logic's Fixed set with Root Key and Copy-to-User is the workflow to copy and
   extend (more temperaments, A-anchored, beat-rate circle, audition) — the design note already
   goes further.
5. **Adaptive**: only Logic (Hermode). Barbershop and JI-per-chord users expect it; candidate for
   a later round (BACKLOG).
6. **Piano realism**: Pianoteq computes temperaments against inharmonicity and separates
   stretch, unison width/balance and per-key detune — Chorda must match this to be credible.
7. **Editor**: Live's piano roll shows tuning note names, Surge has a tuning editor. Expect note
   names from `.ascl` NOTE_NAMES in Cantus's piano roll, and cents/Hz on hover.

---

## 7. Decisions this research enables (for the round-42 brief)

1. **Licences — OK.** MTS-ESP wrappers 0BSD → a Rust port in an MPL crate (`cantus-rt` for the
   FFI) is fine; libMTS loaded at run time from its system folder, **not bundled**. CLAP (MIT)
   and VST3 SDK interfaces (MIT) fine. Surge `tuning-library` test data MIT → fixtures with a
   notice. Scala archive: **no licence** → never committed; downloaded by tests (Wayback URL,
   SHA-256 `d2218f2f…7b9a`). Surge XT factory library GPL-3 → app only, not MPL fixtures.
2. **Parser rules**: Latin-1 for `.scl`/`.kbm`, strict UTF-8 when `@ABL` lines exist; comments
   anywhere; blank description allowed; count 0 allowed; trailing text ignored; tabs; negative
   cents; ratios below 1; non-monotonic; period ≠ 2/1; `u128` ratios; `.kbm` short tail = `x`,
   size 0 = linear, `x` on the reference note = error.
3. **MTS SysEx**: implement 00/01/02/03/04/05/06/07/08/09 both directions; /16384 decode;
   CA-020 checksum on write, lenient on read; `7F 7F 7F` = no change.
4. **MTS-ESP feasible both ways**: client (read the table on the audio thread; register on the
   main thread; per-instrument override wins, per the header's own step 7) and master (one per
   machine — handle "another master is connected" and the IPC reinitialize case in the UI).
   Cantus would be the **first DAW with a built-in MTS-ESP master**.
5. **Presets**: the §5 tables as the built-in historical set (with the Kirnberger II/III and
   Young choices of 5.3), rotated by tonic, A-anchored by default.

### 7.1 For `docs/beta-checklist.md` (only a person or a device can confirm)

- Install MTS-ESP Mini (free) on Windows, macOS and Linux; load a `.scl` in it; confirm Cantus as
  client follows it, and that Cantus as master retunes Surge XT and Pianoteq (trial) in the
  same session; kill Cantus while master and confirm the reinitialize prompt next start.
- Generate an MTS bulk dump and a scale/octave message with Scala (the program) for
  Werckmeister III; import both into Cantus; compare key by key (0.01 c).
- If anyone owns MTS-capable hardware (e.g. a synth that accepts `08 01`/`08 02`), send Cantus's
  dump and tune by ear/tuner.
- macOS: confirm the signed build loads `/Library/Application Support/MTS-ESP/libMTS.dylib`
  (library-validation entitlement).

### 7.2 Not verified (and why)

- The current MMA/AMEI PDF ("MIDI Tuning Updated Specification") — login required; the 2010
  archived MMA page was used instead.
- Barbour, Lindley, Jorgensen, Lehman's own article — books/articles not read; temperament values
  rest on the Scala archive + construction rules + Wikipedia.
- Kirnberger II year, Kellner year, Young's first-publication date, Neidhardt name/number
  assignment, Lehman's comma (5.3).
- Whether "MTS-ESP" is a registered trademark; ODDSound's terms beyond the 0BSD licence (none
  found).
- Adoption of CLAP `clap.tuning/2` by any shipping plug-in.
- Renoise 3.5's MTS-ESP client support from Renoise's own notes (secondary sources only).
- Live's default REFERENCE_PITCH values (search summary only); FL Studio 2025's latest
  build (no documentation found either way).
- Sequential's tuning `.syx` downloads (links behind JavaScript; format unknown).
- AnaMark `.tun` format (not researched; MTS-ESP Mini reads it — a candidate for round 42's
  import list).

---

## Appendix A — exact contents of more test files

Each file below is shown one line per row as a Python string literal (decoded as
Latin-1, line ends removed; `[CRLF]`/`[LF]` says which ends the file uses, `[no final EOL]`
when the last line has none). Leading/trailing spaces and tabs (`\t`) are exact.

**`vallotti.scl`** — Scala archive v94, 247 bytes, [CRLF]

```
'! vallotti.scl'
'!'
'Vallotti & Young scale (Vallotti version) also known as Tartini-Vallotti (1754)'
' 12'
'!'
' 94.13500'
' 196.09000'
' 298.04500'
' 392.18000'
' 501.95500'
' 592.18000'
' 698.04500'
' 796.09000'
' 894.13500'
' 1000.00000'
' 1090.22500'
' 2/1'
```

**`pyth_12.scl`** — Scala archive v94, 207 bytes, [CRLF]

```
'! pyth_12.scl'
'!'
'12-tone Pythagorean scale                                                       '
' 12'
'!'
' 2187/2048'
' 9/8'
' 32/27'
' 81/64'
' 4/3'
' 729/512'
' 3/2'
' 6561/4096'
' 27/16'
' 16/9'
' 243/128'
' 2/1'
```

**`ji_12.scl`** — Scala archive v94, 153 bytes, [CRLF]

```
'! ji_12.scl'
'!'
'Basic JI with 7-limit tritone. Robert Rich: Geometry'
' 12'
'!'
' 16/15'
' 9/8'
' 6/5'
' 5/4'
' 4/3'
' 7/5'
' 3/2'
' 8/5'
' 5/3'
' 9/5'
' 15/8'
' 2/1'
```

**`fokker_31.scl`** — Scala archive v94, 292 bytes, [CRLF]

```
'! fokker_31.scl'
'!'
"Fokker's 31-tone just system"
' 31'
'!'
' 64/63'
' 135/128'
' 15/14'
' 35/32'
' 9/8'
' 8/7'
' 7/6'
' 135/112'
' 315/256'
' 5/4'
' 9/7'
' 21/16'
' 4/3'
' 175/128'
' 45/32'
' 10/7'
' 35/24'
' 3/2'
' 32/21'
' 14/9'
' 45/28'
' 105/64'
' 5/3'
' 12/7'
' 7/4'
' 16/9'
' 945/512'
' 15/8'
' 40/21'
' 63/32'
' 2/1'
```

**`fj-31tet.scl`** — Scala archive v94, 334 bytes, [CRLF]

```
'! fj-31tet.scl'
'!  '
'Franck Jedrzejewski continued fractions approx. of 31-tet '
' 31'
'!  '
'45/44'
'23/22'
'77/72'
'35/32'
'85/76'
'8/7'
'69/59'
'55/46'
'11/9'
'5/4'
'78/61'
'17/13'
'111/83       '
'93/68'
'7/5   \t     '
'10/7'
'19/13'
'166/111'
'26/17'
'61/39'
'8/5'
'18/11'
'97/58\t\t     '
'65/38\t\t '
'7/4         '
'93/52\t\t'
'64/35'
'43/23'
'44/23'
'88/45'
'2/1'
```

**`bohlen-p_et.scl`** — Scala archive v94, 268 bytes, [CRLF]

```
'! bohlen-p_et.scl'
'!'
'13-tone equal division of 3/1. Bohlen-Pierce equal approximation                '
' 13'
'!'
' 146.30423'
' 292.60846'
' 438.91269'
' 585.21692'
' 731.52115'
' 877.82539'
' 1024.12962'
' 1170.43385'
' 1316.73808'
' 1463.04231'
' 1609.34654'
' 1755.65077'
' 3/1'
```

**`arist_chrominv.scl`** — Scala archive v94, 218 bytes, [CRLF]

```
'! ARIST_CHROMINV.scl'
'!'
"Aristoxenos' Inverted Chromatic, Dorian mode, 18 + 6 + 6 parts                  "
' 7'
'!'
' 300.000 cents'
' 400.000 cents'
' 500.000 cents'
' 700.000 cents'
' 1000.000 cents'
' 1100.000 cents'
' 2/1'
```

**`canou1d.scl`** — Scala archive v94, 278 bytes, [CRLF]

```
'! canou1d.scl'
'!'
"Canou I, Flora Canou's first well temperament, D = 0, single circle edition"
' 12'
'!'
' 102.346257054'
' 195.307485892'
' 304.692514108'
' 395.307485892'
' 502.346257054'
' 600.'
' 697.653742946'
' 804.692514108'
' 895.307485892'
' 1004.692514108'
' 1097.653742956'
' 2/1'
```

**`neidhardt3.scl`** — Scala archive v94, 209 bytes, [CRLF]

```
'! neidhardt3.scl'
'!'
"Neidhardt III temperament (1724) 'große Stadt'"
' 12'
'!'
' 96.09000'
' 196.09000'
' 298.04500'
' 394.13500'
' 4/3'
' 596.09000'
' 698.04500'
' 796.09000'
' 894.13500'
' 998.04500'
' 1096.09000'
' 2/1'
```

**`blackwoo.scl`** — Scala archive v94, 683 bytes, [CRLF] [no final EOL]

```
'! blackwoo.scl'
'!'
'Irregular Blackjack from marvel woo tempering of Cartesian scale below'
'21'
'!'
'35.05180'
'116.23027'
'151.28207'
'232.46054'
'267.51234'
'348.69081'
'383.74261'
'468.20980'
'499.97288'
'584.44007'
'616.20315'
'700.67034'
'732.43342'
'816.90061'
'851.95241'
'933.13088'
'968.18268'
'1049.36115'
'1084.41295'
'1165.59142'
'1200.64322'
'!! blackwootrans.scl'
'!!'
'!Fokblock([34171875/33554432, 128/125], [9, 10]) = symmetric mode of Descartes([15/8, 5/4], [6, 2])'
'! 21'
'!!'
'! 16875/16384'
'! 16/15'
'! 1125/1024'
'! 256/225'
'! 75/64'
'! 4096/3375'
'! 5/4'
'! 675/512'
'! 4/3'
'! 45/32'
'! 64/45'
'! 3/2'
'! 1024/675'
'! 8/5'
'! 3375/2048'
'! 128/75'
'! 225/128'
'! 2048/1125'
'! 15/8'
'! 32768/16875'
'! 2/1'
```

**`12-intune-nodesc.scl`** — Surge tuning-library (MIT), 127 bytes, [LF]

```
'! 12 TET with a blank description'
'!'
''
' 12'
'!'
' 100.0'
' 200.0'
' 300.0'
' 400.0'
' 500.0'
' 600.0'
' 700.0'
' 800.0'
' 900.0'
' 1000.0'
' 1100.0'
' 2/1'
```

**`31edo.scl`** — Surge tuning-library (MIT), 436 bytes, [LF]

```
'! C:\\Users\\green\\Documents\\Music_Production\\Scales\\31edo.scl'
'!'
'31 equal divisions of octave'
' 31'
'!'
' 38.70968'
' 77.41935'
' 116.12903'
' 154.83871'
' 193.54839'
' 232.25806'
' 270.96774'
' 309.67742'
' 348.38710'
' 387.09677'
' 425.80645'
' 464.51613'
' 503.22581'
' 541.93548'
' 580.64516'
' 619.35484'
' 658.06452'
' 696.77419'
' 735.48387'
' 774.19355'
' 812.90323'
' 851.61290'
' 890.32258'
' 929.03226'
' 967.74194'
' 1006.45161'
' 1045.16129'
' 1083.87097'
' 1122.58065'
' 1161.29032'
' 2/1'
```

**`mapping-whitekeys-c261.kbm`** — Surge tuning-library (MIT), 742 bytes, [LF]

```
'! Template for a keyboard mapping'
'!'
'! Size of map. The pattern repeats every so many keys:'
'12'
'! First MIDI note number to retune:'
'0'
'! Last MIDI note number to retune:'
'127'
'! Middle note where the first entry of the mapping is mapped to:'
'60'
'! Reference note for which frequency is given:'
'60'
'! Frequency to tune the above note to (floating point e.g. 440.0):'
'261.625565280'
'! Scale degree to consider as formal octave (determines difference in pitch'
'! between adjacent mapping patterns):'
'12'
'! Mapping.'
'! The numbers represent scale degrees mapped to keys. The first entry is for'
'! the given middle note, the next for subsequent higher keys.'
'! For an unmapped key, put in an "x". At the end, unmapped keys may be left out.'
'0'
'x'
'1'
'x'
'2'
'3'
'x'
'4'
'x'
'5'
'x'
'6'
```

**`empty-note69.kbm`** — Surge tuning-library (MIT), 374 bytes, [LF]

```
'! 61-277-61 Concert C#, Db.kbm'
'!'
'! Size of map:'
'0'
'! First MIDI note number to retune:'
'0'
'! Last MIDI note number to retune:'
'127'
'! Middle note where the first entry in the mapping is mapped to:'
'60'
'! Reference note for which frequency is given:'
'69'
'! Frequency to tune the above note to (floating point e.g. 440.0):'
'452'
'! Scale degree to consider as formal octave:'
'0'
'! Mapping.'
```

