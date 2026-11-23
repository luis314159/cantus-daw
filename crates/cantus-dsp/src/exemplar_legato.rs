//! r42:strings-rec — the **scripted legato** (ADR 0158): a sustained note that starts while
//! another sustained note is held slurs out of it, from two ordinary recordings.
//!
//! - **When.** With [`Globals::legato`] on, a note that starts while another sustained note
//!   of the instrument is held — started at least [`MIN_MS`] before, so a chord's notes struck
//!   together never slur into each other — slurs from the most recent one. A note marked
//!   *legato* (`Articulation::Legato`) slurs whether the switch is on or not. Pizzicato,
//!   spiccato and staccato never slur ([`super::art::slurs`]). Poly mode only: mono and legato
//!   modes glide one voice, as they always did.
//! - **How.** The new note's layers start past their attack ([`Globals::legato_skip_ms`]),
//!   silent, and fade in over the crossfade while the old note's fade out (`x(2 − x)` each:
//!   within half a decibel of equal power); both glide — the new from the old pitch, the old
//!   toward the new — over the glide. Crossfade and glide lengthen with the interval: a
//!   twelfth more per semitone, at most twice. The old note plays no release samples (the
//!   line goes on) and keeps its key until its own note-off.
//! - **Look-ahead.** A clip's legato note ties the note before it ahead of time, when the
//!   snapshot compiles (`cantus_core::note_art::tie_legato`), so the overlap is there to
//!   slur from even when the notes were drawn end to end.
//!
//! No transition is recorded, stored or built after the note-on: two plain samples are
//! crossfaded and glided by the interval (Yamaha's US 7,557,288 claims a stored joint
//! built after the next note-on; the overlap legato here predates it).

use super::*;
use crate::exemplar::art::slurs;
use crate::exemplar::voice::CONTROL;
use crate::voices::PlayMode;

/// A note must have sounded this long before another may slur out of it.
pub const MIN_MS: f32 = 20.0;

impl Exemplar {
    /// Before the note in slot `voice` starts its zones: its articulation and bow (`art`,
    /// `bow`: what it asked for), and the held note it slurs from, if any (whose layers
    /// [`Self::slur`] then crossfades; the new layers start past their attack meanwhile).
    pub(super) fn prepare_note(&mut self, voice: usize, art: u8, bow: u8) -> Option<usize> {
        let forced = art == super::art::LEGATO;
        let mut from = None;
        if (self.globals.legato || forced) && self.alloc.mode() == PlayMode::Poly {
            let min = (MIN_MS * self.fs / 1000.0) as u64;
            let mut best: Option<(usize, u64)> = None;
            for (v, s) in self.slots.iter().enumerate() {
                if v == voice
                    || !s.key_down
                    || s.released
                    || s.legato_out
                    || s.layers == 0
                    || self.clock < s.start + min
                    || !slurs(s.kind)
                {
                    continue;
                }
                if best.is_none_or(|(_, start)| s.start > start) {
                    best = Some((v, s.start));
                }
            }
            from = best.map(|b| b.0);
        }
        let slur_bow = from.map(|v| self.slots[v].bow);
        self.choose_art(voice, art, bow, slur_bow);
        if !slurs(self.slots[voice].kind) {
            from = None;
        }
        self.start_skip_ms = if from.is_some() {
            self.globals.legato_skip_ms.max(0.0)
        } else {
            0.0
        };
        from
    }

    /// Slur from the note in slot `from` into the one just started in slot `to`.
    pub(super) fn slur(&mut self, from: usize, to: usize) {
        self.start_skip_ms = 0.0;
        let d = self.slots[to].key as f64 - self.slots[from].key as f64;
        let scale = (1.0 + d.abs() / 12.0).min(2.0) as f32;
        let ms = self.fs / 1000.0;
        let xf = (self.globals.legato_xf_ms.max(0.0) * scale * ms).max(1.0);
        let steps = (self.globals.legato_glide_ms.max(0.0) * scale * ms / CONTROL as f32).floor();
        for i in 0..MAX_LAYERS {
            let (active, slot, zone) = {
                let l = &self.layers[i];
                (l.active, l.slot, l.zone as usize)
            };
            if !active || (slot != to && slot != from) {
                continue;
            }
            let dk = d * self.zones[zone].keytrack as f64 / 100.0;
            let l = &mut self.layers[i];
            if slot == to {
                // In from silence, from the old pitch.
                l.leg = 0.0;
                l.leg_step = 1.0 / xf;
                if steps >= 1.0 {
                    l.glide = -dk;
                    l.glide_step = dk / steps as f64;
                }
            } else {
                // Out, toward the new pitch.
                l.leg_step = -l.leg.max(1e-6) / xf;
                if steps >= 1.0 {
                    l.semis += dk;
                    l.glide -= dk;
                    l.glide_step = -l.glide / steps as f64;
                }
            }
        }
        self.slots[from].legato_out = true;
    }
}
