//! The inspector's folded sections (round 39, des-mixer's `inspector-*` sheets): after the
//! inserts, the EQ and the sends, every other part of the strip is one 28-pt row with its
//! state — Routing (where it goes), Sidechain (the key's source), Recording (its input),
//! Automation (how many lanes), Level (its gain), Freeze — and opens in place with a click
//! (`Sheet::fold`). Level stands where the pinned strip stood: volume, pan and width as
//! parameter rows, automatable and modulatable from here as anywhere.

use super::sheet::{self, Range, Sheet, GAIN};
use super::{Act, Subject};
use crate::app::CantusApp;
use crate::kit::{cache, Function};
use cantus_core::mix_cmd::MixCommand;
use cantus_core::{AutoTarget, Command, MergeKey, MixerStrip, StripId};
use egui::Id;
use std::fmt::Write as _;

/// "−3.5 dB" for the Level row's state.
fn db_state(sh: &mut Sheet<'_>, key: &str, db: f32) -> sheet::NameBuf {
    sheet::name(
        sh.fmt
            .get(cache::key(("insp-fold-db", key)), sheet::bits(db), |b| {
                sheet::db(db, b)
            }),
    )
}

/// Level (des-mixer): Volume, Pan (bipolar, a centre tick) and Width as label · bar ·
/// value, automatable and modulatable as anywhere; then, for a strip, the VCAs it follows,
/// its mute and solo, and whether a snapshot's recall or a solo elsewhere passes it by. It
/// stands where the pinned strip stood (r39:lo-mixer), for a strip, a track's strip and
/// the master; a strip's volume moves the selected strips together, as the desk's does.
pub fn level(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    ms: Option<&MixerStrip>,
    master: bool,
    meter: &mut LevelMeter,
) {
    let p = app.project();
    let (gain, pan, width) = match (ms, master) {
        (Some(m), _) => (m.gain_db, m.pan, m.width),
        (None, true) => (p.master.gain_db, p.master.pan, p.master.width),
        _ => return,
    };
    let state = db_state(sh, "level", gain);
    if !sh.fold("inspector.level", false, &state) {
        return;
    }
    sh.band("inspector.level", Function::Output, None, false, None);
    let id = ms.map(|m| m.id);
    let (tg, tp, tw) = match id {
        Some(s) => (
            AutoTarget::StripGain(s),
            AutoTarget::StripPan(s),
            AutoTarget::StripWidth(s),
        ),
        None => (
            AutoTarget::MasterGain,
            AutoTarget::MasterPan,
            AutoTarget::MasterWidth,
        ),
    };
    super::modulators::param_row(
        sh,
        app,
        "inspector.volume",
        Id::new(("insp-level-gain", id)),
        gain,
        &GAIN,
        true,
        sheet::db,
        Function::Output,
        tg,
        false,
        move |gain_db| match id {
            Some(id) => Act::StripGain(id, gain_db),
            None => Act::Merge(Command::SetMasterGain { gain_db }, MergeKey::of(&"master")),
        },
    );
    level_meter(sh, app, id, meter);
    super::modulators::param_row(
        sh,
        app,
        "inspector.pan",
        Id::new(("insp-level-pan", id)),
        pan,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
        sheet::pan,
        Function::Output,
        tp,
        false,
        move |pan| match id {
            Some(id) => Act::Merge(
                Command::SetStripPan { id, pan },
                MergeKey::of(&("spans", id.0)),
            ),
            None => Act::Merge(Command::SetMasterPan { pan }, MergeKey::of(&"master-pan")),
        },
    );
    super::modulators::param_row(
        sh,
        app,
        "inspector.width",
        Id::new(("insp-level-width", id)),
        width,
        &Range::lin(0.0, 2.0, 1.0),
        false,
        sheet::percent,
        Function::Space,
        tw,
        false,
        move |width| match id {
            Some(id) => Act::Merge(
                Command::SetStripWidth { id, width },
                MergeKey::of(&("swidth", id.0)),
            ),
            None => Act::Merge(
                Command::SetMasterWidth { width },
                MergeKey::of(&"master-width"),
            ),
        },
    );
    let Some(m) = ms else {
        return;
    };
    // The VCAs it follows (the desk's strip menu chooses them).
    let vcas = sheet::name(sh.fmt.get(
        cache::key(("insp-level-vca", m.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| {
            let names = crate::views::mixer::vca_ui::names(p, m.id);
            if names.is_empty() {
                b.push_str(&crate::i18n::t("inspector.none"));
            } else {
                b.push_str(&names);
            }
        },
    ));
    sh.text_row("lo_mixer.vca", &vcas, false, m.vcas.is_empty());
    // Mute and solo: the pinned strip's keys, kept.
    mute_solo(sh, m);
    let id = m.id;
    if sh.switch_row("lo_mixer.safe_snapshots", m.recall_safe, false) {
        sh.acts
            .push(Act::Exec(Command::Mix(MixCommand::SetRecallSafe {
                id,
                safe: !m.recall_safe,
            })));
    }
    if sh.switch_row("lo_mixer.safe_solo", m.solo_safe, false) {
        sh.acts
            .push(Act::Exec(Command::Mix(MixCommand::SetSoloSafe {
                id,
                safe: !m.solo_safe,
            })));
    }
}

/// The Level section's meter's ballistics, as the pinned strip's were: up at once, down at
/// 24 dB a second, the peak held a second and a half.
#[derive(Clone, Copy, Debug)]
pub struct LevelMeter {
    level: [f32; 2],
    hold: [f32; 2],
    held: [f64; 2],
}

impl Default for LevelMeter {
    fn default() -> Self {
        LevelMeter {
            level: [crate::kit::meter::MIN_DB; 2],
            hold: [crate::kit::meter::MIN_DB; 2],
            held: [0.0; 2],
        }
    }
}

/// The slim meter under Level's volume (the pinned strip's meter, kept): two 2-pt bars on
/// the volume bar's own travel, so the handle and the level read on one scale — green to
/// −12, yellow to −3, amber to 0, the clip colour above — and a 1-pt peak hold. Its row is
/// always there (nothing moves when sound starts); it is painted only while something
/// plays into it, and asks for frames only then, through the ticker's meter reason: at rest
/// it costs nothing.
fn level_meter(sh: &mut Sheet<'_>, app: &CantusApp, strip: Option<StripId>, m: &mut LevelMeter) {
    use crate::kit::meter::MIN_DB;
    let s = sh.s;
    let w = sh.width();
    let (row, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(8.0)), egui::Sense::hover());
    let p = app.project();
    let raw = match strip.and_then(|id| p.strip_index(id)) {
        Some(i) => {
            let v = app.engine.meters().strips.get(i).copied().unwrap_or(0.0);
            [v, v]
        }
        None => app.engine.meters().master,
    };
    let to_db = |lin: f32| {
        if lin <= 1e-6 {
            MIN_DB
        } else {
            cantus_core::linear_to_db(lin).max(MIN_DB)
        }
    };
    let now = sh.ui.input(|i| i.time);
    let dt = sh.ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
    let mut live = false;
    for (ch, lin) in raw.into_iter().enumerate() {
        let db = to_db(lin);
        m.level[ch] = if db >= m.level[ch] {
            db
        } else {
            (m.level[ch] - 24.0 * dt).max(db).max(MIN_DB)
        };
        if db >= m.hold[ch] || now - m.held[ch] > 1.5 {
            m.hold[ch] = db.max(m.level[ch]);
            m.held[ch] = now;
        }
        live |= m.level[ch] > MIN_DB + 0.1 || m.hold[ch] > MIN_DB + 0.1;
    }
    if !live {
        return;
    }
    crate::views::workspace_ticker::moving(
        sh.ui.ctx(),
        crate::views::workspace_ticker::Reason::Meters,
    );
    // Under the volume's bar: the value column's left, the bar's width.
    let mg = s.s(sheet::MARGIN);
    let value = egui::Rect::from_min_max(
        egui::pos2(row.left() + mg + sh.col + s.s(sheet::GAP), row.top()),
        egui::pos2(row.right() - mg, row.bottom()),
    );
    let bar = super::look::bar_rect(value);
    let pal = crate::views::mixer::look::Pal::of(&s);
    let colour = |db: f32| {
        if db > 0.0 {
            pal.clip
        } else if db > -3.0 {
            pal.meter_hi
        } else if db > -12.0 {
            pal.meter_mid
        } else {
            pal.meter_lo
        }
    };
    for ch in 0..2 {
        let y = row.top() + s.s(1.0) + ch as f32 * s.s(3.0);
        let lane =
            egui::Rect::from_min_size(egui::pos2(bar.left(), y), egui::vec2(bar.width(), s.s(2.0)));
        sh.b.rect(
            sh.b.snap_rect(lane),
            crate::kit::Fill::Solid(pal.meter_groove),
        );
        let x = |db: f32| lane.left() + lane.width() * sheet::gain_pos(db).clamp(0.0, 1.0);
        let lit = egui::Rect::from_min_max(lane.min, egui::pos2(x(m.level[ch]), lane.bottom()));
        if lit.width() > 0.5 {
            sh.b.rect(
                sh.b.snap_rect(lit),
                crate::kit::Fill::Solid(colour(m.level[ch])),
            );
        }
        if m.hold[ch] > MIN_DB + 0.1 {
            let hx = x(m.hold[ch]);
            sh.b.rect(
                sh.b.snap_rect(egui::Rect::from_min_max(
                    egui::pos2(hx - 0.5, lane.top()),
                    egui::pos2(hx + 0.5, lane.bottom()),
                )),
                crate::kit::Fill::Solid(colour(m.hold[ch])),
            );
        }
    }
}

/// Mute and solo as two of des-mixer's keys on one row (the pinned strip had them).
fn mute_solo(sh: &mut Sheet<'_>, m: &MixerStrip) {
    let row = sh.row();
    sh.label(row.label, "lo_mixer.mute_solo", false);
    let half = (row.value.width() - 6.0) * 0.5;
    let keys = [
        ("kit.mixer.mute", m.muted, 0.0),
        ("kit.mixer.solo", m.soloed, half + 6.0),
    ];
    for (key, on, dx) in keys {
        let r = egui::Rect::from_min_size(
            egui::pos2(row.value.left() + dx, row.value.top()),
            egui::vec2(half.min(84.0), row.value.height()),
        );
        let resp = sh.ui.interact(
            r,
            Id::new(("insp-level-key", key, m.id)),
            egui::Sense::click(),
        );
        let st = crate::kit::State::of(&resp, on);
        let word = sheet::name(sh.fmt.tr(key));
        super::look::key(sh.b, &sh.s, r, st, Some(on), None, &word);
        if resp.hovered() {
            crate::kit::hint::set_text(sh.ui.ctx(), &word, "", crate::kit::hint::Caps::NONE);
        }
        if resp.clicked() {
            sh.acts.push(if key == "kit.mixer.mute" {
                Act::Exec(Command::SetStripMute {
                    id: m.id,
                    muted: !m.muted,
                })
            } else {
                Act::ToggleSolo(m.id)
            });
        }
    }
}

/// Sidechain (des-mixer): for each keyed insert, *Key into* (the key glyph, the unit, its
/// place in the chain) and *From* (the source's colour as a 6-pt square, its name; a list
/// that keys it from another strip).
pub fn sidechain(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>) {
    use super::look::Lead;
    use crate::kit::glyph_mixer::MGlyph;
    let Some(m) = ms else {
        return;
    };
    let keyed: Vec<_> = m.effects.iter().filter(|e| e.sidechain.is_some()).collect();
    let state = sheet::name(sh.fmt.get(
        cache::key(("insp-key-state", m.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match keyed.first().and_then(|e| e.sidechain) {
            Some(src) => super::strip_label(app, src, b),
            None => b.push_str(&crate::i18n::t("inspector.none")),
        },
    ));
    if !sh.fold("inspector.sidechain", false, &state) {
        return;
    }
    sh.band("inspector.sidechain", Function::Dynamics, None, false, None);
    if keyed.is_empty() {
        let none = crate::i18n::t("inspector.none");
        sh.text_row("inspector.key_from", &none, false, true);
        return;
    }
    let p = app.project();
    let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
    for e in keyed {
        let place = m.effects.iter().position(|x| x.id == e.id).unwrap_or(0) + 1;
        let unit = sheet::name(sh.fmt.get(cache::key(("insp-key-unit", e.id)), stamp, |b| {
            let n = crate::history_text::effect_name(e);
            b.push_str(n.split(' ').next().unwrap_or(&n));
        }));
        let num = sheet::name(
            sh.fmt
                .get(cache::key(("insp-key-n", e.id)), place as u64, |b| {
                    let _ = write!(b, "{place}");
                }),
        );
        let row = sh.field_row();
        sh.label(row.label, "inspector.key_into", false);
        super::look::field(
            sh.b,
            &sh.s,
            row.value,
            crate::kit::State::REST,
            Lead::Mixer(MGlyph::Key),
            &unit,
            &num,
            false,
        );
        let src = e.sidechain;
        let from = sheet::name(sh.fmt.get(cache::key(("insp-key-from", e.id)), stamp, |b| {
            if let Some(src) = src {
                super::strip_label(app, src, b);
            }
        }));
        let color = src
            .and_then(|id| {
                let i = p.strip_index(id)?;
                let st = &p.strips[i];
                st.color.or_else(|| {
                    p.tracks
                        .iter()
                        .find(|t| t.strip == Some(id))
                        .map(|t| t.color)
                })
            })
            .map(crate::theme::track_color);
        let lead = color.map_or(Lead::None, Lead::Swatch);
        sh.menu_row_with(
            "inspector.key_from",
            lead,
            &from,
            "",
            super::MenuId::Sidechain(m.id, e.id),
            false,
        );
    }
}

/// Recording: the track's input, monitoring, arm (`inspector_record`), its state the
/// input's name.
pub fn recording(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    let p = app.project();
    let track = match subject {
        Subject::Track(t) | Subject::Clip(t, _) | Subject::Notes(t, _) => p.track(t),
        _ => strip.and_then(|s| p.tracks_on_strip(s).next()),
    };
    let Some(track) = track else {
        return;
    };
    let state = match &track.input {
        Some(_) => crate::i18n::t("inspector.has_input"),
        None => crate::i18n::t("rec.no_input_choice"),
    };
    if sh.fold("rec.recording", false, &state) {
        super::record::show(sh, app, subject, strip);
    }
}

/// Automation (des-mixer): how the strip's track writes (*Read · Touch · Latch · Write*, a
/// segmented control), then one line per lane the strip's parameters have — its name, a
/// 56 × 12 sparkline of the lane in the automation colour, its point count.
pub fn automation(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>) {
    use crate::views::mixer::look::{self, W};
    let Some(m) = ms else {
        return;
    };
    let mut rows: Vec<(&'static str, AutoTarget)> = vec![
        ("inspector.volume", AutoTarget::StripGain(m.id)),
        ("inspector.pan", AutoTarget::StripPan(m.id)),
        ("inspector.width", AutoTarget::StripWidth(m.id)),
    ];
    for s in &m.sends {
        rows.push((
            "inspector.send_level",
            AutoTarget::Send {
                strip: m.id,
                to: s.to,
            },
        ));
    }
    rows.retain(|(_, t)| app.is_automated(t));
    let n = rows.len();
    let mut state = String::new();
    lanes_text(&mut state, n);
    if !sh.fold("inspector.automation", false, &state) {
        return;
    }
    sh.band("inspector.automation", Function::Mod, None, false, None);
    // How the strip's track writes.
    if let Some(t) = app.project().tracks_on_strip(m.id).next() {
        use cantus_core::AutoMode as M;
        let modes = [M::Read, M::Touch, M::Latch, M::Write];
        let cur = modes.iter().position(|x| *x == t.auto_mode).unwrap_or(0);
        let row = sh.row();
        sh.label(row.label, "lo_mixer.mode", false);
        let rect = egui::Rect::from_min_size(
            row.value.min,
            egui::vec2(row.value.width().min(sh.s.s(176.0)), row.value.height()),
        );
        let resp = sh.ui.interact(
            rect,
            Id::new(("insp-auto-mode", m.id)),
            egui::Sense::click(),
        );
        let hovered = resp
            .hover_pos()
            .and_then(|q| crate::kit::segmented::hit(rect, modes.len(), q.x));
        let labels = modes.map(|x| crate::kit::text::Src::Key(x.label_key()));
        crate::kit::segmented::paint(
            sh.b,
            rect,
            &sh.s,
            crate::kit::State::of(&resp, false),
            &labels,
            cur,
            hovered,
        );
        if resp.clicked() {
            if let Some(i) = hovered.filter(|i| *i != cur) {
                sh.acts.push(Act::Exec(Command::SetTrackAutoMode {
                    track: t.id,
                    mode: modes[i],
                }));
            }
        }
    }
    let track = app.project().tracks_on_strip(m.id).next().map(|t| t.id);
    if rows.is_empty() {
        let none = crate::i18n::t("inspector.none");
        sh.text_row("inspector.lanes_label", &none, false, true);
        show_lanes_row(sh, track);
        return;
    }
    let lanes = app.auto_lanes();
    let s = sh.s;
    let pal = look::Pal::of(&s);
    for (key, target) in rows {
        let Some(lane) = lanes.iter().find(|l| l.target == target) else {
            continue;
        };
        let row = sh.row();
        sh.label(row.label, key, false);
        let count = lane.segs.len();
        let text = sheet::name(sh.fmt.get(
            cache::key(("insp-auto-n", key, m.id)),
            count as u64 ^ ((crate::i18n::locale() as u64) << 60),
            |b| {
                let t = crate::i18n::t("lo_mixer.points");
                b.push_str(&t.replace("{n}", &count.to_string()));
            },
        ));
        let tw = look::say(
            sh.b,
            &s,
            &text,
            row.value.right(),
            row.value.center().y + 4.0,
            egui::Align2::RIGHT_BOTTOM,
            s.s(64.0),
            10.0,
            W::Regular,
            pal.ink3,
        );
        // The sparkline: the lane over its own span, 24 points.
        let spark = egui::Rect::from_min_size(
            egui::pos2(
                row.value.right() - tw - s.s(8.0) - s.s(56.0),
                row.value.center().y - s.s(6.0),
            ),
            egui::vec2(s.s(56.0), s.s(12.0)),
        );
        let end = lane
            .segs
            .last()
            .map_or(1.0, |g| g.t1.0.max(g.at.0) as f64)
            .max(1.0);
        let mut pts = [egui::Pos2::ZERO; 24];
        for (i, q) in pts.iter_mut().enumerate() {
            let t = end * i as f64 / 23.0;
            let v = lane.range.pos(lane.value_at(t)).clamp(0.0, 1.0);
            *q = egui::pos2(
                spark.left() + spark.width() * i as f32 / 23.0,
                spark.bottom() - spark.height() * v,
            );
        }
        sh.b.polyline(&pts, 1.2, pal.auto);
        for g in lane.segs.iter().take(8) {
            let x = spark.left() + spark.width() * (g.at.0 as f64 / end) as f32;
            let v = lane.range.pos(g.v0).clamp(0.0, 1.0);
            sh.b.circle(
                egui::pos2(x, spark.bottom() - spark.height() * v),
                1.5,
                crate::kit::Fill::Solid(pal.auto),
            );
        }
    }
    show_lanes_row(sh, track);
}

/// *Show in the arrangement* (des-mixer: Regular 11 in the accent): the track's lanes
/// opened and in view, the arrangement in front.
fn show_lanes_row(sh: &mut Sheet<'_>, track: Option<cantus_core::TrackId>) {
    use crate::views::mixer::look::{self, W};
    let Some(t) = track else {
        return;
    };
    let s = sh.s;
    let pal = look::Pal::of(&s);
    let w = sh.width();
    let (r, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(26.0)), egui::Sense::hover());
    let word = sheet::name(sh.fmt.tr("lo_mixer.show_lanes"));
    let m = s.s(sheet::MARGIN);
    let ww = look::width_of(sh.ui.ctx(), &s, &word, 11.0, W::Regular);
    let hit = egui::Rect::from_min_size(
        egui::pos2(r.left() + m, r.top() + s.s(3.0)),
        egui::vec2(ww + s.s(4.0), s.s(20.0)),
    );
    let resp = sh
        .ui
        .interact(hit, Id::new(("insp-show-lanes", t)), egui::Sense::click());
    let ink = if resp.hovered() {
        crate::kit::color::mix(pal.accent, pal.ink1, 0.3)
    } else {
        pal.accent
    };
    look::say(
        sh.b,
        &s,
        &word,
        hit.left(),
        hit.center().y + 4.0,
        egui::Align2::LEFT_BOTTOM,
        ww + s.s(4.0),
        11.0,
        W::Regular,
        ink,
    );
    if resp.hovered() {
        let tip = sheet::name(sh.fmt.tr("lo_mixer.show_lanes_tip"));
        crate::kit::hint::set_text(sh.ui.ctx(), &word, &tip, crate::kit::hint::Caps::NONE);
    }
    if resp.clicked() {
        sh.acts.push(Act::ShowLanes(t));
    }
}

/// Freeze (des-mixer): the key — the snowflake and *Freeze strip* (frozen: *Unfreeze*, the
/// state "Frozen" in the accent) — and what it does, in a sentence under it. A bounced
/// track's kept delay stays a row above it (`inspector_record`).
pub fn freeze(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    use crate::kit::glyph_mixer::MGlyph;
    let frozen = strip.is_some_and(|s| app.freeze_holding_strip(s).is_some());
    let state = if frozen {
        crate::i18n::t("inspector.frozen")
    } else {
        String::new()
    };
    if !sh.fold("freeze.section", false, &state) {
        return;
    }
    let band = sh.band("freeze.section", Function::Source, None, false, None);
    let _ = band;
    super::record::lateness_row(sh, app, subject);
    let Some((target, holder)) = super::record::freeze_target(app, subject, strip) else {
        return;
    };
    let held = holder.is_some();
    let s = sh.s;
    let w = sh.width();
    let word = sheet::name(sh.fmt.tr(if held {
        "lo_mixer.unfreeze"
    } else {
        "lo_mixer.freeze_strip"
    }));
    let (row, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(32.0)), egui::Sense::hover());
    let m = s.s(sheet::MARGIN);
    let kw = super::look::key_width(sh.ui.ctx(), &s, &word, true);
    let kr = egui::Rect::from_min_size(
        egui::pos2(row.left() + m, row.top() + s.s(4.0)),
        egui::vec2(kw, s.s(24.0)),
    );
    let resp = sh.ui.interact(
        kr,
        Id::new(("insp-freeze-key", strip)),
        egui::Sense::click(),
    );
    let st = crate::kit::State::of(&resp, held);
    super::look::key(sh.b, &s, kr, st, None, Some(MGlyph::Freeze), &word);
    if resp.hovered() {
        let tip = sheet::name(sh.fmt.tr("freeze.row"));
        crate::kit::hint::set_text(sh.ui.ctx(), &word, &tip, crate::kit::hint::Caps::NONE);
    }
    if resp.clicked() {
        sh.acts.push(Act::Freeze(match holder {
            Some(h) => crate::views::freeze_view::InspAct::Unfreeze(h),
            None => crate::views::freeze_view::InspAct::Freeze(target),
        }));
    }
    // What it does, wrapped under the key in Regular 10 ink 3.
    let says = sheet::name(sh.fmt.tr(if held {
        "lo_mixer.frozen_says"
    } else {
        "lo_mixer.freeze_says"
    }));
    let k = s.tk() * 10.0 / 11.0;
    let lines = crate::kit::prose::wrap(
        sh.ui.ctx(),
        &says,
        crate::kit::text::Role::Label,
        k,
        w - 2.0 * m,
    );
    let (rect, _) = sh.ui.allocate_exact_size(
        egui::vec2(w, lines.height() + s.s(8.0)),
        egui::Sense::hover(),
    );
    let pal = crate::views::mixer::look::Pal::of(&s);
    crate::kit::prose::paint(
        sh.b,
        rect.shrink2(egui::vec2(m, 0.0)),
        &lines,
        crate::kit::text::Role::Label,
        k,
        pal.ink3,
        egui::Align2::LEFT_TOP,
    );
    // Frozen: what the render holds — the inserts it took (a strip's), and its file's size.
    // Freeze measures no CPU, so none is claimed.
    let Some(h) = holder else {
        return;
    };
    let p = app.project();
    let rate = app.engine.sample_rate.round() as u32;
    let content = p.frozen(h).map(|f| f.content.as_str());
    let opened = content.and_then(|c| app.engine.frozen.opened_path(c, rate));
    let stamp = app.bus.revision()
        ^ ((crate::i18n::locale() as u64) << 60)
        ^ (u64::from(opened.is_some()) << 59);
    let what = sheet::name(
        sh.fmt
            .get(cache::key(("insp-frz-what", strip)), stamp, |b| {
                let mut parts: Vec<String> = Vec::new();
                if let cantus_core::FreezeTarget::Strip(id) = h {
                    let n = p
                        .strip(id)
                        .map_or(0, |m| m.effects.iter().filter(|e| !e.bypass).count());
                    let t = crate::i18n::t(if n == 1 {
                        "lo_mixer.inserts_rendered_one"
                    } else {
                        "lo_mixer.inserts_rendered"
                    });
                    parts.push(t.replace("{n}", &n.to_string()));
                }
                // The file it plays from, else its first copy (the engine opens a render
                // when it plays it): looked up only when the project or the render changes.
                let file =
                    opened.or_else(|| content.and_then(|c| app.engine.frozen.path_of(c, rate)));
                let len = file
                    .as_ref()
                    .and_then(|f| std::fs::metadata(f).ok())
                    .map(|m| m.len());
                if let Some(len) = len {
                    let mb = len as f64 / 1_048_576.0;
                    parts.push(if mb >= 10.0 {
                        format!("{mb:.0} MB")
                    } else {
                        format!("{mb:.1} MB")
                    });
                }
                b.push_str(&parts.join(" \u{b7} "));
            }),
    );
    if what.is_empty() {
        return;
    }
    let (r, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(16.0)), egui::Sense::hover());
    crate::views::mixer::look::say(
        sh.b,
        &s,
        &what,
        r.left() + m,
        r.top() + s.s(11.0),
        egui::Align2::LEFT_BOTTOM,
        w - 2.0 * m,
        10.0,
        crate::views::mixer::look::W::Regular,
        pal.ink3,
    );
}

/// "{n} lanes" into `out`.
pub fn lanes_text(out: &mut String, n: usize) {
    let t = crate::i18n::t("inspector.lanes");
    match t.split_once("{n}") {
        Some((a, z)) => {
            out.push_str(a);
            let _ = write!(out, "{n}");
            out.push_str(z);
        }
        None => out.push_str(&t),
    }
}
