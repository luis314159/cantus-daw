//! Shaping audio clips by hand: dragging a fade or the gain, double-clicking one back,
//! the curve menu on a fade handle, and the audio entries of the clip's menu.
//!
//! Every edit is `SetClipAudio` through `exec_merging` keyed by the clip, closed by
//! `end_gesture` when the button comes up: one undo step per gesture.

use super::{fade_of, Handle};
use crate::app::CantusApp;
use crate::audio_clips::{self as ac, Fade};
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::waveform::Lookup;
use cantus_core::{AudioRegion, Command, FadeCurve, MergeKey, Ticks};
use egui::Pos2;
use std::fmt::Write as _;

/// A handle being dragged: which, on what, and what it was when the button went down.
#[derive(Clone, Debug)]
pub struct HandleDrag {
    pub r: ClipRef,
    pub handle: Handle,
    region0: AudioRegion,
    press: Pos2,
    px_per_tick: f32,
}

/// The audio part of the arrangement's state.
#[derive(Debug, Default)]
pub struct State {
    pub drag: Option<HandleDrag>,
    /// The handle the context menu was opened on (a fade's curve menu), captured at the
    /// press like the clip the menu is about.
    pub menu: Option<Handle>,
}

fn key(r: ClipRef) -> MergeKey {
    MergeKey::of(&("clip-audio", r.0 .0, r.1 .0))
}

fn apply(app: &mut CantusApp, r: ClipRef, audio: AudioRegion) {
    let Some((clip, now)) = ac::region(app.project(), r) else {
        return;
    };
    if *now == audio {
        return;
    }
    let cmd = ac::set(r.0, clip, audio);
    app.exec_merging(cmd, key(r));
}

/// The button went down on `handle` of `r`.
pub fn begin(app: &mut CantusApp, r: ClipRef, handle: Handle, press: Pos2, px_per_beat: f32) {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return;
    };
    app.arrangement.audio.drag = Some(HandleDrag {
        r,
        handle,
        region0: a.clone(),
        press,
        px_per_tick: (px_per_beat / cantus_core::PPQ as f32).max(1e-6),
    });
}

/// The pointer moved to `pos` while dragging. Shift drags the gain finely.
pub fn update(app: &mut CantusApp, pos: Pos2, fine: bool) {
    let Some(d) = app.arrangement.audio.drag.clone() else {
        return;
    };
    let Some((clip, _)) = ac::region(app.project(), d.r) else {
        return;
    };
    let dx = Ticks(((pos.x - d.press.x) / d.px_per_tick).round() as i64);
    let a = &d.region0;
    let next = match d.handle {
        Handle::FadeIn => ac::with_fade_in(clip, a, a.fade_in + dx),
        Handle::FadeOut => ac::with_fade_out(clip, a, a.fade_out - dx),
        Handle::Gain => {
            let per = if fine { 0.02 } else { 0.2 };
            ac::with_gain(a, a.gain_db - (pos.y - d.press.y) * per)
        }
    };
    apply(app, d.r, next);
}

/// The button came up.
pub fn end(app: &mut CantusApp) {
    if app.arrangement.audio.drag.take().is_some() {
        app.end_gesture();
    }
}

/// Double-click: a fade back to none, the gain back to 0 dB.
pub fn reset(app: &mut CantusApp, r: ClipRef, handle: Handle) {
    let Some((clip, a)) = ac::region(app.project(), r) else {
        return;
    };
    let next = match handle {
        Handle::FadeIn => ac::with_fade_in(clip, a, Ticks::ZERO),
        Handle::FadeOut => ac::with_fade_out(clip, a, Ticks::ZERO),
        Handle::Gain => ac::with_gain(a, 0.0),
    };
    apply(app, r, next);
    app.end_gesture();
}

/// The hint (and the value pill, while dragging) for a handle under the pointer.
pub fn hint(ctx: &egui::Context, app: &CantusApp, r: ClipRef, handle: Handle, live: bool) {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return;
    };
    let bpm = app.project().tempo_bpm;
    let (name, value) = match handle {
        Handle::FadeIn => ("audioclips.fade_in", Value::Time(a.fade_in.to_seconds(bpm))),
        Handle::FadeOut => (
            "audioclips.fade_out",
            Value::Time(a.fade_out.to_seconds(bpm)),
        ),
        Handle::Gain => ("audioclips.gain", Value::Db(a.gain_db)),
    };
    let caps = Caps {
        menu: handle != Handle::Gain,
        ..Caps::NONE
    };
    hint::set(ctx, &t(name), caps, live, |b| value.write(b));
}

enum Value {
    Time(f64),
    Db(f32),
}

impl Value {
    fn write(&self, b: &mut String) -> std::fmt::Result {
        match *self {
            Value::Time(s) if s < 1.0 => write!(b, "{:.0} ms", s * 1000.0),
            Value::Time(s) => write!(b, "{s:.2} s"),
            Value::Db(d) => write!(b, "{d:+.1} dB"),
        }
    }
}

/// The curve keys of the menus: the label of each curve.
pub const CURVE_KEYS: [&str; 5] = [
    "audioclips.curve.linear",
    "audioclips.curve.equal_power",
    "audioclips.curve.exponential",
    "audioclips.curve.logarithmic",
    "audioclips.curve.s_curve",
];

/// Right-click on a fade handle: the curves, the current one checked, and *No fade*.
pub fn fade_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef, fade: Fade) {
    let Some((clip, a)) = ac::region(app.project(), r) else {
        return;
    };
    let current = match fade {
        Fade::In => a.fade_in_curve,
        Fade::Out => a.fade_out_curve,
    };
    let has = match fade {
        Fade::In => a.fade_in > Ticks::ZERO,
        Fade::Out => a.fade_out > Ticks::ZERO,
    };
    let title = match fade {
        Fade::In => "audioclips.fade_in",
        Fade::Out => "audioclips.fade_out",
    };
    let mut items = vec![Item::Band(Src::Key(title))];
    for (c, key) in FadeCurve::ALL.iter().zip(CURVE_KEYS) {
        items.push(Item::Action {
            label: Src::Key(key),
            check: Check::Radio(*c == current),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    items.push(Item::Separator);
    items.push(Item::Action {
        label: Src::Key("audioclips.fade_remove"),
        check: Check::None,
        shortcut: None,
        submenu: false,
        enabled: has,
    });
    let a = a.clone();
    let clip = clip.clone();
    let Some(i) = kit::widgets::menu_list(ui, &items, super::super::MENU_W)
        .1
        .clicked
    else {
        return;
    };
    let next = match i {
        1..=5 => ac::with_curve(&a, fade, FadeCurve::ALL[i - 1]),
        7 => match fade {
            Fade::In => ac::with_fade_in(&clip, &a, Ticks::ZERO),
            Fade::Out => ac::with_fade_out(&clip, &a, Ticks::ZERO),
        },
        _ => return,
    };
    apply(app, r, next);
    app.end_gesture();
    ui.close();
}

/// The fade a handle's menu is about.
pub fn menu_fade(h: Option<Handle>) -> Option<Fade> {
    h.and_then(fade_of)
}

/// The audio clips of the selection (or `r` alone when it is not selected).
fn targets(app: &CantusApp) -> Vec<ClipRef> {
    app.clip_selection()
        .into_iter()
        .filter(|r| ac::region(app.project(), *r).is_some())
        .collect()
}

/// Normalise: each selected audio clip's gain set so its loudest sample inside its
/// window reaches −0.1 dBFS, from the peaks (one undo step). Returns how many were set;
/// clips whose waveform is not read yet are skipped and said so.
pub fn normalize(app: &mut CantusApp) -> usize {
    let mut commands = Vec::new();
    let mut waiting = false;
    for r in targets(app) {
        let p = app.project();
        let Some((clip, a)) = ac::region(p, r) else {
            continue;
        };
        let Some(src) = p.audio_source(a.source) else {
            continue;
        };
        match app.peaks.lookup(src, app.audio_base(), None) {
            Lookup::Ready(peaks) => {
                let (from, to) = ac::window_frames(p, clip, peaks.rate);
                if let Some(db) = ac::normalize_gain(peaks.peak_in(from, to)) {
                    commands.push(ac::set(r.0, clip, ac::with_gain(a, db)));
                }
            }
            Lookup::Pending => waiting = true,
            _ => {}
        }
    }
    if waiting {
        app.set_status(t("audioclips.status.wait_peaks"));
    }
    let n = commands.len();
    if n > 0 {
        app.exec(Command::Batch {
            label_key: "cmd.normalize_audio".into(),
            commands,
        });
    }
    n
}

/// Set one field on every selected audio clip, as one undo step.
pub fn set_all(
    app: &mut CantusApp,
    label: &str,
    f: impl Fn(&cantus_core::Clip, &AudioRegion) -> AudioRegion,
) {
    let commands: Vec<Command> = targets(app)
        .into_iter()
        .filter_map(|r| {
            let (clip, a) = ac::region(app.project(), r)?;
            let next = f(clip, a);
            (next != *a).then(|| ac::set(r.0, clip, next))
        })
        .collect();
    if !commands.is_empty() {
        app.exec(Command::Batch {
            label_key: label.into(),
            commands,
        });
    }
}

#[path = "arrangement_audio_menu.rs"]
mod menu;
pub use menu::clip_menu;
