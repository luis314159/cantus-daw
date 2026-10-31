//! The audio part of a clip's context menu: the two fades' curves as rows of drawn keys
//! (FL draws choices, not words), then normalise, gain, reverse, fades, rename, and the
//! file — shown in the file manager, or located again when it is missing.

use super::super::curve_icon;
use super::{apply, normalize, set_all, CURVE_KEYS};
use crate::app::CantusApp;
use crate::audio_clips::{self as ac, Fade};
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, Style};
use crate::waveform::Lookup;
use cantus_core::{FadeCurve, Ticks};
use egui::{vec2, Align2, Rect, Sense};

/// The audio entries under the clip's own menu (only for an audio clip).
pub fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return;
    };
    let a = a.clone();
    let missing = app
        .project()
        .audio_source(a.source)
        .map(|s| {
            matches!(
                app.peaks.lookup(s, app.audio_base(), None),
                Lookup::Missing | Lookup::Failed(_)
            )
        })
        .unwrap_or(true);
    curve_row(app, ui, r, Fade::In, a.fade_in_curve);
    curve_row(app, ui, r, Fade::Out, a.fade_out_curve);
    let fades = a.fade_in > Ticks::ZERO || a.fade_out > Ticks::ZERO;
    let action = |label: &'static str, check: Check, enabled: bool| Item::Action {
        label: Src::Key(label),
        check,
        shortcut: None,
        submenu: false,
        enabled,
    };
    // Round 31 (ADR 0070): the overlap it starts in, crossed or summed.
    let pair = crate::clipfx::pair(app, r);
    let crossed = a.cross.is_some() && pair.is_some();
    let rename_key = app
        .keymap
        .bindings_for(crate::keymap::Action::RenameClip)
        .first()
        .map(|b| b.display());
    let items = [
        Item::Band(Src::Key("audioclips.menu.band")),
        // Round 28 (ADR 0054): the clip's notes in the pitch editor.
        action("pitch.menu.edit", Check::None, !missing),
        action("audioclips.menu.normalize", Check::None, !missing),
        action("audioclips.menu.reset_gain", Check::None, a.gain_db != 0.0),
        action(
            "audioclips.menu.reverse",
            if a.reverse { Check::On } else { Check::Off },
            true,
        ),
        action("audioclips.fade_remove", Check::None, fades),
        action(
            "clipfx.menu.show_gain",
            if app.settings.show_clip_gain {
                Check::On
            } else {
                Check::Off
            },
            true,
        ),
        action("clipfx.menu.reset_env", Check::None, a.gain_env.is_some()),
        action(
            "clipfx.menu.crossfade",
            if crossed { Check::On } else { Check::Off },
            pair.is_some(),
        ),
        Item::Action {
            label: Src::Key("clip.rename"),
            check: Check::None,
            shortcut: rename_key.as_deref(),
            submenu: false,
            enabled: true,
        },
        Item::Separator,
        if missing {
            action("audioclips.menu.locate", Check::None, true)
        } else {
            action("audioclips.menu.reveal", Check::None, true)
        },
        // Round 28 (ADR 0054): what the pitch editor makes, without opening it.
        Item::Separator,
        action("pitch.midi_new", Check::None, !missing),
        action("pitch.menu.double_third", Check::None, !missing),
        action("pitch.menu.double_fifth", Check::None, !missing),
        // Round 34 (ADR 0085): its parts by a model — the selection's when it is in it.
        Item::Separator,
        action(
            "stems.menu.separate",
            Check::None,
            !missing && app.settings.ai.enabled,
        ),
        action(
            "transcribe.menu",
            Check::None,
            !missing && app.settings.ai.enabled,
        ), // r34:transcribe
    ];
    let Some(i) = kit::widgets::menu_list(ui, &items, super::super::super::MENU_W)
        .1
        .clicked
    else {
        return;
    };
    match i {
        1 => app.open_pitch_editor(r.0, r.1),
        2 => {
            normalize(app);
        }
        3 => set_all(app, "cmd.set_clip_audio", |_, a| ac::with_gain(a, 0.0)),
        4 => {
            let on = !a.reverse;
            set_all(app, "cmd.set_clip_audio", |_, a| cantus_core::AudioRegion {
                reverse: on,
                ..a.clone()
            });
        }
        5 => set_all(app, "cmd.set_clip_audio", |c, a| {
            ac::with_fade_out(c, &ac::with_fade_in(c, a, Ticks::ZERO), Ticks::ZERO)
        }),
        6 => app.toggle_clip_gain(),
        7 => set_all(app, "cmd.clipfx.reset_env", |_, a| {
            crate::clipfx::cleared(a)
        }),
        8 => {
            let curve = (!crossed).then_some(app.settings.crossfade_curve);
            // The selection's overlaps when this clip is among them, else this one's.
            if app.clip_selection().contains(&r) {
                app.crossfade_selection(curve);
            } else {
                app.set_crossfade(r, curve);
            }
        }
        9 => app.run_action(crate::keymap::Action::RenameClip),
        11 if missing => app.relink_audio_dialog(a.source),
        11 => app.reveal_audio(a.source),
        13 => app.pitch_make(r.0, r.1, None),
        14 => app.pitch_make(r.0, r.1, Some(cantus_core::pitch::midi::Interval::Third)),
        15 => app.pitch_make(r.0, r.1, Some(cantus_core::pitch::midi::Interval::Fifth)),
        17 => {
            let sel = app.clip_selection();
            let clips = if sel.contains(&r) { sel } else { vec![r] };
            app.open_separate_stems(crate::stems_ui::Target::Clips(clips));
        }
        18 => crate::transcribe_ui::run::open(app, r.0, r.1), // r34:transcribe
        _ => {}
    }
    ui.close();
}

/// One fade's curves as a row: its name, then five drawn keys, the current one lit.
fn curve_row(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef, fade: Fade, current: FadeCurve) {
    let s = Style::current();
    let k = s.k;
    let key = s.s(22.0);
    ui.horizontal(|ui| {
        let (label, _) = ui.allocate_exact_size(vec2(s.s(64.0), key), Sense::hover());
        let name = match fade {
            Fade::In => "audioclips.fade_in",
            Fade::Out => "audioclips.fade_out",
        };
        kit::with_brush(ui.painter(), |b| {
            let tx = Text::key(name, Role::Body, s.t.menu_text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, label.shrink2(vec2(6.0 * k, 0.0)), &tx, s.tk());
        });
        for (c, tip) in FadeCurve::ALL.iter().copied().zip(CURVE_KEYS) {
            let (rect, resp) = ui.allocate_exact_size(vec2(key, key), Sense::click());
            let lit = c == current;
            paint_key(ui, rect, &s, lit, resp.hovered(), c, fade);
            if resp.hovered() {
                let name = t(tip);
                hint::set_text(ui.ctx(), &name, "", Caps::NONE);
            }
            if resp.clicked() {
                if let Some((_, a)) = ac::region(app.project(), r) {
                    let next = ac::with_curve(a, fade, c);
                    apply(app, r, next);
                    app.end_gesture();
                }
                ui.close();
            }
        }
    });
}

fn paint_key(
    ui: &egui::Ui,
    rect: Rect,
    s: &Style,
    lit: bool,
    hovered: bool,
    c: FadeCurve,
    fade: Fade,
) {
    let k = s.k;
    kit::with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "fade_curve",
            &[kit::Tok::MenuHover, kit::Tok::MenuText, kit::Tok::Selection],
            &[],
        );
        let r = rect.shrink(1.0 * k);
        if lit {
            b.rrect(
                r,
                s.r(3.0),
                kit::Fill::Solid(kit::color::with_alpha(s.t.selection, 0.22)),
            );
            b.rrect_stroke(r, s.r(3.0), b.px(), s.t.selection);
        } else if hovered {
            b.rrect(r, s.r(3.0), kit::Fill::Solid(s.t.menu_hover));
        }
        let ink = if hovered && !lit {
            s.t.menu_text_hover
        } else {
            s.t.menu_text
        };
        curve_icon(b, r, c, fade, ink, k);
    });
}
