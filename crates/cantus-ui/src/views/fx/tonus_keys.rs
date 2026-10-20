//! Tonus's one-octave keyboard: each pitch class a target (a dot), removed (a cross) or
//! bypassed (a ring). A click makes a note a target or removes it, a right-click bypasses
//! it — one undo step each. With a MIDI source it shows the notes held, lit.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use crate::kit::color::shade;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::tonus::NOTE_KEYS;
use cantus_dsp::tonus::{scale, Allowed, NoteState, Source, TonusSettings};
use cantus_engine::fx::tonus::Header;
use egui::{pos2, vec2, Align2, Rect, Sense, Vec2};

/// The white keys' pitch classes, and after which white key each black one sits.
const WHITES: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];
const BLACKS: [(usize, usize); 5] = [(1, 1), (3, 2), (6, 4), (8, 5), (10, 6)];

/// The keys' rects in `rect`, C … B.
pub(super) fn layout(rect: Rect) -> [Rect; 12] {
    let mut out = [Rect::NOTHING; 12];
    let ww = rect.width() / 7.0;
    for (i, pc) in WHITES.iter().enumerate() {
        out[*pc] = Rect::from_min_size(
            pos2(rect.left() + ww * i as f32, rect.top()),
            vec2(ww, rect.height()),
        );
    }
    for (pc, after) in BLACKS {
        let bw = ww * 0.6;
        out[pc] = Rect::from_min_size(
            pos2(rect.left() + ww * after as f32 - bw * 0.5, rect.top()),
            vec2(bw, rect.height() * 0.58),
        );
    }
    out
}

/// The key under a point: black keys first (they sit on top).
pub(super) fn hit(keys: &[Rect; 12], p: egui::Pos2) -> Option<usize> {
    BLACKS
        .iter()
        .map(|b| b.0)
        .find(|pc| keys[*pc].contains(p))
        .or_else(|| WHITES.iter().copied().find(|pc| keys[*pc].contains(p)))
}

/// What a click does to a note: a target is removed (or, if the scale lacks it, goes back
/// to the scale), anything else becomes a target. A right-click bypasses it, or takes the
/// bypass off.
pub(super) fn after_click(ss: &TonusSettings, pc: usize, right: bool) -> NoteState {
    let allowed = scale::resolve(ss.scale, ss.key, &ss.notes)[pc];
    let in_scale = ss.scale.contains(ss.key, pc as u8);
    if right {
        return if allowed == Allowed::Bypass {
            NoteState::Scale
        } else {
            NoteState::Bypass
        };
    }
    match (allowed, in_scale) {
        (Allowed::Target, true) => NoteState::Remove,
        (Allowed::Target, false) => NoteState::Scale,
        (_, true) => NoteState::Scale,
        (_, false) => NoteState::On,
    }
}

/// The keyboard in a box of `size`, with its legend; returns the keys' rects.
pub(super) fn show(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    size: Vec2,
    ss: &TonusSettings,
    h: &Header,
) -> [Rect; 12] {
    let k = s.k;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let legend_h = 16.0 * k;
    let kb = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - legend_h));
    let keys = layout(kb);
    let editable = ss.source == Source::Scale;
    let resp = ui.interact(kb, ui.id().with("tonus-keys"), Sense::click());
    // Each note of the scale is a parameter: in the learn mode each key is outlined and a
    // click learns it; a mapped one wears the mark — the right-click stays the note's own
    // cycle (round 29, review 2 of ADR 0062, #44).
    let learning = learn_keys(host, ui, s, &keys);
    if editable && !learning {
        let right = resp.secondary_clicked();
        if resp.clicked() || right {
            if let Some(pc) = resp.interact_pointer_pos().and_then(|p| hit(&keys, p)) {
                let next = after_click(ss, pc, right);
                host.set(NOTE_KEYS[pc], next.index() as f32, None);
            }
        }
    }
    let allowed = scale::resolve(ss.scale, ss.key, &ss.notes);
    if resp.hovered() {
        match resp.hover_pos().and_then(|p| hit(&keys, p)) {
            Some(pc) if editable => {
                let state = match allowed[pc] {
                    Allowed::Target => "fx.tonus.note.1",
                    Allowed::Removed => "fx.tonus.note.2",
                    Allowed::Bypass => "fx.tonus.note.3",
                };
                let label = [
                    "fx.tonus.n0",
                    "fx.tonus.n1",
                    "fx.tonus.n2",
                    "fx.tonus.n3",
                    "fx.tonus.n4",
                    "fx.tonus.n5",
                    "fx.tonus.n6",
                    "fx.tonus.n7",
                    "fx.tonus.n8",
                    "fx.tonus.n9",
                    "fx.tonus.n10",
                    "fx.tonus.n11",
                ][pc];
                hint::set_text(ui.ctx(), frame::tr(label), frame::tr(state), Caps::NONE);
            }
            _ if editable => hint::set_text(ui.ctx(), frame::tr("tonus.keys_tip"), "", Caps::NONE),
            _ => hint::set_text(ui.ctx(), frame::tr("tonus.midi_held"), "", Caps::NONE),
        }
    }
    if !ui.is_rect_visible(rect) {
        return keys;
    }
    let ink = Ink::of(s);
    let target_pc = h.target.map(|t| t.rem_euclid(12) as usize);
    with_brush(ui.painter(), |b| {
        b.register(
            kb,
            "keyboard",
            crate::kit::shell::KEYBOARD_TOKENS,
            &[s.hue_tok],
        );
        let state = |pc: usize| -> Allowed {
            if editable {
                allowed[pc]
            } else if h.held & (1 << pc) != 0 {
                Allowed::Target
            } else {
                Allowed::Removed
            }
        };
        for pc in WHITES {
            key(
                b,
                s,
                &ink,
                keys[pc],
                false,
                state(pc),
                target_pc == Some(pc),
                editable,
            );
        }
        for (pc, _) in BLACKS {
            key(
                b,
                s,
                &ink,
                keys[pc],
                true,
                state(pc),
                target_pc == Some(pc),
                editable,
            );
        }
        let y = rect.bottom() - legend_h * 0.5;
        if !editable {
            canvas::put_key(
                b,
                pos2(rect.left() + 6.0 * k, y),
                Align2::LEFT_CENTER,
                "tonus.midi_held",
                Role::Micro,
                ink.dim,
                s.tk(),
            );
            return;
        }
        // The legend: a dot, a cross, a ring.
        let mut x = rect.left() + 6.0 * k;
        for (i, word) in ["tonus.legend.on", "tonus.legend.off", "tonus.legend.bypass"]
            .iter()
            .enumerate()
        {
            glyph(b, s, &ink, pos2(x, y), i, ink.text);
            let r = canvas::put_key(
                b,
                pos2(x + 8.0 * k, y),
                Align2::LEFT_CENTER,
                word,
                Role::Micro,
                ink.dim,
                s.tk(),
            );
            x = r.right() + 14.0 * k;
        }
    });
    keys
}

/// One key: its face, and the glyph of its state.
#[allow(clippy::too_many_arguments)]
fn key(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    black: bool,
    state: Allowed,
    current: bool,
    editable: bool,
) {
    let k = s.k;
    let t = &s.t;
    let base = if black { t.key_black } else { t.key_white };
    let face = match state {
        _ if current => s.hue,
        Allowed::Removed if editable => shade(base, if black { 0.10 } else { -0.22 }),
        Allowed::Target if !editable => s.hue,
        _ => base,
    };
    let r = b.snap_rect(r.shrink2(vec2(0.5 * k, 0.0)));
    b.rect(r, Fill::Solid(shade(face, -0.30)));
    let inner = Rect::from_min_max(r.min, pos2(r.right() - b.px(), r.bottom() - b.px()));
    b.rrect(
        inner,
        2.0 * k,
        Fill::V(shade(face, -0.05), shade(face, 0.04)),
    );
    if !editable {
        return;
    }
    let on_dark = black || current;
    let c = if on_dark { t.key_white } else { t.key_black };
    let at = pos2(
        r.center().x,
        r.bottom() - (if black { 10.0 } else { 12.0 }) * k,
    );
    let i = match state {
        Allowed::Target => 0,
        Allowed::Removed => 1,
        Allowed::Bypass => 2,
    };
    glyph(b, s, ink, at, i, if i == 0 && !current { s.hue } else { c });
}

/// The state's glyph at `c`: 0 a dot, 1 a cross, 2 a ring.
fn glyph(b: &mut Brush, s: &Style, _ink: &Ink, c: egui::Pos2, which: usize, color: egui::Color32) {
    let k = s.k;
    match which {
        0 => b.dot(c, 3.0 * k, color),
        1 => {
            let d = 3.0 * k;
            b.segment(c + vec2(-d, -d), c + vec2(d, d), 1.5 * k, color, true);
            b.segment(c + vec2(-d, d), c + vec2(d, -d), 1.5 * k, color, true);
        }
        _ => b.circle_stroke(c, 3.5 * k, 1.5 * k, color),
    }
}

/// The keys as their notes' parameters for MIDI learn: `true` while the learn mode is on.
fn learn_keys(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, keys: &[Rect; 12]) -> bool {
    use crate::kit::automate as a;
    let mut mode = false;
    // The white keys first, the black keys over them: the last spot registered takes the
    // click, and a black key sits across two white ones (review 3, #43).
    const ORDER: [usize; 12] = [0, 2, 4, 5, 7, 9, 11, 1, 3, 6, 8, 10];
    for pc in ORDER {
        let r = &keys[pc];
        let key = NOTE_KEYS[pc];
        let pos = host.spec().get(key) / 3.0;
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
        let Some((m, _)) = host.modulation(key, pos, s, &mut rings) else {
            continue;
        };
        let Some(l) = m.learn else {
            continue;
        };
        if l.mode {
            mode = true;
            if a::learn_spot(ui, *r, ui.id().with(("tonus-learn", pc)), l) {
                host.modulate(key, a::Choice::Learn, *r);
            }
        } else if l.mapped {
            a::learn_mark_over(ui, *r, ui.clip_rect());
        }
    }
    mode
}
