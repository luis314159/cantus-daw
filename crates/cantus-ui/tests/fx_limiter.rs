//! Terminus's editor: it paints what it measures, its loudness is the conformance-tested
//! loudness, the history is kept in song time (a second take of a bar replaces the first),
//! the display is the control, and a delivery profile is one undo step.

mod support;
use cantus_core::*;
use cantus_dsp::terminus::{Column, LimSettings, Terminus, T_SEQ};
use cantus_ui::views::fx::limiter::{self, profile_of, verdict, State, View, PROFILES};
use cantus_ui::CantusApp;
use support::*;

fn with_limiter() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Limiter);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

/// A stereo 1 kHz sine at `dbfs` through a real limiter that has nothing to do, delivered
/// to the editor as the engine would: one telemetry block per "frame" of 800 samples.
fn play_sine(a: &mut CantusApp, dbfs: f32, secs: f32) {
    let mut dsp = Terminus::new(48_000.0);
    dsp.set(&LimSettings {
        ceiling_db: 0.0,
        ..LimSettings::default()
    });
    let amp = 10f32.powf(dbfs / 20.0);
    let n = (secs * 48_000.0) as usize;
    let mut i = 0;
    while i < n {
        let len = 800.min(n - i);
        let mut l: Vec<f32> = (i..i + len)
            .map(|k| amp * (std::f32::consts::TAU * 1_000.0 * k as f32 / 48_000.0).sin())
            .collect();
        let mut r = l.clone();
        dsp.process(&mut l, &mut r, None);
        a.fx.limiter
            .ingest(&dsp.telemetry(), false, Ticks::ZERO, 120.0);
        i += len;
    }
}

#[test]
fn the_editor_paints_its_parts_at_rest() {
    let (mut a, _, _) = with_limiter();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    // Labels and section names are set in capitals, and a label wider than its cell
    // takes its short form (the full name is on the help line): "LOOK", "LINK T".
    let lower = text.to_lowercase();
    for want in [
        "Clean",
        "Punch",
        "Loud",
        "Bus",
        "Safe",
        "latency",
        "Integrated",
        "Short-term",
        "Loudness range",
        "True peak max",
        "Spotify",
        "gain reduction",
        "Look",
        "Link t",
        "Link r",
        "Dither",
        "1:1",
        "Match",
    ] {
        assert!(
            lower.contains(&want.to_lowercase()),
            "the editor never painted {want:?}: {text}"
        );
    }
}

/// The editor's loudness comes from the limiter's 10 ms columns through the same code the
/// EBU conformance tests check: a stereo −23 dBFS sine reads −23.0 LUFS integrated.
#[test]
fn the_editor_reads_a_minus_23_sine_as_minus_23_lufs() {
    let (mut a, _, _) = with_limiter();
    play_sine(&mut a, -23.0, 12.0);
    let st = &a.fx.limiter;
    let i = st.loud.integrated().expect("measured");
    assert!((i - -23.0).abs() < 0.1, "integrated {i}");
    assert!((st.loud.short_term - -23.0).abs() < 0.1);
    assert!((st.loud.momentary - -23.0).abs() < 0.1);
    // Every column arrived once: 12 s is 1 200 columns.
    assert_eq!(st.live.len(), 1_200);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(
        text.contains("\u{2212}23.0"),
        "the number is on screen: {text}"
    );
}

/// A telemetry block read twice adds nothing; a frame that comes late, up to the ring's
/// length, loses nothing.
#[test]
fn columns_arrive_once_whatever_the_frame_rate() {
    let mut st = State::default();
    let mut dsp = Terminus::new(48_000.0);
    let mut total = 0;
    for (frame, len) in [480usize, 1_700, 0, 3_000, 9, 5_000].iter().enumerate() {
        let mut l = vec![0.1f32; *len];
        let mut r = l.clone();
        dsp.process(&mut l, &mut r, None);
        st.ingest(&dsp.telemetry(), false, Ticks::ZERO, 120.0);
        st.ingest(&dsp.telemetry(), false, Ticks::ZERO, 120.0);
        total += len;
        assert_eq!(
            st.live.len(),
            total / 480,
            "frame {frame}: {} columns for {total} samples",
            st.live.len()
        );
    }
    assert_eq!(dsp.telemetry()[T_SEQ] as usize, total / 480);
}

fn col(gr: f32, energy: f32) -> Column {
    Column {
        energy,
        input_db: -3.0,
        output_db: -1.2,
        gr_db: gr,
        clip_db: 0.0,
    }
}

/// Song time: bars 1–4 played, then bars 2–3 played again louder. The history holds four
/// bars, bars 2–3 as they were heard the second time — replaced, not appended.
#[test]
fn a_second_take_of_a_bar_replaces_the_first() {
    let mut st = State::default();
    let bar = PPQ * 4;
    // 120 BPM: a bar is 2 s = 200 columns; a column is 9.6 ticks.
    let per_col = PPQ as f64 * 2.0 * 0.01;
    st.feed(&vec![col(1.0, 1e-3); 800], Some(0), per_col);
    let first = st.song.len();
    st.feed(&vec![col(6.0, 1e-3); 400], Some(bar), per_col);
    assert_eq!(st.song.len(), first, "the second take added buckets");
    let gr_at = |st: &State, tick: i64| {
        st.song
            .range(..=tick / (PPQ / 32))
            .next_back()
            .map(|(_, (c, _))| c.gr_db)
            .unwrap()
    };
    assert_eq!(gr_at(&st, bar / 2), 1.0, "bar 1 kept its take");
    assert_eq!(gr_at(&st, bar + bar / 2), 6.0, "bar 2 is the new take");
    assert_eq!(gr_at(&st, 3 * bar + bar / 2), 1.0, "bar 4 kept its take");
    // The whole song measures four bars.
    let sum = st.measure(None, None, 120.0, bar);
    assert_eq!(sum.bars, (1, 4));
    assert!((sum.seconds - 8.0).abs() < 0.05, "{} s", sum.seconds);
    // A selection of bar 3 alone.
    let sel = st.measure(Some(2 * bar), Some(3 * bar), 120.0, bar);
    assert_eq!(sel.bars, (3, 3));
}

/// Loudness measured from the song history agrees with the meter that heard it.
#[test]
fn the_song_is_measured_as_bs1770_measures_it() {
    let (mut a, _, _) = with_limiter();
    // Play at 120 BPM from bar 1: the dummy engine's position is set by hand each frame.
    let mut dsp = Terminus::new(48_000.0);
    dsp.set(&LimSettings {
        ceiling_db: 0.0,
        ..LimSettings::default()
    });
    let amp = 10f32.powf(-18.0 / 20.0);
    let mut k = 0usize;
    for frame in 0..600 {
        let mut l: Vec<f32> = (k..k + 800)
            .map(|j| amp * (std::f32::consts::TAU * 1_000.0 * j as f32 / 48_000.0).sin())
            .collect();
        let mut r = l.clone();
        dsp.process(&mut l, &mut r, None);
        k += 800;
        let tick = Ticks(((frame + 1) as f64 * 800.0 / 48_000.0 * 2.0 * PPQ as f64) as i64);
        a.fx.limiter.ingest(&dsp.telemetry(), true, tick, 120.0);
    }
    let meter = a.fx.limiter.loud.integrated().unwrap();
    let bar = PPQ * 4;
    let song = a.fx.limiter.measure(None, None, 120.0, bar);
    let i = song.integrated.unwrap();
    assert!((meter - -18.0).abs() < 0.1, "meter {meter}");
    assert!((i - meter).abs() < 0.2, "song {i} vs meter {meter}");
}

/// Run a frame of the editor window with pointer events.
fn frames(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    })
    .textures_delta
    .clear();
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    frames(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frames(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frames(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frames(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frames(a, ctx, vec![]);
}

/// The ceiling is a line you drag, and the drag is one undo step.
#[test]
fn dragging_the_ceiling_line_sets_the_ceiling_in_one_step() {
    let (mut a, s0, _) = with_limiter();
    let ctx = ctx();
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let geo = a.fx.limiter.geometry.clone().expect("the display drew");
    let x = geo.levels.center().x;
    let from = egui::pos2(x, geo.ceiling_y);
    let to = egui::pos2(x, geo.ceiling_y + geo.levels.height() * 0.25);
    drag(&mut a, &ctx, from, to);
    let c = spec(&a, s0).get("ceiling");
    assert!(c < -3.0, "the ceiling did not follow the drag: {c}");
    a.undo();
    assert_eq!(spec(&a, s0).get("ceiling"), -1.0, "one undo restores it");
}

/// The gain strip on the left edge: drag up for more gain.
#[test]
fn dragging_the_gain_strip_raises_the_gain() {
    let (mut a, s0, _) = with_limiter();
    let ctx = ctx();
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let geo = a.fx.limiter.geometry.clone().unwrap();
    let from = geo.gain_handle.center();
    let up = egui::vec2(0.0, geo.gain_handle.height() * 0.2);
    drag(&mut a, &ctx, from, from - up);
    let g = spec(&a, s0).get("gain");
    assert!((4.0..8.0).contains(&g), "gain {g}");
    a.undo();
    assert_eq!(spec(&a, s0).get("gain"), 0.0, "one undo restores it");
}

/// A peak label in the song view is a button: clicking it moves the playhead to it.
#[test]
fn clicking_a_peak_label_goes_there() {
    let (mut a, _, _) = with_limiter();
    let ctx = ctx();
    let per_col = PPQ as f64 * 2.0 * 0.01;
    let mut cols = vec![col(0.0, 1e-3); 1_600];
    cols[1_000] = col(4.5, 1e-3);
    a.fx.limiter.feed(&cols, Some(0), per_col);
    a.fx.limiter.view = View::Song;
    a.fx.limiter.follow = false;
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let geo = a.fx.limiter.geometry.clone().unwrap();
    let (tag, tick) = geo
        .labels
        .iter()
        .find(|(_, t)| t.is_some())
        .cloned()
        .expect("a labelled peak");
    let at = tag.center();
    frames(&mut a, &ctx, vec![egui::Event::PointerMoved(at)]);
    frames(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, true)],
    );
    frames(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, false)],
    );
    assert_eq!(a.engine.position(), Ticks(tick.unwrap()));
    let want = (1_000.0 * per_col) as i64;
    assert!((a.engine.position().0 - want).abs() <= PPQ / 32);
}

/// A delivery profile sets the target, the ceiling and true peak together: one undo step.
#[test]
fn a_profile_is_one_step_and_names_itself() {
    let (mut a, s0, fx) = with_limiter();
    let atsc = PROFILES.iter().find(|p| p.key.ends_with("atsc")).unwrap();
    a.set_fx_param(s0, fx, "tp", 0.0, false);
    a.exec(Command::Batch {
        label_key: "cmd.lim_profile".into(),
        commands: [
            ("target", atsc.target),
            ("ceiling", atsc.ceiling),
            ("tp", 1.0),
        ]
        .into_iter()
        .map(|(k, v)| Command::SetEffectParam {
            strip: s0,
            effect: fx,
            key: k.into(),
            value: v,
        })
        .collect(),
    });
    let sp = spec(&a, s0);
    assert_eq!(profile_of(sp.get("target"), sp.get("ceiling")), Some(atsc));
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("ATSC A/85"), "{text}");
    assert!(
        text.contains("\u{2212}24 LKFS"),
        "the rule is written out: {text}"
    );
    a.undo();
    let sp = spec(&a, s0);
    assert_eq!((sp.get("target"), sp.get("ceiling")), (-14.0, -1.0));
    assert_eq!(sp.get("tp"), 0.0);
}

/// The verdict is words: on target, turned down by how much, and a true peak over.
#[test]
fn the_verdict_says_what_the_platform_will_do() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let spotify = &PROFILES[0];
    let (ok, words) = verdict(Some(-14.3), -1.2, -14.0, -1.0, Some(spotify));
    assert!(ok, "{words}");
    let (ok, words) = verdict(Some(-9.0), -1.2, -14.0, -1.0, Some(spotify));
    assert!(!ok);
    assert!(
        words.contains("5.0") && words.contains("turned down"),
        "{words}"
    );
    let (ok, words) = verdict(Some(-14.0), -0.4, -14.0, -1.0, Some(spotify));
    assert!(!ok && words.contains("true peak 0.6 dB over"), "{words}");
    assert!(!verdict(None, -1.0, -14.0, -1.0, None).0);
}

/// Every factory preset names only parameters the limiter has, in range, and has a name.
#[test]
fn the_factory_presets_are_valid() {
    assert!(limiter::FACTORY_PRESETS.len() >= 6);
    for (label, params) in limiter::FACTORY_PRESETS {
        let name = cantus_ui::i18n::t(label);
        assert!(!name.starts_with("lim."), "{label} has no text");
        for (k, v) in params.iter() {
            let info = EffectKind::Limiter
                .param(k)
                .unwrap_or_else(|| panic!("{label}: no parameter {k}"));
            assert!((info.min..=info.max).contains(v), "{label}: {k} = {v}");
        }
    }
}

/// The demo the screenshots use paints a real history in both themes: labels, loudness.
#[test]
fn the_demo_paints_labels_and_loudness_in_both_themes() {
    for dark in [false, true] {
        let mut a = app(cantus_ui::welcome_project());
        let theme = if dark {
            cantus_ui::theme::ink_and_gold()
        } else {
            cantus_ui::theme::default_theme()
        };
        let ctx = ctx();
        cantus_ui::theme::apply(&theme, &ctx);
        a.demo_editor("limiter");
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Limiter);
        assert!(!a.fx.limiter.song.is_empty(), "the demo played the song");
        frames(&mut a, &ctx, vec![]);
        let geo = a.fx.limiter.geometry.clone().unwrap();
        assert!(!geo.labels.is_empty(), "no peak labels");
        let i = a.fx.limiter.loud.integrated().unwrap();
        assert!((-20.0..-4.0).contains(&i), "demo loudness {i}");
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}
