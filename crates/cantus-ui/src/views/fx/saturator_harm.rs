//! Fervor's harmonics card: a test tone through the real DSP — every filter, the
//! oversampling, the crush — and the spectrum of what comes out. Harmonics are bars (even
//! and odd in two hues); what lands between them — aliasing, and the noise of reduced bits —
//! is red. The oversampling is chosen here, watching the red.

use super::*;
use cantus_dsp::fervor::Fervor;

/// The analysis length, and the test tone's whole number of cycles in it: an odd count,
/// so harmonics that fold back past Nyquist land between the harmonics, not on them.
pub const N: usize = 8_192;
pub const CYCLES: usize = 123;
/// The rate the test runs at.
pub const SR: f32 = 48_000.0;
/// Samples run before the analysed ones, so every filter has settled (the DC blocker's
/// 5 Hz corner the slowest: 0.68 s takes its start down by more than 180 dB).
const WARM: usize = 32_768;
/// Bins either side of a component's centre that belong to it (the window's main lobe).
const LOBE: usize = 8;
/// A 7-term Blackman-Harris window (Albrecht): sidelobes under −180 dB, so the floor is the
/// FFT's own rounding, not the window's leakage.
const WINDOW: [f32; 7] = [
    0.271_051_4,
    -0.433_297_94,
    0.218_123,
    -0.065_925_45,
    0.010_811_742,
    -0.000_776_584_8,
    0.000_013_887_217,
];
/// The harmonics shown, the fundamental included.
pub const SHOWN: usize = 16;
/// The floor of the display, dB below the fundamental.
pub const FLOOR_DB: f32 = -140.0;

/// The test tone's frequency.
pub fn f0() -> f32 {
    CYCLES as f32 * SR / N as f32
}

/// A measured test tone.
#[derive(Clone, Debug)]
pub struct Harmonics {
    settings: SatSettings,
    level: usize,
    /// Each bin's level, dB re the fundamental's peak (bins 0 … N/2).
    pub spectrum: Vec<f32>,
    /// Whether a bin belongs to a harmonic.
    pub harmonic: Vec<bool>,
    /// Harmonics 1 … 16, dB re the fundamental (the first is 0).
    pub harm: [f32; SHOWN],
    /// Total harmonic distortion, %.
    pub thd_pct: f32,
    /// The even harmonics' energy over the odd ones' (from the third), dB.
    pub even_odd_db: f32,
    /// The loudest component between the harmonics, dB re the fundamental.
    pub alias_db: f32,
}

/// Run the test tone at `level_db` (peak dBFS) through Fervor at these settings.
pub fn measure(ss: &SatSettings, level: usize) -> Harmonics {
    let level_db = TEST_LEVELS[level.min(TEST_LEVELS.len() - 1)];
    let mut f = Fervor::new(SR);
    f.set(ss);
    f.reset();
    let amp = 10f32.powf(level_db / 20.0);
    let total = WARM + N;
    // The phase exactly periodic in N: (i · CYCLES mod N) / N of a turn.
    let x: Vec<f32> = (0..total)
        .map(|i| {
            let ph = ((i * CYCLES) % N) as f64 / N as f64;
            amp * (std::f64::consts::TAU * ph).sin() as f32
        })
        .collect();
    let (mut l, mut r) = (x.clone(), x);
    f.process(&mut l, &mut r);
    let mut re: Vec<f64> = l[WARM..]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let a = std::f64::consts::TAU * i as f64 / N as f64;
            let w: f64 = WINDOW
                .iter()
                .enumerate()
                .map(|(k, c)| *c as f64 * (k as f64 * a).cos())
                .sum();
            *v as f64 * w
        })
        .collect();
    let mut im = vec![0.0; N];
    fft64(&mut re, &mut im);
    let half = N / 2;
    let power: Vec<f64> = (0..=half).map(|b| re[b] * re[b] + im[b] * im[b]).collect();
    let band = |centre: usize| -> f64 {
        let lo = centre.saturating_sub(LOBE);
        let hi = (centre + LOBE).min(half);
        power[lo..=hi].iter().sum()
    };
    let fund_peak = power[CYCLES.saturating_sub(LOBE)..=CYCLES + LOBE]
        .iter()
        .copied()
        .fold(0.0, f64::max)
        .max(1e-30);
    let p1 = band(CYCLES).max(1e-30);
    let mut harm = [FLOOR_DB; SHOWN];
    let (mut sum, mut even, mut odd) = (0.0f64, 0.0f64, 0.0f64);
    for (h, slot) in harm.iter_mut().enumerate() {
        let n = h + 1;
        let c = n * CYCLES;
        if c + LOBE > half {
            break;
        }
        let p = band(c);
        *slot = (10.0 * (p / p1).log10()).max(FLOOR_DB as f64) as f32;
        if n >= 2 {
            sum += p;
            if n % 2 == 0 {
                even += p;
            } else {
                odd += p;
            }
        }
    }
    // Harmonics past the shown ones still count toward the distortion.
    for n in SHOWN + 1.. {
        let c = n * CYCLES;
        if c + LOBE > half {
            break;
        }
        sum += band(c);
    }
    let harmonic: Vec<bool> = (0..=half)
        .map(|b| {
            let n = (b as f32 / CYCLES as f32).round() as usize;
            n >= 1 && b.abs_diff(n * CYCLES) <= LOBE
        })
        .collect();
    let spectrum: Vec<f32> = power
        .iter()
        .map(|p| (10.0 * (p / fund_peak).log10()).max(FLOOR_DB as f64) as f32)
        .collect();
    let alias_db = spectrum
        .iter()
        .zip(&harmonic)
        .enumerate()
        .filter(|(b, (_, h))| *b > LOBE && !**h)
        .map(|(_, (v, _))| *v)
        .fold(FLOOR_DB, f32::max);
    Harmonics {
        settings: ss.clone(),
        level,
        spectrum,
        harmonic,
        harm,
        thd_pct: ((sum / p1).sqrt() * 100.0) as f32,
        even_odd_db: if even > 0.0 && odd > 0.0 {
            (10.0 * (even / odd).log10()) as f32
        } else if even > 0.0 {
            60.0
        } else {
            -60.0
        },
        alias_db,
    }
}

/// The harmonics card: the display, then the oversampling, its filters and the test level.
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &SatSettings,
) {
    // Measured again when the settings or the level change — at most ten times a second
    // while a knob moves.
    let now = ui.input(|i| i.time);
    let stale = st
        .harmonics
        .as_ref()
        .is_none_or(|h| h.settings != *ss || h.level != st.test_level);
    let at_id = ui.id().with("sat-harm-at");
    let last = ui.data(|d| d.get_temp::<f64>(at_id)).unwrap_or(-1.0);
    if stale && (st.harmonics.is_none() || now - last > 0.1) {
        st.harmonics = Some(measure(ss, st.test_level));
        ui.data_mut(|d| d.insert_temp(at_id, now));
    }
    if stale {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    let k = s.k;
    let tools_h = controls::ROW + 8.0;
    let disp = (rect.height() / k - band::TITLE - tools_h - 1.0).max(40.0);
    controls::card_with(
        ui,
        s,
        rect,
        Src::Key("sat.card.harmonics"),
        None,
        Some(disp),
        |ui, cr| {
            if let (Some(d), Some(h)) = (cr.display, st.harmonics.as_ref()) {
                display(ui, s, d, h);
            }
            let w = cr.body.width() / k;
            let seg = ((w - 16.0 - 8.0) / 3.0).max(60.0);
            let os = [
                Src::Key("sat.os.0"),
                Src::Key("sat.os.1"),
                Src::Key("sat.os.2"),
                Src::Key("sat.os.3"),
            ];
            controls::segmented_param(ui, s, host, "os", &os, seg);
            ui.add_enabled_ui(ss.oversampling > 1, |ui| {
                let ph = [Src::Key("sat.phase_short.0"), Src::Key("sat.phase_short.1")];
                controls::segmented_param(ui, s, host, "phase", &ph, seg);
            });
            let levels = [
                Src::Key("sat.test.0"),
                Src::Key("sat.test.1"),
                Src::Key("sat.test.2"),
                Src::Key("sat.test.3"),
            ];
            if let Some(i) = controls::segmented(
                ui,
                s,
                st.test_level,
                &levels,
                frame::tr("sat.test_tip"),
                seg,
            ) {
                st.test_level = i;
            }
        },
    );
}

/// The odd harmonics' colour: the dynamics hue as ever; in a face, the look's second voice
/// (Fervor's filament yellow) — every function hue there is the accent, the even
/// harmonics' colour.
pub fn odd_colour(s: &Style, ink: &Ink) -> egui::Color32 {
    ink.voice(s.t.hue(Function::Dynamics), true)
}

/// The spectrum on a log axis, the harmonics as bars, what lies between them in red.
fn display(ui: &mut egui::Ui, s: &Style, outer: Rect, h: &Harmonics) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    // The odd harmonics in the look's second voice (Fervor's filament yellow): in a face
    // every function hue is the accent, so they would match the even ones; plain, as ever.
    let odd = odd_colour(s, &ink);
    let head = 20.0 * k;
    let rect = Rect::from_min_max(
        pos2(outer.left() + 26.0 * k, outer.top() + head),
        pos2(outer.right() - 6.0 * k, outer.bottom() - 14.0 * k),
    );
    let (f_lo, f_hi) = (20.0f32, SR * 0.5);
    let x_of =
        |hz: f32| rect.left() + (hz.max(f_lo) / f_lo).ln() / (f_hi / f_lo).ln() * rect.width();
    let y_of = |db: f32| rect.top() + (db / FLOOR_DB).clamp(0.0, 1.0) * rect.height();
    let bin_hz = SR / N as f32;
    let resp = ui.interact(outer, ui.id().with("sat-harm"), Sense::hover());
    with_brush(&ui.painter_at(outer), |b| {
        canvas::ground_b(b, outer, &ink, s.hue_tok);
        let px = b.px();
        for db in (1..=7).map(|i| -20.0 * i as f32) {
            let y = b.snap_mid(y_of(db));
            b.hline(rect.left(), rect.right(), y, px, ink.grid);
            canvas::putf(
                b,
                pos2(rect.left() - 3.0 * k, y),
                Align2::RIGHT_CENTER,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{db:.0}"),
            );
        }
        for (hz, word) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
            let x = b.snap_mid(x_of(hz));
            b.vline(x, rect.top(), rect.bottom(), px, ink.grid);
            canvas::put(
                b,
                pos2(x, rect.bottom() + 1.0 * k),
                Align2::CENTER_TOP,
                &Text::new(word, Role::Micro, ink.dim),
                tk,
                None,
            );
        }
        // What lies between the harmonics, column by column, in red.
        let cols = (rect.width() / (2.0 * k)).max(8.0) as usize;
        let mut prev: Option<Pos2> = None;
        let red = Ink::a(ink.danger, 0.9);
        for c in 0..cols {
            let fa = f_lo * (f_hi / f_lo).powf(c as f32 / cols as f32);
            let fb = f_lo * (f_hi / f_lo).powf((c + 1) as f32 / cols as f32);
            let b0 = (fa / bin_hz) as usize;
            let b1 = ((fb / bin_hz) as usize).max(b0 + 1).min(h.spectrum.len());
            let mut top = FLOOR_DB;
            for bin in b0..b1 {
                if !h.harmonic[bin] && bin > LOBE {
                    top = top.max(h.spectrum[bin]);
                }
            }
            let p = pos2(x_of(fa), y_of(top));
            if top > FLOOR_DB + 1.0 {
                if let Some(q) = prev {
                    b.segment(q, p, 1.2 * k, red, false);
                }
                b.rect(
                    Rect::from_min_max(p, pos2(x_of(fb), rect.bottom())),
                    Fill::Solid(Ink::a(ink.danger, 0.18)),
                );
                prev = Some(p);
            } else {
                prev = None;
            }
        }
        // The harmonics: the fundamental in the text colour, even and odd in two hues.
        let f0 = f0();
        for (i, db) in h.harm.iter().enumerate() {
            let n = i + 1;
            let hz = f0 * n as f32;
            if hz >= f_hi || *db <= FLOOR_DB + 1.0 {
                continue;
            }
            let x = x_of(hz);
            let col = if n == 1 {
                ink.text
            } else if n % 2 == 0 {
                ink.hue
            } else {
                odd
            };
            let w = (4.0 * k).max(2.0);
            b.rect(
                Rect::from_min_max(
                    pos2(x - w * 0.5, y_of(*db)),
                    pos2(x + w * 0.5, rect.bottom()),
                ),
                Fill::Solid(col),
            );
            if n <= 9 && *db > -120.0 {
                canvas::putf(
                    b,
                    pos2(x, y_of(*db) - 2.0 * k),
                    Align2::CENTER_BOTTOM,
                    Role::Micro,
                    col,
                    tk,
                    Some(ink.plate),
                    format_args!("{n}"),
                );
            }
        }
        // The readouts, left to right.
        let mid = outer.top() + head * 0.5;
        let r = canvas::tag(
            b,
            pos2(outer.left() + 8.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("sat.thd"),
            ink.text,
            tk,
            None,
            format_args!("{:.2}%", h.thd_pct),
        );
        let r = canvas::tag(
            b,
            pos2(r.right() + 12.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("sat.even_odd"),
            ink.text,
            tk,
            None,
            format_args!("{:+.1} dB", h.even_odd_db),
        );
        let alias_ink = if h.alias_db > -80.0 {
            ink.danger
        } else {
            ink.text
        };
        let r = canvas::tag(
            b,
            pos2(r.right() + 12.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("sat.alias"),
            alias_ink,
            tk,
            None,
            format_args!("{:.0} dB", h.alias_db.max(FLOOR_DB)),
        );
        if r.right() + 130.0 * k < outer.right() {
            canvas::tag(
                b,
                pos2(outer.right() - 8.0 * k, mid),
                Align2::RIGHT_CENTER,
                frame::tr("sat.tone_at"),
                ink.dim,
                tk,
                None,
                format_args!(
                    "{:.0} Hz {:.0} dBFS",
                    f0,
                    TEST_LEVELS[h.level.min(TEST_LEVELS.len() - 1)]
                ),
            );
        }
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("sat.card.harmonics"),
            frame::tr("sat.harm_tip"),
            Caps::NONE,
        );
    }
}

/// An in-place radix-2 FFT in f64 with every twiddle computed directly: the display's
/// analyser (`cantus_dsp::spectrum`) steps its twiddles by recurrence in f32, which is
/// right for a picture but leaves a floor near −95 dB — above the aliasing this card is
/// there to show.
fn fft64(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wi, wr) = (step * k as f64).sin_cos();
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod face_colour_tests {
    use super::*;
    use crate::kit::color::{contrast, Hsl};
    use egui::Color32;

    fn gap(a: Color32, b: Color32) -> f32 {
        let (a, b) = (Hsl::of(a), Hsl::of(b));
        if a.s < 0.15 || b.s < 0.15 {
            return 180.0;
        }
        let d = (a.h - b.h).rem_euclid(360.0);
        d.min(360.0 - d)
    }

    fn face(base: crate::kit::Tokens) -> (Style, Ink) {
        let sheet = crate::kit::skin::of_effect(cantus_core::EffectKind::Saturator);
        let s = Style::new(base).skinned(sheet).function(Function::Tone);
        let ink = Ink::of(&s);
        (s, ink)
    }

    fn pairs(base: crate::kit::Tokens) -> Vec<(&'static str, Color32, Color32)> {
        let (s, ink) = face(base);
        vec![(
            "the odd and the even harmonics",
            odd_colour(&s, &ink),
            ink.hue,
        )]
    }

    fn grounds(base: crate::kit::Tokens) -> Vec<(&'static str, Color32, Color32)> {
        let (s, ink) = face(base);
        vec![("the odd harmonics", odd_colour(&s, &ink), ink.ground)]
    }

    /// Round 32, the review of delivery 2: colours that tell two things apart inside the
    /// display stay apart in the unit's face (at least 20° of hue) and read on the look's
    /// ground (3:1), in every preset.
    #[test]
    fn colours_with_a_meaning_stay_apart_in_the_face() {
        let mut bad = Vec::new();
        for (key, make) in crate::theme::PRESETS {
            let base = make().kit_tokens();
            for (what, a, b) in pairs(base) {
                if gap(a, b) < 20.0 {
                    bad.push(format!("{key}: {what} {:.0}° apart", gap(a, b)));
                }
            }
            for (what, c, ground) in grounds(base) {
                if contrast(c, ground) < 2.99 {
                    bad.push(format!(
                        "{key}: {what} at {:.2} on the ground",
                        contrast(c, ground)
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
