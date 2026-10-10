//! What an offline render writes (round 23, the export audit): a selection's tail rings
//! out what was playing and plays nothing that starts after the selection, and sample 0 of
//! a render is the first tick of the range — a latent effect (a limiter's look-ahead) does
//! not make the file late, nor cut its last samples.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;
use support::*;

fn opts(end: Option<Ticks>, tail: f64) -> RenderOptions {
    RenderOptions {
        end,
        tail_seconds: tail,
        block_frames: 256,
        compensate_latency: true,
        ..Default::default()
    }
}

fn render_project(p: &Project, o: &RenderOptions) -> Vec<f32> {
    render(Arc::new(Snapshot::compile(p, 1)), o)
        .unwrap()
        .samples
}

/// `p` without the notes that start at or after `at` (on the timeline).
fn without_notes_from(p: &Project, at: Ticks) -> Project {
    let mut q = p.clone();
    for t in &mut q.tracks {
        for c in &mut t.clips {
            let start = c.start;
            c.notes.retain(|n| start + n.start < at);
        }
    }
    q
}

#[test]
fn a_selection_tail_rings_out_without_the_music_after_it() {
    let p = demo_project();
    let to = Ticks::beats(4);
    // Every note before beat 4 ends before it, so the selection's music is exactly the
    // project without what starts at beat 4 or later.
    let cut = without_notes_from(&p, to);
    for block in [64usize, 256, 1000] {
        let mut o = opts(Some(to), 1.0);
        o.block_frames = block;
        let sel = render_project(&p, &o);
        let want = render_project(&cut, &o);
        assert_eq!(sel.len(), want.len());
        assert_null(
            &sel,
            &want,
            0.0,
            &format!("block {block}: the tail played music from after the selection"),
        );
    }
    // And the tail is not silence: the chord's release and the strips still ring.
    let sel = render_project(&p, &opts(Some(to), 1.0));
    let content = (to.to_seconds(p.tempo_bpm) * 48_000.0).ceil() as usize;
    assert!(
        sel[content * 2..content * 2 + 200]
            .iter()
            .any(|x| *x != 0.0),
        "the release rings into the tail"
    );
}

/// The lead alone, centred, at unity on its strip.
fn lead_alone() -> Project {
    let mut p = demo_project();
    p.tracks.truncate(1);
    let s = strip_of(&mut p, 0);
    s.pan = 0.0;
    s.gain_db = 0.0;
    p
}

/// A limiter that does nothing but delay: 0 dB of gain, the ceiling at 0 dBFS over a mix
/// that peaks lower, true peak on (the longest look-ahead).
fn with_idle_limiter(p: &Project, on_master: bool) -> Project {
    let mut q = p.clone();
    let mut slot = q.make_effect(EffectKind::Limiter);
    slot.spec.set("ceiling", 0.0).unwrap();
    slot.spec.set("tp", 1.0).unwrap();
    if on_master {
        q.master.effects.push(slot);
    } else {
        strip_of(&mut q, 0).effects.push(slot);
    }
    q
}

#[test]
fn a_latent_effect_does_not_make_the_compensated_render_late() {
    let dry = lead_alone();
    let plain = render_project(&dry, &opts(None, 0.25));
    let peak = plain.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak < 0.9, "the test mix is too loud for this test: {peak}");
    for on_master in [false, true] {
        let wet = with_idle_limiter(&dry, on_master);
        let snap = Arc::new(Snapshot::compile(&wet, 1));
        let r = render(snap, &opts(None, 0.25)).unwrap();
        assert_eq!(r.samples.len(), plain.len(), "the same length");
        assert_null(
            &r.samples,
            &plain,
            0.0,
            &format!("master={on_master}: an idle limiter must not shift or cut the render"),
        );
        // Without the option the render is what playing gives: late by the latency.
        let raw = RenderOptions {
            compensate_latency: false,
            ..opts(None, 0.25)
        };
        let late = render(Arc::new(Snapshot::compile(&wet, 1)), &raw).unwrap();
        assert_ne!(
            late.samples, plain,
            "master={on_master}: the raw render is late"
        );
    }
}

#[test]
fn the_mix_equals_the_master_stem_with_a_latent_master() {
    let p = with_idle_limiter(&demo_project(), true);
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let o = opts(None, 0.25);
    let mix = render(snap.clone(), &o).unwrap();
    let specs = [StemSpec {
        source: StemSource::Master,
        file_name: "00 Master.wav".into(),
    }];
    let stems = render_stems(
        snap,
        &StemOptions {
            render: o,
            ..Default::default()
        },
        &specs,
    )
    .unwrap();
    assert_eq!(stems[0].samples.len(), mix.samples.len());
    assert_null(&stems[0].samples, &mix.samples, 0.0, "00 Master is the mix");
}

/// A note exactly on the selection's end, at tempos and rates where the end's sample is a
/// whole number that a seconds-based sum lands one ulp above (44.1 kHz, 140 bpm, three
/// bars, found by the round's review): the stop must come before the note fires.
#[test]
fn a_note_on_the_selections_end_tick_does_not_play_at_any_tempo() {
    for (bpm, rate, beats) in [
        (140.0, 44_100, 12),
        (140.0, 44_100, 24),
        (100.0, 44_100, 36),
        (75.0, 48_000, 44),
        (128.0, 48_000, 16),
    ] {
        let mut p = Project::new("edge");
        p.tempo_bpm = bpm;
        let mut t = p.make_track("Keys");
        let end = Ticks::beats(beats);
        let mut c = p.make_clip("c", Ticks::ZERO, end + Ticks::beats(4));
        let first = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        let on_the_end = p.make_note(end, Ticks::beats(2), 72, 127);
        c.notes.extend([first, on_the_end]);
        t.clips.push(c);
        p.tracks.push(t);
        let cut = without_notes_from(&p, end);
        let o = RenderOptions {
            sample_rate: rate,
            ..opts(Some(end), 1.0)
        };
        let sel = render_project(&p, &o);
        let want = render_project(&cut, &o);
        assert_null(
            &sel,
            &want,
            0.0,
            &format!("{bpm} bpm, {rate} Hz, {beats} beats: the note on the end tick played"),
        );
    }
}

#[test]
fn a_stems_selection_tail_plays_nothing_after_the_selection() {
    let p = demo_project();
    let to = Ticks::beats(4);
    let cut = without_notes_from(&p, to);
    let specs = [
        StemSpec {
            source: StemSource::Master,
            file_name: "00 Master.wav".into(),
        },
        StemSpec {
            source: StemSource::Track(1),
            file_name: "02 Bass.wav".into(),
        },
    ];
    let stems_of = |q: &Project| {
        render_stems(
            Arc::new(Snapshot::compile(q, 1)),
            &StemOptions {
                render: opts(Some(to), 1.0),
                keep_silent: true,
                ..Default::default()
            },
            &specs,
        )
        .unwrap()
    };
    let (a, b) = (stems_of(&p), stems_of(&cut));
    for (x, y) in a.iter().zip(&b) {
        assert_null(&x.samples, &y.samples, 0.0, &x.spec.file_name);
    }
}
