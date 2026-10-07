# Round 22 — requests between builders

Each entry: who asks, who owns the code, what, why, and what the asker does meanwhile.

## export → pool: progress and stop for the mix render

- **Owner:** pool (`crates/cantus-engine/src/render.rs`).
- **What:** a variant of `render_with_instruments` that takes a progress callback with
  the same contract as `render_stems_to_folder`'s — `&mut dyn FnMut(f32) -> bool`, called
  after every block with the fraction done, the render stopping (with an error such as
  `RenderError::Cancelled`) when it answers `false`. For example:

  ```rust
  pub fn render_with_progress(
      snapshot: Arc<Snapshot>,
      opts: &RenderOptions,
      units: Vec<EffectUnit>,
      hosted: Vec<crate::inst::NativeUnit>,
      progress: &mut dyn FnMut(f32) -> bool,
  ) -> Result<Rendered, RenderError>;
  ```

  The existing functions would call it with `&mut |_| true`, so nothing else changes and
  the render stays bit-identical.
- **Why:** the export dialog (ADR 0033) shows a progress bar and has *Stop*. Stems already
  report and stop through `render_stems_to_folder`; the mix render cannot, so today the
  bar pulses while the song renders (the phase says *Rendering*) and a *Stop* pressed then
  takes effect when the render returns — nothing is written either way, but a long song
  makes the user wait for a render that will be thrown away.
- **Meanwhile:** `cantus_export::export_mix` calls `render_with_instruments` and sets
  `Control::render_is_indeterminate`; switching to the new function is a three-line
  change in `crates/cantus-export/src/job.rs` (`export_mix`), marked there.

## export → integrator (after beta-docs merges): LGPL in the notices allow-list

- **Owner:** beta-docs' `xtask notices` (`xtask/src/notices.rs`, not on `r22/export`).
- **What:** the integrator's ruling (LGPL is compatible with Cantus's GPL-3.0-or-later)
  applied to the allow-list, then the notices regenerated:

  ```rust
  // in ALLOWED, after "MPL-2.0":
  // LAME (MP3 export, ADR 0033): LGPL code linked into a GPL-3.0-or-later program
  // whose whole source is published; the crates declare the old id "LGPL-3.0".
  "LGPL-3.0",
  "LGPL-3.0-only",
  "LGPL-3.0-or-later",
  "LGPL-2.1-or-later",
  ```

  then `cargo run -p xtask -- notices` and `cargo test -p xtask`.
- **The crates `r22/export` adds to the shipped program** (licence as the crate declares
  it): `mp3lame-encoder` 0.2.5 and `mp3lame-sys` 0.1.x (**LGPL-3.0**; LAME 3.100 itself
  is LGPL-2.0-or-later), `vorbis_rs` 0.5.6, `aotuv_lancer_vorbis_sys` 0.1.6,
  `ogg_next_sys` 0.1.5 (BSD-3-Clause; libvorbis with aoTuV/Lancer, libogg), `flacenc`
  0.5.1 (Apache-2.0) and its `built`, `crc`, `crc-catalog`, `heapless`, `hash32`,
  `md-5`, `seq-macro`, `core_detect`, `multiversion_no_op`, `extended`, `lazy_static`
  (MIT and/or Apache-2.0); build-only `autotools` (MIT). Test-only, never shipped:
  `symphonia` 0.5.5 and its `-core`, `-metadata`, `-bundle-mp3`, `-format-riff`
  (MPL-2.0), `encoding_rs` ((Apache-2.0 OR MIT) AND BSD-3-Clause).
- **Why not done on `r22/export`:** the notices generator exists only on `r22/beta-docs`;
  adding it here would duplicate the file and conflict at the merge.
