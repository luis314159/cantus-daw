//! Every dropdown of the app opens on an opaque sheet beside its field (beta.14, the owner:
//! "muy cristalino, no se distingue" — the audio popover's *Input device* list rose over
//! the *Driver* row, Settings ▸ Audio's *Output device* list was drawn on nothing over the
//! rows beneath; "check whether other options are like this too"). Its own test binary: a
//! look is installed for the process.
//!
//! Through `CantusApp::assemble`, in Slate (the default), Slate Paper (the owner's light
//! look) and High contrast light: on every screen that holds a dropdown — the eleven
//! Settings pages, the audio popover, the inspector, the desk, an instrument's editor, an
//! effect's editor, the export dialog, the top bar's tempo — every field the help line
//! names as one that opens a list (`Caps::menu`) is clicked the way a user does, and the
//! list that opens is
//!
//! - **opaque**: everything else on the screen moved away for one frame and a probe colour
//!   painted beneath, no point inside its sheet shows the probe;
//! - **beside its field**: under it (or above it), its left edge over the field's span,
//!   never covering the field, and inside the window;
//! - **legible**: no two of its rows' words overlap, and none is cut at its sides (cut at
//!   the foot or the head only where the list scrolls).

mod paths_rig;

use cantus_ui::kit::appearance::Mode;
use egui::{LayerId, Order, Rect};
use paths_rig::*;
use std::collections::HashSet;

/// The looks every dropdown is opened in: (preset, light).
const LOOKS: [(&str, bool); 3] = [
    ("slate_night", false),
    ("slate_paper", true),
    ("contrast_light", true),
];

/// The floating layers drawn now, and their rects.
fn floating(r: &Rig) -> Vec<(LayerId, Rect)> {
    r.ctx.memory(|m| {
        let mut v: Vec<(LayerId, Rect)> = m
            .areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| matches!(l.order, Order::Foreground | Order::Tooltip))
            .filter_map(|l| Some((l, m.area_rect(l.id)?)))
            .filter(|(_, r)| r.is_positive())
            .collect();
        v.sort_by_key(|(l, _)| l.id.value());
        v
    })
}

/// One finding: which list, where, what is wrong.
#[derive(Debug)]
struct Finding {
    look: &'static str,
    screen: String,
    field: String,
    what: String,
}

/// What one opened list was measured as.
struct Opened {
    sheet: Rect,
}

/// The probe colour: nothing of the app is this magenta.
const PROBE: egui::Color32 = egui::Color32::from_rgb(255, 0, 255);

/// Measure the list `layers` opened from `field` (named `name`) on `screen`.
fn measure(
    r: &mut Rig,
    layers: &[(LayerId, Rect)],
    field: Rect,
    look: &'static str,
    screen: &str,
    name: &str,
    out: &mut Vec<Finding>,
) -> Opened {
    let mut bad = |what: String| {
        out.push(Finding {
            look,
            screen: screen.to_string(),
            field: name.to_string(),
            what,
        })
    };
    // The list's sheet: the largest of what opened (a submenu or a tooltip beside it is
    // measured with it only for opacity).
    let sheet = layers
        .iter()
        .map(|(_, r)| *r)
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap();
    let window = r.ctx.content_rect().expand(0.5);
    if !window.contains_rect(sheet) {
        bad(format!("runs off the window: {sheet:?} in {window:?}"));
    }
    let inner = field.shrink(1.0);
    if sheet.intersects(inner) {
        bad(format!("covers its own field: {sheet:?} over {field:?}"));
    }
    let under = sheet.top() >= field.bottom() - 1.0;
    let over = sheet.bottom() <= field.top() + 1.0;
    if !(under || over) {
        bad(format!(
            "not under or above its field: {sheet:?} from {field:?}"
        ));
    }
    let gap = if under {
        sheet.top() - field.bottom()
    } else {
        field.top() - sheet.bottom()
    };
    if gap > 12.0 {
        bad(format!(
            "{gap:.0} pt away from its field: {sheet:?} from {field:?}"
        ));
    }
    if sheet.left() > field.right() - 4.0 || sheet.right() < field.left() + 4.0 {
        bad(format!("beside another place: {sheet:?} from {field:?}"));
    }
    // Opaque: the probe painted over every layer drawn beneath the list (and over the
    // panels), for a frame: nothing of it may show inside the list's sheet.
    let keep: HashSet<LayerId> = layers.iter().map(|(l, _)| *l).collect();
    let rank = |l: &LayerId, i: usize| (l.order as usize, i);
    let below: Vec<LayerId> = r.ctx.memory(|m| {
        let order: Vec<LayerId> = m.layer_ids().collect();
        let lowest = order
            .iter()
            .enumerate()
            .filter(|(_, l)| keep.contains(l))
            .map(|(i, l)| rank(l, i))
            .min()
            .unwrap_or((usize::MAX, 0));
        order
            .iter()
            .enumerate()
            .filter(|(i, l)| !keep.contains(l) && rank(l, *i) < lowest)
            .map(|(_, l)| *l)
            .collect()
    });
    r.probe = Some(PROBE);
    r.probe_layers = below;
    r.idle(1);
    let c = r.canvas();
    let texts = r.texts_above_probe();
    r.probe = None;
    r.probe_layers.clear();
    r.idle(1);
    let ppp = r.ctx.pixels_per_point();
    let probe = sheet.shrink(3.0);
    let (mut through, mut all) = (0, 0);
    let mut y = probe.top();
    while y < probe.bottom() {
        let mut x = probe.left();
        while x < probe.right() {
            let p = c.pixel((x * ppp) as usize, (y * ppp) as usize);
            all += 1;
            if p[0] > 200 && p[1] < 60 && p[2] > 200 {
                through += 1;
            }
            x += 4.0;
        }
        y += 4.0;
    }
    if through > 0 {
        bad(format!(
            "see-through: the probe shows through {through} of {all} points of {sheet:?}"
        ));
    }
    // Legible: its words, seen inside the sheet.
    let words: Vec<_> = texts
        .iter()
        .filter(|t| !t.text.trim().is_empty() && t.seen.is_positive() && sheet.intersects(t.seen))
        .collect();
    for t in &words {
        if t.seen.width() + 0.5 < t.rect.width() {
            bad(format!(
                "“{}” cut at its side: {:?} seen of {:?}",
                t.text, t.seen, t.rect
            ));
        }
        // Cut at its head or foot only at the sheet's edge, where a long list scrolls.
        let at_edge = t.seen.top() - sheet.top() < 14.0 || sheet.bottom() - t.seen.bottom() < 14.0;
        if t.seen.height() + 0.5 < t.rect.height() && !at_edge {
            bad(format!(
                "“{}” cut: {:?} seen of {:?}",
                t.text, t.seen, t.rect
            ));
        }
    }
    for (i, a) in words.iter().enumerate() {
        for b in &words[i + 1..] {
            let x = a.seen.intersect(b.seen);
            if x.width() > 2.0 && x.height() > 3.0 {
                bad(format!("“{}” and “{}” overlap at {x:?}", a.text, b.text));
            }
        }
    }
    Opened { sheet }
}

/// A screen that holds dropdowns: its name and how a user gets there.
struct Screen {
    name: &'static str,
    open: fn(&mut Rig),
    /// Where its fields lie (the screen's window or pane), when it is drawn.
    region: fn(&Rig) -> Option<Rect>,
}

fn area(r: &Rig, id: &str) -> Option<Rect> {
    r.ctx.memory(|m| m.area_rect(egui::Id::new(id)))
}

fn settings(page: cantus_ui::views::workspace_settings::SettingsPage) -> impl Fn(&mut Rig) {
    move |r: &mut Rig| {
        cantus_ui::views::workspace_settings::open(&mut r.app, page);
        r.settle();
    }
}

/// The owner's machine, as his screenshots show it: two drivers, four ASIO drivers among
/// the outputs, a microphone array among the inputs.
fn owner_scan() -> cantus_ui::views::settings_audio::Scan {
    use cantus_engine::{DeviceInfo, DriverInfo, InputDeviceInfo};
    let out = |n: &str, d: bool| DeviceInfo {
        name: n.into(),
        is_default: d,
    };
    cantus_ui::views::settings_audio::Scan {
        driver: None,
        drivers: vec![
            DriverInfo {
                name: "WASAPI".into(),
                is_default: true,
            },
            DriverInfo {
                name: "ASIO".into(),
                is_default: false,
            },
        ],
        outputs: vec![
            out("ASIO4ALL v2", true),
            out("FL Studio ASIO", false),
            out("FlexASIO", false),
        ],
        inputs: vec![InputDeviceInfo {
            name: "Microphone Array (AMD Audio Device)".into(),
            channels: 2,
            is_default: true,
        }],
    }
}

fn screens() -> Vec<Screen> {
    use cantus_ui::views::workspace_settings::SettingsPage as P;
    fn window(r: &Rig) -> Option<Rect> {
        area(r, "cantus-settings")
    }
    macro_rules! page {
        ($name:literal, $p:expr) => {
            Screen {
                name: $name,
                open: |r| {
                    if $p == P::Audio {
                        r.app.settings_page.audio.scan = Some(owner_scan());
                    }
                    settings($p)(r)
                },
                region: window,
            }
        };
    }
    vec![
        page!("settings-general", P::General),
        page!("settings-audio", P::Audio),
        page!("settings-midi", P::Midi),
        page!("settings-editing", P::Editing),
        page!("settings-appearance", P::Appearance),
        page!("settings-saving", P::Saving),
        page!("settings-cache", P::Cache),
        page!("settings-updates", P::Updates),
        page!("settings-plugins", P::Plugins),
        page!("settings-ai", P::Ai),
        page!("settings-models", P::Models),
        Screen {
            name: "audio-popover",
            open: |r| {
                r.app.show_settings = false;
                cantus_ui::audio_setup::demo::setup(&mut r.app);
                r.app.audio_setup.popover = true;
                r.settle();
            },
            region: |r| area(r, "cantus-audio-popover"),
        },
        Screen {
            name: "inspector",
            open: |r| {
                r.app.audio_setup.popover = false;
                let t = r.app.project().tracks[0].id;
                r.app.select_track(t);
                r.app
                    .workspace
                    .reveal(cantus_ui::views::workspace::Pane::Inspector);
                r.settle();
            },
            region: |r| r.leaf_of(cantus_ui::views::workspace::Pane::Inspector),
        },
        Screen {
            name: "desk",
            open: |r| {
                r.app
                    .workspace
                    .reveal(cantus_ui::views::workspace::Pane::Mixer);
                r.settle();
            },
            region: |r| r.leaf_of(cantus_ui::views::workspace::Pane::Mixer),
        },
        Screen {
            name: "top-bar",
            open: |r| r.settle(),
            region: |r| cantus_ui::views::transport::last_layout(&r.ctx).map(|(b, _)| b),
        },
        Screen {
            name: "export",
            open: |r| {
                cantus_ui::views::export::open(&mut r.app, None);
                r.settle();
            },
            region: |r| area(r, "cantus-export"),
        },
    ]
}

/// The effects whose editors hold dropdowns, on the first strip.
const EFFECTS: [cantus_core::EffectKind; 8] = [
    cantus_core::EffectKind::Delay,
    cantus_core::EffectKind::Gate,
    cantus_core::EffectKind::Limiter,
    cantus_core::EffectKind::Space,
    cantus_core::EffectKind::Multiband,
    cantus_core::EffectKind::Aula,
    cantus_core::EffectKind::DeEsser,
    cantus_core::EffectKind::Compressor,
];

/// The song every dropdown is reached in: the starter's tracks, the effects above on the
/// first strip, Chorda and Tympanum on tracks of their own.
fn song() -> cantus_core::Project {
    use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
    let mut p = cantus_ui::starter_project();
    let s = p.tracks[0].strip.unwrap();
    for k in EFFECTS {
        let fx = p.make_effect(k);
        p.strip_mut(s).unwrap().effects.push(fx);
    }
    for (name, kind) in [
        ("Piano", InstrumentKind::Chorda),
        ("Drums", InstrumentKind::AcousticDrums),
    ] {
        let mut t = p.make_track(name);
        let strip = p.make_strip(name);
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        p.tracks.push(t);
    }
    p
}

/// The editors that open over the workspace: their fields are those that appear with them.
fn editor_screens(r: &Rig) -> Vec<(String, Box<dyn Fn(&mut Rig)>)> {
    let mut v: Vec<(String, Box<dyn Fn(&mut Rig)>)> = Vec::new();
    let p = r.app.project();
    let s = p.tracks[0].strip.unwrap();
    for (k, fx) in EFFECTS.iter().zip(&p.strip(s).unwrap().effects) {
        let id = fx.id;
        v.push((
            format!("fx-{k:?}"),
            Box::new(move |r: &mut Rig| {
                r.app.open_effect_editor(s, id);
                r.settle();
            }),
        ));
    }
    for t in &p.tracks {
        if let cantus_core::InstrumentSpec::Native(n) = &t.instrument {
            if matches!(
                n.kind,
                cantus_core::InstrumentKind::Chorda | cantus_core::InstrumentKind::AcousticDrums
            ) {
                let id = t.id;
                for view in [
                    cantus_ui::views::inst::View::Play,
                    cantus_ui::views::inst::View::Mixer,
                ] {
                    v.push((
                        format!("inst-{:?}-{view:?}", n.kind),
                        Box::new(move |r: &mut Rig| {
                            r.app.open_inst_editor(id);
                            r.app.inst.view = view;
                            r.settle();
                        }),
                    ));
                }
            }
        }
    }
    v
}

/// The fields that open a list drawn in a frame, seen whole.
fn fields(r: &mut Rig) -> Vec<(Rect, String)> {
    cantus_ui::kit::widgets::record_fields(true);
    r.idle(1);
    let all = cantus_ui::kit::widgets::take_fields();
    cantus_ui::kit::widgets::record_fields(false);
    // Seen: its value's words painted whole in it (a field scrolled half out of its page,
    // or under a window, is not clicked).
    all.into_iter()
        .filter(|(f, v)| {
            v.trim().is_empty()
                || r.texts.iter().any(|t| {
                    !t.text.trim().is_empty()
                        && f.expand(1.0).contains_rect(t.rect)
                        && t.seen == t.rect
                })
        })
        .filter(|(f, _)| r.ctx.content_rect().contains_rect(*f))
        .collect()
}

/// Open every dropdown of `screen` (reached by `open`), measure each list, close it.
#[allow(clippy::too_many_arguments)]
fn audit(
    r: &mut Rig,
    look: &'static str,
    screen: &str,
    open: &dyn Fn(&mut Rig),
    region: Option<fn(&Rig) -> Option<Rect>>,
    before: &[(Rect, String)],
    out: &mut Vec<Finding>,
    log: &mut Vec<String>,
) {
    open(r);
    let mut done: Vec<String> = Vec::new();
    for pass in 0..12 {
        let room = match region {
            Some(f) => match f(r) {
                Some(x) => x,
                None => {
                    out.push(Finding {
                        look,
                        screen: screen.into(),
                        field: String::new(),
                        what: "the screen did not open".into(),
                    });
                    return;
                }
            },
            None => r.ctx.content_rect(),
        };
        let here: Vec<(Rect, String)> = fields(r)
            .into_iter()
            .filter(|(f, _)| room.contains_rect(*f))
            .filter(|x| region.is_some() || !before.iter().any(|b| b.0 == x.0))
            .collect();
        let mut fresh = 0;
        for (field, value) in here {
            let key = format!("{value}@{:.0},{:.0}", field.left(), field.top());
            if done.contains(&key) {
                continue;
            }
            fresh += 1;
            done.push(key);
            let was: HashSet<LayerId> = floating(r).into_iter().map(|(l, _)| l).collect();
            r.click(field.center());
            r.idle(30);
            let opened: Vec<(LayerId, Rect)> = floating(r)
                .into_iter()
                .filter(|(l, _)| !was.contains(l))
                .collect();
            let name = format!("{value:?} at {:.0},{:.0}", field.left(), field.top());
            if opened.is_empty() {
                log.push(format!("{look} {screen}: {name}: opened nothing"));
            } else {
                let o = measure(r, &opened, field, look, screen, &name, out);
                log.push(format!("{look} {screen}: {name}: list {:?}", o.sheet));
                let tag: String = value
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(16)
                    .collect();
                r.shot(&format!("{look}-{screen}-{tag}-{:.0}", field.top()));
            }
            // Closed again by its own field, as a person closes it.
            r.click(field.center());
            r.idle(20);
            let still = floating(r)
                .into_iter()
                .any(|(l, _)| opened.iter().any(|o| o.0 == l));
            if still {
                r.key(egui::Key::Escape, NONE);
                r.idle(20);
                out.push(Finding {
                    look,
                    screen: screen.into(),
                    field: name.clone(),
                    what: "a second click on its field did not close it".into(),
                });
                open(r);
            }
        }
        if region.is_none() || (pass > 0 && fresh == 0) {
            break;
        }
        // The rest of a page that scrolls.
        r.wheel(room.center(), -4.0, NONE);
    }
}

/// Close whatever a screen left open.
fn reset(r: &mut Rig) {
    r.app.show_settings = false;
    r.app.audio_setup.popover = false;
    r.app.audio_setup.pop_list = None;
    r.app.close_inst_editor();
    r.app.close_fx_editor();
    r.app.close_comp_editor();
    r.app.export.dialog = None;
    r.idle(4);
}

#[test]
fn every_dropdown_opens_opaque_beside_its_field_in_every_look() {
    let mut r = Rig::new(song());
    r.idle(4);
    let mut out = Vec::new();
    let mut log = Vec::new();
    for (look, light) in LOOKS {
        r.app.appearance.mode = if light { Mode::Light } else { Mode::Dark };
        if light {
            r.app.appearance.light.base = format!("preset:{look}");
        } else {
            r.app.appearance.dark.base = format!("preset:{look}");
        }
        r.app.apply_appearance();
        r.idle(4);
        for sc in screens() {
            reset(&mut r);
            let before = fields(&mut r);
            audit(
                &mut r,
                look,
                sc.name,
                &sc.open,
                Some(sc.region),
                &before,
                &mut out,
                &mut log,
            );
        }
        for (name, open) in editor_screens(&r) {
            reset(&mut r);
            let before = fields(&mut r);
            audit(
                &mut r, look, &name, &*open, None, &before, &mut out, &mut log,
            );
        }
    }
    reset(&mut r);
    let opened = log.iter().filter(|l| l.contains(": list ")).count();
    println!("{}", log.join("\n"));
    assert!(
        opened >= 20,
        "the audit opened only {opened} lists:\n{}",
        log.join("\n")
    );
    assert!(
        out.is_empty(),
        "{} problems in {opened} lists:\n{}",
        out.len(),
        out.iter()
            .map(|f| format!("{} {} {}: {}", f.look, f.screen, f.field, f.what))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
