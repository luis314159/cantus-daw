//! The top bar's **sync** group (round 33, ADR 0078–0079): three cells in wells, each a
//! lamp, a word and a value, as the inputs' cells are — LINK and Q over EXT, the bar's two
//! rows, so the group is as narrow as its widest row.
//!
//! - **LINK** — Ableton Link on or off (a click), its lamp lit while on, the value the
//!   number of other apps in the session; red when Link could not open the network. A
//!   right-click opens Settings ▸ MIDI.
//! - **Q** — Link's quantum in beats (a click steps 1, 2, 3, 4, 8): the phase matched.
//! - **EXT** — the source followed (Link, MIDI clock, time code; none: dim), its lamp the
//!   lock light (lit locked, amber pulling in or waiting for its moment), the value the
//!   tempo it plays. A click opens Settings ▸ MIDI, where the source is chosen.

use super::{Style, STEP};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::tokens::Tok;
use crate::kit::{led, light, with_brush};
use crate::views::transport::sync_service::{self as svc, SyncSource};
use cantus_engine::transport::sync::Lock;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::sync::atomic::Ordering;

const WORDS: [&str; 3] = ["sync.topbar.link", "sync.topbar.quantum", "sync.topbar.ext"];
/// The widest value each cell shows.
const VALUES: [&str; 3] = ["88", "8", "888.8"];

fn cell_w(ctx: &egui::Context, s: &Style, i: usize) -> f32 {
    let word = text::measure(
        ctx,
        &Text::key(WORDS[i], Role::Micro, s.t.text).caps(true),
        s.tk(),
    )
    .x;
    let value = text::measure(ctx, &Text::new(VALUES[i], Role::Value, s.t.text), s.tk()).x;
    // The quantum's cell has no lamp: a word and a number.
    let lamp = if i == 1 { 16.0 } else { 26.0 };
    word + value + lamp * s.k
}

/// The group's width: its wider row and a step either side.
pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    let top = cell_w(ctx, s, 0) + STEP * s.k + cell_w(ctx, s, 1);
    top.max(cell_w(ctx, s, 2)) + 2.0 * STEP * s.k
}

/// A lamp (none for `None`; dark for `Some(None)`), a word and a value in a well.
fn cell(
    ui: &Ui,
    r: Rect,
    s: &Style,
    lamp: Option<Option<Tok>>,
    word: &str,
    value: &str,
    dim: bool,
) {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        light::sunken(b, r, s.r(3.0), s.t.well, 1.0, s.t.depth);
        let c = match lamp {
            Some(l) => {
                let c = Pos2::new(r.left() + 9.0 * k, r.center().y);
                led::paint(
                    b,
                    c,
                    7.0 * k,
                    l.is_some(),
                    s,
                    l.unwrap_or(Tok::Led),
                    Tok::Well,
                    1.0,
                );
                c
            }
            None => Pos2::new(r.left() - 1.0 * k, r.center().y),
        };
        let word_w = text::measure(
            ui.ctx(),
            &Text::key(word, Role::Micro, s.t.text).caps(true),
            s.tk(),
        )
        .x;
        let wr = Rect::from_min_size(
            Pos2::new(c.x + 7.0 * k, r.top()),
            Vec2::new(word_w + 2.0 * k, r.height()),
        );
        let _ = text::paint(
            b,
            wr,
            &Text::key(word, Role::Micro, s.t.text_dim)
                .caps(true)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let vr = Rect::from_min_max(
            Pos2::new(wr.right() + 2.0 * k, r.top()),
            Pos2::new(r.right() - 4.0 * k, r.bottom()),
        );
        let ink = if dim { s.t.text_dim } else { s.t.text };
        let _ = text::paint(
            b,
            vr,
            &Text::new(value, Role::Value, ink)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// What the group shows, read once per frame.
pub struct Facts {
    pub source: SyncSource,
    pub link_on: bool,
    pub link_failed: bool,
    pub peers: u32,
    pub quantum: f64,
    pub lock: Lock,
    pub tempo: f64,
}

pub fn facts(app: &CantusApp) -> Facts {
    let st = &app.sync.status;
    let (lock, tempo) = app
        .sync
        .shared()
        .map_or((Lock::Idle, 0.0), |sh| (sh.lock_state(), sh.tempo()));
    Facts {
        source: app.settings.sync.source,
        link_on: st.link_on.load(Ordering::Relaxed),
        link_failed: st.link_failed.load(Ordering::Relaxed),
        peers: st.peers.load(Ordering::Relaxed),
        quantum: app.settings.sync.quantum,
        lock,
        tempo,
    }
}

/// The tempo the transport follows, for the tempo well: Link's session tempo while Link is
/// followed, the clock's or the code's while one is locked; `None` plays the project's.
pub fn followed_tempo(app: &CantusApp) -> Option<f64> {
    let f = facts(app);
    match f.source {
        SyncSource::Link if f.link_on => Some(f64::from_bits(
            app.sync.status.link_tempo.load(Ordering::Relaxed),
        ))
        .filter(|t| *t > 0.0),
        SyncSource::Clock | SyncSource::Mtc if f.lock != Lock::Idle && f.tempo > 0.0 => {
            Some(f.tempo)
        }
        _ => None,
    }
}

/// A gesture on the tempo well (`by` BPM): while Link is followed it asks the session for
/// its tempo plus `by` and leaves the project's alone (returns 0); otherwise it is the
/// project's (returned as it came).
pub fn take_tempo_gesture(app: &mut CantusApp, by: f64) -> f64 {
    if by == 0.0 || app.settings.sync.source != SyncSource::Link {
        return by;
    }
    match followed_tempo(app) {
        Some(t) => {
            app.sync.request_tempo((t + by).clamp(20.0, 999.0));
            0.0
        }
        None => by,
    }
}

/// The source's name (the tips, Settings).
pub fn source_name(source: SyncSource) -> String {
    t(match source {
        SyncSource::None => "sync.source.none",
        SyncSource::Link => "sync.source.link",
        SyncSource::Clock => "sync.source.clock",
        SyncSource::Mtc => "sync.source.mtc",
    })
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let f = facts(app);
    // Two rows: LINK and Q over EXT, each cell as high as a row allows.
    let row = (r.height() - 3.0 * STEP * k) / 2.0;
    let h = s.d(20.0).min(row);
    let x0 = r.left() + STEP * k;
    let inner = r.width() - 2.0 * STEP * k;
    let top_y = r.top() + STEP * k + (row - h) / 2.0;
    let bottom_y = r.top() + 2.0 * STEP * k + row + (row - h) / 2.0;
    let q_w = cell_w(ui.ctx(), s, 1);
    let link_w = inner - q_w - STEP * k;
    let rects = [
        Rect::from_min_size(Pos2::new(x0, top_y), Vec2::new(link_w, h)),
        Rect::from_min_size(Pos2::new(x0 + link_w + STEP * k, top_y), Vec2::new(q_w, h)),
        Rect::from_min_size(Pos2::new(x0, bottom_y), Vec2::new(inner, h)),
    ];
    let ids = ["topbar-link", "topbar-quantum", "topbar-ext"];
    let resp: Vec<egui::Response> = (0..3)
        .map(|i| ui.interact(rects[i], Id::new(ids[i]), Sense::click()))
        .collect();
    // LINK.
    let link_lamp = if f.link_failed {
        Some(Tok::Danger)
    } else {
        f.link_on.then_some(Tok::Led)
    };
    let peers = f.peers.to_string();
    cell(
        ui,
        rects[0],
        s,
        Some(link_lamp),
        WORDS[0],
        &peers,
        !f.link_on,
    );
    // Q.
    let q = format!("{}", f.quantum);
    let link = f.source == SyncSource::Link;
    cell(ui, rects[1], s, None, WORDS[1], &q, !link);
    // EXT.
    let lamp = match f.lock {
        Lock::Locked => Some(Tok::Led),
        Lock::Seeking | Lock::Queued => Some(Tok::MeterHigh),
        Lock::Idle => None,
    };
    let following = f.source != SyncSource::None;
    let tempo = if following && f.tempo > 0.0 {
        format!("{:.1}", f.tempo)
    } else {
        "\u{2014}".into()
    };
    cell(ui, rects[2], s, Some(lamp), WORDS[2], &tempo, !following);
    if f.link_on || following {
        // The peers, the lock and the tempo change without input: look again soon.
        crate::views::workspace_ticker::wake_after(ui.ctx(), std::time::Duration::from_millis(250));
    }
    // Tips and clicks.
    if resp[0].hovered() {
        let tip = if f.link_failed {
            t("sync.tip.link_failed")
        } else if f.link_on {
            crate::i18n::tf_count("sync.tip.link_on", f.peers as usize, &[])
        } else {
            t("sync.tip.link_off")
        };
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp[0].clone().on_hover_text(tip);
    }
    if resp[0].clicked() {
        svc::toggle_link(app);
    }
    if resp[0].secondary_clicked() || resp[2].clicked() {
        crate::views::workspace_settings::open(
            app,
            crate::views::workspace_settings::SettingsPage::Midi,
        );
    }
    if resp[1].hovered() {
        let tip = tf("sync.tip.quantum", &[("q", &q)]);
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp[1].clone().on_hover_text(tip);
    }
    if resp[1].clicked() {
        let qs = svc::QUANTA;
        let i = qs
            .iter()
            .position(|x| *x == f.quantum)
            .map_or(3, |i| (i + 1) % qs.len());
        app.settings.sync.quantum = qs[i];
        svc::apply(app);
        app.store_settings();
    }
    if resp[2].hovered() {
        let state = t(match f.lock {
            Lock::Locked => "sync.lock.locked",
            Lock::Seeking => "sync.lock.seeking",
            Lock::Queued => "sync.lock.queued",
            Lock::Idle => "sync.lock.idle",
        });
        let tip = if following {
            tf(
                "sync.tip.ext",
                &[
                    ("source", &source_name(f.source)),
                    ("tempo", &tempo),
                    ("state", &state),
                ],
            )
        } else {
            t("sync.tip.ext_none")
        };
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp[2].clone().on_hover_text(tip);
    }
}
