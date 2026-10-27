# ADR 0070 — Crossfades between overlapping clips, clip gain envelopes

**Status:** Accepted · **Date:** 2026-10-26 · **Round:** 31 (editing) · **Extends:** ADR 0004
(commands), ADR 0005 (the audio thread), ADR 0025 (audio clips), ADR 0027 (automation's
shapes), ADR 0058 (comp seams), ADR 0060 (the tempo map), ADR 0061 (warp), ADR 0064
(freeze and bounce)

## Context

BACKLOG §0.4 item 3, *editing*: two audio clips laid over each other on a track played
summed over the overlap — a doubling, a click where one starts — and the only way to
cross them was to draw two fades by hand that never quite matched. Studio One, Cubase and
Reaper cross an overlap the moment an edit makes it. And a clip had one gain for its whole
length: riding a word down, a breath out, a hit up meant automation on the track, which
moves with the track, not with the audio. Every DAW of the reference set has a **clip gain
envelope** (Pro Tools' clip gain line, Studio One's event envelopes, Reaper's take volume
envelope, Cubase's event volume curve).

Round 28 (ADR 0058) already renders a crossfade in the engine: a comp **seam**, where two
playing-lane windows abut, crossed equal-power over one stretch of samples the snapshot
names (`AudioClipSnapshot::{xin, xout}`).

## Decisions

### 1. An overlap's crossfade is the later clip's `AudioRegion::cross`

Two audio clips on a track's playing lane **overlap** when the later one starts after the
earlier one starts and before it ends, and ends at or after it ends
(`cantus_core::crossfade::partner`: of several, the latest starting). With the later
clip's `cross: Option<XfadeCurve>` set they cross over **the whole overlap**; `None` (every
clip before schema 44, and an overlap whose crossfade was taken away) plays the overlap
summed, as it always did, to the bit. A clip lying wholly inside another is layered over
it, never crossed; two clips that meet at an edge are a butt join (or a comp seam), never
an overlap.

Linked copies (ADR 0037) share their region, so they share the curve, as they share
their fades and gain; a copy that overlaps nothing keeps it unused. The overlaps are found
in one sweep over the clips by start (`crossfade::pairs_of_track`), so a track of
thousands of clips costs what sorting them does — in the bus, the snapshot and the
arrangement's drawing alike.

*The crossfade's length is the overlap.* There is nothing in the model to keep in step
with the clips' edges: move either clip and the crossfade follows, lengthen it and the
clips' edges move (§4). *Why not explicit fades on both clips* (Reaper's way)? The same
reason as ADR 0058's: two fades edited apart drift apart, and two fades that only
approximate each other round to different sample counts; one curve rendered from one
stretch cannot disagree with itself.

### 2. Automatic crossfades: made by the edit that makes the overlap, in its step

`CommandBus::set_auto_crossfades(Some(curve))`: after a command that can make an overlap
(`crossfade::may_overlap`: a clip inserted, moved, resized, unmuted, put on the playing
lane, moved to a track, looped — or a batch holding one), the bus compares the overlaps
before and after (`crossfade::pairs`) and gives every **new** one whose later clip has no
crossfade the curve — composed into the same command and inverse (`apply_fix`, as the
tempo's window fix), so undo takes both back and redo brings both. An overlap that was
already there keeps what it has: a crossfade taken away stays away while the clips stay
put. The application sets the bus from *Settings ▸ Editing ▸ Crossfades ▸ Automatic* (on
by default; `Settings::auto_crossfade`, `crossfade_curve`) in `CantusApp::assemble` and in
an app built from parts (`apply_editing_settings`); a bare bus has them off, so the core's
own tests see the edit alone.

### 3. One engine path with comp seams

The snapshot (`snapshot::crossfades`, after `seams`) tells the earlier clip's pieces that
reach into the overlap `xout = (from, to)` and the later clip's `xin`, with
`xin_curve`/`xout_curve`; the engine's `add_clips` multiplies by
`XfadeCurve::gain_in/gain_out(u)` at `u = (s − from + ½)/n` over **the very same samples**.
Neither window moves (an overlap already is one; a seam overlaps its two windows by its
length). A seam is equal power, and `XfadeCurve::EqualPower` is `sin(π/2·u)` and
`cos(π/2·u)` exactly as ADR 0058 wrote them: every seam renders to the bit as before
(golden hashes unchanged, `DSP_VERSION` stays). The fade a crossfade covers (the earlier
clip's fade out, the later's fade in) gives way to it, in the engine and in the drawing.

### 4. Curves and length

`XfadeCurve`: **equal power** (the default: two different recordings stay level across it),
**linear** and **S** (`3u² − 2u³`), whose halves sum to one (two takes of the same thing,
correlated). The length is edited by the overlap's square (drag sideways: longer or shorter
about its centre) and by the crossfade editor (double-click the square: the curve drawn, a
segmented *No crossfade · Equal power · Linear · S-curve*, *Length* in ms). Both go
through `crossfade::resize`: the later clip's start (content kept in place) and the
earlier clip's end move, each only as far as its file has audio (`crossfade::room`) and
never past its other edge; one undo step per gesture (`cmd.clipfx.xfade_length`).

### 5. A clip gain envelope in the clip's content time

`AudioRegion::gain_env: Option<Box<GainEnvelope>>` (`cantus_core::clip_gain`): points
(content tick, dB from −60 — silence — to +12, and automation's shape and tension to the
next point, ADR 0027), at most 4 096, in order; `SetClipAudio`, `InsertClip` and every
command that validates a clip refuse anything else. Content time — the ticks of
`Clip::offset`, tick 0 the file's first frame — makes the line belong to the audio it
shapes:

- **slip** moves the audio and its line together; **trim** hides points and a trim back
  shows them again; **split** gives both halves the same line (each hears its own
  stretch); **glue** keeps one line when the pieces share it, or joins each piece's line
  over the content it showed (`GainEnvelope::glued`, a jump at a join where two lines
  differ); **reverse** plays the line mirrored with its audio — the engine mirrors it in
  samples across the window, where it mirrors the audio; a **warp** keeps the line where
  it is drawn while markers stretch the audio under it (the line is in the clip's content
  time, which a warp maps to the source, as Ableton's clip envelopes are); *bounce the
  warp* moves the line onto the new file's content time.
- The envelope applies on top of `gain_db` and the fades. A line at 0 dB everywhere (or no
  points) is no envelope: the snapshot drops it (`is_flat`), and the clip plays as before
  to the bit.
- **MIDI clips have none**: their notes' velocity (and round 31's CC lanes, ADR 0068) is
  their gain.

### 6. Sample-accurate in the engine, allocation-free

`cantus_engine::clip_env::EnvRead`: each point lands on the transport sample its timeline
tick rounds to (`start − offset + tick`, through the tempo map), and between two points the
value is the shape at the sample's own fraction of the way — the same at any block size,
live and offline. A cursor over the points keeps its segment's two edges in samples (one
bisection per block, a step per crossing), and a level stretch costs no `exp`. dB to gain
by `cantus_dsp::math::db_to_gain`; the floor is exact silence. The line itself is the
smoothing: it is continuous wherever its points are apart, and two points at one tick are a
jump the user drew.

### 7. In the arrangement

The crossfade is drawn on the later clip (it lies over the earlier there), as Studio One
draws it: the overlap veiled, the earlier clip's fade out falling and the later's fade in
rising on the curve, one square at the top (hollow, with dashed edges, for a summed
overlap). Right-click the square: *No crossfade* or a curve, *Crossfade editor…*. The clip's
menu has *Crossfade the overlap* (the selection's, when the clip is in it). The timeline's
menu has *Automatic crossfades* and *Show clip gain envelopes*.

With the envelopes shown (`Settings::show_clip_gain`), every audio clip draws its line on
its own scale — +12 dB at the top, 0 dB a quarter down, a square-root scale to −∞ at the
bottom, so 0 to −20 dB have most of the height — and the waveform is drawn with the line,
the fades and the crossfades applied. On a hovered or selected clip: a press on the line
adds a point and drags it; points drag (Shift: a tenth), double-click deletes one; each
bending segment has a handle half-way along (drag to bend, double-click to straighten);
right-click a point or the line for its segment's shape (automation's six), *Delete point*,
*Reset gain envelope*. Each gesture is one undo step.

### 8. Freeze, bounce, stems, export

All render the snapshot, so all hear the crossfades and envelopes. A bounce of chosen clips
that takes one clip of a crossed overlap and not the other is refused (*choose both, a time
range, or the whole track* — `clipfx.bounce_overlap`), and a time range's edges move out
past an overlap's crossfade, as for a comp seam.

### 9. DAWproject

A crossed overlap is written as DAWproject says a crossfade: the later clip's `fadeInTime`
and the earlier clip's `fadeOutTime`, each the overlap's length (a curve other than linear
is reported: *they come back linear*). Read back, two overlapping audio clips whose fades
both span the overlap become a linear crossfade. A gain envelope is the clip's
gain expression: the clip's content becomes `<Lanes>` holding its `<Warps>`/`<Audio>` and a
`<Points unit="linear">` with `<Target expression="gain"/>`, times in content beats, linear
or held between points (a curved segment is reported: *they come back straight*); read
back from either linear gain or `decibel`, in beats or seconds. A clip's plain gain
(`gain_db`) is still reported as DAWproject has no word for it.

### 10. Schema 44

`AudioRegion::{cross, gain_env}`, both `#[serde(default, skip_serializing_if = …)]`. The
migration writes nothing (an older file has neither); `fixtures/v44.cantus` holds a track of
two clips crossing on an S-curve, the first with an envelope (a dip, a hold, a bend).

## Consequences

- New: `cantus_core::{crossfade, clip_gain}`, `AudioClipSnapshot::{xin_curve, xout_curve,
  gain_env}`, `CommandBus::{set_auto_crossfades, auto_crossfades}`,
  `cantus_engine::clip_env`, `cantus_ui::clipfx`, the views
  `arrangement_audio_clipfx{,_edit}.rs`, Settings ▸ Editing, the `clipfx` strings.
- No new command: a crossfade and an envelope are an `AudioRegion`'s, set by `SetClipAudio`
  (labelled `cmd.clipfx.*` in batches); the undo fuzz draws them and runs every other
  sequence with automatic crossfades on.
- Tests: the engine's crossfade on every curve at every sample of the overlap (each half
  the curve's at the same `u`), an overlap without one summed to the bit, block sizes 1 …
  1 000 and the live callback at tolerance 0, the stems' master to the bit, no envelope or a
  flat one to the bit, an envelope landing on its points, carried by a slip, mirrored on a
  reversed clip, the floor silent, no allocation on the audio thread (serial and pooled);
  the core's automatic crossfade (one step, undone and redone, off on a bare bus, a removed
  one staying removed), the snapshot's stretches, validation, split; the application's
  gestures and menus from the real arrangement, the setting from boot, freeze and bounce;
  schema 44; DAWproject both ways.


## Amended by review 1 (round 31)

- **A split's pieces are one window.** `crossfade::overlaps_of_track` joins the pieces of a
  split (`continues`: one file, the content going on, neither looping) before applying
  §1's rule, so a split inside a crossed overlap plays exactly as before: the earlier
  window's second half still fades out under the later one, the later window's first
  half still fades in (to the bit, every curve). `Overlap` names both windows' pieces,
  the piece that ends it (`left_last`) and the one that starts it and carries the curve
  (`right_first`); `partner` and `pairs_of_track` are gone.
- **The line stays on its audio through a tempo track.** A move, a copy, a paste or a
  slip that re-derives a window's offset in seconds (`audio_time::relocate`, `slipped`)
  maps the envelope's points through the same seconds (`audio_time::remap_env`); a
  paste between projects of different tempo resolution scales them with the content.
- **A damaged envelope never reaches the audio thread.** `clip_gain::repair` on load
  (values not numbers dropped, values and tensions clamped, points sorted, at most 4 096),
  counted in the load report (`gain_envs_repaired`, said in the status line).
- **On a looping window the line drawn is the line heard**: the arrangement reads and
  draws the envelope per repetition, each from the clip's offset as the engine plays it
  (`clipfx::content_at`, `window_ats`); a point added in any repetition sounds in every one.
- **A crossfade's length gesture resizes from where it began** (`clipfx::XfadeBase`): a
  drag of the square or a turn of the editor's knob gives the same clips whatever the
  number of frames. The editor's range is what the clips' audio allows, not a fixed 4 s.
- **A stale curve goes.** The bus's fix runs on every edit that can make or end an
  overlap: with automatic crossfades on, new overlaps are crossed; whatever the setting, a
  clip that leaves its overlap loses its curve in the same step (a linked copy keeps it:
  its siblings share it). A copy (`clipboard::copy_of`) carries none.
- **The overlap's square keeps off the clip's fade squares** (and its gain square).
- **Costs**: the snapshot, the arrangement's drawing cache and the DAWproject export each
  find a track's overlaps once and look each clip up by id; tests hold 20 000 clips to one
  sweep and a snapshot of 5 000 crossed clips to well under a second.
