# ADR 0058 — Comping: seams that crossfade, alternate comps, grouped comping

**Status:** accepted (round 28) · **Date:** 2026-10-15 · **Extends:** ADR 0025 §5 (take
lanes and comping), 0004 (commands), 0054 (pitch edits on audio clips)

## Context

Round 19 made comping: takes on lanes above 0, the comp as lane-0 windows onto the takes
(`ContentKey`), swipe / promote / delete unused / explode. Against Logic's and Studio One's
comping it lacked crossfades at the seams (butt joins), boundaries edited as a unit, quick
swipe as a mode, alternate comps, flatten and merge, and grouped comping. A pitch review
of this round also found that a swipe which grew a window dropped that window's pitch
edits.

## Decisions

### 1. A seam's crossfade is the right window's `AudioRegion::xfade`

What plays is still lane 0. A **seam** is where two playing-lane audio windows of a track
meet (the left one's end is the right one's start, neither reversed, neither looping); its
crossfade length is the right window's `xfade` (ticks, 0…a beat — refused beyond, in
`validate_clip` and `SetClipAudio`). The model stays a butt join — round 19's windows,
`ContentKey`s and lit ranges are unchanged — and the **crossfade is rendered by the
engine**: the snapshot (`snapshot::seams`) overlaps the two windows by the length,
centred on the seam (each reads on into its own source, the right window's offset moved
with its start so its content stays put), and tells both the stretch `(from, to)`; in
`processor_rec::add_clips` the left fades out with `cos`, the right in with `sin` of
`π/2 · (s − from + ½)/n` over **the very same samples** (both round the same two ticks),
so the power is constant at every sample. A window with a crossfade but nothing ending at
its start has no seam: the length waits, unused. Zero is the old butt join, to the bit.

*Why not an overlap with explicit fades in the model?* Every clip edit would have to keep
two fades and an overlap consistent, round 19's windows would stop abutting, and two fades
that merely approximate each other round to different sample counts. One number on one
window, rendered from one stretch, cannot disagree with itself.

New seams (a swipe's two ends) get the project's default (`RecordSettings::comp_xfade_ms`,
10 ms) unless they already carry one; a seam's menu sets 0, 5, 10, 20 or 50 ms, for it or
for every seam of the track.

### 2. A seam moves as a unit — enforced by its command

`Command::SetCompSeam { track, left, right, at }` moves where `left` ends and `right`
begins to `at`: the left's length, the right's start, length and offset together, the
clips re-sorted as every clip edit keeps them; the inverse moves it back. `apply` refuses
what is not a seam (two windows that do not meet, another lane, a reversed window) and a
position outside the two. A drag merges (it supersedes). The arrangement moves seams from a
handle at the foot of the playing lane, and a window's edge trimmed at a seam moves the
seam (`comping::seam_trim`).

### 3. Swipes keep what the comp carries

A swipe that grows a window takes the absorbed window's **pitch edits** (ADR 0054), gain
and crossfade onto the grown window — the comp's edits are the comp's, not the take's.
Split and glue already keep `AudioRegion::pitch` (a piece clones the region).

### 4. Alternate comps: kept whole, swapped in one step

`Track::comps: Vec<Comp { name, windows }>` and `Track::active_comp`. The playing comp's
windows are lane 0's (its `windows` empty — enforced); every other keeps its windows
**whole** (ids, pitch edits, fades, crossfades). `Command::SetTrackComps` replaces the list
(checked: at most 26, the playing one present, kept windows lane-0 clips whose ids nothing
else uses, their sources in the pool). Switching is the playing windows removed, the list
replaced, the other's windows inserted — one batch, one step, and back gives exactly what
was. *New comp* copies the playing one (the copy is played; the original keeps copies under
fresh ids), named by the next letter; rename and delete (not the playing one; the last one
left is the plain lane again). A source a kept window plays cannot leave the pool
(`RemoveAudioSource` and *delete unused takes* see the kept windows).

**Flatten**: the takes and the other comps go, the windows stay, the lanes close, sources
nothing plays leave the pool (files stay on disk). **Flatten and merge**: the comp —
windows, seams and pitch edits as they play — rendered offline (the windows alone, to the
master at unity) into one recording beside the takes, which becomes the track's one clip;
then flatten. Each one step.

### 5. Grouped comping: the mixer's groups

Tracks whose strips share a **mixer group** (`MixerStrip::group`, ADR 0016) comp together:
a swipe (or a promote) on one swipes the same range of the same lane on each — what a drum
kit recorded together needs, since a pass lands on the same lane on every track it
recorded. `RecordSettings::comp_grouped` (on) switches it off.

### 6. Quick swipe

`RecordSettings::quick_swipe` (on): a drag across a take lane comps (round 19). Off
(Logic's switch), the drag selects time and a click selects the take clip, so takes can be
edited on their lanes.

## Consequences

- Schema **29**: `AudioRegion::xfade`, `Track::comps`, `Track::active_comp`,
  `RecordSettings::{comp_xfade_ms, quick_swipe, comp_grouped, record_automation}`; the
  migration writes nothing (an older file butt-joins, has one comp, the defaults of every
  earlier version); `fixtures/v29.cantus` holds a crossfaded seam, two comps (B keeping a
  window with pitch edits), the record options' switches.
- New: `cantus_core::comp`, `Command::{SetCompSeam, SetTrackComps}`,
  `AudioClipSnapshot::{xin, xout}`; `cantus_ui::comping::{seams, seam_trim,
  set_seam_xfade, comp_group, comp_windows, switch_comp, duplicate_comp, delete_comp,
  rename_comp, flatten}`, `CantusApp::{move_comp_seam, merge_comp, …}`,
  `views::arrangement::comp` (seams, take numbers, the comp's chip and menu).
- Tests: the engine's crossfade equal-power at every sample of the stretch, centred, the
  butt join to the bit at 0, block sizes 1…1 000 and live = offline at tolerance 0; the
  core's seam command and its refusals, the comps' checks, the fuzz arm; the
  application's swipes (default crossfades, pitch kept), seams by hand from the real
  arrangement, comps switched and back whole, flatten, merge (a file on disk), grouped
  comping, quick swipe; schema 29.
