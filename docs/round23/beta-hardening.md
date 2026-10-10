# Round 23 — beta hardening: the export dialog, text after a theme change, CPU tests

Branch: **`r23/beta-hardening`** (worktree `daw-r23/beta-hardening`, base master 2ad043b).
Decision record: ADR 0035. The fourth blocker of the round, copy and paste, was built on
`r23/clipboard` and merged here (`docs/round23/clipboard.md`, ADR 0036).

How the round was run: a read-only audit first (a workflow of two export auditors —
format/bits/dither/rate, and range/tail/loudness/stems/name/progress — each finding
checked by two skeptics trying to refute it; a theme-text investigator; a CPU-test
surveyor; a clipboard designer), then tests that reproduce each finding, then the fixes.
All 27 export findings survived the skeptics (54 votes, none refuted).

## 1. The export dialog (BACKLOG §1)

### What was broken, and what each one was

| # | Option | Finding | Fixed by |
|---|---|---|---|
| 1 | tail, quality, rate, LUFS target, ceiling | **Every list opened under the dialog and could not be clicked**: the dialog moves itself to the top of the foreground every frame, and egui's popups are foreground areas that ask to be on top only on their first frame | each list is a *sublayer* of the dialog (`ctx.set_sublayer`), drawn and hit just above it |
| 2 | keys | Esc on an open list closed the dialog too | a list open at the start of the frame takes Esc |
| 3 | keys | the window's shortcuts acted behind the dialog: Esc cleared the time selection, Delete/Ctrl+Z hit the project, Ctrl+E reset the choices; Enter did nothing | `global_shortcuts` stands aside while the dialog is open; Enter exports; Ctrl+E keeps the open dialog |
| 4 | range | Selection with no selection snapped back to Song with a one-frame warning | the segment stays unchosen and the note stays until the dialog closes |
| 5 | range / tail | **a selection's tail played the music after the selection** | the transport stops at the range's end; the tail only rings out (ADR 0035 §1) |
| 6 | range | no Loop range | `Scope::Loop`, the loop brace |
| 7 | range | the note named the bar after the selection ("Bars 1–5" for four bars) | the last bar is the one holding the last tick |
| 8 | range | **a latent project made the mix late by L and cut its last L samples**; it differed from its `00 Master` stem | `RenderOptions::compensate_latency`, on for exports (ADR 0035 §2) |
| 9 | progress | the bar pulsed through the mix render and Stop waited for the whole render | `render_with_progress` with `Control::render`; the bar fills, Stop stops at the next block |
| 10 | progress | the status line never showed progress | the phase and the percentage, updated as it moves |
| 11 | stems | **hosted plug-ins were wires and hosted instruments silent in stems** | the stems take the hosted instances (ADR 0035 §3) |
| 12 | stems | a stopped re-export into the same folder deleted the earlier stems | every stem written whole before any is put in place |
| 13 | stems | replaced same-named files without asking | a confirmation lists them first |
| 14 | stems | a one-file stems export read like a mix | the stems summary always (and "1 stem") |
| 15 | name | a name typed with another extension got the chosen format's bytes (MP3 in a `.wav`) | the extension follows the format (or is added) |
| 16 | rate | **FLAC at 192 kHz failed** (flacenc takes 96 kHz at most) — found by the new per-option tests | FLAC is written at 96 / 88.2 kHz, the rate list offers what the format carries, the dialog says so |
| 17 | loudness | a missed target was not mentioned | the summary says it and why |
| 18 | bits | overs clipped in integer and lossy files without a word | the summary counts the clipped samples |
| 19 | presets | the stems card stayed lit for tracks / through the master | the stems choices are part of the match |
| 20 | presets | hidden loudness fields kept a card dark | only shown choices are compared |
| 21 | MP3 | switching CBR/VBR forgot the bitrate and level | each mode keeps its last choice |
| 22 | open | *Export audio…* after *Export stems…* opened on stems | it opens on the mix |
| 23 | errors | every error but "empty" was English in Spanish | one key per error, the system's reason inside |
| 24 | stems | the old stems dialog in `views::interop`, unreachable since round 22 | removed (with its strings, test and shot) |
| — | Ogg | a decoder (lewton) hands back the whole last block | not a bug: the file's last granule position is exact; the tests read the length from it |

Kept as they were, on purpose: the stems' float scratch folder stays beside the
destination (same volume, no copy across disks; a crash can leave it); a hosted insert on
the master is a wire in the *through the master* copies (one instance cannot be in two
places — ADR 0035 §3).

### The tests

- `crates/cantus-ui/tests/export_options.rs` (17): every control clicked through egui's
  input — the dialog publishes where it drew each one (`export_dialog::geo`) — and the
  keyboard rules. Before the fix, six failed: every list and Esc.
- `crates/cantus-ui/tests/export_files.rs` (12): through the application, the written
  file against the dialog: the song is the engine's render to the bit at three tails, the
  selection and the loop, rate × channels × depth, loudness and its summary, the
  extension, the stems summary, the bar during the render and the status line, errors in
  Spanish.
- `crates/cantus-export/tests/options.rs` (10): every format × rate (session and the five
  offered) × channels, every depth, every MP3 bitrate (every frame header), every VBR
  level, every Ogg quality, every dither choice (and that it touches nothing it does not
  apply to), every preset — read back by hound, claxon, symphonia and lewton.
- `crates/cantus-export/tests/jobs.rs` (+4): the bar through the render and a stop in it;
  the render's end reported; a stopped stems re-export keeps the earlier stems.
- `crates/cantus-engine/tests/render_range.rs` (5) and
  `crates/cantus-host/tests/stems_hosted.rs` (2): ADR 0035's tests.
- `examples/shoot_export.rs`: from five kinds to eleven — the rate list open over the
  dialog (clicked through `raster::shoot_events`, new), the selection, the loop, a missing
  range, stems of tracks through the master, a finished export — in both themes.

### The review

The finished branch went to three adversarial reviewers (engine and render; the export in
the real app; the theme fix and the tests' quality), each finding checked by a skeptic:
13 of 14 real, all fixed with a test seen failing first —

- **the stop frame** was computed in seconds, the transport in ticks per sample: at
  44.1 kHz and 140 bpm a three-bar selection stopped one sample late and a note on its
  end tick fired (`transport::sample_at` is now shared; tested at five tempo/rate pairs);
- **Enter confirming a tag** started the export (egui's single-line field lets go of the
  focus during the frame; "typing" is now read before anything is drawn);
- **the stems commit** is all or none (a destination held open by a player used to leave
  half the folder new); a mix's corrected extension asks before replacing;
- **the settings file**: Loop is stored as the song (older builds cannot read it and would
  reset every setting), and `Settings::load` keeps every field it can read;
- menus left open behind the dialog are closed; progress waits six seconds behind an
  error; held typing-piano notes are released;
- five tests made able to fail: the selection and loop tests now use music that sounds
  (and checks the range's position and length), the shortcuts test checks the bus
  revision after each key and shows Delete working with the dialog closed, the Esc test
  checks the list closed and that a second Esc closes the dialog, the stress test holds
  the cold first frame to its own ceiling; and the FLAC/MP3 rate policy in numbers.

## 2. Text after a theme change (BACKLOG §1)

**The cause.** A dark↔light switch changes egui's text options (its colour transfer
function), so egui rebuilds its font atlas and every glyph moves. The kit keeps laid-out
text across frames (`KitCache`) and guessed a rebuild from a drop in the atlas's fill
ratio — but on a desktop GPU the atlas is 16 384 texels wide, a session's glyphs sit in
its first row, and a fresh atlas has the same ratio. So the cached labels kept pointing
at where the glyphs used to be: blank, or other glyphs. "Sometimes", because it needs a
dark↔light pair (or the OS flipping it under *System*) early in a session.

**The test** (`crates/cantus-ui/tests/theme_roundtrip.rs`): every kit text role drawn on
one context with an atlas as wide as a GPU's; switched into Paper it must equal starting
in Paper, and Slate → Paper → Slate must equal Slate, with a zoom, a language and every
theme in between; then the real transport and arrangement switched mid-frame through
`apply_appearance` (as the Appearance page does), three round trips, one with Spanish in
between. Before the fix: 4 729 pixels differed — every label blank.

**The fix** (`kit/cache.rs`): the cache watches what egui rebuilds on — the text options
and the font faces — besides the fill ratio, and counts the rebuilds it saw
(`Stats::font_rebuilds`). With it: `theme::apply` asks for a repaint (a mid-frame switch
no longer leaves a half-old frame on screen), and keeps the kit's fader cap, which every
switch used to lose.

## 3. CPU tests robust to a loaded machine

`inst_bass::a_cpu_budget_holds` and Calor's `cpu_per_voice_at_each_quality` timed one run;
both now take **the best of five windows** of the same audio (the house rule of Lamina's
and the organ's budgets: preemption can only add time, so the minimum is the instrument's
own cost — a median does not help when the load lasts the whole run). The same for the
next most fragile: the sampler's sixteen-voice budget (1.6× margin under load), the
stress project's frames (now the fastest frame is held to 250 ms; the slowest flaked at
266 ms before), and the hot-plug watch's 50 ms bounds (150 ms, still well under the
300 ms scan it proves nothing waits for).

## Left

- A real-device check of the export with hosted plug-ins in stems (the test plug-ins
  prove the path; a commercial plug-in has not been tried).
- The scratch folder of a stems export could move to the temp folder when it is on the
  same volume.
- ADR 0035 §3: a hosted insert on the master in *through the master* stems.
