//! r42:strings-rec — how a note is played (ADR 0158): its **articulation** and **bow**, and
//! the **continuous controller crossfade** between dynamic layers.
//!
//! - **Articulation.** A zone may say which articulation it plays ([`ZoneParams::art`]: an
//!   articulation's code, `cantus_core::note_art::Articulation`'s index + 1). A note that
//!   names one ([`Exemplar::note_art`], handed just before its note-on) plays the zones of
//!   it — or of the nearest one the instrument has on that key ([`FALLBACK`]: a staccato
//!   where only a spiccato was recorded) — whatever keyswitch is in force; zones without an
//!   articulation always play. A note that names none plays as it always did: the keyswitch
//!   in force chooses (SFZ `sw_last`), so a key-switched instrument is played both ways.
//! - **Bow.** A zone may be a down- or an up-bow recording ([`ZoneParams::bow`]). A note's
//!   bow is its own when it names one; otherwise the bow turns at each new note while the
//!   instrument sounds, and starts down after a silence (as a player does), and a legato note
//!   keeps the bow it slurs from. A bow the key has no recording of plays the other.
//! - **Controller crossfade.** A zone may fade in and out across a controller's ranges (SFZ
//!   `xfin_loccN`/`xfin_hiccN`/`xfout_loccN`/`xfout_hiccN`, `xf_cccurve`): its gain follows
//!   the controller **continuously** while the note sounds, ramped sample by sample — never a
//!   switch between layers at thresholds (the design ADR 0158 holds to; Fable Sounds' US
//!   7,915,514 claims the stepped switch). Equal power by default: the squares of two layers
//!   crossfaded over one range sum to 1, so the level follows the layers' own loudness.
//!
//! Allocation-free: everything here runs on the audio thread at a note-on or a control step.

use super::*;

/// Articulation codes, as [`ZoneParams::art`] and [`Exemplar::note_art`] hold them
/// (`cantus_core::note_art::Articulation::ALL`'s order + 1; append only).
pub const SUSTAIN: u8 = 1;
pub const LEGATO: u8 = 2;
pub const STACCATO: u8 = 3;
pub const SPICCATO: u8 = 4;
pub const PIZZICATO: u8 = 5;
pub const TREMOLO: u8 = 6;
/// Bow codes ([`ZoneParams::bow`]).
pub const DOWN: u8 = 1;
pub const UP: u8 = 2;

/// For each articulation (by code − 1), the ones played in its place, nearest first, when
/// the key has no zone of it.
pub const FALLBACK: [[u8; 3]; 6] = [
    [SUSTAIN, LEGATO, 0],
    [LEGATO, SUSTAIN, 0],
    [STACCATO, SPICCATO, SUSTAIN],
    [SPICCATO, STACCATO, SUSTAIN],
    [PIZZICATO, 0, 0],
    [TREMOLO, SUSTAIN, 0],
];

/// Whether a note of this kind (an articulation code, 0 untagged) can be slurred: a
/// sustained bowed note — never a pizzicato, a spiccato or a staccato.
pub fn slurs(kind: u8) -> bool {
    matches!(kind, 0 | SUSTAIN | LEGATO | TREMOLO)
}

/// A zone's gain at controller value `c` (0…127) for its crossfade ranges (SFZ's rule:
/// below `xfin[0]` silent, from `xfin[1]` full; full to `xfout[0]`, silent above `xfout[1]`).
/// Power curve unless `gain`: the square root of the linear ramp.
#[inline]
pub fn cc_crossfade(xfin: [u8; 2], xfout: [u8; 2], gain: bool, c: f32) -> f32 {
    let ramp_in = if c < xfin[0] as f32 {
        0.0
    } else if c >= xfin[1] as f32 {
        1.0
    } else {
        (c - xfin[0] as f32) / (xfin[1] as f32 - xfin[0] as f32)
    };
    let ramp_out = if c <= xfout[0] as f32 {
        1.0
    } else if c > xfout[1] as f32 {
        0.0
    } else {
        (xfout[1] as f32 - c) / (xfout[1] as f32 - xfout[0] as f32)
    };
    let x = ramp_in * ramp_out;
    if gain {
        x
    } else {
        x.sqrt()
    }
}

impl Exemplar {
    /// How the note `id` that starts next is played: an articulation code (0 none) and a
    /// bow code (0: the instrument turns the bow). Held until that note's note-on; a note-on
    /// of another id forgets it.
    pub fn note_art(&mut self, id: u64, art: u8, bow: u8) {
        self.pending_art = Some((id, art.min(TREMOLO), bow.min(UP)));
    }

    /// The articulation and bow handed for note `id`, taken.
    pub(super) fn take_art(&mut self, id: u64) -> (u8, u8) {
        match self.pending_art.take() {
            Some((i, a, b)) if i == id => (a, b),
            _ => (0, 0),
        }
    }

    /// The articulations (bit `code`) and, for articulation `art` (0: any), the bows (bit
    /// `code`) that some attack zone on `key` plays.
    fn art_on_key(&self, key: u8, art: u8) -> (u8, u8) {
        let (mut arts, mut bows) = (0u8, 0u8);
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            let z = &self.zones[zi];
            if key < z.lo || key > z.hi || self.sources[zi].is_empty() {
                continue;
            }
            if !matches!(z.trig, Trig::Attack | Trig::First | Trig::Legato) {
                continue;
            }
            arts |= 1 << z.art.min(7);
            if art == 0 || z.art == art {
                bows |= 1 << z.bow.min(3);
            }
        }
        (arts, bows)
    }

    /// The note in slot `voice` given its articulation and bow (`art`, `bow`: what it asked
    /// for, 0 none): the articulation its zones are chosen by (after [`FALLBACK`]), what it
    /// plays, and its bow — its own, the one it slurs from (`slur_bow`), or the next turn.
    pub(super) fn choose_art(&mut self, voice: usize, art: u8, bow: u8, slur_bow: Option<u8>) {
        let key = self.slots[voice].key;
        let (arts, _) = self.art_on_key(key, 0);
        let chosen = if art == 0 {
            0
        } else {
            FALLBACK[(art - 1) as usize]
                .iter()
                .copied()
                .find(|a| *a != 0 && arts & (1 << a) != 0)
                .unwrap_or(0)
        };
        let kind = if chosen != 0 {
            chosen
        } else {
            self.kind_by_switch(voice)
        };
        let (_, bows) = self.art_on_key(key, chosen);
        let wanted = if bow != 0 {
            bow
        } else if let Some(b) = slur_bow {
            b
        } else if self.layers.iter().any(|l| l.active && l.slot != usize::MAX) {
            if self.bow_last == DOWN {
                UP
            } else {
                DOWN
            }
        } else {
            DOWN
        };
        // A bow the key has no recording of plays the other (never both: a doubled note).
        let other = if wanted == DOWN { UP } else { DOWN };
        let played = if bows & (1 << wanted) != 0 || bows & (1 << other) == 0 {
            wanted
        } else {
            other
        };
        self.bow_last = wanted;
        let s = &mut self.slots[voice];
        s.art = chosen;
        s.kind = kind;
        s.bow = played;
    }

    /// What a note of no articulation plays: the articulation of the first attack zone the
    /// keyswitch in force gives its key (0 untagged).
    fn kind_by_switch(&self, voice: usize) -> u8 {
        let s = self.slots[voice];
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            let z = &self.zones[zi];
            if z.art != 0
                && s.key >= z.lo
                && s.key <= z.hi
                && matches!(z.trig, Trig::Attack | Trig::First | Trig::Legato)
                && self.art_ok(zi, &s)
            {
                return z.art;
            }
        }
        0
    }

    /// What the articulation and bow ask of zone `zi` for the note `s`, the keyswitch with
    /// them: a note that chose an articulation plays the zones of it (and the untagged),
    /// whatever switch is in force; otherwise the switch decides, as always.
    #[inline]
    pub(super) fn art_ok(&self, zi: usize, s: &Slot) -> bool {
        let z = &self.zones[zi];
        if s.art != 0 && z.art != 0 {
            if z.art != s.art {
                return false;
            }
        } else if z.sw >= 0 {
            let sw = if self.keyswitch >= 0 {
                self.keyswitch
            } else {
                self.globals.sw_default
            };
            if z.sw != sw {
                return false;
            }
        }
        !(z.bow != 0 && s.bow != 0 && z.bow != s.bow)
    }
}
