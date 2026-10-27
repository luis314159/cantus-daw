//! Round 31, editing (ADRs 0070, 0071) through the application: automatic crossfades on
//! an app's bus, the crossfade's square and editor and an envelope's points from the real
//! arrangement (each gesture one undo step), the menus, and effects inserted on their
//! starting preset (and neutral when asked), every starting preset a factory preset.

mod support;

use cantus_core::clip_gain::{GainEnvelope, GainPoint};
use cantus_core::crossfade::XfadeCurve;
use cantus_core::*;
use cantus_ui::{views, CantusApp};
use egui::{pos2, Pos2};
use std::path::Path;

/// A mono 32-bit WAV, `frames` long, a quiet square wave.
fn wav(path: &Path, frames: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample(if i % 2 == 0 { 0.4f32 } else { -0.4 })
            .unwrap();
    }
    w.finalize().unwrap();
}

/// A track with two clips of one file: the first from beat 0 to 4, the second from beat
/// `second` for four beats (its content from beat 8). An app built from parts (its
/// automatic crossfades on, as the settings' default).
fn two_clips(dir: &Path, second: i64) -> (CantusApp, TrackId, ClipId, ClipId) {
    let file = dir.join("take.wav");
    wav(&file, 48_000 * 10);
    let mut p = Project::empty("Edit");
    let mut t = p.make_track("Vox");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: file.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 10,
        sha256: None,
        recorded: false,
    });
    let a = p.make_audio_clip("a", id, Ticks::ZERO, Ticks::beats(4));
    let mut b = p.make_audio_clip("b", id, Ticks::beats(second), Ticks::beats(4));
    b.offset = Ticks::beats(8);
    let (ia, ib) = (a.id, b.id);
    t.clips.extend([a, b]);
    let tid = t.id;
    p.tracks.push(t);
    (support::app(p), tid, ia, ib)
}

fn region(a: &CantusApp, t: TrackId, c: ClipId) -> AudioRegion {
    a.project()
        .track(t)
        .and_then(|t| t.clip(c))
        .and_then(|c| c.audio.clone())
        .unwrap()
}

fn clip(a: &CantusApp, t: TrackId, c: ClipId) -> Clip {
    a.project()
        .track(t)
        .and_then(|t| t.clip(c))
        .cloned()
        .unwrap()
}

fn show(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1400.0, 500.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(a, ui));
    })
    .textures_delta
    .clear();
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press at what `aim` picks from the frame just drawn, travel, release.
fn drag(a: &mut CantusApp, aim: impl Fn(&CantusApp) -> (Pos2, Pos2)) {
    drag_steps(a, 4, aim);
}

/// [`drag`] in `steps` frames of travel.
fn drag_steps(a: &mut CantusApp, steps: usize, aim: impl Fn(&CantusApp) -> (Pos2, Pos2)) {
    let ctx = support::ctx();
    show(a, &ctx, vec![]);
    show(a, &ctx, vec![]);
    let (from, to) = aim(a);
    show(a, &ctx, vec![egui::Event::PointerMoved(from)]);
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=steps {
        let at = from + (to - from) * (i as f32 / steps as f32);
        show(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    }
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    show(a, &ctx, vec![]);
}

/// A double-click at what `aim` picks.
fn double_click(a: &mut CantusApp, aim: impl Fn(&CantusApp) -> Pos2) {
    let ctx = support::ctx();
    show(a, &ctx, vec![]);
    show(a, &ctx, vec![]);
    let at = aim(a);
    show(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..2 {
        show(a, &ctx, vec![button(at, true)]);
        show(a, &ctx, vec![button(at, false)]);
    }
    show(a, &ctx, vec![]);
}

// ---- crossfades --------------------------------------------------------------------------

#[test]
fn an_app_crosses_the_overlap_an_edit_makes_unless_told_not_to() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, _, b) = two_clips(dir.path(), 8);
    assert!(a.settings.auto_crossfade, "on by default");
    assert_eq!(a.bus.auto_crossfades(), Some(XfadeCurve::EqualPower));
    let steps = a.bus.history().len();
    assert!(a.exec(Command::SetClipBounds {
        track: t,
        clip: b,
        start: Ticks::beats(3),
        length: Ticks::beats(4),
    }));
    assert_eq!(region(&a, t, b).cross, Some(XfadeCurve::EqualPower));
    assert_eq!(a.bus.history().len(), steps + 1, "one step");
    a.undo();
    assert_eq!(region(&a, t, b).cross, None);
    // Off: the overlap is left summed.
    a.set_auto_crossfade(false);
    assert_eq!(a.bus.auto_crossfades(), None);
    assert!(a.exec(Command::SetClipBounds {
        track: t,
        clip: b,
        start: Ticks::beats(3),
        length: Ticks::beats(4),
    }));
    assert_eq!(region(&a, t, b).cross, None);
    // The clip's menu entry crosses it by hand, one step; again takes it away.
    assert!(a.set_crossfade((t, b), Some(XfadeCurve::Linear)));
    assert_eq!(region(&a, t, b).cross, Some(XfadeCurve::Linear));
    assert!(a.set_crossfade((t, b), None));
    assert_eq!(region(&a, t, b).cross, None);
}

#[test]
fn dragging_the_crossfades_square_makes_it_longer_about_its_centre_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, ia, ib) = two_clips(dir.path(), 8);
    assert!(a.exec(Command::SetClipBounds {
        track: t,
        clip: ib,
        start: Ticks::beats(3),
        length: Ticks::beats(4),
    }));
    let steps = a.bus.history().len();
    drag(&mut a, |a| {
        let h = a.arrangement.audio_handles((t, ib)).expect("handles");
        let x = h.xfade.expect("the crossfade's square");
        let g = a.arrangement.geometry().unwrap();
        (
            x.square.center(),
            x.square.center() + egui::vec2(g.px_per_beat * 0.25, 0.0),
        )
    });
    // A quarter beat each way: from one beat to a beat and a half, about beat 3.5.
    let (l, r) = (clip(&a, t, ia), clip(&a, t, ib));
    let len = l.end() - r.start;
    assert!((len.0 - 1_440).abs() <= 4, "{len:?}");
    assert!(((l.end().0 + r.start.0) / 2 - 3_360).abs() <= 2, "centred");
    assert_eq!(
        r.offset - Ticks::beats(8),
        r.start - Ticks::beats(3),
        "content stays"
    );
    assert_eq!(a.bus.history().len(), steps + 1, "one gesture, one step");
    a.undo();
    assert_eq!(clip(&a, t, ib).start, Ticks::beats(3));
    assert_eq!(clip(&a, t, ia).end(), Ticks::beats(4));
}

#[test]
fn a_double_click_on_the_square_opens_its_editor_and_the_editor_sets_the_curve() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, _, ib) = two_clips(dir.path(), 3);
    assert!(a.set_crossfade((t, ib), Some(XfadeCurve::EqualPower)));
    double_click(&mut a, |a| {
        a.arrangement
            .audio_handles((t, ib))
            .and_then(|h| h.xfade)
            .expect("the square")
            .square
            .center()
    });
    assert_eq!(a.arrangement.xfade_editor(), Some((t, ib)));
    // Esc closes it.
    let ctx = support::ctx();
    show(
        &mut a,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(a.arrangement.xfade_editor(), None);
}

#[test]
fn a_crossfade_replaces_the_fades_it_covers_on_screen() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, ia, ib) = two_clips(dir.path(), 3);
    let mut r = region(&a, t, ib);
    r.fade_in = Ticks::QUARTER;
    r.cross = Some(XfadeCurve::SCurve);
    assert!(a.exec(Command::SetClipAudio {
        track: t,
        clip: ib,
        audio: Some(r),
    }));
    let ctx = support::ctx();
    show(&mut a, &ctx, vec![]);
    show(&mut a, &ctx, vec![]);
    let hb = a.arrangement.audio_handles((t, ib)).unwrap();
    assert!(hb.xfade.is_some());
    assert!(
        hb.fade_in.width().is_nan() || hb.fade_in.width() <= 0.0,
        "no fade-in square under a crossfade"
    );
    let ha = a.arrangement.audio_handles((t, ia)).unwrap();
    assert!(
        ha.fade_out.width().is_nan() || ha.fade_out.width() <= 0.0,
        "nor the earlier clip's fade out"
    );
    assert!(ha.xfade.is_none(), "drawn on the later clip");
}

// ---- gain envelopes --------------------------------------------------------------------

#[test]
fn a_press_on_the_line_adds_a_point_and_drags_it_and_a_double_click_deletes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, ia, _) = two_clips(dir.path(), 8);
    a.settings.show_clip_gain = true;
    a.select_clip(t, ia);
    let steps = a.bus.history().len();
    drag(&mut a, |a| {
        let h = a.arrangement.audio_handles((t, ia)).unwrap();
        let e = h.env.expect("the line, shown");
        let g = a.arrangement.geometry().unwrap();
        // Beat 1: clear of the clip gain's square at the centre.
        let x = g.x(1.0);
        let at = pos2(x, e.line_y(x));
        assert_eq!(
            h.at(at),
            Some(views::arrangement::AudioHandle::EnvLine(960))
        );
        (at, at + egui::vec2(0.0, 40.0))
    });
    let env = region(&a, t, ia).gain_env.expect("an envelope");
    assert_eq!(env.points.len(), 1);
    let p = env.points[0];
    assert!((p.tick.0 - 960).abs() <= 12, "at beat 1: {:?}", p.tick);
    assert!(p.db < -1.0, "dragged down: {}", p.db);
    assert_eq!(
        a.bus.history().len(),
        steps + 1,
        "added and dragged, one step"
    );
    // A double-click on the point takes it away.
    double_click(&mut a, |a| {
        let e = a.arrangement.audio_handles((t, ia)).unwrap().env.unwrap();
        e.pts[0]
    });
    assert!(
        region(&a, t, ia).gain_env.is_none(),
        "no points, no envelope"
    );
    a.undo();
    a.undo();
    assert!(region(&a, t, ia).gain_env.is_none());
}

#[test]
fn a_point_is_dragged_and_held_between_its_neighbours() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, ia, _) = two_clips(dir.path(), 8);
    let mut r = region(&a, t, ia);
    r.gain_env = Some(Box::new(GainEnvelope::new(vec![
        GainPoint::new(Ticks::beats(1), 0.0),
        GainPoint::new(Ticks::beats(2), -6.0),
        GainPoint::new(Ticks::beats(3), 0.0),
    ])));
    assert!(a.exec(Command::SetClipAudio {
        track: t,
        clip: ia,
        audio: Some(r),
    }));
    a.settings.show_clip_gain = true;
    a.select_clip(t, ia);
    drag(&mut a, |a| {
        let e = a.arrangement.audio_handles((t, ia)).unwrap().env.unwrap();
        let i = (0..e.n).find(|&i| e.idx[i] == 1).unwrap();
        let g = a.arrangement.geometry().unwrap();
        (e.pts[i], e.pts[i] + egui::vec2(g.px_per_beat * 3.0, -20.0))
    });
    let env = region(&a, t, ia).gain_env.unwrap();
    assert_eq!(env.points[1].tick, Ticks::beats(3), "held at its neighbour");
    assert!(env.points[1].db > -6.0, "and raised");
    assert!(env.is_valid());
}

// ---- effects on insert -----------------------------------------------------------------

fn a_strip(a: &CantusApp) -> StripId {
    a.project().strips[0].id
}

#[test]
fn an_insert_starts_on_its_starting_preset_and_neutral_when_asked() {
    let mut a = support::app(Project::new("Inserts"));
    let s = a_strip(&a);
    assert!(!a.settings.insert_neutral, "starting presets by default");
    let steps = a.bus.history().len();
    a.add_effect(s, EffectKind::Chorus);
    assert_eq!(a.bus.history().len(), steps + 1, "one step");
    let fx = a.project().effects_of(s).unwrap().last().unwrap().clone();
    assert_eq!(
        fx.spec,
        cantus_core::starting::starting_spec(EffectKind::Chorus)
    );
    assert_eq!(fx.spec.get("mix"), 0.5, "Juno I sounds");
    // A unit that starts neutral starts at its defaults.
    a.add_effect(s, EffectKind::Limiter);
    let lim = a.project().effects_of(s).unwrap().last().unwrap().clone();
    assert_eq!(lim.spec, EffectSpec::new(EffectKind::Limiter));
    // A swap takes the new unit's starting preset.
    let picked = cantus_ui::picker::PickChoice::Effect(EffectKind::Flanger);
    assert!(a.replace_effect_with(s, fx.id, &picked));
    let swapped = a
        .project()
        .effects_of(s)
        .unwrap()
        .iter()
        .find(|e| e.spec.kind == EffectKind::Flanger)
        .unwrap()
        .clone();
    assert_eq!(
        swapped.spec,
        cantus_core::starting::starting_spec(EffectKind::Flanger)
    );
    // Neutral settings: the defaults.
    a.set_insert_neutral(true);
    a.add_effect(s, EffectKind::Chorus);
    let neutral = a.project().effects_of(s).unwrap().last().unwrap().clone();
    assert_eq!(neutral.spec, EffectSpec::new(EffectKind::Chorus));
}

#[test]
fn every_starting_preset_is_its_units_factory_preset() {
    for kind in EffectKind::ALL {
        let Some(p) = cantus_core::starting::starting_preset(kind) else {
            continue;
        };
        let factory = views::fx::factory_presets(kind)
            .iter()
            .find(|(k, _)| *k == p.label_key)
            .unwrap_or_else(|| panic!("{kind:?}: no factory preset {}", p.label_key));
        let mut a: Vec<(&str, f32)> = p.params.to_vec();
        let mut b: Vec<(&str, f32)> = factory.1.to_vec();
        a.sort_by(|x, y| x.0.cmp(y.0));
        b.sort_by(|x, y| x.0.cmp(y.0));
        assert_eq!(a, b, "{kind:?}: {}", p.label_key);
    }
}

#[test]
fn an_editor_opened_on_a_starting_preset_names_it() {
    use views::fx::frame::{read_presets, recognise};
    for kind in [
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::VolumeShaper,
    ] {
        let spec = cantus_core::starting::starting_spec(kind);
        let list = read_presets(kind, false);
        let loaded = recognise(&spec, &list).expect("named");
        let key = cantus_core::starting::starting_preset(kind)
            .unwrap()
            .label_key;
        assert_eq!(loaded.name, cantus_ui::i18n::t(key), "{kind:?}");
        // At its defaults, no preset is named.
        assert!(
            recognise(&EffectSpec::new(kind), &list).is_none(),
            "{kind:?}"
        );
    }
}

// ---- review 1 ---------------------------------------------------------------------------

/// Finding 5: the crossfade's square resizes from where the drag began — the same travel
/// gives the same crossfade in 4 frames or 13, even with one clip that cannot grow (a file
/// dropped at its first frame).
#[test]
fn the_squares_drag_ends_where_the_pointer_does_whatever_the_frames() {
    let mut ends = Vec::new();
    for steps in [4usize, 13] {
        let dir = tempfile::tempdir().unwrap();
        let (mut a, t, ia, ib) = two_clips(dir.path(), 3);
        assert!(a.exec(Command::SetClipOffset {
            track: t,
            clip: ib,
            offset: Ticks::ZERO,
        }));
        assert!(a.set_crossfade((t, ib), Some(XfadeCurve::EqualPower)));
        drag_steps(&mut a, steps, |a| {
            let x = a.arrangement.audio_handles((t, ib)).unwrap().xfade.unwrap();
            let g = a.arrangement.geometry().unwrap();
            (
                x.square.center(),
                x.square.center() + egui::vec2(g.px_per_beat, 0.0),
            )
        });
        ends.push((clip(&a, t, ia).end(), clip(&a, t, ib).start));
    }
    assert_eq!(ends[0], ends[1], "{ends:?}");
    // The right clip had nothing before its start: only the left end moved — by the whole
    // lengthening (a beat each way about the centre), where the drag left it, not where
    // frames of creeping centres took it.
    assert_eq!(ends[0].1, Ticks::beats(3));
    assert_eq!(ends[0].0, Ticks::beats(5));
}

/// Finding 7: the editor's length reaches as far as the clips' audio lets it, past 4 s.
#[test]
fn the_editors_length_is_bounded_by_the_audio_not_a_cap() {
    let mut p = Project::empty("x");
    let a = p.make_audio_clip("a", AudioSourceId(1), Ticks::ZERO, Ticks::beats(16));
    let b = p.make_audio_clip("b", AudioSourceId(1), Ticks::beats(8), Ticks::beats(16));
    let base = cantus_ui::clipfx::XfadeBase {
        left: a,
        right: b,
        room: (Ticks::beats(100), Ticks::beats(100)),
    };
    assert_eq!(base.len(), Ticks::beats(8));
    let (lo, hi) = base.span();
    // Two beats a second at 120 BPM: about twelve seconds of reach; the cap was four.
    assert!((hi - lo).0 as f64 / 960.0 / 2.0 > 11.0, "{lo:?} {hi:?}");
}

/// Finding 8: on a short summed overlap the crossfade's square keeps off the later clip's
/// fade-in square, which keeps its hits.
#[test]
fn the_crossfades_square_keeps_off_the_fade_squares() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, t, _, ib) = two_clips(dir.path(), 3);
    let mut r = region(&a, t, ib);
    r.fade_in = Ticks(480);
    assert!(a.exec(Command::SetClipAudio {
        track: t,
        clip: ib,
        audio: Some(r),
    }));
    let ctx = support::ctx();
    show(&mut a, &ctx, vec![]);
    show(&mut a, &ctx, vec![]);
    let h = a.arrangement.audio_handles((t, ib)).unwrap();
    let sq = h.xfade.expect("a summed overlap has its square").square;
    assert!(!sq.intersects(h.fade_in), "{sq:?} {:?}", h.fade_in);
    assert_eq!(
        h.at(h.fade_in.center()),
        Some(views::arrangement::AudioHandle::FadeIn)
    );
    assert_eq!(
        h.at(sq.center()),
        Some(views::arrangement::AudioHandle::Xfade)
    );
}

/// A project under a tempo change (120 BPM, then 60 from beat 8) with an enveloped clip.
fn under_tempo(dir: &Path) -> (Project, TrackId, ClipId) {
    let file = dir.join("take.wav");
    wav(&file, 48_000 * 20);
    let mut p = Project::empty("Tempo");
    p.tempo_bpm = 120.0;
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(8),
            bpm: 60.0,
            ramp: false,
        },
    ];
    let mut t = p.make_track("Vox");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: file.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 20,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("c", id, Ticks::beats(2), Ticks::beats(4));
    c.offset = Ticks::beats(1);
    c.audio.as_mut().unwrap().gain_env = Some(Box::new(GainEnvelope::new(vec![
        GainPoint::new(Ticks::beats(1), 0.0),
        GainPoint::new(Ticks::beats(2), -12.0),
        GainPoint::new(Ticks::beats(3), 0.0),
    ])));
    let cid = c.id;
    t.clips.push(c);
    let tid = t.id;
    p.tracks.push(t);
    (p, tid, cid)
}

/// The source second each envelope point of `c` lies on.
fn point_seconds(p: &Project, c: &Clip) -> Vec<f64> {
    let map = p.tempo_map();
    let origin = c.start - c.offset;
    c.audio
        .as_ref()
        .and_then(|a| a.gain_env.as_deref())
        .unwrap()
        .points
        .iter()
        .map(|pt| {
            cantus_core::audio_time::source_seconds(
                &map,
                c.start,
                c.offset,
                (origin + pt.tick).0 as f64,
            )
        })
        .collect()
}

/// Finding 2: moved (or copied) across a tempo change, an envelope stays on its audio —
/// each point on the source second it marked.
#[test]
fn a_moved_envelope_stays_on_its_audio_through_a_tempo_change() {
    let dir = tempfile::tempdir().unwrap();
    let (p, t, c) = under_tempo(dir.path());
    let before = p.track(t).unwrap().clip(c).unwrap().clone();
    let want = point_seconds(&p, &before);
    let mut moved = before.clone();
    cantus_ui::clips::relocate(&p, &before, &mut moved, Ticks::beats(10));
    assert_ne!(moved.offset, before.offset, "re-anchored in seconds");
    let got = point_seconds(&p, &moved);
    for (g, w) in got.iter().zip(&want) {
        assert!((g - w).abs() < 0.002, "{got:?} vs {want:?}");
    }
}

/// Finding 2: slipped across a tempo change, likewise.
#[test]
fn a_slipped_envelope_stays_on_its_audio_through_a_tempo_change() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, t, c) = under_tempo(dir.path());
    // Over the change: beats 6 to 10.
    Command::SetClipBounds {
        track: t,
        clip: c,
        start: Ticks::beats(6),
        length: Ticks::beats(4),
    }
    .apply(&mut p)
    .unwrap();
    let before = p.track(t).unwrap().clip(c).unwrap().clone();
    let was = point_seconds(&p, &before);
    for cmd in cantus_ui::clips::slip(&p, t, &before, Ticks(-480)) {
        cmd.apply(&mut p).unwrap();
    }
    let now = point_seconds(&p, p.track(t).unwrap().clip(c).unwrap());
    for (g, w) in now.iter().zip(&was) {
        assert!((g - w).abs() < 0.002, "{now:?} vs {was:?}");
    }
}

/// Finding 4: on a looping clip the line drawn is the line heard — a dip put where the
/// fourth repetition shows it plays there.
#[test]
fn a_looping_clips_line_is_heard_where_it_is_drawn() {
    use cantus_engine::{render, RenderOptions, SampleCache};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("flat.wav");
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&file, spec).unwrap();
        for _ in 0..48_000 * 4 {
            w.write_sample(0.5f32).unwrap();
        }
        w.finalize().unwrap();
    }
    let mut p = Project::empty("Loop");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Loop");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "flat".into(),
        path: file.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 4,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("c", id, Ticks::ZERO, Ticks::beats(4));
    c.content_length = Some(Ticks::beats(1));
    // A dip where the fourth repetition is half-way through.
    let d = Ticks::beats(3) + Ticks(480);
    let ct = cantus_ui::clipfx::content_at(c.offset, c.length, c.period(), false, d);
    assert_eq!(ct, Ticks(480), "the repetition's own content");
    c.audio.as_mut().unwrap().gain_env = Some(Box::new(GainEnvelope::new(vec![
        GainPoint::new(Ticks::ZERO, 0.0),
        GainPoint::new(ct - Ticks(100), 0.0),
        GainPoint::new(ct, -12.0),
        GainPoint::new(ct + Ticks(100), 0.0),
    ])));
    t.clips.push(c);
    p.tracks.push(t);
    let mut s = cantus_core::Snapshot::compile(&p, 1);
    assert!(SampleCache::new().attach_audio(&mut s, 48_000).is_empty());
    let out = render(
        std::sync::Arc::new(s),
        &RenderOptions {
            sample_rate: 48_000,
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    // 25 frames a tick at 120 BPM and 48 kHz.
    let f = (d.0 * 25) as usize;
    let want = 0.5 * cantus_core::clip_gain::gain_of(-12.0);
    assert!((out[2 * f] - want).abs() < 1e-4, "{} vs {want}", out[2 * f]);
}
