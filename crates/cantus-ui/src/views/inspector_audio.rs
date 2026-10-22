//! The audio clip in the inspector (ADR 0025): the whole file with the clip's window on
//! it, the file (its name, format, path — or *Locate…* when it is missing), and the clip's
//! gain, fades with their curves, and reverse — every edit `SetClipAudio`, one undo step
//! per drag.

use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::audio_clips::{self as ac, Fade};

use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, well, Function, State};
use crate::waveform::Lookup;
use cantus_core::{AudioSourceId, ClipId, MergeKey, TrackId};
use egui::{pos2, Align2, Id, Sense, Vec2};
use std::fmt::Write as _;

/// What the section asks the application to do.
#[derive(Clone, Debug)]
pub enum AudioAct {
    Locate(AudioSourceId),
    Reveal(AudioSourceId),
    Normalize(TrackId, ClipId),
    /// Open the pitch editor on the clip (round 28, ADR 0054).
    EditPitch(TrackId, ClipId),
}

pub fn apply(app: &mut CantusApp, a: AudioAct) {
    match a {
        AudioAct::Locate(id) => app.relink_audio_dialog(id),
        AudioAct::Reveal(id) => app.reveal_audio(id),
        AudioAct::Normalize(t, c) => {
            app.select_clip(t, c);
            crate::views::arrangement::normalize_selection(app);
        }
        AudioAct::EditPitch(t, c) => app.open_pitch_editor(t, c),
    }
}

fn key(t: TrackId, c: ClipId) -> MergeKey {
    MergeKey::of(&("clip-audio", t.0, c.0))
}

/// The section, under the clip's own rows.
pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId) {
    let Some((clip, a)) = ac::region(app.project(), (t, c)) else {
        return;
    };
    let Some(src) = app.project().audio_source(a.source) else {
        return;
    };
    let s = sh.s;
    sh.band(
        "audioclips.inspector.band",
        Function::Source,
        None,
        false,
        None,
    );
    let looked = app.peaks.lookup(src, app.audio_base(), Some(sh.ui.ctx()));
    let missing = matches!(looked, Lookup::Missing | Lookup::Failed(_));
    overview(sh, app, clip, a, &looked);
    let rev = app.bus.revision();

    // The file.
    let name = sheet::name(
        sh.fmt
            .get(cache::key(("insp-audio-name", a.source)), rev, |b| {
                b.push_str(&src.name)
            }),
    );
    let row = sh.text_row("audioclips.inspector.file", &name, false, missing);
    if missing {
        sh.b.dot(
            pos2(row.value.right() - s.s(10.0), row.value.center().y),
            s.s(3.0),
            s.t.danger,
        );
    }
    let format = sheet::name(
        sh.fmt
            .get(cache::key(("insp-audio-fmt", a.source)), rev, |b| {
                let secs = src.frames as f64 / src.sample_rate.max(1) as f64;
                let ch = if src.channels >= 2 {
                    crate::i18n::t("audioclips.inspector.stereo")
                } else {
                    crate::i18n::t("audioclips.inspector.mono")
                };
                let (m, sec) = ((secs / 60.0).floor(), secs % 60.0);
                let _ = write!(
                    b,
                    "{:.1} kHz · {ch} · {m:.0}:{sec:05.2}",
                    src.sample_rate as f64 / 1000.0
                );
            }),
    );
    sh.text_row("audioclips.inspector.format", &format, true, missing);
    let row = sh.row();
    sh.label(row.label, "audioclips.inspector.path", missing);
    let r = sh
        .ui
        .interact(row.value, Id::new(("insp-audio-path", c)), Sense::hover());
    let tx = Text::new(&src.path, Role::Body, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::Middle));
    well::paint(sh.b, row.value, &s, State::REST, None);
    let _ = text::paint(
        sh.b,
        row.value.shrink2(Vec2::new(s.s(6.0), 0.0)),
        &tx,
        s.tk(),
    );
    if r.hovered() {
        hint::set_text(sh.ui.ctx(), &src.path, "", Caps::NONE);
    }
    let (word, act) = if missing {
        ("audioclips.menu.locate", AudioAct::Locate(a.source))
    } else {
        ("audioclips.menu.reveal", AudioAct::Reveal(a.source))
    };
    if button(sh, ("insp-audio-file", c), word, missing) {
        sh.acts.push(Act::Audio(act));
    }

    // How the clip plays it.
    // Durations where the clip lies, through the tempo map (ADR 0060): a fade in from the
    // clip's start, a fade out back from its end, as the engine plays them (review 2).
    let map = crate::views::bars::map(app.project());
    let (from, to) = (clip.start, clip.end());
    let gain = a.gain_db;
    let at = a.clone();
    sh.param_row(
        "audioclips.gain",
        Id::new(("insp-audio-gain", c)),
        gain,
        &Range::lin(ac::GAIN_MIN_DB, ac::GAIN_MAX_DB, 0.0).bipolar(),
        false,
        |v, b| {
            let _ = write!(b, "{v:+.1} dB");
        },
        Function::Dynamics,
        false,
        |v| Act::Merge(ac::set(t, clip, ac::with_gain(&at, v)), key(t, c)),
    );
    let most = crate::views::bars::seconds_between(app.project(), clip.start, clip.end()) as f32;
    for fade in [Fade::In, Fade::Out] {
        let (label, now, curve) = match fade {
            Fade::In => ("audioclips.fade_in", a.fade_in, a.fade_in_curve),
            Fade::Out => ("audioclips.fade_out", a.fade_out, a.fade_out_curve),
        };
        let at = a.clone();
        sh.param_row(
            label,
            Id::new(("insp-audio-fade", c, fade == Fade::In)),
            crate::audio_clips::fade_seconds(&map, from, to, now, fade == Fade::Out) as f32,
            &Range::lin(0.0, most.max(0.001), 0.0),
            false,
            |v, b| {
                let _ = if v < 1.0 {
                    write!(b, "{:.0} ms", v * 1000.0)
                } else {
                    write!(b, "{v:.2} s")
                };
            },
            Function::Source,
            false,
            |v| {
                let len =
                    crate::audio_clips::fade_ticks(&map, from, to, v as f64, fade == Fade::Out);
                let next = match fade {
                    Fade::In => ac::with_fade_in(clip, &at, len),
                    Fade::Out => ac::with_fade_out(clip, &at, len),
                };
                Act::Merge(ac::set(t, clip, next), key(t, c))
            },
        );
        if let Some(pick) = curves(sh, (c, fade == Fade::In), fade, curve) {
            let next = ac::with_curve(a, fade, pick);
            sh.acts.push(Act::Exec(ac::set(t, clip, next)));
        }
    }
    if sh.led_row(
        "audioclips.menu.reverse",
        "audioclips.badge.reverse",
        a.reverse,
        Function::Source,
        false,
    ) {
        let next = cantus_core::AudioRegion {
            reverse: !a.reverse,
            ..a.clone()
        };
        sh.acts.push(Act::Exec(ac::set(t, clip, next)));
    }
    if button(
        sh,
        ("insp-audio-norm", c),
        "audioclips.menu.normalize",
        false,
    ) && !missing
    {
        sh.acts.push(Act::Audio(AudioAct::Normalize(t, c)));
    }
    // The pitch editor (round 28, ADR 0054): open it, and — when the clip plays edits —
    // hear them or the original (A/B).
    if let Some(p) = a.pitch.as_deref().filter(|p| p.edited()) {
        if pitch_row(sh, !p.bypass) {
            let mut x = p.clone();
            x.bypass = !x.bypass;
            sh.acts.push(Act::Exec(cantus_core::Command::SetClipPitch {
                track: t,
                clip: c,
                pitch: Some(Box::new(x)),
                op: cantus_core::pitch::PitchOp::Compare,
            }));
        }
    }
    if button_with(
        sh,
        ("insp-audio-pitch", c),
        "pitch.inspector.edit",
        false,
        Some("pitch.inspector.edit_tip"),
        !missing,
    ) {
        sh.acts.push(Act::Audio(AudioAct::EditPitch(t, c)));
    }
}

/// The A/B row of a clip that plays pitch edits: an LED (lit while the edits are heard)
/// with its explanation on the help line. Returns a click.
fn pitch_row(sh: &mut Sheet<'_>, on: bool) -> bool {
    let row = sh.row();
    sh.label(row.label, "pitch.inspector.edited", false);
    let w = row.value.width().min(sh.s.s(110.0));
    let br = egui::Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
    let r = sh.ui.interact(br, Id::new("insp-pitch-ab"), Sense::click());
    if r.hovered() {
        let name = sh.fmt.tr("pitch.inspector.edited").to_string();
        // What it plays now, and what a click does (review 2: the same words either way).
        let key = if on {
            "pitch.inspector.edited_tip"
        } else {
            "pitch.inspector.original_tip"
        };
        let tip = sh.fmt.tr(key).to_string();
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    }
    crate::kit::led::button(
        sh.b,
        br,
        &sh.s.function(Function::Source),
        State::of(&r, on),
        crate::kit::text::Src::Key("pitch.inspector.badge"),
    );
    r.clicked()
}

#[path = "inspector_audio_parts.rs"]
mod parts;
use parts::{button, button_with, curves, overview};
