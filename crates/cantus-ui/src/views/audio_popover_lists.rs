//! The popover's lists (round 20): the interfaces connected, and the inputs to record
//! from — the interface's own first. Kit menus, opened under their field.

use crate::app::CantusApp;
use crate::audio_setup::{self as setup, interfaces::Kind, Hit, PopList};
use crate::i18n::t;
use crate::kit::menu::{self, Check, Item};
use crate::kit::text::Src;
use crate::kit::widgets::menu_list;
use crate::kit::Style;
use egui::Id;

fn kind_key(k: Kind) -> &'static str {
    match k {
        Kind::Interface => "devices.kind.interface",
        Kind::BuiltIn => "devices.kind.builtin",
        Kind::Headphones => "devices.kind.headphones",
        Kind::Display => "devices.kind.display",
        Kind::Virtual => "devices.kind.virtual",
    }
}

/// What a row of the open list does.
enum Choice {
    Interface(String),
    Input(Option<String>),
    Asio(String),
}

/// Draw the open list, if any.
pub fn show(app: &mut CantusApp, ctx: &egui::Context, key: Option<&str>) {
    let Some(which) = app.audio_setup.pop_list else {
        return;
    };
    let s = Style::current();
    let mut labels: Vec<(String, Option<String>, bool, Option<Choice>)> = Vec::new();
    let scan = app.audio_setup.scan.clone();
    match which {
        PopList::Interface => match &scan {
            Some(sc) if !sc.interfaces.is_empty() => {
                for i in &sc.interfaces {
                    let mut side = t(kind_key(i.kind));
                    if i.has_asio() && cantus_engine::devices::ASIO_BUILT {
                        side.push_str(" · ASIO");
                    }
                    labels.push((
                        i.key.clone(),
                        Some(side),
                        Some(i.key.as_str()) == key,
                        Some(Choice::Interface(i.key.clone())),
                    ));
                }
            }
            _ => labels.push((t("devices.scanning"), None, false, None)),
        },
        PopList::Asio => {
            let now = app.settings.audio.device.clone();
            match &scan {
                Some(sc) if !sc.asio.is_empty() => {
                    for d in &sc.asio {
                        let on = now.as_deref() == Some(d.as_str());
                        labels.push((d.clone(), None, on, Some(Choice::Asio(d.clone()))));
                    }
                }
                Some(_) => labels.push((t("devices.drivers.none_installed"), None, false, None)),
                None => labels.push((t("devices.scanning"), None, false, None)),
            }
        }
        PopList::Input => {
            let now = app.settings.audio.input_device.clone();
            labels.push((
                t("devices.input.none"),
                None,
                now.as_deref() == Some(""),
                Some(Choice::Input(Some(String::new()))),
            ));
            let own: Vec<String> = key
                .and_then(|k| scan.as_ref()?.interface(k).map(|i| i.inputs.clone()))
                .unwrap_or_default();
            let all: Vec<(String, u16)> = scan
                .as_ref()
                .map(|s| {
                    s.inputs
                        .iter()
                        .map(|i| (i.name.clone(), i.channels))
                        .collect()
                })
                .unwrap_or_default();
            let mut ordered: Vec<(String, u16)> = all
                .iter()
                .filter(|(n, _)| own.contains(n))
                .cloned()
                .collect();
            ordered.extend(all.iter().filter(|(n, _)| !own.contains(n)).cloned());
            for (n, ch) in ordered {
                let side = (ch > 0).then(|| crate::i18n::tf("devices.channels", &[("n", &ch)]));
                let on = now.as_deref() == Some(n.as_str());
                labels.push((n.clone(), side, on, Some(Choice::Input(Some(n)))));
            }
        }
    }
    let items: Vec<Item<'_>> = labels
        .iter()
        .map(|(l, side, on, c)| Item::Action {
            label: Src::Str(l),
            check: Check::Radio(*on),
            shortcut: side.as_deref(),
            submenu: false,
            enabled: c.is_some(),
        })
        .collect();
    let at = app.audio_setup.list_at;
    let width = 300.0 * s.k;
    let mut chosen = None;
    let mut list_rect = egui::Rect::NOTHING;
    let area = egui::Area::new(Id::new("cantus-audio-popover-list"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (resp, c) = menu_list(ui, &items, width);
            list_rect = resp.rect;
            chosen = c.clicked;
        });
    app.audio_setup.hit(ctx, Hit::List, area.response.rect);
    for i in 0..items.len() {
        let r = menu::row(list_rect, &items, s.d(1.0), i);
        let h = match which {
            PopList::Interface => Hit::InterfaceRow(i),
            PopList::Input => Hit::InputRow(i),
            PopList::Asio => Hit::AsioRow(i),
        };
        app.audio_setup.hit(ctx, h, r);
    }
    let Some(i) = chosen else {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.audio_setup.pop_list = None;
        }
        return;
    };
    app.audio_setup.pop_list = None;
    match labels.into_iter().nth(i).and_then(|l| l.3) {
        Some(Choice::Interface(k)) => setup::choose_interface(app, &k),
        Some(Choice::Asio(name)) => {
            let mut a = app.settings.audio.clone();
            a.driver = Some("ASIO".into());
            a.device = Some(name);
            a.input_device = None;
            setup::apply(app, a);
        }
        Some(Choice::Input(name)) => {
            let mut a = app.settings.audio.clone();
            a.input_device = name;
            setup::apply(app, a);
        }
        None => {}
    }
}
