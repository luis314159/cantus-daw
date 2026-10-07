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
