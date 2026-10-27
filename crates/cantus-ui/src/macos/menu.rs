//! The global menu bar on macOS (round 30, ADR 0066): the application's menus mirrored into
//! AppKit's bar at the top of the screen, as every Mac application has them — the
//! application menu (About, Settings, Hide, Quit), File, Edit, View, Window and Help.
//!
//! The model ([`model`]) is plain data built from the keymap and the interface's language,
//! so it is tested on every system; `cantus_launch::macmenu` turns it into `NSMenu`s on a
//! Mac (this crate has no unsafe code). A chosen
//! item is a [`Pick`] that the next frame runs through the same paths as the in-window
//! menus and the shortcuts ([`run`]). The shortcut an item shows is the keymap's own (its
//! key equivalent, which AppKit then handles): a remapped key moves in the menu too.
//!
//! One subtlety: AppKit answers a key equivalent before the window sees the key, so with a
//! text field focused, ⌘C must still copy the field's text. The text-editing items (Undo,
//! Redo, Cut, Copy, Paste, Select all) are then handed to egui as the events the key would
//! have made ([`inject`]), and the field takes them as typed.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::keymap::{Action, Binding, Keymap};
use egui::Key;

/// What a menu item does, beside the keymap's actions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Action(Action),
    About,
    Quit,
    Shortcuts,
    Manual,
    Notes,
    Bug,
    Updates,
    ImportAudio,
    /// Settings, on one of its pages (the Options menu, round 30).
    Settings(crate::views::workspace_settings::SettingsPage),
    /// Look for new or changed plug-ins now (round 30).
    ScanPlugins,
}

/// An AppKit key equivalent: its characters and the modifiers held with it.
pub use cantus_launch::macmenu::Key as Equivalent;

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item {
        label: String,
        pick: Pick,
        key: Option<Equivalent>,
    },
    /// An item AppKit itself answers, by its action selector (`hide:`, `performZoom:`…).
    System {
        label: String,
        selector: &'static str,
        key: Option<Equivalent>,
    },
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub title: String,
    pub entries: Vec<Entry>,
    /// AppKit fills it with the open windows (the Window menu).
    pub windows: bool,
    /// AppKit adds its search field to it (the Help menu).
    pub help: bool,
}

/// The characters of a key as AppKit's key equivalents spell them (`NSF5FunctionKey` and
/// the arrows are private-use code points). `None` for keys a menu should not take.
fn chars(key: Key) -> Option<String> {
    // AppKit's `NSUpArrowFunctionKey`… (private-use code points, written as numbers).
    let arrow = |n: u32| char::from_u32(0xF700 + n);
    let c = match key {
        Key::ArrowUp => arrow(0)?,
        Key::ArrowDown => arrow(1)?,
        Key::ArrowLeft => arrow(2)?,
        Key::ArrowRight => arrow(3)?,
        Key::Comma => ',',
        Key::Period => '.',
        Key::Minus => '-',
        Key::Equals | Key::Plus => '=',
        Key::Slash => '/',
        Key::Backslash => '\\',
        Key::Semicolon => ';',
        Key::OpenBracket => '[',
        Key::CloseBracket => ']',
        Key::Backtick => '`',
        Key::Quote => '\'',
        _ => {
            let name = key.name();
            if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
                if (1..=20).contains(&n) {
                    return char::from_u32(0xF703 + n).map(String::from);
                }
                return None;
            }
            let mut it = name.chars();
            match (it.next(), it.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
                _ => return None,
            }
        }
    };
    Some(c.to_string())
}

/// The key equivalent for a binding: one with Command (or Option), or a bare function key. A bare
/// letter, digit or arrow belongs to the views (the typing piano, the faders) and is
/// never taken by the menu bar.
pub fn equivalent(b: &Binding) -> Option<Equivalent> {
    let function_key = b.key.name().starts_with('F') && b.key.name().len() > 1;
    if !(b.command || b.alt || function_key) {
        return None;
    }
    Some(Equivalent {
        chars: chars(b.key)?,
        command: b.command,
        shift: b.shift,
        alt: b.alt,
        control: false,
    })
}

/// The equivalent of the first binding of `action` a menu can show.
fn key_of(keymap: &Keymap, action: Action) -> Option<Equivalent> {
    keymap.bindings_for(action).iter().find_map(equivalent)
}

fn cmd(c: &str, shift: bool, alt: bool) -> Option<Equivalent> {
    Some(Equivalent {
        chars: c.to_string(),
        command: true,
        shift,
        alt,
        control: false,
    })
}

/// FL's *Options* menu (round 30, the owner's friction "where is Settings?"): Settings on
/// its shortcut, each of its pages, and *Scan plug-ins*. The window's own menu bar shows the
/// same entries (`views::transport_menus`), and the Mac's bar has it beside View.
pub fn options(_keymap: &Keymap) -> Menu {
    use crate::views::workspace_settings::SettingsPage;
    let page = |p: SettingsPage| Entry::Item {
        label: t(&format!("{}_item", p.key())),
        pick: Pick::Settings(p),
        key: None,
    };
    Menu {
        title: t("ship.menu.options"),
        entries: vec![
            // Its ⌘, belongs to the application menu's Settings (one chord, one item); the
            // window's own Options menu shows the keymap's binding beside it.
            Entry::Item {
                label: t("menu.settings"),
                pick: Pick::Action(Action::OpenSettings),
                key: None,
            },
            Entry::Separator,
            page(SettingsPage::Audio),
            page(SettingsPage::Midi),
            page(SettingsPage::Editing),
            page(SettingsPage::Appearance),
            page(SettingsPage::Saving),
            page(SettingsPage::Plugins),
            Entry::Separator,
            Entry::Item {
                label: t("host.manager.scan"),
                pick: Pick::ScanPlugins,
                key: None,
            },
        ],
        windows: false,
        help: false,
    }
}

/// The menu bar for this keymap, in the interface's language.
pub fn model(keymap: &Keymap) -> Vec<Menu> {
    let act = |label: &str, a: Action| Entry::Item {
        label: t(label),
        pick: Pick::Action(a),
        key: key_of(keymap, a),
    };
    let named = |a: Action| act(&a.label_key(), a);
    let pick = |label: &str, p: Pick| Entry::Item {
        label: t(label),
        pick: p,
        key: None,
    };
    let sys = |label: &str, selector: &'static str, key: Option<Equivalent>| Entry::System {
        label: t(label),
        selector,
        key,
    };
    let menu = |title: String, entries: Vec<Entry>| Menu {
        title,
        entries,
        windows: false,
        help: false,
    };
    use Action::*;
    use Entry::Separator;
    let app = menu(
        "Cantus".into(),
        vec![
            pick("menu.about", Pick::About),
            Separator,
            act("menu.settings", OpenSettings),
            Separator,
            sys("macos.hide", "hide:", cmd("h", false, false)),
            sys(
                "macos.hide_others",
                "hideOtherApplications:",
                cmd("h", false, true),
            ),
            sys("macos.show_all", "unhideAllApplications:", None),
            Separator,
            Entry::Item {
                label: t("macos.quit"),
                pick: Pick::Quit,
                key: cmd("q", false, false),
            },
        ],
    );
    let file = menu(
        t("menu.file"),
        vec![
            act("menu.new", NewProject),
            named(NewFromTemplate),
            act("demos.menu", DemoSongs),
            act("menu.open", Open),
            Separator,
            act("menu.save", Save),
            act("menu.save_as", SaveAs),
            named(SaveAsTemplate),
            named(SaveTrackTemplate),
            Separator,
            pick("audioclips.menu.import", Pick::ImportAudio),
            act("menu.export_wav", ExportWav),
        ],
    );
    let edit = menu(
        t("menu.edit"),
        vec![
            act("menu.undo", Undo),
            act("menu.redo", Redo),
            Separator,
            act("menu.cut", Cut),
            act("menu.copy", Copy),
            act("menu.paste", Paste),
            act("menu.duplicate", Duplicate),
            Separator,
            act("menu.select_all", SelectAll),
            act("menu.delete", Delete),
            act("menu.quantize", Quantize),
        ],
    );
    let view = menu(
        t("menu.view"),
        vec![
            named(ViewArrangement),
            named(ViewChannelRack),
            named(ViewPianoRoll),
            named(ViewMixer),
            named(ViewRouting),
            named(ToggleBrowser),
            named(ToggleInspector),
            named(ToggleClips),
            named(ViewControllers),
            named(ViewHistory),
            named(CommandPalette),
            Separator,
            named(ZoomIn),
            named(ZoomOut),
            named(ZoomReset),
            Separator,
            sys(
                "macos.full_screen",
                "toggleFullScreen:",
                Some(Equivalent {
                    control: true,
                    ..cmd("f", false, false).expect("an equivalent")
                }),
            ),
        ],
    );
    let window = Menu {
        windows: true,
        ..menu(
            t("macos.window"),
            vec![
                sys(
                    "macos.minimize",
                    "performMiniaturize:",
                    cmd("m", false, false),
                ),
                sys("macos.zoom", "performZoom:", None),
                Separator,
                sys("macos.front", "arrangeInFront:", None),
            ],
        )
    };
    let help = Menu {
        help: true,
        ..menu(
            t("menu.help"),
            vec![
                pick("about.menu.manual", Pick::Manual),
                pick("menu.shortcuts", Pick::Shortcuts),
                pick("about.menu.notes", Pick::Notes),
                Separator,
                pick("about.menu.bug", Pick::Bug),
                pick("update.menu.check", Pick::Updates),
            ],
        )
    };
    vec![app, file, edit, view, options(keymap), window, help]
}

/// The items a focused text field takes as keys rather than as Cantus's actions.
const TEXT_EDITS: [Action; 6] = [
    Action::Undo,
    Action::Redo,
    Action::Cut,
    Action::Copy,
    Action::Paste,
    Action::SelectAll,
];

thread_local! {
    /// Text edits waiting to be handed to egui at the start of the next pass.
    static INJECT: std::cell::RefCell<Vec<Action>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run one chosen item — the same paths as the window's own menus.
pub fn run(app: &mut CantusApp, ctx: &egui::Context, pick: Pick) {
    match pick {
        Pick::Action(a) if TEXT_EDITS.contains(&a) && ctx.egui_wants_keyboard_input() => {
            INJECT.with(|q| q.borrow_mut().push(a));
            ctx.request_repaint();
        }
        Pick::Action(a) => app.run_action(a),
        Pick::About => app.show_about = true,
        Pick::Quit => app.request_quit = true,
        Pick::Shortcuts => app.show_shortcuts = true,
        Pick::Manual => crate::docs::open(crate::docs::Doc::Manual),
        Pick::Notes => crate::docs::open(crate::docs::Doc::ReleaseNotes),
        Pick::Bug => crate::docs::open(crate::docs::Doc::ReportBug),
        Pick::Updates => crate::views::update::check_now(app),
        Pick::ImportAudio => app.import_audio_dialog(),
        Pick::Settings(page) => crate::views::workspace_settings::open(app, page),
        Pick::ScanPlugins => crate::views::plugins::scan_now(app),
    }
}

/// The events a text edit's key would have made: egui's own clipboard events for cut,
/// copy and paste (`paste`: the clipboard's text), key presses for the rest.
pub fn events_for(action: Action, paste: Option<String>) -> Vec<egui::Event> {
    let key = |key: Key, shift: bool| {
        let modifiers = egui::Modifiers {
            shift,
            ..egui::Modifiers::COMMAND
        };
        [true, false].map(|pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        })
    };
    match action {
        Action::Cut => vec![egui::Event::Cut],
        Action::Copy => vec![egui::Event::Copy],
        Action::Paste => paste.map(egui::Event::Paste).into_iter().collect(),
        Action::Undo => key(Key::Z, false).to_vec(),
        Action::Redo => key(Key::Z, true).to_vec(),
        Action::SelectAll => key(Key::A, false).to_vec(),
        _ => Vec::new(),
    }
}

/// Before a pass (the shell's `raw_input_hook`): the text edits a menu chose while a field
/// had the focus, as the events their keys make.
pub fn inject(raw: &mut egui::RawInput) {
    let queued: Vec<Action> = INJECT.with(|q| std::mem::take(&mut *q.borrow_mut()));
    for action in queued {
        let paste = if action == Action::Paste {
            pasteboard_text()
        } else {
            None
        };
        raw.events.extend(events_for(action, paste));
    }
}

fn pasteboard_text() -> Option<String> {
    cantus_launch::macmenu::pasteboard_text()
}

/// The model as `cantus_launch::macmenu` takes it: each Cantus item tagged with its index
/// in the returned table of picks.
pub fn spec(menus: &[Menu]) -> (Vec<cantus_launch::macmenu::Menu>, Vec<Pick>) {
    use cantus_launch::macmenu as mm;
    let mut table = Vec::new();
    let out = menus
        .iter()
        .map(|m| mm::Menu {
            title: m.title.clone(),
            windows: m.windows,
            help: m.help,
            entries: m
                .entries
                .iter()
                .map(|e| match e {
                    Entry::Item { label, pick, key } => {
                        table.push(*pick);
                        mm::Entry::Item {
                            label: label.clone(),
                            tag: table.len() - 1,
                            key: key.clone(),
                        }
                    }
                    Entry::System {
                        label,
                        selector,
                        key,
                    } => mm::Entry::System {
                        label: label.clone(),
                        selector,
                        key: key.clone(),
                    },
                    Entry::Separator => mm::Entry::Separator,
                })
                .collect(),
        })
        .collect();
    (out, table)
}

thread_local! {
    /// What the installed bar was made from, and what its tags stand for.
    static BUILT: std::cell::RefCell<Option<(crate::i18n::Locale, Keymap, Vec<Pick>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Each pass (`CantusApp::frame`): on a Mac, the bar made or remade when the keymap or the
/// language changed, and the items chosen since the last pass run. Elsewhere nothing.
pub fn serve(app: &mut CantusApp, ctx: &egui::Context) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let locale = crate::i18n::locale();
    let stale = BUILT.with(|b| {
        !b.borrow()
            .as_ref()
            .is_some_and(|(l, k, _)| *l == locale && *k == app.keymap)
    });
    if stale {
        let (menus, table) = spec(&model(&app.keymap));
        let wake = ctx.clone();
        cantus_launch::macmenu::install(&menus, Box::new(move || wake.request_repaint()));
        BUILT.with(|b| *b.borrow_mut() = Some((locale, app.keymap.clone(), table)));
    }
    for tag in cantus_launch::macmenu::take() {
        let pick = BUILT.with(|b| {
            b.borrow()
                .as_ref()
                .and_then(|(_, _, table)| table.get(tag).copied())
        });
        if let Some(pick) = pick {
            run(app, ctx, pick);
        }
    }
}
