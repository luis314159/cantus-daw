//! A MIDI clip's content in the arrangement, by zoom (round 39, `des-views`, *Clip
//! content: the rules*). Four renderers chosen by pixels a beat:
//!
//! | Zoom (px a beat) | MIDI | Drums |
//! |---|---|---|
//! | Far (< 6) | a pitch contour: per column, lowest to highest note sounding (+3 px) | the pulse: a 1-px tick per kick or clap, its height the velocity |
//! | Default (6–20) | rows ranked by pitch, 3.5 high; a melody linear in pitch, 3–6 high | a row a piece, 2-px ticks, hats at 62 % |
//! | Near (20–60) | true length, brightness by velocity; melody rows 4–7 | hits at their length, brightness by velocity |
//! | Close (≥ 60) | as near, plus a brighter attack column; melody rows 5–8 | as near |
//!
//! Nothing here allocates once warm: the contour's columns live in a scratch buffer the
//! arrangement keeps.

use super::TimelineGeometry;
use crate::kit::color::mix;
use crate::kit::{Brush, Fill};
use crate::views::ticks_to_beats;
use cantus_core::{Clip, ClipWindow, Ticks};
use egui::{pos2, Color32, Rect};

/// The four zooms of the clip-content rules.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zoom {
    Far,
    Default,
    Near,
    Close,
}

impl Zoom {
    /// The zoom band for `px_per_beat` (logical points a beat).
    pub fn of(px_per_beat: f32) -> Zoom {
        if px_per_beat < 6.0 {
            Zoom::Far
        } else if px_per_beat < 20.0 {
            Zoom::Default
        } else if px_per_beat < 60.0 {
            Zoom::Near
        } else {
            Zoom::Close
        }
    }
}

/// What the rows need from a clip's notes, worked out once per edit rather than every
/// frame (r39:el-views — a 20 000-note clip walked for it each frame cost the arrangement
/// its warm frame): the pitches in use, and whether two notes sound at once.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Summary {
    pub mask: u128,
    pub poly: bool,
}

impl Summary {
    pub fn of(clip: &Clip) -> Summary {
        let (mut mask, mut poly, mut end) = (0u128, false, Ticks(i64::MIN));
        for n in &clip.notes {
            mask |= 1u128 << (n.pitch & 127);
            poly |= n.start < end;
            end = end.max(n.end());
        }
        Summary { mask, poly }
    }
}

/// What the content is drawn into and with.
pub(super) struct Midi<'a> {
    pub clip: &'a Clip,
    /// Its notes' [`Summary`], from the arrangement's cache.
    pub sum: Summary,
    pub shown: ClipWindow,
    pub inner: Rect,
    /// The visible span of the lanes, left and right, and in beats.
    pub left: f32,
    pub right: f32,
    pub vis: (f32, f32),
    /// The note ink and the body's ground under it.
    pub ink: Color32,
    pub ground: Color32,
    pub drums: bool,
    /// A kit's pitches: which carry the beat and which sit back ([`Roles::of`]).
    pub roles: Roles,
    pub px_per_beat: f32,
    /// What the attack column mixes toward, and how much (white in Night, black in
    /// Paper).
    pub attack: (Color32, f32),
}

/// What a kit's pitches do in the drawing: the hits that carry the beat (a kick, a snare,
/// a clap — the far zoom keeps them as the pulse) and the time-keepers that sit back (the
/// hats and the ride, at 62 %). Read from the kit's own pads — their names first, then
/// their voice (r39:lo-rest: a kit that plays its hats on other notes than General MIDI's
/// drew them at full ink); a pitch no pad plays, or a pad with no name, by General MIDI's
/// drum map.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Roles {
    pub pulse: u128,
    pub hats: u128,
}

/// A name that says one of `words`, in any case (no allocation).
fn says(name: &str, words: &[&str]) -> bool {
    words
        .iter()
        .any(|w| crate::views::samples_paint::find_ci(name, w).is_some())
}

impl Roles {
    /// General MIDI's drum map: kick 35/36, snare 38/40, side stick 37 is not the beat,
    /// clap 39; the hats 42/44/46 and the ride 51/59.
    pub fn gm() -> Roles {
        let bits = |ps: &[u8]| ps.iter().fold(0u128, |m, p| m | 1u128 << p);
        Roles {
            pulse: bits(&[35, 36, 38, 39, 40]),
            hats: bits(&[42, 44, 46, 51, 59]),
        }
    }

    /// The roles of a kit's pitches (General MIDI's for a track that is not a kit).
    pub fn of(kit: Option<&cantus_core::pads::PulsusParams>) -> Roles {
        let mut r = Roles::gm();
        let Some(kit) = kit else {
            return r;
        };
        for pad in &kit.pads {
            let bit = 1u128 << (pad.pitch & 127);
            let name = pad.name.trim();
            let (pulse, hat) = if name.is_empty() {
                continue;
            } else if says(name, &["hat", "hh", "charles", "ride"]) {
                (false, true)
            } else if says(name, &["kick", "bombo", "snare", "caja", "clap", "palma"]) {
                (true, false)
            } else {
                use cantus_core::pads::PadVoice;
                (
                    matches!(pad.voice, PadVoice::Kick | PadVoice::Snare | PadVoice::Clap),
                    false,
                )
            };
            r.pulse = if pulse { r.pulse | bit } else { r.pulse & !bit };
            r.hats = if hat { r.hats | bit } else { r.hats & !bit };
        }
        r
    }

    /// A kick, a snare or a clap: the hits the far zoom keeps as the pulse.
    pub fn is_pulse(&self, p: u8) -> bool {
        self.pulse >> (p & 127) & 1 == 1
    }

    /// A hat or the ride, sitting back so kick and clap carry the beat.
    pub fn is_hat(&self, p: u8) -> bool {
        self.hats >> (p & 127) & 1 == 1
    }
}

/// The content, by the zoom's rule. `at` and `cols` are the arrangement's scratch.
pub(super) fn midi(
    b: &mut Brush,
    geo: &TimelineGeometry,
    m: &Midi<'_>,
    at: &mut Vec<Ticks>,
    cols: &mut Vec<(f32, f32)>,
) {
    let zoom = Zoom::of(m.px_per_beat);
    let inner = m.inner;
    let px = b.px();
    // The pitches in use, and whether no two notes sound at once (cached per edit).
    let Summary { mask, poly } = m.sum;
    if mask == 0 {
        return;
    }
    let (lo_p, hi_p) = (
        mask.trailing_zeros() as u8,
        127 - mask.leading_zeros() as u8,
    );
    let melody = !m.drums && !poly && m.clip.notes.len() > 1;
    // The rows: ranked (a row per pitch in use, centred) or, for a melody, linear in
    // pitch with rows of 3–6, 4–7 or 5–8 points by zoom.
    let ranked = |p: u8| {
        mask.checked_shr(u32::from(p & 127) + 1)
            .unwrap_or(0)
            .count_ones() as f32
    };
    let (rows, span) = if melody {
        let (lo, hi) = match zoom {
            Zoom::Far | Zoom::Default => (3.0, 6.0),
            Zoom::Near => (4.0, 7.0),
            Zoom::Close => (5.0, 8.0),
        };
        let n = (hi_p - lo_p) as f32 + 1.0;
        // The design's rows where they fit; a wide melody in a short lane, what fits.
        let fit = inner.height() / n;
        (if fit < lo { fit } else { fit.min(hi) }, n)
    } else {
        let n = mask.count_ones().max(3) as f32;
        (inner.height() / n, mask.count_ones() as f32)
    };
    let pad = ((inner.height() - span * rows) * 0.5).max(0.0);
    let row_of = |p: u8| {
        if melody {
            (hi_p - p) as f32
        } else {
            ranked(p)
        }
    };
    let y_of = |p: u8| inner.top() + pad + row_of(p) * rows + rows * 0.5;
    let bar_h = match zoom {
        Zoom::Far | Zoom::Default => rows.clamp(1.0, 3.5),
        _ => (rows - px).clamp(1.0, rows.max(1.0)),
    };
    let lit = |v: u8| mix(m.ground, m.ink, 0.62 + 0.38 * v as f32 / 127.0);
    let x_of = |t: Ticks| geo.x(ticks_to_beats(t));
    let (vis_l, vis_r) = m.vis;
    if zoom == Zoom::Far {
        far(b, geo, m, at, cols, &y_of, bar_h);
        return;
    }
    let mut last_end = [f32::NEG_INFINITY; 128];
    // Sorted by start: a clip that does not repeat starts at the first note that can reach
    // the left edge (a long lookback for held notes) and stops past the right one.
    let to_content =
        |beats: f32| crate::views::beats_to_ticks(beats.max(0.0)) - m.shown.start + m.shown.offset;
    let once = m.shown.period().is_none();
    let skip = if once {
        let first = to_content(vis_l) - Ticks::beats(64);
        m.clip.notes.partition_point(|n| n.start < first)
    } else {
        0
    };
    let last_on = to_content(vis_r);
    for n in &m.clip.notes[skip..] {
        if once && n.start > last_on {
            break;
        }
        m.shown.timeline_positions(n.start, at);
        for on in at.iter().copied() {
            let (b0, b1) = (ticks_to_beats(on), ticks_to_beats(on + n.length));
            if b1 < vis_l || b0 > vis_r {
                continue;
            }
            let x0 = x_of(on).max(m.left);
            let full = x_of(on + n.length).min(m.right);
            let y = y_of(n.pitch);
            let row = usize::from(n.pitch & 127);
            let c = if m.drums && m.roles.is_hat(n.pitch) {
                mix(m.ground, m.ink, 0.62)
            } else if zoom >= Zoom::Near {
                lit(n.velocity)
            } else {
                m.ink
            };
            let x1 = if m.drums && zoom == Zoom::Default {
                (x0 + 2.0 * px.max(1.0)).min(m.right)
            } else {
                full.max(x0 + 2.0 * px)
            };
            // A pixel of ground between two notes that follow each other on a row.
            let x0 = if x0 < last_end[row] + px {
                if x1 <= last_end[row] + px {
                    continue;
                }
                last_end[row] + px
            } else {
                x0
            };
            if x0 >= m.right || x1 <= x0 {
                continue;
            }
            last_end[row] = x1;
            let r = Rect::from_min_max(pos2(x0, y - bar_h * 0.5), pos2(x1, y + bar_h * 0.5));
            b.rect(b.snap_rect(r), Fill::Solid(c));
            if zoom == Zoom::Close && r.width() > 4.0 * px {
                // The attack: a brighter first column, so repeated notes part.
                let col = Rect::from_min_max(r.min, pos2(r.left() + 2.0 * px, r.bottom()));
                b.rect(
                    b.snap_rect(col),
                    Fill::Solid(mix(c, m.attack.0, m.attack.1)),
                );
            }
        }
    }
}

/// Far: a melody's contour (per pixel column, lowest to highest note sounding, +3 px),
/// or a kit's pulse (a 1-px tick a kick or clap, as tall as its velocity).
fn far(
    b: &mut Brush,
    geo: &TimelineGeometry,
    m: &Midi<'_>,
    at: &mut Vec<Ticks>,
    cols: &mut Vec<(f32, f32)>,
    y_of: &dyn Fn(u8) -> f32,
    bar_h: f32,
) {
    let px = b.px();
    let inner = m.inner;
    let (vis_l, vis_r) = m.vis;
    let x_of = |t: Ticks| geo.x(ticks_to_beats(t));
    if m.drums {
        let pulse = mix(m.ground, m.ink, 0.62);
        let mut last = f32::NEG_INFINITY;
        // A kit's kicks and claps; a clip of hats or percussion alone, every hit.
        let any_pulse = m.clip.notes.iter().any(|n| m.roles.is_pulse(n.pitch));
        for n in m
            .clip
            .notes
            .iter()
            .filter(|n| !any_pulse || m.roles.is_pulse(n.pitch))
        {
            m.shown.timeline_positions(n.start, at);
            for on in at.iter().copied() {
                let x = b.snap(x_of(on));
                let beats = ticks_to_beats(on);
                if beats < vis_l || beats > vis_r || x < m.left || x >= m.right || x <= last {
                    continue;
                }
                last = x;
                let h = inner.height() * (0.25 + 0.75 * n.velocity as f32 / 127.0);
                b.vline(x + 0.5 * px, inner.bottom() - h, inner.bottom(), px, pulse);
            }
        }
        return;
    }
    // One entry a physical column: the highest and lowest y sounding there.
    let ppp = b.ppp();
    let c0 = (m.left * ppp).floor();
    let n_cols = ((m.right * ppp).ceil() - c0).max(0.0) as usize;
    cols.clear();
    cols.resize(n_cols, (f32::MAX, f32::MIN));
    for n in &m.clip.notes {
        m.shown.timeline_positions(n.start, at);
        for on in at.iter().copied() {
            let (b0, b1) = (ticks_to_beats(on), ticks_to_beats(on + n.length));
            if b1 < vis_l || b0 > vis_r {
                continue;
            }
            let a = ((x_of(on) * ppp).floor() - c0).max(0.0) as usize;
            let z = ((x_of(on + n.length) * ppp).ceil() - c0).max(0.0) as usize;
            let y = y_of(n.pitch);
            for c in cols.iter_mut().take(z.min(n_cols)).skip(a) {
                c.0 = c.0.min(y);
                c.1 = c.1.max(y);
            }
        }
    }
    // Runs of equal columns, one rect each.
    let mut i = 0;
    while i < n_cols {
        let (top, bottom) = cols[i];
        let mut j = i + 1;
        while j < n_cols && cols[j] == cols[i] {
            j += 1;
        }
        if top <= bottom {
            let r = Rect::from_min_max(
                pos2((c0 + i as f32) / ppp, top - bar_h * 0.5),
                pos2(
                    (c0 + j as f32) / ppp,
                    bottom + bar_h * 0.5 + 3.0 * px.min(1.0),
                ),
            );
            b.rect(r, Fill::Solid(m.ink));
        }
        i = j;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four zooms of des-views' rules, by pixels a beat.
    #[test]
    fn the_zoom_bands_are_the_design_s() {
        assert_eq!(Zoom::of(2.0), Zoom::Far);
        assert_eq!(Zoom::of(5.9), Zoom::Far);
        assert_eq!(Zoom::of(6.0), Zoom::Default);
        assert_eq!(Zoom::of(19.9), Zoom::Default);
        assert_eq!(Zoom::of(20.0), Zoom::Near);
        assert_eq!(Zoom::of(59.0), Zoom::Near);
        assert_eq!(Zoom::of(60.0), Zoom::Close);
        assert!(Zoom::Far < Zoom::Default && Zoom::Near < Zoom::Close);
    }

    /// r39:lo-rest — a kit's hats are found by its pads' names, not by General MIDI's
    /// pitches: a kit that plays its hats on 60 and 61 sits them back, and a shaker a kit
    /// put on 42 is not a hat. A pad with no name keeps General MIDI's role; a track that is
    /// not a kit has General MIDI's map.
    #[test]
    fn a_kit_s_hats_are_read_from_its_pad_names() {
        use cantus_core::pads::{DrumPad, PadVoice, PulsusParams};
        let pad = |pitch: u8, name: &str, voice: PadVoice| DrumPad {
            pitch,
            name: name.into(),
            voice,
            ..DrumPad::default()
        };
        let kit = PulsusParams {
            pads: vec![
                pad(60, "Closed hat", PadVoice::HiHat),
                pad(61, "Open Hat", PadVoice::Sample),
                pad(42, "Shaker", PadVoice::HiHat),
                pad(48, "Big kick", PadVoice::Sample),
                pad(50, "Low tom", PadVoice::Tom),
                pad(52, "", PadVoice::Sample),
                pad(38, "Rim", PadVoice::Snare),
            ],
            ..PulsusParams::default()
        };
        let r = Roles::of(Some(&kit));
        assert!(r.is_hat(60) && r.is_hat(61), "named hats sit back");
        assert!(
            !r.is_hat(42),
            "a shaker on the closed hat's note is not a hat"
        );
        assert!(r.is_pulse(48) && !r.is_hat(48), "a kick by its name");
        assert!(!r.is_pulse(50) && !r.is_hat(50), "a tom is neither");
        assert!(r.is_pulse(38), "a snare voice carries the beat");
        assert!(
            r.is_hat(46),
            "a pitch no pad plays keeps General MIDI's role"
        );
        assert_eq!(Roles::of(None), Roles::gm());
        assert!(Roles::gm().is_hat(42) && Roles::gm().is_pulse(36));
    }
}
