//! The native instruments' output levels (round 19, fixes-mix):
//!
//! - **headroom**: no factory preset of any native instrument (the native kinds and
//!   Pulsus II's kits) passes 0 dBFS on a dense riff played at velocity 127, at unity
//!   (channel volume 0 dB, fader 0 dB). The slap bass once peaked at +15 dBFS there.
//! - **staging** (ignored, a report): each instrument's default patch on a C-major triad at
//!   velocity 100 for two seconds — the numbers `docs/round19/fixes-mix.md` gives.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, PulsusParams, TrackIcon};
use cantus_engine::{render, RenderOptions, Rendered};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use std::sync::Arc;

mod measure;
use measure::{db, lufs, true_peak_db};

const KINDS: [InstrumentKind; 8] = [
    InstrumentKind::Wavetable,
    InstrumentKind::Analog,
    InstrumentKind::Fm,
    InstrumentKind::Sampler,
    InstrumentKind::Bass,
    InstrumentKind::ElectricPiano,
    InstrumentKind::Organ,
    InstrumentKind::Chorda,
];

/// One part playing `evs` (beats) at 120 BPM, at unity, nothing on the master.
fn song_of(spec: InstrumentSpec, evs: &[Ev], bars: f64) -> Project {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut s = Song::new("level", 120.0, 1);
    let part = s.part("part", spec, colors::BASS, TrackIcon::Keys);
    s.feel(part, Feel::TIGHT);
    s.clip(part, "riff", 0.0, bars, evs);
    s.finish()
}

fn render_all(p: &Project, tail: f64) -> Rendered {
    let snap = cantus_core::Snapshot::compile(p, 0);
    render(
        Arc::new(snap),
        &RenderOptions {
            tail_seconds: tail,
            ..Default::default()
        },
    )
    .expect("it renders")
}

/// Two bars of a dense riff at velocity 127: sixteenths over two octaves (repeated notes,
/// octave jumps, a run) under four-note chords on every beat — the most a part asks of an
/// instrument at once. A mono instrument plays it as a line; a kit hears every pad.
fn dense_riff(low: u8) -> Vec<Ev> {
    let line = [0, 0, 12, 0, 7, 0, 10, 12, 0, 0, 12, 3, 5, 7, 10, 12];
    let mut evs = Vec::new();
    for bar in 0..2 {
        for (i, step) in line.iter().enumerate() {
            let at = bar as f64 * 4.0 + i as f64 * 0.25;
            evs.push(Ev::new(at, 0.2, low + step, 127));
        }
        for beat in 0..4 {
            let at = bar as f64 * 4.0 + beat as f64;
            for n in [24, 28, 31, 36] {
                evs.push(Ev::new(at, 0.9, low + n, 127));
            }
        }
    }
    evs
}

/// Two bars of a busy groove on a kit at velocity 127, as a drummer plays it (four limbs:
/// at most four pads at once): kick, snare or clap on two and four, closed hats in
/// sixteenths, a crash on the one, an open hat, a tom fill at the end. A pad the kit lacks
/// is played by the next pad of the kit not already in the groove, so every kit is busy.
fn kit_riff(kit: &PulsusParams) -> Vec<Ev> {
    let have: Vec<u8> = kit.pads.iter().map(|p| p.pitch).collect();
    let mut used: Vec<u8> = Vec::new();
    let mut pick = |wanted: &[u8]| -> Option<u8> {
        let p = wanted
            .iter()
            .copied()
            .find(|w| have.contains(w) && !used.contains(w))
            .or_else(|| have.iter().copied().find(|h| !used.contains(h)))?;
        used.push(p);
        Some(p)
    };
    let kick = pick(&[36, 35]);
    let snare = pick(&[38, 40, 39, 37]);
    let hat = pick(&[42, 44]);
    let crash = pick(&[49, 57, 55]);
    let open = pick(&[46]);
    let toms: Vec<u8> = [&[50u8, 48][..], &[47, 45], &[43, 41]]
        .iter()
        .filter_map(|w| pick(w))
        .collect();
    let mut evs = Vec::new();
    let mut hit = |pad: Option<u8>, at: f64| {
        if let Some(p) = pad {
            evs.push(Ev::new(at, 0.2, p, 127));
        }
    };
    for bar in 0..2 {
        let b = bar as f64 * 4.0;
        for step in 0..16 {
            let at = b + step as f64 * 0.25;
            let fill = bar == 1 && step >= 12;
            if !fill && step != 14 {
                hit(hat, at);
            }
            if fill && !toms.is_empty() {
                hit(Some(toms[(step - 12) % toms.len()]), at);
            }
        }
        for at in [0.0, 1.5, 2.0, 2.75] {
            hit(kick, b + at);
        }
        hit(snare, b + 1.0);
        hit(snare, b + 3.0);
        hit(open, b + 3.5);
    }
    hit(crash, 0.0);
    hit(kick, 8.0 - 0.25);
    evs
}

/// Every factory preset with what it is (kind or kit) and its spec.
fn every_preset() -> Vec<(String, InstrumentSpec, bool)> {
    let mut out = Vec::new();
    for kind in KINDS {
        // The default patch too: what a new instance is (ADR 0039 raised two of them).
        out.push((
            format!("{kind:?} default"),
            InstrumentSpec::Native(NativeSpec::new(kind)),
            false,
        ));
        for preset in cantus_ui::views::inst::factory_presets(kind) {
            let spec = cantus_ui::views::inst::preset_spec(kind, preset);
            out.push((
                preset.label_key.to_string(),
                InstrumentSpec::Native(spec),
                false,
            ));
        }
    }
    for kit in PulsusParams::factory() {
        out.push((
            kit.label_key.to_string(),
            InstrumentSpec::Pulsus(kit.kit),
            true,
        ));
    }
    out
}

/// The register a dense riff is played in: a bass in its own, everything else from C2.
fn low_note(spec: &InstrumentSpec) -> u8 {
    match spec {
        InstrumentSpec::Native(n) if n.kind == InstrumentKind::Bass => 28,
        _ => 36,
    }
}

fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|c| {
                let f = &f;
                scope.spawn(move || c.iter().map(f).collect::<Vec<R>>())
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a render panicked"))
            .collect()
    })
}

#[test]
fn no_factory_preset_passes_full_scale_on_a_dense_riff() {
    let presets = every_preset();
    let results = par_map(&presets, |(name, spec, is_kit)| {
        let evs = match spec {
            InstrumentSpec::Pulsus(k) if *is_kit => kit_riff(k),
            _ => dense_riff(low_note(spec)),
        };
        let r = render_all(&song_of(spec.clone(), &evs, 2.0), 1.0);
        assert!(
            r.samples.iter().all(|x| x.is_finite()),
            "{name}: a sample that is not a number"
        );
        (name.clone(), db(r.peak), true_peak_db(&r))
    });
    let mut over = Vec::new();
    for (name, peak, tp) in &results {
        eprintln!("{name:<32} peak {peak:>6.1} dBFS  {tp:>6.1} dBTP");
        if *peak > 0.0 {
            over.push(format!("{name}: {peak:+.1} dBFS"));
        }
    }
    assert!(over.is_empty(), "past full scale: {over:?}");
}

/// The default patch of each instrument on a C-major triad (C4 E4 G4), velocity 100,
/// two seconds: integrated loudness and peaks. Pulsus II: the default kit's kick, snare
/// and closed hat together, each beat. `cargo test --release -p cantus-ui --test
/// instrument_levels -- --ignored --nocapture`.
#[test]
#[ignore]
fn default_patches_staging_report() {
    let mut cases: Vec<(String, InstrumentSpec, Vec<Ev>)> = Vec::new();
    let triad: Vec<Ev> = [60u8, 64, 67]
        .iter()
        .map(|&n| Ev::new(0.0, 4.0, n, 100))
        .collect();
    for kind in KINDS {
        let spec = InstrumentSpec::Native(NativeSpec::new(kind));
        let evs = if kind == InstrumentKind::Bass {
            // A mono bass: the triad's root, an octave down (where a bass plays).
            vec![Ev::new(0.0, 4.0, 48, 100)]
        } else {
            triad.clone()
        };
        cases.push((format!("{kind:?}"), spec, evs));
    }
    let kit = PulsusParams::default();
    let hits: Vec<Ev> = (0..4)
        .flat_map(|b| {
            [36u8, 38, 42]
                .into_iter()
                .map(move |n| Ev::new(b as f64, 0.25, n, 100))
        })
        .collect();
    cases.push(("Pulsus II".into(), InstrumentSpec::Pulsus(kit), hits));
    let rows = par_map(&cases, |(name, spec, evs)| {
        let r = render_all(&song_of(spec.clone(), evs, 1.0), 0.0);
        (name.clone(), lufs(&r), db(r.peak), true_peak_db(&r))
    });
    println!("| Instrument | LUFS | Peak dBFS | dBTP |");
    println!("|---|---|---|---|");
    for (name, l, p, tp) in rows {
        println!("| {name} | {l:.1} | {p:.1} | {tp:.1} |");
    }
}

/// Every factory preset of every native instrument on the staging report's material (a
/// C-major triad at velocity 100 for two seconds; a bass its root an octave down; a kit
/// its kick, snare and hat on every beat): where each preset sits beside its instrument's
/// default patch. `cargo test --release -p cantus-ui --test instrument_levels --
/// --ignored --nocapture factory_presets_staging_report`.
#[test]
#[ignore]
fn factory_presets_staging_report() {
    let triad: Vec<Ev> = [60u8, 64, 67]
        .iter()
        .map(|&n| Ev::new(0.0, 4.0, n, 100))
        .collect();
    let root = vec![Ev::new(0.0, 4.0, 48, 100)];
    let mut cases: Vec<(String, InstrumentSpec, Vec<Ev>)> = Vec::new();
    for kind in KINDS {
        let evs = if kind == InstrumentKind::Bass {
            root.clone()
        } else {
            triad.clone()
        };
        cases.push((
            format!("{kind:?} (default)"),
            InstrumentSpec::Native(NativeSpec::new(kind)),
            evs.clone(),
        ));
        for preset in cantus_ui::views::inst::factory_presets(kind) {
            let spec = cantus_ui::views::inst::preset_spec(kind, preset);
            cases.push((
                format!("{kind:?} {}", preset.label_key),
                InstrumentSpec::Native(spec),
                evs.clone(),
            ));
        }
    }
    let rows = par_map(&cases, |(name, spec, evs)| {
        let r = render_all(&song_of(spec.clone(), evs, 1.0), 0.0);
        (name.clone(), lufs(&r), db(r.peak))
    });
    println!("| Preset | LUFS | Peak dBFS |");
    println!("|---|---|---|");
    for (name, l, p) in rows {
        println!("| {name} | {l:.1} | {p:.1} |");
    }
}
