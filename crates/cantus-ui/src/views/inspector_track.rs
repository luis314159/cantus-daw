//! What sits above the channel when a track, a clip or notes were clicked last: the
//! track's own rows (name, colour, instrument, its level and pan before the strip, which
//! strip it feeds), the instrument's section, or the clip's and the notes' rows.
//!
//! The track's level and pan are the *instrument channel's* (FL's channel volume and
//! pan): they are named so, and the strip's fader and pan live only in the pinned
//! column — the same control never appears twice (`cantus-audit.md` §4.7).

use super::menus::MenuId;
use super::sheet::{self, Range, Sheet, GAIN};
use super::{Act, Inspector};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{cache, Function, State, Style};
use cantus_core::{
    ClipId, Color, Command, FirmusParam, InstrumentParam, MergeKey, PadField, TrackId,
};
use egui::{pos2, Id, Rect, Sense, Ui, Vec2};
use std::fmt::Write as _;

/// The track band and its rows.
pub fn rows(sh: &mut Sheet<'_>, app: &CantusApp, id: TrackId) {
    let Some(track) = app.project().track(id) else {
        return;
    };
    sh.band("inspector.track", Function::Source, None, false, None);
    sh.text_row("track.name", &track.name, false, false);
    swatches(sh, id, track.color);
    // An audio track plays no instrument: its input and its audio are what it is.
    if !crate::record_session::is_audio_track(track) {
        // Round 20: a click opens the device picker to swap it; a right-click, the list.
        // A hosted instrument goes by its own name (ADR 0028), not the word "Plug-in".
        let name = match track.instrument.external() {
            Some(p) => sheet::name(&p.name),
            None => sheet::name(sh.fmt.tr(track.instrument.name_key())),
        };
        let row = sh.row();
        sh.label(row.label, "inspector.title", false);
        let purpose = crate::picker::PickPurpose::ReplaceInstrument { track: id };
        sh.pick_field(
            row.value,
            &name,
            purpose,
            MenuId::Instrument(id),
            "swap.tip.instrument",
        );
        crate::views::device_chrome::publish(
            sh.ui.ctx(),
            crate::views::device_chrome::Spot::InspectorInstrument(id),
            row.value,
        );
    }
    sh.param_row(
        "inspector.track_level",
        Id::new(("insp-track-gain", id)),
        track.gain_db,
        &GAIN,
        true,
        sheet::db,
        Function::Source,
        false,
        |gain_db| {
            Act::Merge(
                Command::SetTrackGain { id, gain_db },
                MergeKey::of(&("gain", id.0)),
            )
        },
    );
    sh.param_row(
        "inspector.track_pan",
        Id::new(("insp-track-pan", id)),
        track.pan,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
        sheet::pan,
        Function::Source,
        false,
        |pan| {
            Act::Merge(
                Command::SetTrackPan { id, pan },
                MergeKey::of(&("pan", id.0)),
            )
        },
    );
    let strip = track.strip;
    let text = sh.fmt.get(
        cache::key(("insp-route", id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match strip.and_then(|s| app.project().strip_number(s)) {
            Some(n) => b.push_str(&crate::i18n::tf("track.route_strip", &[("n", &n)])),
            None => b.push_str(&crate::i18n::t("mixer.master")),
        },
    );
    let text = sheet::name(text);
    sh.menu_row("track.route", &text, MenuId::Route(id), false);
}

/// The track's colour: the palette in a well, the chosen one ringed.
fn swatches(sh: &mut Sheet<'_>, id: TrackId, current: Color) {
    let s = sh.s;
    let row = sh.row();
    sh.label(row.label, "track.color", false);
    crate::kit::well::paint(sh.b, row.value, &s, State::REST, None);
    let n = Color::PALETTE.len() as f32;
    let pad = s.s(3.0);
    let cell = ((row.value.width() - pad) / n).min(row.value.height());
    let side = (cell - pad).max(s.s(6.0));
    for (i, c) in Color::PALETTE.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(
                row.value.left() + pad + i as f32 * cell,
                row.value.center().y - side * 0.5,
            ),
            Vec2::splat(side),
        );
        let resp = sh
            .ui
            .interact(r, Id::new(("insp-swatch", id, i)), Sense::click());
        let fill = crate::theme::track_color(*c);
        sh.b.rrect(sh.b.snap_rect(r), s.r(2.0), crate::kit::Fill::Solid(fill));
        if *c == current {
            sh.b.rrect_stroke(
                sh.b.snap_rect(r.expand(s.s(1.5))),
                s.r(3.0),
                s.s(1.5),
                s.t.text,
            );
        } else if resp.hovered() {
            sh.b.rrect_stroke(sh.b.snap_rect(r), s.r(2.0), sh.b.px(), s.t.text_dim);
        }
        if resp.hovered() {
            let name = sh.fmt.tr("track.color");
            hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
        }
        if resp.clicked() && *c != current {
            sh.acts
                .push(Act::Exec(Command::SetTrackColor { id, color: *c }));
        }
    }
}

/// The instrument's section. A native instrument's strip is the instrument editor's own
/// (egui underneath), so the brush has been flushed before this is called.
pub fn instrument(
    st: &mut Inspector,
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    col: f32,
    t: TrackId,
) {
    let rev = app.bus.revision();
    let Some(track) = app.project().track(t) else {
        return;
    };
    if track.instrument.native().is_some() {
        // A copy of the spec, taken when the project changes rather than every frame.
        if st
            .native
            .as_ref()
            .is_none_or(|(r, id, _)| *r != rev || *id != t)
        {
            st.native = track.instrument.native().cloned().map(|n| (rev, t, n));
        }
        let painter = ui.painter().clone();
        st.brush.begin(ui.ctx());
        {
            let mut sh = st.sheet(ui, s, col);
            sh.band("inspector.title", Function::Source, None, false, None);
        }
        st.brush.flush(&painter);
        if let Some((_, _, spec)) = st.native.as_ref() {
            let m = s.s(sheet::MARGIN) as i8;
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(m, 4))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(s.s(4.0));
                    crate::views::inst::inspector_strip(app, ui, t, spec);
                });
        }
        return;
    }
    let painter = ui.painter().clone();
    st.brush.begin(ui.ctx());
    {
        let mut sh = st.sheet(ui, s, col);
        let app: &CantusApp = app;
        let Some(track) = app.project().track(t) else {
            return;
        };
        if let Some(kit) = track.instrument.pulsus() {
            super::pads::show(&mut sh, app, t, &kit.pads);
        } else if let Some(params) = track.instrument.firmus() {
            firmus(&mut sh, t, params);
        } else if let Some(plugin) = track.instrument.external() {
            super::hosted::show(&mut sh, app, t, plugin);
        }
    }
    st.brush.flush(&painter);
}

/// Firmus: its sections as label | value rows — no wall of knobs.
fn firmus(sh: &mut Sheet<'_>, id: TrackId, params: &cantus_core::FirmusParams) {
    sh.band("inspector.oscillators", Function::Source, None, false, None);
    for p in [FirmusParam::Osc1Wave, FirmusParam::Osc2Wave] {
        let w = cantus_core::Waveform::ALL
            .into_iter()
            .find(|w| w.index() as f32 == p.get(params))
            .unwrap_or(cantus_core::Waveform::ALL[0]);
        let name = sheet::name(sh.fmt.tr(w.label_key()));
        sh.menu_row(p.info().label_key, &name, MenuId::Wave(id, p), false);
    }
    let groups: [(&'static str, Function, &[FirmusParam]); 5] = [
        (
            "",
            Function::Source,
            &[
                FirmusParam::Osc2Semitones,
                FirmusParam::Osc2Detune,
                FirmusParam::OscMix,
            ],
        ),
        (
            "inspector.filter",
            Function::Tone,
            &[
                FirmusParam::Cutoff,
                FirmusParam::Resonance,
                FirmusParam::FilterEnvAmount,
                FirmusParam::KeyTrack,
            ],
        ),
        (
            "inspector.amp_env",
            Function::Dynamics,
            &[
                FirmusParam::AmpAttack,
                FirmusParam::AmpDecay,
                FirmusParam::AmpSustain,
                FirmusParam::AmpRelease,
            ],
        ),
        (
            "inspector.filter_env",
            Function::Mod,
            &[
                FirmusParam::FilterAttack,
                FirmusParam::FilterDecay,
                FirmusParam::FilterSustain,
                FirmusParam::FilterRelease,
            ],
        ),
        ("inspector.output", Function::Output, &[FirmusParam::Gain]),
    ];
    for (band, f, list) in groups {
        if !band.is_empty() {
            sh.band(band, f, None, false, None);
        }
        for &p in list {
            let info = p.info();
            let range = Range {
                min: info.min,
                max: info.max,
                default: info.default,
                taper: if info.logarithmic {
                    crate::widgets::knob::Taper::Log
                } else {
                    crate::widgets::knob::Taper::Linear
                },
                stepped: info.stepped,
                bipolar: info.min < 0.0 && info.max > 0.0,
            };
            let unit = info.unit;
            sh.param_row(
                info.label_key,
                Id::new(("insp-firmus", id, p as u8)),
                p.get(params),
                &range,
                false,
                |v, b| sheet::unit(v, unit, b),
                f,
                false,
                |value| {
                    Act::Merge(
                        Command::SetInstrumentParam {
                            track: id,
                            param: p.into(),
                            value,
                        },
                        MergeKey::of(&("param", id.0, p as u8)),
                    )
                },
            );
        }
    }
}

/// The command that sets one field of the pad at `pad` (by index; the command names the
/// pad itself).
pub fn pad_cmd(
    app: &CantusApp,
    id: TrackId,
    pad: usize,
    field: PadField,
    value: f32,
) -> Option<Command> {
    let pad_id = app
        .project()
        .track(id)
        .and_then(|t| t.instrument.pulsus())
        .and_then(|k| k.pad(pad))
        .map(|p| p.id)?;
    Some(Command::SetInstrumentParam {
        track: id,
        param: InstrumentParam::Pulsus { pad: pad_id, field },
        value,
    })
}

/// "3.2.1" — bar, beat, sixteenth — for a position; `length` counts from zero.
fn bbt(app: &CantusApp, t: cantus_core::Ticks, length: bool, b: &mut String) {
    let ts = app.project().time_signature;
    let (bar, beat, tick) = ts.bbt(t);
    let sixteenth = tick / (cantus_core::PPQ / 4).max(1);
    let o = if length { 1 } else { 0 };
    let _ = write!(b, "{}.{}.{}", bar - o, beat - o, sixteenth + 1 - o);
}

/// A clip: its name, where it sits, how long it is, whether it loops or is muted.
pub fn clip(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId) {
    let Some(clip) = app.project().track(t).and_then(|x| x.clip(c)) else {
        return;
    };
    sh.band("inspector.ctx.clip", Function::Source, None, false, None);
    sh.text_row("track.name", &clip.name, false, false);
    let rev = app.bus.revision();
    let start = sheet::name(sh.fmt.get(cache::key(("insp-clip-start", c)), rev, |b| {
        bbt(app, clip.start, false, b)
    }));
    sh.text_row("inspector.start", &start, true, false);
    let len = sheet::name(sh.fmt.get(cache::key(("insp-clip-len", c)), rev, |b| {
        bbt(app, clip.length, true, b)
    }));
    sh.text_row("inspector.length", &len, true, false);
    let looping = clip.content_length.is_some();
    // An audio clip plays its file once (ADR 0025): the repeat does not apply.
    let audio = clip.audio.is_some();
    if sh.led_row(
        "inspector.loop",
        "inspector.loop",
        looping,
        Function::Source,
        audio,
    ) {
        let content_length = if looping { None } else { Some(clip.length) };
        sh.acts.push(Act::Exec(Command::SetClipLoop {
            track: t,
            clip: c,
            content_length,
        }));
    }
    if sh.led_row(
        "inspector.mute",
        "kit.mixer.mute",
        clip.muted,
        Function::Dynamics,
        false,
    ) {
        sh.acts.push(Act::Exec(Command::SetClipMute {
            track: t,
            clip: c,
            muted: !clip.muted,
        }));
    }
    if audio {
        return;
    }
    let n = clip.notes.len();
    let count = sheet::name(sh.fmt.get(cache::key(("insp-clip-n", c)), n as u64, |b| {
        let _ = write!(b, "{n}");
    }));
    sh.text_row("inspector.note_count", &count, true, n == 0);
}

/// "C4" for 60.
fn note_name(p: u8, b: &mut String) {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let _ = write!(b, "{}{}", NAMES[(p % 12) as usize], p as i32 / 12 - 1);
}

/// Selected notes: how many, their velocity (moved together), their pitch range.
pub fn notes(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId) {
    let Some(clip) = app.project().track(t).and_then(|x| x.clip(c)) else {
        return;
    };
    sh.band("inspector.ctx.notes", Function::Source, None, false, None);
    let sel = &app.selected_notes;
    let chosen = || clip.notes.iter().filter(|n| sel.contains(&n.id));
    let n = chosen().count();
    let count = sheet::name(sh.fmt.get(cache::key(("insp-notes-n", c)), n as u64, |b| {
        let _ = write!(b, "{n}");
    }));
    sh.text_row("inspector.note_count", &count, true, false);
    if n == 0 {
        return;
    }
    let avg = chosen().map(|x| x.velocity as f32).sum::<f32>() / n as f32;
    let d = {
        let row = sh.row();
        sh.label(row.label, "inspector.velocity", false);
        let name = sheet::name(sh.fmt.tr("inspector.velocity"));
        sh.value(
            row.value,
            Id::new(("insp-vel", c)),
            avg.round(),
            &Range::lin(1.0, 127.0, 100.0).stepped(true),
            false,
            &name,
            |v, b| {
                let _ = write!(b, "{v:.0}");
            },
            Function::Dynamics,
            false,
        )
    };
    if let Some(v) = d.value {
        let delta = v - avg.round();
        let notes: Vec<cantus_core::Note> = chosen()
            .map(|x| cantus_core::Note {
                velocity: (x.velocity as f32 + delta).round().clamp(1.0, 127.0) as u8,
                ..*x
            })
            .collect();
        sh.acts.push(Act::Merge(
            Command::SetNotes {
                track: t,
                clip: c,
                notes,
            },
            MergeKey::of(&("vel", c.0)),
        ));
    }
    if d.ended {
        sh.acts.push(Act::EndGesture);
    }
    let lo = chosen().map(|x| x.pitch).min().unwrap_or(0);
    let hi = chosen().map(|x| x.pitch).max().unwrap_or(0);
    let range = sheet::name(sh.fmt.get(
        cache::key(("insp-notes-pitch", c)),
        (lo as u64) << 8 | hi as u64,
        |b| {
            note_name(lo, b);
            if hi != lo {
                b.push_str(" \u{2013} ");
                note_name(hi, b);
            }
        },
    ));
    sh.text_row("inspector.pitch", &range, false, false);
}
