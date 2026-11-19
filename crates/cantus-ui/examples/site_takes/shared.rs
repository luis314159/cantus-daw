//! The website's recorded takes and how an effect is heard on them, shared by `render_tryit`
//! (Try it) and `render_unit_sounds` (the cards' excerpts) — `docs/round-fx/sounds.md`.
//!
//! - **Guitar**: Carbonthief's DI strums (CC0, `refs/audio/carbonthief/x/SampleData`) as Am,
//!   F, C, G, a chord a bar at 90 BPM, struck on beats 1 and 3, eight bars, peak −6 dBFS.
//! - **Drums**: a rock groove at 90 BPM, eight bars, played by Tympanum on the recorded kit of
//!   Gogodze Phu Vol. II (Karoryfer Samples, CC0) through the engine's offline render.
//! - **Mix**: those drums, Gravis on the roots and the guitar, eight bars at 90 BPM.
//! - **Voice**: Britton Rea's reading of the Gettysburg Address (public domain), from a file.
//!
//! Every effect runs on the engine's own unit with the song playing from bar 1 and its
//! latency taken off; the site keeps bars 5 to 8, so the loop joins without a seam.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::*;
use cantus_engine::fx::{FxNote, SongPosition};
use cantus_engine::{render, EffectMemory, EffectUnit, EngineHandle, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::Path;

pub const RATE: u32 = 48_000;
pub const BPM: f64 = 90.0;
pub const BARS: usize = 8;
/// Every take's loudness, LUFS (dry and wet alike, by a gain alone).
pub const TARGET: f64 = -19.3;
/// No take's peak above this, dBFS (MP3 adds a little on decoding).
pub const CEILING: f64 = -1.5;

pub const GUITAR: (&str, &str) = (
    "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI)",
    "Guitarra: Carbonthief, CC0 (Strummed Guitar Chords, DI)",
);
pub const VOICE: (&str, &str) = (
    "Voice: Britton Rea, public domain (Wikimedia Commons)",
    "Voz: Britton Rea, dominio público (Wikimedia Commons)",
);
pub const DRUMS: (&str, &str) = (
    "Drums: Gogodze Phu Vol. II by Karoryfer Samples, CC0, played by Cantus's Tympanum",
    "Batería: Gogodze Phu Vol. II de Karoryfer Samples, CC0, tocada por Tympanum de Cantus",
);
pub const MIX: (&str, &str) = (
    "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI); drums: Gogodze Phu Vol. II by Karoryfer Samples, CC0, played by Cantus's Tympanum; bass: Cantus's Gravis",
    "Guitarra: Carbonthief, CC0 (Strummed Guitar Chords, DI); batería: Gogodze Phu Vol. II de Karoryfer Samples, CC0, tocada por Tympanum de Cantus; bajo: Gravis de Cantus",
);
pub const PIANO: (&str, &str) = (
    "Piano: Salamander Grand Piano V3 by Alexander Holm (FLAC edition by FreePats), CC BY 3.0, played by Cantus's Chorda",
    "Piano: Salamander Grand Piano V3 de Alexander Holm (edición FLAC de FreePats), CC BY 3.0, tocado por Chorda de Cantus",
);
pub const PAD: (&str, &str) = (
    "Pad: Cantus's own Calor (Velvet pad)",
    "Pad: Calor de Cantus (Pad de terciopelo)",
);
/// The recorded kit (`<content root>/gogodze-phu-vol-ii`).
pub const KIT: &str = "pack:gogodze-phu-vol-ii/kit";

/// The chord of each bar (`refs/audio/README.md`).
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];
/// The bass's roots under them: A1, F1, C2, G1.
const ROOTS: [u8; 4] = [33, 29, 36, 31];

pub type Stereo = (Vec<f32>, Vec<f32>);
pub type Values = std::collections::BTreeMap<String, f32>;

fn beat_frames(bpm: f64) -> usize {
    (60.0 / bpm * f64::from(RATE)) as usize
}

/// Eight bars at 90 BPM, in frames.
pub fn eight_bars() -> usize {
    BARS * 4 * beat_frames(BPM)
}

fn interleaved_to_stereo(x: Vec<f32>, ch: usize, rate: u32) -> Stereo {
    let x = if rate != RATE {
        cantus_engine::samples::resample::resample(&x, ch, rate, RATE)
    } else {
        x
    };
    if ch == 1 {
        (x.clone(), x)
    } else {
        (
            x.iter().step_by(ch).copied().collect(),
            x.iter().skip(1).step_by(ch).copied().collect(),
        )
    }
}

/// A file decoded by Cantus's own decoder, stereo at 48 kHz.
pub fn take(p: &Path) -> Stereo {
    let d = cantus_engine::SampleCache::decode_source(p)
        .unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
    interleaved_to_stereo(d.frames.to_vec(), d.channels as usize, d.rate)
}

/// One strum (the `.flac`, or a `.wav` made from it where the decoder cannot read the FLAC —
/// Carbonthief's are 32-bit).
fn strum(dir: &Path, stem: &str) -> Stereo {
    let p = ["flac", "wav"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.exists() && cantus_engine::SampleCache::decode_source(p).is_ok())
        .unwrap_or_else(|| panic!("{stem}: not in {}", dir.display()));
    take(&p)
}

/// The eight bars, two strums a bar, each faded over 20 ms before the next, peak −6 dBFS.
pub fn progression(dir: &Path) -> Stereo {
    let beat = beat_frames(BPM);
    let n = eight_bars();
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let fade = (0.02 * f64::from(RATE)) as usize;
    let strums: Vec<Stereo> = CHORDS.iter().map(|c| strum(dir, c)).collect();
    for bar in 0..BARS {
        for half in 0..2 {
            let at = (bar * 4 + half * 2) * beat;
            let len = 2 * beat;
            let (sl, sr) = &strums[bar % 4];
            for i in 0..len.min(sl.len()) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                l[at + i] += sl[i] * g;
                r[at + i] += sr[i] * g;
            }
        }
    }
    scale_to_peak(&mut l, &mut r, -6.0);
    (l, r)
}

fn scale_to_peak(l: &mut [f32], r: &mut [f32], db: f32) {
    let peak = l
        .iter()
        .chain(r.iter())
        .fold(0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    let g = cantus_dsp::math::db_to_gain(db) / peak;
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
}

fn tick(bar: usize, sixteenth: usize) -> Ticks {
    Ticks((bar * 4) as i64 * PPQ + sixteenth as i64 * PPQ / 4)
}

/// The rock groove on the recorded kit: kick on one, the and of two and three; the snare on
/// two and four with a ghost or two; closed hats in eighths, opened on the last and of every
/// second bar; a tom fill closing bars four and eight. Gogodze has no cymbals, so none plays.
fn drum_project() -> Project {
    let mut p = Project::new("drums");
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(KIT.into())).expect("a kit");
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Groove", Ticks::ZERO, tick(BARS, 0));
    let mut cc4: Vec<automation::AutoPoint> = Vec::new();
    let mut set_cc = |at: Ticks, v: f32| {
        let before = cc4.last().map_or(v, |x: &automation::AutoPoint| x.value);
        cc4.push(automation::AutoPoint {
            tick: Ticks(at.0.max(1) - 1),
            value: before,
            tension: 0.0,
            shape: Default::default(),
        });
        cc4.push(automation::AutoPoint::new(at, v));
    };
    let mut notes: Vec<(Ticks, u8, u8)> = Vec::new();
    for bar in 0..BARS {
        let fill = bar % 4 == 3;
        for (s, v) in [(0, 112), (6, 96), (8, 108)] {
            notes.push((tick(bar, s), 36, v));
        }
        if bar % 2 == 1 {
            notes.push((tick(bar, 11), 36, 84));
        }
        let snares: &[(usize, u8)] = if fill {
            &[(4, 112), (7, 26)]
        } else {
            &[(4, 112), (12, 116), (7, 26), (15, 30)]
        };
        for &(s, v) in snares {
            notes.push((tick(bar, s), 38, v));
        }
        for s in (0..16).step_by(2) {
            if fill && s >= 12 {
                continue;
            }
            let open = bar % 2 == 1 && s == 14;
            set_cc(tick(bar, s), if open { 0.9 } else { 0.06 });
            let v = if s % 4 == 0 { 98 } else { 72 };
            notes.push((tick(bar, s), 46, if open { v + 12 } else { v }));
        }
        if fill {
            for (s, key, v) in [(12, 48, 104), (13, 48, 88), (14, 45, 100), (15, 43, 112)] {
                notes.push((tick(bar, s), key, v));
            }
        }
    }
    for (at, key, vel) in notes {
        let n = p.make_note(at, Ticks(PPQ / 8), key, vel);
        c.notes.push(n);
    }
    c.lanes.push(ClipLane::new(LaneKind::Cc(4), cc4));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// The progression's chords held a bar each (A minor, F, C, G), played by `spec`.
fn pad_project(spec: NativeSpec) -> Project {
    const CHORDS: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];
    let mut p = Project::new("pad");
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Pad");
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Chords", Ticks::ZERO, tick(BARS, 0));
    for bar in 0..BARS {
        for key in CHORDS[bar % 4] {
            let n = p.make_note(tick(bar, 0), Ticks(4 * PPQ - PPQ / 8), key, 92);
            c.notes.push(n);
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// A synthesiser of Cantus's (`spec`, a factory patch) holding the progression's chords, at
/// `lufs_at`, eight bars.
pub fn pad(spec: NativeSpec, lufs_at: f64) -> Stereo {
    let (mut l, mut r) = render_project(&pad_project(spec), 3);
    set_lufs(&mut l, &mut r, lufs_at);
    (l, r)
}

/// Gravis on the roots in eighths, the last eighth of each bar up an octave.
fn bass_project() -> Project {
    let mut p = Project::new("bass");
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Bass");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Bass));
    let mut c = p.make_clip("Roots", Ticks::ZERO, tick(BARS, 0));
    for bar in 0..BARS {
        let root = ROOTS[bar % 4];
        for e in 0..8 {
            let key = if e == 7 { root + 12 } else { root };
            let vel = if e % 2 == 0 { 104 } else { 84 };
            let n = p.make_note(tick(bar, e * 2), Ticks(PPQ / 2 - PPQ / 16), key, vel);
            c.notes.push(n);
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn render_project(p: &Project, id: u64) -> Stereo {
    let (mut engine, _proc) = EngineHandle::manual(f64::from(RATE));
    let snap = engine.compile_for_render(p, id);
    let r = render(
        snap,
        &RenderOptions {
            sample_rate: RATE,
            start: Ticks::ZERO,
            end: Some(tick(BARS, 0)),
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .expect("it renders");
    let (mut l, mut r): Stereo = interleaved_to_stereo(r.samples, 2, RATE);
    l.resize(eight_bars(), 0.0);
    r.resize(eight_bars(), 0.0);
    (l, r)
}

/// The recorded drums (`cantus_ui::tympanum_load::install` the content root first), at
/// `lufs`, eight bars.
pub fn drums(lufs_at: f64) -> Stereo {
    let def = cantus_engine::inst::tympanum::cache::recorded(
        &cantus_core::inst::tympanum::KitRef::parse(KIT).expect("pack/kit"),
    )
    .expect("the recorded kit reads (is Gogodze Phu Vol. II installed in the content root?)");
    for line in &def.log {
        println!("kit: {line}");
    }
    let (mut l, mut r) = render_project(&drum_project(), 1);
    set_lufs(&mut l, &mut r, lufs_at);
    (l, r)
}

/// The small mix: the guitar at −21 LUFS, the drums at −21, the bass at −24 (by their own
/// gains), summed, then brought to `lufs_at` as one.
pub fn mix(guitar: &Stereo, lufs_at: f64) -> Stereo {
    let (dl, dr) = drums(-21.0);
    let (mut bl, mut br) = render_project(&bass_project(), 2);
    set_lufs(&mut bl, &mut br, -24.0);
    let (mut gl, mut gr) = guitar.clone();
    set_lufs(&mut gl, &mut gr, -21.0);
    let n = eight_bars();
    let mut l: Vec<f32> = (0..n).map(|i| dl[i] + bl[i] + gl[i]).collect();
    let mut r: Vec<f32> = (0..n).map(|i| dr[i] + br[i] + gr[i]).collect();
    set_lufs(&mut l, &mut r, lufs_at);
    (l, r)
}

/// Brings a take to `target` LUFS by a gain.
pub fn set_lufs(l: &mut [f32], r: &mut [f32], target: f64) {
    let g = cantus_dsp::math::db_to_gain((target - lufs(l, r)) as f32);
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
}

/// The take through `slot` on the engine's own unit, the song playing from bar 1, its
/// latency taken off (aligned with the dry take).
pub fn through(
    l: &[f32],
    r: &[f32],
    slot: &EffectSlot,
    bpm: f64,
    notes: &[(usize, FxNote)],
) -> Stereo {
    let mut u = EffectUnit::new(slot, RATE as f32);
    u.configure(&slot.spec);
    if let Some(need) = u.memory_need() {
        drop(u.supply(EffectMemory::build(need, RATE as f32)));
    }
    let lat = u.latency();
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    ol.extend(std::iter::repeat_n(0.0, lat));
    or.extend(std::iter::repeat_n(0.0, lat));
    let mut at = 0usize;
    while at < ol.len() {
        let n = 480.min(ol.len() - at);
        for (when, note) in notes.iter().filter(|(w, _)| (at..at + n).contains(w)) {
            u.note(when - at, *note);
        }
        u.set_position(&SongPosition {
            playing: true,
            tempo_bpm: bpm,
            tempo_now: bpm,
            beats: at as f64 * bpm / 60.0 / f64::from(RATE),
            samples: at as i64,
            ..Default::default()
        });
        u.process(&mut ol[at..at + n], &mut or[at..at + n]);
        at += n;
    }
    (ol[lat..].to_vec(), or[lat..].to_vec())
}

/// Bars 5 to 8 at `bpm`.
pub fn bars_5_to_8(x: &[f32], bpm: f64) -> Vec<f32> {
    let half = BARS / 2 * 4 * beat_frames(bpm);
    x[half..(2 * half).min(x.len())].to_vec()
}

pub fn lufs(l: &[f32], r: &[f32]) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    m.process(l, r);
    m.integrated().unwrap_or(-120.0)
}

pub fn peak_db(l: &[f32], r: &[f32]) -> f64 {
    let p = l
        .iter()
        .chain(r)
        .fold(0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    20.0 * cantus_dsp::math::log10(f64::from(p))
}

/// The gain that brings a take to [`TARGET`], lowered as one where a peak would pass
/// [`CEILING`].
pub fn site_gain(l: &[f32], r: &[f32]) -> f64 {
    (TARGET - lufs(l, r)).min(CEILING - peak_db(l, r))
}

/// The take at `gain_db` as the site's MP3 (CBR 96 kbit/s, 48 kHz); its size.
pub fn mp3_by(
    l: &[f32],
    r: &[f32],
    gain_db: f64,
    path: &Path,
    title: &str,
    artist: &str,
    album: &str,
) -> u64 {
    let g = cantus_dsp::math::db_to_gain(gain_db as f32);
    let samples: Vec<f32> = l
        .iter()
        .zip(r)
        .flat_map(|(a, b)| [*a * g, *b * g])
        .collect();
    let s = ExportSettings {
        format: Format::Mp3,
        mp3: Mp3Mode::Cbr(96),
        rate: Rate::Hz(RATE),
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: artist.into(),
        album: album.into(),
        ..Default::default()
    };
    let audio = Audio {
        rate: RATE,
        channels: 2,
        samples,
    };
    export_rendered(audio, &s, &tags, path, &Control::new()).expect("the take is written");
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// The app with the unit's editor open, fed as playing, the preset loaded as the browser
/// loads it; and the slot as it is then — heard alone on the take: no key from another
/// track, no notes, all wet.
pub fn app_with(demo: &str, kind: EffectKind, values: &Values) -> (CantusApp, EffectSlot) {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(demo);
    views::fx::demo_feed(&mut app, kind);
    let (strip, effect, _) = app.fx.open.expect("the editor is open");
    app.apply_fx_params(
        strip,
        effect,
        values,
        cantus_ui::preset_browser::APPLY_LABEL,
    );
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let mut slot = app
        .project()
        .effect(strip, effect)
        .cloned()
        .expect("the slot");
    slot.id = EffectId(1);
    slot.bypass = false;
    slot.mix = 1.0;
    slot.sidechain = None;
    slot.midi_from = None;
    (app, slot)
}

/// A factory preset's values, by its kind and the last part of its id.
pub fn preset_values(kind: EffectKind, id: &str) -> Values {
    cantus_ui::preset_catalog::entries(cantus_ui::preset_catalog::Unit::Effect(kind), false)
        .into_iter()
        .find_map(|e| match e.payload {
            cantus_ui::preset_catalog::Payload::Effect(v)
                if e.id.rsplit('.').next() == Some(id) =>
            {
                Some(v)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no preset {id}"))
}

/// A slot of `kind` with a factory preset's values, all wet, heard alone.
pub fn slot_of(kind: EffectKind, values: &Values) -> EffectSlot {
    let mut scratch = Project::new("scratch");
    let mut slot = scratch.make_effect(kind);
    slot.spec.params = cantus_ui::preset_audition::effect_params_after(&slot.spec, values);
    slot.mix = 1.0;
    slot
}

/// The share of a take's energy above 6 kHz, dB (a fourth-order Butterworth high-pass, so the
/// 2–4 kHz presence a drive and a cabinet keep is not counted): the fizz a distortion leaves
/// on a direct guitar with no cabinet after it.
pub fn fizz_db(l: &[f32], r: &[f32]) -> f64 {
    let w = 2.0 * std::f64::consts::PI * 6000.0 / f64::from(RATE);
    let (c, s) = (cantus_dsp::math::cos(w), cantus_dsp::math::sin(w));
    // The two sections of a fourth-order Butterworth.
    let sections: Vec<[f64; 5]> = [0.541_196_1, 1.306_563]
        .iter()
        .map(|q| {
            let alpha = s / (2.0 * q);
            let a0 = 1.0 + alpha;
            [
                (1.0 + c) / 2.0 / a0,
                -(1.0 + c) / a0,
                (1.0 + c) / 2.0 / a0,
                -2.0 * c / a0,
                (1.0 - alpha) / a0,
            ]
        })
        .collect();
    let (mut high, mut all) = (0f64, 0f64);
    for x in [l, r] {
        let mut st = [[0f64; 4]; 2];
        for v in x {
            let v = f64::from(*v);
            all += v * v;
            let mut y = v;
            for (k, [b0, b1, b2, a1, a2]) in sections.iter().enumerate() {
                let [x1, x2, y1, y2] = st[k];
                let out = b0 * y + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                st[k] = [y, x1, out, y1];
                y = out;
            }
            high += y * y;
        }
    }
    10.0 * cantus_dsp::math::log10((high / all.max(1e-12)).max(1e-12))
}
