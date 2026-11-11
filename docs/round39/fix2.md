# Round 39 close-out — builder `fix2`: the integrated tree's defects

Branch `r39/fix2` (worktree `F:\projects\daw-r39\fix2`), from `r39/integration` at
`d6a81531` (every round 39 branch but `themes`). Four defects of the integrated tree, each
fixed at its cause with a test that fails without the fix, and the coordinator's finding on
the mixer's fold (README hero, `docs/images/window-night.png`).

## 1. Undo did not take back a track's icon (`arrange_e2e`, line 383)

**Cause.** Not the icon's undo: lo-mixer's merge (`fa769a57`, the desk fills its width with
FL's next empty strips, and a click on one makes the strips up to it in one undo step). The
test closes the icon picker with a click away at (1000, 700); in the 1920 × 1080 window that
point is now one of the desk's painted empty strips, so the click that closed the picker
*also* made 13 strips (16 → 29), and `Undo` took those back instead of the icon. The icon's
own undo was always right.

**Which was wrong, the test's click or the app?** The app: a click whose purpose is to close
a popover must not act on what lies beneath it. The instrument picker already works that way
(its scrim takes the click); the icon picker let the click through, so dismissing it over the
desk made strips, over a fader moved it, over a mute muted. Fix
(`views/track_glyphs.rs` `show`): a catcher over the window, under the popover
(`Order::Middle`, the popover is `Foreground`), with `click_and_drag` sense, while the picker
is open — the click away closes the picker and lands on nothing. The empty strips' own click
(make strips up to it) stays as lo-mixer designed it.

**Test.** `arrange_e2e::the_arrangements_structure_end_to_end` now also holds that the click
away leaves the project's strips as they were (it failed with 29 ≠ 16 before the catcher),
that the strip head shows the star (`MixerView::head_icon`) before Undo, and that after Undo
neither the track header (`track_icon`) nor the strip head draws it. The picker's *Recent*
row is the user's history of picks (settings, not the project), so Undo rightly leaves it.

## 2. The mixer folded strips where there was room (the coordinator's finding)

lo-mixer gave the overflow's pill ("n ›") a 36-pt slot each side of a scrolling middle, so
the pill never covers a strip's meter — but **both** slots always, even at the start where
nothing is hidden on the left. That empty left slot sat between the master and strip 1 and
took a strip's room: at 1280 × 720 in Wide (the window golden) the desk folded three strips
into "3 ›" where it had folded two; the returns docked right then floated off the desk's edge
with the leftover beyond them (the "empty well").

Fix (`views/mixer.rs` `geometry`, `views/mixer_paint.rs` `overflow`): a slot only on a side
with strips hidden beyond it (`Geo::slot_l`/`slot_r`, widths, 0 when none):
- at the start, no left slot: the middle begins where it begins on a desk that fits;
- the right slot takes what is left of the room after the whole strips (between 36 pt and
  36 pt + one strip), the pill centred in it, so the right dock stays at the desk's edge and
  the room beside the pill can never hold another strip;
- paged to the end, no right slot: the middle runs to the dock (the strip cut at its left edge
  sits under the fade, beside the left pill);
- a selection followed from elsewhere settles with the slot its scroll brings (the middle is
  narrower once scrolled; `follow_selection` lays out again up to three times).

**Is "3 ›" at 1280 Wide right?** Measured on the render: the middle has about 715 pt; ten
strips take 620, an eleventh would need 682 + the pill's 36 = 718. So ten strips and "3 ›" is
right at that width — the golden's eleventh strip had the pill painted over its meter, the very defect lo-mixer
fixed. The difference from before is where the leftover goes: beside the pill, not before
strip 1 and after the returns. *(Superseded by §5: with the slot fitted to its count, 30 pt
for one digit, the eleventh strip fits and the desk shows "2 ›".)*

**README hero (1920, Large, pitch 96).** Measured on the reshot `docs/images/window-night.png`:
the middle has about 958 pt (strip 1 at x 426, the returns from 1392): nine strips take 864,
and a tenth would need 960 even with no pill at all, so "2 ›" (Rise and the Drum bus) is right
there. What changed is the look: no empty 36 pt before strip 1, the "2 ›" in the room left
beside Choir, the returns at the desk's edge (no empty well after Delay).

**Test.** `mixer_desk::the_desk_folds_only_what_does_not_fit` — at 1920 Large, 1280 Wide,
1280 Compact and 1600 Wide, a desk of 96 strips: at the start no left pill, the middle starts
where a fitting desk's does (fails before: 36 pt later), whole strips, less than a strip's room
beside the pill, the returns at the desk's edge; paged to the end, no right pill, the left one
shown, no gap before the dock. lo-mixer's `the_overflow_pill_never_covers_a_strip` and
`the_selected_strip_is_scrolled_into_view` still hold.

## 3. A bus fed only by strips counted as a free strip (`Project::strip_is_free`)

**Cause.** The rule (r37:fixes) said a strip is free when no track plays into it — through
its own output or a pad's — and no pad's chain or strip plays into it. A bus fed only by
other strips (the house song's drum bus: Kick, Clap, Hats and Percussion leave into it, no
track plays into it directly) passed, so the next new track took it: in the house demo the
dancer (added last) landed on the drum bus, renamed it "Dancer" (`Song::part` names a part's
strip), and the four drum strips read "→ Dancer". Any track a user added to the start-up song
did the same.

**What the function intends** (its doc, and every path that makes tracks: a new track, a
sample on the rack, imported audio or MIDI, pitch and transcription tracks): the next unused
channel of the desk, as FL hands out its next free insert. Fix (`cantus-core/src/model.rs`):
a strip is free only when
- it is an audio strip, and no track plays into it (as before);
- **nothing routes into it** — no strip's output, no send, no effect keyed from it (a
  sidechain's key is in use);
- **nothing was set on it** — it equals `MixerStrip::fresh` (new: the one constructor
  `Project::make_strip` now uses) in all but its id and its dock: no name, colour or group,
  no inserts, EQ or modulators, at unity, centred, not muted or soloed, to the master with no
  sends, under no VCA, not frozen.

No GPL code; no command changed, so the undo fuzz is untouched (green). The house demo's
own doc (`build_without_dancer`) no longer describes the old behaviour as a given.

**Tests.** `cantus-core` `model::tests::a_bus_fed_by_strips_or_a_dressed_strip_is_not_free`
(a bus, the strip routed into it, a send's destination, a sidechain's key, a strip with an
insert, and each of eight settings alone; a docked fresh strip stays free).
End to end, `crates/cantus-ui/tests/free_strips_demo.rs`:
`a_track_added_to_the_startup_song_does_not_take_the_drum_bus` (through
`CantusApp::assemble` and *Add track* three times: no new track on a strip other strips
route into, the bus keeps its name) and `no_demo_puts_a_track_on_a_bus` (every demo song;
the whole house song's dancer on a strip of its own, the four drum strips still into "Drum
bus"). The dancer is silent, so no demo's audio changes; no golden audio hash moved.

## 4. Slate's window golden (`slate_window_golden`)

The images were blessed by lapis2 at `034a1ac0` (on `4c52b48b`), before lo-mixer's last merge,
shots' fixes and themes. Every pixel of difference was rendered and looked at (diff images
and before/after side by side, three renders: the old goldens, fix2 before `themes`, fix2
after merging `r39/integration` at `9c973bc5`):

- **Arrangement, Night and Paper** — the docked mixer's foot right of the returns: lo-mixer's
  next empty strips (16–27, numbered, a groove and a ghost cap each) painted to the panel's
  edge (`fa769a57`). Then themes: the bar lines at 3:1 and the beat lines at 1.35:1 on the
  lanes (Night: 438 pixels, every one a grid line — (43,51,59) → (97,115,133) and
  (33,40,46) → (50,61,70)); in Paper also the headers' track stripes darker (3:1) and the clip
  bodies with more of their colour under half the veil.
- **Mixer (maximised, Wide), Night and Paper** — at `d6a81531` the whole middle shifted 36 pt
  right (lo-mixer's empty left slot) and strip 11 folded. With item 2's fix strips 1–10 are
  where the old image has them, to the pixel; what differs is right of strip 10: strip 11
  folded into "3 ›" in the pill's slot (it does not fit, item 2), the returns at the desk's
  edge, the scrollbar's thumb. Themes changed nothing in this window (identical before and
  after `themes`, Night and Paper): no strip is soloed, and the desk's faders are drawn by the
  mixer's own look, not `fader_slate`.
- **Piano roll, Night and Paper** — its window shows the arrangement's lanes above the roll:
  only themes' grid lines (and in Paper the clip bodies and stripes), as in the arrangement.
- shots' menus: no menu is open in these windows, so no pixel.

**Slate Night's colours did not otherwise move.** Re-blessed with `CANTUS_BLESS=1` on the
merged tree; the test's doc says what the images are (`r39/fix2` at `7edb9fb1`) and lists
the intended changes since `4c52b48b`.

**A finding for themes (not changed here).** themes.md says Slate Paper's mixer strips'
colour bands were taken to 3:1 (`kit::color::track_mark`), but the desk's strip head draws
its band from `theme::track_color` directly (`views/mixer_strip.rs`, `hue`); `strip_color`
(`track_mark`) feeds only the cables. Measured in the blessed Paper mixer, the head bands of
the demo's strips stand at 1.6–2.5:1 on their strips (Hats' yellow 1.57:1). Whether to take
the desk's bands through `track_mark` is a look change for Slate Paper; I left it to the
coordinator.

## The pictures

`CANTUS_SLOT=/f/projects/daw-r39/slot.sh bash tools/shots/reshoot.sh rest` on the merged tree
(after `themes`): 56 pictures rewritten in `docs/images/`, `docs/manual/img/` and
`web/assets/` (the program picture, the README's, studio/session/ai and the manual's). Looked
at: `docs/images/window-night.png` (above) and `docs/images/mixer.png` (nine strips, "1 ›" in
its slot, the returns at the edge). The rest moved by themes' grid lines and Slate Paper's
marks and bodies, or by the docked desk's next empty strips.

## What ran

On `r39/fix2` after merging `r39/integration` at `9c973bc5` (themes):
- `cargo test -p cantus-ui -p cantus-core --no-fail-fast`: **3 409 passed, 0 failed**; after §5, **3 410 passed, 0 failed**, and clippy (`-D warnings`, both crates, all targets) clean
  (cantus-core alone, before the merge: 761 passed, the undo fuzz among them).
- `cargo test -p cantus-project -p cantus-ai` (the other users of `free_strips`), before the
  merge: 238 passed, 0 failed.
- `RUSTFLAGS="-D warnings" cargo clippy -p cantus-core -p cantus-ui --all-targets`, before the
  merge: clean. `cargo fmt -p cantus-ui -- --check` and `-p cantus-core`: clean.
- Before the merge the only failures in `cantus-ui` were `slate_window_golden`'s four (item 4);
  there were no other pre-existing failures to fix.

## 5. The two left open, closed (the coordinator, after the first report)

- **The desk's strip-head stripe at 3:1 on light themes.** `views/mixer_look.rs`
  `Pal::stripe(colour, k)`: on a light theme the stripe at rest and selected is taken down in
  lightness (hue and saturation kept, `ensure_contrast_all`, cached like `track_mark`) until
  it reads at 3:1 on the strip's top and body and on the selected body; a dark theme's is
  `Pal::hue` as before (Slate Night does not move), and a muted or silenced strip's dimmed
  stripe stays dim (it says that state). `mixer_strip` paints through it. Held in every theme
  by `arrangement_legibility`'s new `desk_marks` measure (3:1 in light themes; a dark theme
  at Slate Night's own ×0.85, as the other marks).
- **The overflow slot fitted to its count.** `MixerView::slot_for(n)`: the pill's digits
  (measured each frame in its face), the chevron and 20 pt, and 2 pt clear each side — 30 pt
  for one digit, 36 for two. The right slot is fitted to the count beside the room a
  one-digit slot leaves, and widened if that count has two digits. At 1280 Wide the eleventh
  strip now fits (682 + 30 of 715) and "2 ›" counts 12–13. At 1920 Large (the README hero) a
  tenth strip needs 960 of 958 even with no pill, so it stays "2 ›". Test:
  `mixer_desk::the_overflow_slot_fits_its_count` (80 widths from 1240 to 1320: a one-digit
  pill wholly in its slot and less than a strip's room beside it; a one-digit slot ≤ 31 pt,
  more than 3 pt under a two-digit one); `the_desk_folds_only_what_does_not_fit` now checks
  the pill wholly in its slot, 2 pt clear, and the room beside the pill itself.
- Slate's window goldens re-blessed again (Night's mixer moved only where strip 11 and the
  pill are; Paper's stripes in both mixers); the pictures reshot again.

## What is left

- Not verified on a Linux build (WSL check not run by this builder).
