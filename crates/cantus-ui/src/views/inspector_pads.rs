//! Pulsus in the inspector: the kit's pads as tiles (▸ name, the level ring, the LED is
//! the pad sounding — off is muted), then the selected pad's rows.
//!
//! One pad at a time rather than all of them. Seven pads with nine parameters each is
//! sixty-three controls, which is not an inspector, it is a wall — and the pad being
//! worked on is the one the channel rack already knows about, so the two panels agree
//! without either being told.

use super::menus::MenuId;
use super::sheet::{self, Range, Sheet};
use super::track::pad_cmd;
use super::{Act, MenuOpen};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::tile::{self, TileLook};
use crate::kit::{cache, led, Function, State};
use cantus_core::{DrumPad, MergeKey, PadField, PadVoice, TrackId};
use egui::{pos2, Id, Rect, Sense, Vec2};
use std::fmt::Write as _;

pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, pads: &[DrumPad]) {
    let s = sh.s;
    let band = sh.band("inspector.pads", Function::Source, None, true, None);
    if let (true, Some(anchor)) = (band.plus, band.plus_rect) {
        *sh.menu = Some(MenuOpen {
            id: MenuId::AddPad(t),
            anchor,
        });
    }
    if pads.is_empty() {
        let row = sh.row();
        let m = s.s(sheet::MARGIN);
        let r = Rect::from_min_max(
            pos2(row.full.left() + m, row.value.top()),
            pos2(row.full.right() - m, row.value.bottom()),
        );
        sheet::placeholder_well(sh.b, r, &s, State::REST, "pad.none_yet");
        return;
    }
    let selected = app.rack.selected_pad.min(pads.len() - 1);
    for (i, pad) in pads.iter().enumerate() {
        let w = sh.width();
        let (full, _) = sh
            .ui
            .allocate_exact_size(Vec2::new(w, s.d(tile::HEIGHT) + s.s(1.0)), Sense::hover());
        let m = s.s(sheet::MARGIN - 2.0);
        let rect = Rect::from_min_max(
            pos2(full.left() + m, full.top()),
            pos2(full.right() - m, full.bottom() - s.s(1.0)),
        );
        let parts = tile::rects(rect, &s);
        let body = sh
            .ui
            .interact(rect, Id::new(("insp-pad", t, i)), Sense::click());
        let lamp = sh.ui.interact(
            parts.led.expand(s.s(3.0)),
            Id::new(("insp-pad-led", t, i)),
            Sense::click(),
        );
        let level_info = PadField::Level.info(pad.voice);
        let level = PadField::Level.get(pad);
        let ring = sheet::drag(
            sh.ui,
            Id::new(("insp-pad-level", t, i)),
            parts.mix.expand(s.s(2.0)),
            level,
            &Range::lin(level_info.min, level_info.max, level_info.default),
            false,
        );
        if body.clicked() && i != selected {
            sh.acts.push(Act::SelectPad(i));
        }
        if lamp.clicked() {
            if let Some(c) = pad_cmd(app, t, i, PadField::Mute, (!pad.muted) as u8 as f32) {
                sh.acts.push(Act::Exec(c));
            }
        }
        if let Some(v) = ring.value {
            if let Some(c) = pad_cmd(app, t, i, PadField::Level, v) {
                sh.acts.push(Act::Merge(
                    c,
                    MergeKey::of(&("pad", t.0, i, PadField::Level)),
                ));
            }
        }
        if ring.ended {
            sh.acts.push(Act::EndGesture);
        }
        let span = (level_info.max - level_info.min).max(1e-6);
        let look = TileLook {
            name: Some(Src::Str(&pad.name)),
            placeholder: Src::Str(""),
            mix: Some((ring.value.unwrap_or(level) - level_info.min) / span),
            on: !pad.muted,
        };
        tile::paint(sh.b, rect, &s, State::of(&body, i == selected), &look);
        if lamp.hovered() {
            let name = sh.fmt.tr("pad.mute");
            hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
        } else if body.hovered() {
            hint::set_text(sh.ui.ctx(), &pad.name, "", Caps::NONE);
        }
    }
    let pad = &pads[selected];
    // The selected pad: its voice, its file, its choke group, the editor, then its sound.
    let title = sheet::name(sh.fmt.get(
        cache::key(("insp-pad-title", t, selected)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| {
            let _ = write!(
                b,
                "{} · {}",
                pad.name,
                crate::i18n::t(pad.voice.label_key())
            );
        },
    ));
    sh.band("inspector.pad_sound", Function::Source, None, false, None);
    sh.text_row("inspector.pad_name", &title, false, false);
    let voice = sheet::name(sh.fmt.tr(pad.voice.label_key()));
    sh.menu_row("pad.voice", &voice, MenuId::PadVoice(t, selected), false);
    if pad.voice == PadVoice::Sample {
        sample_row(sh, app, t, selected, pad);
    }
    let choke = sheet::name(sh.fmt.get(
        cache::key(("insp-choke", t, selected)),
        pad.choke as u64 ^ ((crate::i18n::locale() as u64) << 60),
        |b| {
            if pad.choke == 0 {
                b.push_str(&crate::i18n::t("pad.none"));
            } else {
                b.push_str(&crate::i18n::tf("pad.group", &[("n", &pad.choke)]));
            }
        },
    ));
    sh.menu_row("pad.choke", &choke, MenuId::Choke(t, selected), false);
    // Two actions in one row: the pad's own editor, and taking the pad out.
    {
        let row = sh.row();
        sh.label(row.label, "inspector.pad_actions", false);
        let gap = s.s(4.0);
        let w = (row.value.width() - gap) * 0.5;
        let a = Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
        let b = Rect::from_min_size(pos2(a.right() + gap, a.top()), a.size());
        let ra = sh
            .ui
            .interact(a, Id::new(("insp-pad-edit", t)), Sense::click());
        let rb = sh
            .ui
            .interact(b, Id::new(("insp-pad-remove", t)), Sense::click());
        led::button(
            sh.b,
            a,
            &s.function(Function::Source),
            State::of(&ra, false),
            Src::Key("inspector.pad_edit"),
        );
        led::button(
            sh.b,
            b,
            &s.function(Function::Dynamics),
            State::of(&rb, false),
            Src::Key("inspector.pad_remove"),
        );
        if ra.clicked() {
            sh.acts.push(Act::OpenPulsus(t, pad.id));
        }
        if rb.clicked() {
            sh.acts.push(Act::RemovePad(t, selected));
        }
        if rb.hovered() {
            let name = sh.fmt.tr("pad.remove_tip");
            hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
        }
    }
    // Only the fields this voice has: a slider that does nothing is worse than none.
    // Pulsus I's fields here; the whole of Pulsus II's section is in its editor.
    for field in PadField::for_voice(pad.voice)
        .filter(|f| PadField::CLASSIC.contains(f) && f.applies(pad))
        .filter(|f| {
            !matches!(
                f,
                PadField::Voice | PadField::Mute | PadField::Choke | PadField::Level
            )
        })
    {
        let info = field.info(pad.voice);
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
        let cmd = pad_cmd(app, t, selected, field, 0.0);
        sh.param_row(
            info.label_key,
            Id::new(("insp-padf", t, selected, field)),
            field.get(pad),
            &range,
            false,
            |v, b| sheet::unit(v, unit, b),
            Function::Source,
            cmd.is_none(),
            |v| {
                let mut c = cmd.clone().expect("dimmed without a pad");
                if let cantus_core::Command::SetInstrumentParam { value, .. } = &mut c {
                    *value = v;
                }
                Act::Merge(c, MergeKey::of(&("pad", t.0, selected, field)))
            },
        );
    }
}

/// The sample a pad plays: its file and whether it loaded — what is shown is what will
/// sound — and a click on it to choose another.
fn sample_row(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, pad: usize, drum: &DrumPad) {
    use cantus_engine::SampleStatus;
    let s = sh.s;
    let row = sh.row();
    sh.label(row.label, "pad.sample", false);
    let r = sh
        .ui
        .interact(row.value, Id::new(("insp-sample", t, pad)), Sense::click());
    let st = State::of(&r, false);
    match drum.sample.as_deref() {
        None => sheet::placeholder_well(sh.b, row.value, &s, st, "pad.sample_none"),
        Some(path) => {
            let status = app.engine.samples.status(std::path::Path::new(path));
            let text = sheet::name(sh.fmt.get(
                cache::key(("insp-sample", t, pad)),
                app.bus.revision(),
                |b| {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string());
                    b.push_str(&name);
                },
            ));
            let s2 = match status {
                SampleStatus::Failed(_) => s.hue_of(crate::kit::Tok::Danger),
                _ => s,
            };
            crate::kit::well::readout(sh.b, row.value, &s2, st, &text, false);
            if let SampleStatus::Failed(_) = status {
                sh.b.dot(
                    pos2(row.value.right() - s.s(10.0), row.value.center().y),
                    s.s(3.0),
                    s.t.danger,
                );
            }
            if r.hovered() {
                let tip = sh.fmt.get(
                    cache::key(("insp-sample-tip", t, pad)),
                    app.bus.revision(),
                    |b| match status {
                        SampleStatus::Loaded {
                            rate,
                            channels,
                            frames,
                        } => {
                            let secs = frames as f32 / rate.max(1) as f32;
                            let _ = write!(
                                b,
                                "{secs:.2} s · {rate} Hz · {}",
                                if channels >= 2 { "stereo" } else { "mono" }
                            );
                        }
                        SampleStatus::Failed(_) => {
                            b.push_str(&crate::i18n::t("pad.sample_missing"))
                        }
                        SampleStatus::Unknown => {}
                    },
                );
                hint::set_text(sh.ui.ctx(), path, tip, Caps::NONE);
            }
        }
    }
    if r.clicked() {
        sh.acts.push(Act::LoadSample(t, pad));
    }
}
