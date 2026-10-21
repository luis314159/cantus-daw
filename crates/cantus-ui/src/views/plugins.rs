//! Hosted plug-ins in the interface (ADR 0019).
//!
//! - **The insert menu's plug-in list**: every effect the scans found, by vendor or by
//!   category, with a search field; a click puts it in the strip's chain.
//! - **The slot**: the plug-in's own name and a small format badge; *missing* in the danger
//!   colour, with the reason, when the plug-in is not on this computer.
//! - **The generic editor**, inside the shared FX frame (`views::fx`): the plug-in's
//!   parameters grouped as the plug-in groups them, the house knob for each, the plug-in's
//!   own text for its values; host-kept A/B and presets as saved states; the sidechain
//!   choice when the plug-in has a second input; and *Open plug-in window* for its own
//!   editor.
//! - **The plug-in manager** in Settings: folders, rescans, and the blocked list.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band;
use crate::kit::shell::{self, Grid};
use crate::kit::text::Src;
use crate::kit::Style;
use crate::views::fx::controls::{self, Size};
use cantus_core::{
    Command, EffectId, EffectKind, EffectSlot, EffectSpec, MergeKey, PluginState, StripId,
};
use cantus_host::{ParamInfo, PluginInfo};
use egui::Sense;
use std::collections::{BTreeMap, HashMap};

/// The interface's own state for hosted plug-ins.
#[derive(Debug, Default)]
pub struct State {
    /// What is typed in the insert menu's search field.
    pub search: String,
    /// The insert menu groups by category rather than by vendor.
    pub by_category: bool,
    /// What is typed in the editor's parameter filter.
    pub filter: String,
    /// The other side of A/B for each hosted effect that has one — its state and values —
    /// and whether B is the side showing.
    pub(crate) ab: HashMap<EffectId, (PluginState, BTreeMap<u32, f64>, bool)>,
    /// A folder being typed in the manager.
    pub folder: String,
    /// The generic editor's own state.
    pub(crate) editor: Editor,
}

/// A small outlined tag: the plug-in's format, in a readout well.
pub fn badge(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let s = Style::current();
    let w = crate::kit::text::measure(
        ui.ctx(),
        &crate::kit::text::Text::new(text, crate::kit::text::Role::Body, s.t.text_dim),
        s.tk(),
    )
    .x + 16.0;
    controls::readout(ui, &s, text, false, "", w)
}

/// The words of a plug-in's features that say what it is ("equalizer", "reverb"), without
/// the ones that only say what it plugs into.
fn category(p: &PluginInfo) -> String {
    const PLUMBING: [&str; 9] = [
        "audio-effect",
        "instrument",
        "note-effect",
        "note-detector",
        "stereo",
        "mono",
        "surround",
        "ambisonic",
        "fx",
    ];
    p.features
        .iter()
        .find(|f| !PLUMBING.contains(&f.to_lowercase().as_str()))
        .map(|f| {
            let mut c = f.replace(['-', '_'], " ");
            if let Some(first) = c.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            c
        })
        .unwrap_or_else(|| t("host.menu.other"))
}

/// The plug-ins part of a strip's insert menu: a search field, vendor | category, and the
/// plug-ins as the kit's menu rows — each group under a band, each plug-in's format at
/// the row's right.
pub fn insert_menu(app: &mut CantusApp, ui: &mut egui::Ui, strip: StripId) {
    let s = Style::current();
    let width = 300.0;
    band_title(ui, &s, "host.menu.title", width);
    let effects: Vec<PluginInfo> = app.plugins.db.effects().into_iter().cloned().collect();
    if effects.is_empty() {
        controls::line(ui, &s, &t("host.menu.none"), s.t.text_dim, width);
        let scanning = app.plugins.scanning();
        ui.add_enabled_ui(!scanning, |ui| {
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.scan"),
                &t("host.manager.scan_tip"),
                120.0,
                scanning,
            )
            .clicked()
            {
                app.plugins.start_scan(false);
            }
        });
        return;
    }
    let state = &mut app.fx.hosted;
    controls::text_field(
        ui,
        &s,
        ui.id().with("host-search"),
        &mut state.search,
        &t("host.menu.search"),
        width,
    );
    let labels = [
        Src::Key("host.menu.by_vendor"),
        Src::Key("host.menu.by_category"),
    ];
    if let Some(i) = controls::segmented(
        ui,
        &s,
        state.by_category as usize,
        &labels,
        &t("host.menu.title"),
        width,
    ) {
        state.by_category = i == 1;
    }
    let query = state.search.trim().to_lowercase();
    let by_category = state.by_category;
    // The rows: the matches, or every plug-in under its group's band.
    let mut rows: Vec<(Option<usize>, String, &'static str)> = Vec::new();
    if !query.is_empty() {
        for (i, p) in effects.iter().enumerate() {
            if p.name.to_lowercase().contains(&query)
                || p.vendor.to_lowercase().contains(&query)
                || p.features.iter().any(|f| f.to_lowercase().contains(&query))
            {
                rows.push((
                    Some(i),
                    p.name.clone(),
                    cantus_host::to_project_format(p.format).badge(),
                ));
            }
        }
        if rows.is_empty() {
            controls::line(ui, &s, &t("host.menu.no_match"), s.t.text_dim, width);
            return;
        }
    } else {
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, p) in effects.iter().enumerate() {
            let key = if by_category {
                category(p)
            } else if p.vendor.is_empty() {
                t("host.menu.no_vendor")
            } else {
                p.vendor.clone()
            };
            groups.entry(key).or_default().push(i);
        }
        for (group, list) in groups {
            rows.push((None, format!("{group} ({})", list.len()), ""));
            for i in list {
                let p = &effects[i];
                rows.push((
                    Some(i),
                    p.name.clone(),
                    cantus_host::to_project_format(p.format).badge(),
                ));
            }
        }
    }
    let items: Vec<crate::kit::menu::Item<'_>> = rows
        .iter()
        .map(|(i, name, badge)| match i {
            None => crate::kit::menu::Item::Band(Src::Str(name)),
            Some(_) => crate::kit::menu::Item::Action {
                label: Src::Str(name),
                check: crate::kit::menu::Check::None,
                shortcut: Some(badge),
                submenu: false,
                enabled: true,
            },
        })
        .collect();
    let mut chosen: Option<PluginInfo> = None;
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let (_, clicks) = crate::kit::widgets::menu_list(ui, &items, width);
            if let Some(i) = clicks.clicked {
                chosen = rows[i].0.map(|n| effects[n].clone());
            }
        });
    if let Some(p) = chosen {
        ui.close();
        app.add_plugin(strip, &p);
    }
}

/// A section band with a title, `width` wide.
/// The manager's *Where plug-ins run* (round 29, ADR 0063): this computer's default for
/// every third-party plug-in its project does not place, what each choice means, and the
/// plug-ins that stopped working here before, which the database remembers.
fn isolation_section(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style, width: f32) {
    use cantus_core::PluginIsolation as P;
    band_title(ui, s, "isolation.manager.band", width);
    let now = P::ALL
        .iter()
        .position(|m| *m == app.settings.plugin_isolation)
        .unwrap_or(1);
    let labels = P::ALL.map(|m| Src::Key(m.label_key()));
    if let Some(i) = controls::segmented(
        ui,
        s,
        now,
        &labels,
        &t("isolation.manager.band"),
        width.min(560.0),
    ) {
        crate::views::plugin_notice::set_default(app, P::ALL[i]);
    }
    let what = t(match app.settings.plugin_isolation {
        P::Within => "isolation.manager.within",
        P::Together => "isolation.manager.together",
        P::ByVendor => "isolation.manager.by_vendor",
        P::Individually => "isolation.manager.individually",
    });
    controls::line(ui, s, &what, s.t.text_dim, width);
    controls::line(ui, s, &t("isolation.manager.cost"), s.t.text_dim, width);
    controls::line(ui, s, &t("isolation.manager.own"), s.t.text_dim, width);
    if !app.plugins.isolation_available() {
        controls::line(ui, s, &t("isolation.unavailable"), s.t.danger, width);
    }
    let crashes: Vec<cantus_host::CrashRecord> = app.plugins.db.crashes.clone();
    if crashes.is_empty() {
        return;
    }
    ui.add_space(6.0);
    band_title(ui, s, "isolation.manager.crashed", width);
    let mut forget = None;
    for c in &crashes {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            controls::line(ui, s, &c.name, s.t.danger, (width * 0.4).max(140.0));
            badge(ui, cantus_host::to_project_format(c.format).badge());
            let times = crate::i18n::tf_count("isolation.manager.times", c.count as usize, &[]);
            controls::line(ui, s, &times, s.t.text_dim, (width * 0.3).max(110.0));
            if controls::key(
                ui,
                s,
                Src::Key("isolation.manager.forget"),
                &t("isolation.manager.forget_tip"),
                100.0,
                false,
            )
            .clicked()
            {
                forget = Some((c.format, c.id.clone()));
            }
        });
    }
    if let Some((format, id)) = forget {
        app.plugins.db.forget_crashes(format, &id);
        app.plugins.save_db_now();
    }
}

fn band_title(ui: &mut egui::Ui, s: &Style, key: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, band::BAND), Sense::hover());
    crate::kit::with_brush(ui.painter(), |b| {
        band::section(
            b,
            rect,
            s,
            crate::kit::State::REST,
            Src::Key(key),
            None,
            false,
        )
    });
}

impl CantusApp {
    /// Put a hosted plug-in at the end of a strip's chain: one command, one undo step.
    pub fn add_plugin(&mut self, strip: StripId, info: &PluginInfo) {
        match self.plugins.make_slot(&mut self.bus, strip, info) {
            Ok(slot) => {
                let index = self.project().effects_of(strip).map_or(0, |s| s.len());
                // An isolated plug-in's values, known once it loads, join this step.
                let key = cantus_host::PluginHost::load_key(slot.id);
                self.exec_merging(
                    Command::InsertEffect {
                        strip,
                        index,
                        slot: Box::new(slot),
                    },
                    key,
                );
            }
            Err(e) => self.set_error(tf(
                "host.load_failed",
                &[("name", &info.name), ("error", &e)],
            )),
        }
    }

    /// Set one parameter of a hosted plug-in; a gesture merges frame to frame into one
    /// undo step, exactly as the plug-in's own window does.
    pub fn set_plugin_param(
        &mut self,
        strip: StripId,
        effect: EffectId,
        id: u32,
        value: f64,
        gesture: bool,
    ) {
        let cmd = Command::SetPluginParam {
            strip,
            effect,
            id,
            value: Some(value),
        };
        if gesture {
            self.exec_merging(cmd, MergeKey::of(&("plugin", effect.0, id)));
        } else {
            self.exec(cmd);
        }
    }

    /// Replace a hosted plug-in's state and values, as one command.
    pub fn set_plugin_state(
        &mut self,
        strip: StripId,
        effect: EffectId,
        state: PluginState,
        params: BTreeMap<u32, f64>,
    ) {
        self.exec(Command::SetPluginState {
            strip,
            effect,
            state,
            params,
        });
    }
}

/// A slot's title in the inspector: the plug-in's name, its format, and *missing* when it
/// is not on this computer.
pub fn slot_title(app: &CantusApp, ui: &mut egui::Ui, slot: &EffectSlot) {
    let s = Style::current();
    let Some(p) = slot.spec.external.as_deref() else {
        let name = t(slot.spec.kind.name_key());
        controls::line(ui, &s, &name, s.t.text, 160.0);
        return;
    };
    let w = crate::kit::text::measure(
        ui.ctx(),
        &crate::kit::text::Text::new(&p.name, crate::kit::text::Role::Body, s.t.text),
        s.tk(),
    )
    .x
    .min(200.0)
        + 4.0;
    controls::line(ui, &s, &p.name, s.t.text, w);
    badge(ui, p.format.badge());
    if let Some(why) = app.plugins.missing(slot.id) {
        // On this computer but not loaded: said as such, never as one to install (review
        // isolation-5 #45).
        let not_loaded = app.plugins.load_error(slot.id).is_some();
        let (word, tip_key) = if not_loaded {
            (t("isolation.not_loaded"), "isolation.not_loaded_tip")
        } else {
            (t("host.missing"), "host.missing_tip")
        };
        let r = ui.scope(|ui| controls::line(ui, &s, &word, s.t.danger, 80.0));
        if r.response.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &word,
                &tf(
                    tip_key,
                    &[
                        ("vendor", &p.vendor),
                        ("why", &crate::views::plugin_notice::load_words(why)),
                    ],
                ),
                crate::kit::hint::Caps::NONE,
            );
        }
    }
}

/// The generic editor's *Runs* card (round 29, ADR 0063): where the plug-in runs — as this
/// computer says, or one of the four — and what it costs, as its slot's tip says it.
fn isolation_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    s: &Style,
    rect: egui::Rect,
    effect: EffectId,
) {
    use crate::views::plugin_notice::{self as notice, Target};
    use cantus_core::PluginIsolation as P;
    let k = s.k;
    let target = Target::Insert(effect);
    let Some(chosen) = notice::chosen(app, target) else {
        return;
    };
    controls::card(
        ui,
        s,
        rect,
        Src::Key("isolation.slot.band"),
        None,
        |ui, cr| {
            let w = cr.body.width() / k;
            let choices: [Option<P>; 5] = [
                None,
                Some(P::Within),
                Some(P::Together),
                Some(P::ByVendor),
                Some(P::Individually),
            ];
            let labels = [
                Src::Key("isolation.short.default"),
                Src::Key("isolation.short.within"),
                Src::Key("isolation.short.together"),
                Src::Key("isolation.short.by_vendor"),
                Src::Key("isolation.short.individually"),
            ];
            let now = choices.iter().position(|c| *c == chosen).unwrap_or(0);
            let seg_w = (w * 0.62).clamp(260.0, 520.0);
            if let Some(i) =
                controls::segmented(ui, s, now, &labels, frame_tr("isolation.slot.band"), seg_w)
            {
                notice::set_isolation(app, target, choices[i]);
            }
            // A stopped plug-in costs nothing: its last reading is not shown.
            let cpu = app
                .plugins
                .cpu(effect)
                .filter(|_| app.plugins.crashed(effect).is_none())
                .map(|c| tf("isolation.tip.cpu", &[("share", &notice::percent(c.share))]))
                .unwrap_or_default();
            controls::line(ui, s, &cpu, s.t.text_dim, (w - seg_w - 8.0).max(60.0));
        },
    );
}

/// The inspector's picture of a hosted plug-in: how many values it holds, and its latency.
pub fn preview(app: &CantusApp, ui: &mut egui::Ui, slot: &EffectSlot) -> egui::Response {
    let s = Style::current();
    let n = slot.spec.external.as_deref().map_or(0, |p| p.params.len());
    let text = match app.plugins.hosted(slot.id) {
        Some(h) => tf("host.preview", &[("n", &n), ("latency", &h.latency())]),
        None => t("host.preview_missing"),
    };
    let w = ui.available_width();
    let r = controls::readout(ui, &s, &text, false, "", w);
    ui.interact(r.rect, r.id.with("open"), Sense::click())
}

/// Every hosted slot's spec, for code that has only a spec (the frame's preview).
pub fn is_hosted(spec: &EffectSpec) -> bool {
    spec.external.is_some()
}

// ---- the generic editor ---------------------------------------------------------------

/// The value a parameter shows: the project's, or the plug-in's own when the project holds
/// none for it.
fn shown_value(
    spec: &EffectSpec,
    hosted: Option<&cantus_host::session::Hosted>,
    p: &ParamInfo,
) -> f64 {
    spec.external
        .as_deref()
        .and_then(|e| e.params.get(&p.id).copied())
        .or_else(|| hosted.and_then(|h| h.param_value(p.id)))
        .unwrap_or(p.default)
}

/// The parameters' groups: a title and the indices of the parameters in it.
type Groups = Vec<(String, Vec<usize>)>;

/// A hosted editor's words for each value shown, by parameter, for one effect: asked when
/// the value moved, and again while an isolated plug-in has not answered (it answers
/// later) — the number shows meanwhile and is never kept as its words; an in-process
/// plug-in's "none" is its answer, never asked again every frame (review 4, #63); another
/// effect's editor starts them again (round 29, review 3 of ADR 0062, #59).
#[derive(Debug, Default)]
pub struct ValueWords {
    of: Option<EffectId>,
    words: HashMap<u32, (u64, Option<String>)>,
}

impl ValueWords {
    /// The words of `effect`'s parameter `id` at the value with `bits`, asked of `ask`
    /// when not known.
    pub fn get(
        &mut self,
        effect: EffectId,
        id: u32,
        bits: u64,
        later: bool,
        ask: impl FnOnce() -> Option<String>,
    ) -> Option<&str> {
        if self.of != Some(effect) {
            self.of = Some(effect);
            self.words.clear();
        }
        let e = self.words.entry(id).or_insert((u64::MAX, None));
        if e.0 != bits || (later && e.1.is_none()) {
            e.0 = bits;
            e.1 = ask();
        }
        e.1.as_deref()
    }
}

/// What the editor keeps between frames: the groups as laid out for the filter, and each
/// parameter's text in the plug-in's own words for the value it was asked about.
#[derive(Debug, Default)]
pub struct Editor {
    /// (effect, parameter count, filter) the groups were made for, and the groups: a title
    /// and the indices of the parameters in it.
    groups: Option<(EffectId, usize, String, Groups)>,
    /// The plug-in's words for the values shown.
    texts: ValueWords,
    /// "CLAP · Vendor", per effect.
    badge: Option<(EffectId, String)>,
    /// Where each group's card goes, reused.
    layout: Vec<egui::Rect>,
    /// The key field's words, and what they were made for.
    #[allow(clippy::type_complexity)]
    key_label: Option<((Option<StripId>, u64, crate::i18n::Locale), String)>,
    /// The height the editor wants at 100 %, from what it laid out last frame.
    pub(crate) wanted: Option<f32>,
}

/// The generic editor of a hosted plug-in, in the device frame (ADR 0024 §8): the frame's
/// header carries the plug-in's name, its saved states as presets, A/B of whole states,
/// the undo of this device and the bypass; the body its parameters, grouped as the plug-in
/// groups them, each a cell with the plug-in's own text for its value.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> crate::views::fx::frame::Out {
    use crate::views::fx::{frame, host::DawHost};
    let mut chrome = std::mem::take(&mut app.fx.chrome);
    chrome.reset_for(effect.0);
    let mut chips = std::mem::take(&mut app.fx.chips);
    let mut ed = std::mem::take(&mut app.fx.hosted.editor);
    if ed.badge.as_ref().is_none_or(|b| b.0 != effect) {
        let words = app
            .project()
            .effect(strip, effect)
            .and_then(|e| e.spec.external.as_deref())
            .map(|p| {
                if p.vendor.is_empty() {
                    p.format.badge().to_string()
                } else {
                    format!("{} \u{b7} {}", p.format.badge(), p.vendor)
                }
            })
            .unwrap_or_default();
        ed.badge = Some((effect, words));
    }
    let latency = app.plugins.hosted(effect).map_or(0, |h| h.latency()) as f32;
    let sr = (app.engine.sample_rate as f32).max(1.0);
    let badge = ed.badge.take();
    let chip = [
        badge.as_ref().map_or("", |b| b.1.as_str()),
        chips.latency(latency, sr),
    ];
    let look = frame::Look {
        kind: EffectKind::External,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let out = {
        let mut host = DawHost::new(app, strip, effect, "plugin");
        frame::show(ui, &mut host, &mut chrome, &look, |ui, s, body, host| {
            if let Some((app, strip, effect)) = host.daw() {
                body_ui(app, &mut ed, ui, s, body, strip, effect);
            }
        })
    };
    ed.badge = badge;
    app.fx.hosted.editor = ed;
    app.fx.chips = chips;
    app.fx.chrome = chrome;
    out
}

/// The height the open generic editor wants at 100 %, from what it laid out last frame
/// (the window fits its parameters).
pub fn wanted_height(app: &CantusApp) -> Option<f32> {
    app.fx.hosted.editor.wanted
}

/// What a cell asked for this frame, applied once the drawing is done.
enum Change {
    Set(u32, f64, bool),
    /// A parameter's menu asked to automate it (ADR 0057: hosted parameters too).
    Automate(u32),
    /// What its modulation did (ADR 0057), at the cell's rect.
    Mod(u32, crate::kit::automate::Choice, egui::Rect),
}

#[allow(clippy::too_many_arguments)]
fn body_ui(
    app: &mut CantusApp,
    ed: &mut Editor,
    ui: &mut egui::Ui,
    s: &Style,
    body: egui::Rect,
    strip: StripId,
    effect: EffectId,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let Some(sidechain) = app
        .project()
        .effect(strip, effect)
        .and_then(|e| e.spec.external.as_deref())
        .map(|p| p.sidechain)
    else {
        return;
    };
    // The plug-in: its own window, its key, the filter.
    let top = g.full((band::TITLE + 8.0 + controls::ROW + 8.0) * k);
    controls::card(ui, s, top, Src::Key("fx.hosted.card"), None, |ui, cr| {
        let w = cr.body.width() / k;
        let (has_editor, open, floating) = app
            .plugins
            .hosted(effect)
            .map_or((false, false, false), |h| {
                (h.has_editor(), h.window_open(), h.window_floating())
            });
        let label = window_key(open, floating);
        // A stopped plug-in has no window until it is reloaded (review #31).
        let stopped = app.plugins.crashed(effect).map(|why| {
            tf(
                "isolation.tip.stopped",
                &[("reason", &crate::views::plugin_notice::stopped_words(why))],
            )
        });
        let tip = match &stopped {
            Some(why) => why.as_str(),
            None if has_editor => frame_tr("host.editor.window_tip"),
            None => frame_tr("host.editor.no_window"),
        };
        ui.add_enabled_ui(has_editor && stopped.is_none(), |ui| {
            if controls::key(ui, s, Src::Key(label), tip, 170.0, open).clicked() {
                app.show_plugin_window(effect);
            }
        });
        if sidechain {
            key_field(app, ed, ui, s, strip, effect, 190.0);
        }
        let filter_w = (w - 170.0 - if sidechain { 194.0 } else { 0.0 } - 120.0 - 16.0).max(80.0);
        controls::text_field(
            ui,
            s,
            ui.id().with(("host-filter", effect.0)),
            &mut app.fx.hosted.filter,
            frame_tr("host.editor.filter"),
            filter_w,
        );
        let n = app.plugins.hosted(effect).map_or(0, |h| h.params.len());
        controls::count(ui, s, "host.editor.count", n, 116.0);
    });
    // Where it runs and what it costs; a plug-in that stopped, and its Reload (round 29,
    // ADR 0063) — one that could not be loaded too: changing where it runs tries again
    // (review #15).
    {
        let row = g.full((band::TITLE + 8.0 + controls::ROW + 8.0) * k);
        isolation_card(app, ui, s, row, effect);
    }
    if let Some(why) = app.plugins.crashed(effect).map(str::to_string) {
        let rest = g.rest();
        controls::card(
            ui,
            s,
            rest,
            Src::Key("isolation.stopped_card"),
            None,
            |ui, cr| {
                let w = cr.body.width() / k;
                controls::column(ui, s, w, |ui| {
                    let name = app
                        .plugins
                        .hosted(effect)
                        .map(|h| h.info.name.clone())
                        .or_else(|| {
                            app.project()
                                .effect(strip, effect)
                                .and_then(|e| e.spec.external.as_deref())
                                .map(|p| p.name.clone())
                        })
                        .unwrap_or_default();
                    let why = crate::views::plugin_notice::stopped_words(&why);
                    let line = tf(
                        "isolation.editor.stopped",
                        &[("name", &name), ("reason", &why)],
                    );
                    controls::line(ui, s, &line, s.t.danger, w);
                    controls::line(ui, s, frame_tr("isolation.editor.kept"), s.t.text_dim, w);
                    if controls::key(
                        ui,
                        s,
                        Src::Key("isolation.slot.reload"),
                        frame_tr("isolation.slot.reload"),
                        170.0,
                        false,
                    )
                    .clicked()
                    {
                        crate::views::plugin_notice::reload(
                            app,
                            crate::views::plugin_notice::Target::Insert(effect),
                            &name,
                        );
                    }
                });
            },
        );
        return;
    }
    if let Some(why) = app.plugins.missing(effect).map(str::to_string) {
        // On this computer but not loaded: said as such, with Reload (review isolation-5
        // #45); not here at all: to install.
        let not_loaded = app.plugins.load_error(effect).is_some();
        let (title, line_key, kept) = if not_loaded {
            (
                "isolation.not_loaded_card",
                "isolation.editor.not_loaded",
                "isolation.editor.not_loaded_kept",
            )
        } else {
            (
                "host.missing",
                "host.editor.missing",
                "host.editor.missing_kept",
            )
        };
        let rest = g.rest();
        controls::card(ui, s, rest, Src::Key(title), None, |ui, cr| {
            let w = cr.body.width() / k;
            controls::column(ui, s, w, |ui| {
                let name = app
                    .project()
                    .effect(strip, effect)
                    .and_then(|e| e.spec.external.as_deref())
                    .map_or(String::new(), |p| p.name.clone());
                let why = crate::views::plugin_notice::load_words(&why);
                let line = tf(line_key, &[("name", &name), ("why", &why)]);
                controls::line(ui, s, &line, s.t.danger, w);
                controls::line(ui, s, frame_tr(kept), s.t.text_dim, w);
                if not_loaded
                    && controls::key(
                        ui,
                        s,
                        Src::Key("isolation.slot.reload"),
                        frame_tr("isolation.slot.reload"),
                        170.0,
                        false,
                    )
                    .clicked()
                {
                    crate::views::plugin_notice::reload(
                        app,
                        crate::views::plugin_notice::Target::Insert(effect),
                        &name,
                    );
                }
            });
        });
        return;
    }
    // The groups, made again only when the parameters or the filter change.
    let count = app.plugins.hosted(effect).map_or(0, |h| h.params.len());
    let typed = app.fx.hosted.filter.as_str();
    let stale = ed
        .groups
        .as_ref()
        .is_none_or(|(e, n, f, _)| *e != effect || *n != count || f != typed);
    if stale {
        let raw = typed.to_string();
        let filter = typed.trim().to_lowercase();
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        if let Some(h) = app.plugins.hosted(effect) {
            for (i, p) in h.params.iter().enumerate() {
                if p.hidden || (!filter.is_empty() && !p.name.to_lowercase().contains(&filter)) {
                    continue;
                }
                match groups.iter_mut().find(|(g, _)| *g == p.group) {
                    Some((_, list)) => list.push(i),
                    None => groups.push((p.group.clone(), vec![i])),
                }
            }
        }
        for (title, _) in groups.iter_mut() {
            *title = if title.is_empty() {
                t("host.editor.ungrouped")
            } else {
                title.replace('/', " \u{203a} ")
            };
        }
        ed.groups = Some((effect, count, raw, groups));
    }
    let Some((_, _, _, groups)) = ed.groups.as_ref() else {
        return;
    };
    // Cards flow left to right and wrap; a group wider than the body wraps its cells.
    let area = g.rest();
    let cell = crate::kit::cell::size(Size::M) * k;
    let gap = s.d(4.0);
    let pitch = cell.x + gap;
    let pad = 16.0 * k;
    let per_row_max = ((area.width() - pad) / pitch).floor().max(1.0) as usize;
    let mut layout = std::mem::take(&mut ed.layout);
    layout.clear();
    let (mut x, mut y, mut row_h) = (0.0f32, 0.0f32, 0.0f32);
    for (_, list) in groups {
        let per_row = list.len().clamp(1, per_row_max);
        let rows = list.len().div_ceil(per_row).max(1);
        let w = (pad + per_row as f32 * pitch - gap)
            .max(150.0 * k)
            .min(area.width());
        let h = (band::TITLE + 8.0) * k + rows as f32 * (cell.y + gap) - gap + 8.0 * k;
        if x > 0.0 && x + w > area.width() + 0.5 {
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        layout.push(egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(w, h),
        ));
        x += w + gap;
        row_h = row_h.max(h);
    }
    let total = y + row_h;
    // The window fits what it holds: the frame, the plug-in's card, the groups.
    let wanted_h =
        (shell::HEADER + shell::FOOTER + 16.0 + band::TITLE + 16.0 + controls::ROW + 4.0)
            + total / k;
    ed.wanted = Some(wanted_h.clamp(300.0, 760.0));
    let mut changes: Vec<Change> = Vec::new();
    let hosted = app.plugins.hosted(effect);
    let spec = app.bus.project().effect(strip, effect).map(|e| &e.spec);
    let texts = &mut ed.texts;
    let mut area_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut area_ui, |ui| {
            let (origin, _) =
                ui.allocate_exact_size(egui::vec2(area.width(), total), Sense::hover());
            for ((title, list), r) in groups.iter().zip(&layout) {
                let r = r.translate(origin.min.to_vec2());
                if !ui.is_rect_visible(r) {
                    continue;
                }
                controls::card(ui, s, r, Src::Str(title), None, |ui, cr| {
                    let Some(h) = hosted else {
                        return;
                    };
                    // Whole cells per row, rows top to bottom.
                    let per_row = ((cr.body.width() + gap) / pitch).floor().max(1.0) as usize;
                    for (row, chunk) in list.chunks(per_row).enumerate() {
                        let rr = egui::Rect::from_min_size(
                            cr.body.min + egui::vec2(0.0, row as f32 * (cell.y + gap)),
                            egui::vec2(cr.body.width(), cell.y),
                        );
                        let mut row_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rr)
                                .layout(egui::Layout::left_to_right(egui::Align::Min)),
                        );
                        let ui = &mut row_ui;
                        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                        for &i in chunk {
                            let Some(p) = h.params.get(i) else {
                                continue;
                            };
                            let (Some(spec), true) = (spec, p.max != p.min) else {
                                continue;
                            };
                            let mut value = shown_value(spec, Some(h), p);
                            let (lo, hi) = (p.min.min(p.max), p.max.max(p.min));
                            // Automation and modulation of a hosted parameter (ADR 0057):
                            // the curve is its normalised value; the cell shows where the
                            // song has it, wears the marks, offers the menu.
                            let target = cantus_core::AutoTarget::Plugin { effect, id: p.id };
                            let automated = app.automated_value(&target);
                            if let Some(pos) = automated.filter(|x| x.is_finite()) {
                                value = lo + f64::from(pos) * (hi - lo);
                            }
                            let base = if hi > lo {
                                ((value - lo) / (hi - lo)) as f32
                            } else {
                                0.0
                            };
                            let mut rings = [(0.0, egui::Color32::TRANSPARENT);
                                crate::views::modulator::MAX_RINGS];
                            let (offer, n_rings) = app.mod_offer(&target, base, s, &mut rings);
                            if crate::kit::automate::probe::noting_learn() {
                                crate::kit::automate::probe::note_learn(
                                    &format!("hosted:{target:?}"),
                                    offer.learn.is_some(),
                                );
                            }
                            // The plug-in's words for the value, asked once per value — again
                            // while not answered (an isolated plug-in answers later).
                            let later = h.isolation() != cantus_core::PluginIsolation::Within;
                            let text = texts.get(effect, p.id, value.to_bits(), later, || {
                                h.param_text(p.id, value)
                            });
                            let fmt = |_: f32, b: &mut String| match text {
                                Some(t) => b.push_str(t),
                                None => {
                                    use std::fmt::Write as _;
                                    let _ = write!(b, "{value:.2}");
                                }
                            };
                            let out = ui
                                .add_enabled_ui(!p.read_only, |ui| {
                                    controls::Cell::new(
                                        value as f32,
                                        lo as f32..=hi as f32,
                                        p.default as f32,
                                    )
                                    .stepped(p.stepped)
                                    .label(Src::Str(&p.name))
                                    .name(&p.name)
                                    .format(&fmt)
                                    .automation(automated.is_some())
                                    .modulation(offer)
                                    .rings(&rings[..n_rings])
                                    .show(ui, s)
                                })
                                .inner;
                            if let Some(v) = out.changed {
                                changes.push(Change::Set(p.id, f64::from(v), false));
                            }
                            if out.done {
                                changes.push(Change::Set(p.id, f64::NAN, true));
                            }
                            if out.automate {
                                changes.push(Change::Automate(p.id));
                            }
                            if let Some(c) = out.modulation {
                                changes.push(Change::Mod(p.id, c, out.resp.rect));
                            }
                        }
                    }
                });
            }
        });
    ed.layout = layout;
    for c in changes {
        match c {
            Change::Set(id, v, done) => {
                if done {
                    app.end_gesture();
                } else {
                    app.set_plugin_param(strip, effect, id, v, true);
                }
            }
            Change::Automate(id) => app.automate(cantus_core::AutoTarget::Plugin { effect, id }),
            Change::Mod(id, c, r) => {
                app.modulation_choice(&cantus_core::AutoTarget::Plugin { effect, id }, c, r)
            }
        }
    }
}

fn frame_tr(key: &'static str) -> &'static str {
    crate::views::fx::frame::tr(key)
}

/// Which strip keys the plug-in's sidechain input: a menu field of the desk's strips.
fn key_field(
    app: &mut CantusApp,
    ed: &mut Editor,
    ui: &mut egui::Ui,
    s: &Style,
    strip: StripId,
    effect: EffectId,
    width: f32,
) {
    let current = app
        .project()
        .effect(strip, effect)
        .and_then(|e| e.sidechain)
        .filter(|k| app.project().strip(*k).is_some());
    // The field's words, made when the key, the project or the language changes.
    let stamp = (current, app.bus.revision(), crate::i18n::locale());
    if ed.key_label.as_ref().is_none_or(|(k, _)| *k != stamp) {
        let words = match current {
            Some(k) => tf(
                "host.editor.key",
                &[(
                    "strip",
                    &app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
                )],
            ),
            None => t("host.editor.key_none"),
        };
        ed.key_label = Some((stamp, words));
    }
    let label = ed.key_label.as_ref().map_or("", |(_, w)| w.as_str());
    let id = ui.id().with(("host-key", effect.0));
    let Some(at) = controls::field(ui, s, id, label, frame_tr("host.editor.key_tip"), width) else {
        return;
    };
    let options: Vec<(StripId, String)> = app
        .project()
        .strips
        .iter()
        .map(|x| x.id)
        .filter(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)))
        .map(|id| {
            (
                id,
                app.capture_label(cantus_engine::CaptureTarget::Strip(id)),
            )
        })
        .collect();
    let mut items = vec![crate::views::fx::frame::radio(
        Src::Key("host.editor.key_none"),
        current.is_none(),
    )];
    items.extend(
        options
            .iter()
            .map(|(id, text)| crate::views::fx::frame::radio(Src::Str(text), current == Some(*id))),
    );
    if let Some(i) = controls::list(ui, s, id, at, width.max(200.0), &items) {
        let source = if i == 0 {
            None
        } else {
            options.get(i - 1).map(|o| o.0)
        };
        drop(items);
        if source != current {
            app.exec(Command::SetEffectSidechain {
                strip,
                effect,
                source,
            });
        }
    }
}

impl CantusApp {
    /// Open the plug-in's own window, or close it.
    pub fn toggle_plugin_window(&mut self, effect: EffectId) {
        if self.plugins.close_window(effect) {
            return;
        }
        if self.window_of_stopped(crate::views::plugin_notice::Target::Insert(effect)) {
            return;
        }
        let project = self.project();
        let title = project
            .effect_host(effect)
            .and_then(|host| project.effect(host, effect).map(|e| (host, e)))
            .map(|(host, e)| {
                let name = e
                    .spec
                    .external
                    .as_deref()
                    .map_or_else(|| t("host.plugin"), |p| p.name.clone());
                format!(
                    "{name} \u{2014} {}",
                    self.capture_label(cantus_engine::CaptureTarget::Strip(host))
                )
            })
            .unwrap_or_default();
        let owner = self.plugins.owner_window;
        if let Err(e) = self.plugins.open_window(effect, &title, owner) {
            let name = crate::views::plugin_notice::effect_name(self, effect);
            crate::views::plugin_notice::window_failed(self, &name, &e);
        }
    }
}

// ---- A/B and presets, kept by the host ------------------------------------------------

/// Switch between A and B: the showing side is kept, the other one applied — one command.
pub(crate) fn toggle_ab(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let Some(current) = app.plugins.capture(effect) else {
        return;
    };
    let (other_state, other_params, on_b) = app
        .fx
        .hosted
        .ab
        .remove(&effect)
        .unwrap_or_else(|| (current.0.clone(), current.1.clone(), false));
    app.set_plugin_state(strip, effect, other_state, other_params);
    app.fx
        .hosted
        .ab
        .insert(effect, (current.0, current.1, !on_b));
}

/// The folder a plug-in's presets live in: one per plug-in id.
pub fn preset_dir(plugin_id: &str) -> std::path::PathBuf {
    let clean: String = plugin_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    crate::app::data_dir()
        .join("presets")
        .join("plugins")
        .join(clean)
}

/// A saved state, as a preset file.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct PresetFile {
    pub state: PluginState,
    pub params: BTreeMap<u32, f64>,
}

pub(crate) fn presets(plugin_id: &str) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir(preset_dir(plugin_id)) {
        for e in dir.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("json") {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push((name, path));
            }
        }
    }
    out.sort();
    out
}

pub(crate) fn save_preset(
    app: &mut CantusApp,
    effect: EffectId,
    plugin_id: &str,
    name: &str,
) -> Result<(), String> {
    let clean: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    if clean.trim().is_empty() {
        return Err(t("preset.empty_name"));
    }
    let (state, params) = app
        .plugins
        .capture(effect)
        .ok_or_else(|| t("host.preview_missing"))?;
    let dir = preset_dir(plugin_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text =
        serde_json::to_string_pretty(&PresetFile { state, params }).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{}.json", clean.trim())), text).map_err(|e| e.to_string())
}

// ---- the plug-in manager, in Settings -------------------------------------------------

/// Folders, rescans, what was found, and the blocked list — in the kit's parts: action
/// keys for the verbs, section bands for the lists, boxed lines for what they hold.
pub fn manager(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    let width = ui.available_width().max(360.0);
    let found = app.plugins.db.plugins().len();
    let blocked = app.plugins.db.blocked().len();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let scanning = app.plugins.scanning();
        ui.add_enabled_ui(!scanning, |ui| {
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.scan"),
                &t("host.manager.scan_tip"),
                110.0,
                false,
            )
            .clicked()
            {
                app.plugins.start_scan(false);
            }
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.rescan_all"),
                &t("host.manager.rescan_all_tip"),
                130.0,
                false,
            )
            .clicked()
            {
                app.plugins.start_scan(true);
            }
        });
        let status = if scanning {
            t("host.manager.scanning")
        } else {
            tf(
                "host.manager.summary",
                &[("found", &found), ("blocked", &blocked)],
            )
        };
        controls::line(ui, &s, &status, s.t.text_dim, (width - 260.0).max(120.0));
    });
    ui.add_space(8.0);
    isolation_section(app, ui, &s, width);
    ui.add_space(8.0);
    band_title(ui, &s, "host.manager.folders", width);
    let standard: Vec<std::path::PathBuf> = cantus_host::backends()
        .iter()
        .flat_map(|b| b.search_paths())
        .collect();
    let standard_word = t("host.manager.standard");
    for f in &standard {
        ui.horizontal(|ui| {
            controls::line(ui, &s, &f.display().to_string(), s.t.text, width - 130.0);
            controls::line(ui, &s, &standard_word, s.t.text_dim, 120.0);
        });
    }
    let mut remove = None;
    for f in &app.plugins.db.folders {
        ui.horizontal(|ui| {
            controls::line(ui, &s, &f.display().to_string(), s.t.text, width - 130.0);
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.remove"),
                &t("host.manager.remove"),
                110.0,
                false,
            )
            .clicked()
            {
                remove = Some(f.clone());
            }
        });
    }
    if let Some(f) = remove {
        app.plugins.remove_folder(&f);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        controls::text_field(
            ui,
            &s,
            ui.id().with("host-folder"),
            &mut app.fx.hosted.folder,
            &t("host.manager.folder_hint"),
            (width - 250.0).max(160.0),
        );
        let typed = app.fx.hosted.folder.trim().to_string();
        ui.add_enabled_ui(!typed.is_empty(), |ui| {
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.add"),
                &t("host.manager.add"),
                110.0,
                false,
            )
            .clicked()
            {
                app.plugins.add_folder(std::path::PathBuf::from(&typed));
                app.fx.hosted.folder.clear();
            }
        });
        if controls::key(
            ui,
            &s,
            Src::Key("host.manager.browse"),
            &t("host.manager.browse"),
            120.0,
            false,
        )
        .clicked()
        {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                app.plugins.add_folder(dir);
            }
        }
    });
    // What the scans found.
    let found_list: Vec<PluginInfo> = app.plugins.db.plugins().into_iter().cloned().collect();
    ui.add_space(8.0);
    let title = tf("host.manager.found", &[("n", &found_list.len())]);
    band_title_text(ui, &s, &title, width);
    let not_effect = t("host.manager.not_effect");
    for p in &found_list {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            controls::line(ui, &s, &p.name, s.t.text, (width * 0.4).max(140.0));
            badge(ui, cantus_host::to_project_format(p.format).badge());
            let who = format!("{} \u{b7} {}", p.vendor, p.version);
            controls::line(ui, &s, &who, s.t.text_dim, (width * 0.3).max(100.0));
            if p.kind != cantus_host::PluginKind::Effect {
                controls::line(ui, &s, &not_effect, s.t.text_dim, 120.0);
            }
        });
    }
    let blocked: Vec<(std::path::PathBuf, String)> = app
        .plugins
        .db
        .blocked()
        .into_iter()
        .map(|e| {
            let why = match &e.status {
                cantus_host::ScanStatus::Blocked(w) => w.clone(),
                _ => String::new(),
            };
            (e.path.clone(), why)
        })
        .collect();
    ui.add_space(8.0);
    band_title(ui, &s, "host.manager.blocked", width);
    let hint = if blocked.is_empty() {
        t("host.manager.blocked_none")
    } else {
        t("host.manager.blocked_hint")
    };
    controls::line(ui, &s, &hint, s.t.text_dim, width);
    let mut retry = None;
    for (path, why) in &blocked {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            controls::line(
                ui,
                &s,
                &path.display().to_string(),
                s.t.danger,
                (width * 0.45).max(160.0),
            );
            controls::line(ui, &s, why, s.t.text_dim, (width * 0.35).max(120.0));
            if controls::key(
                ui,
                &s,
                Src::Key("host.manager.retry"),
                &t("host.manager.retry_tip"),
                90.0,
                false,
            )
            .clicked()
            {
                retry = Some(path.clone());
            }
        });
    }
    if let Some(path) = retry {
        app.plugins.rescan_file(&path);
    }
    let failed = app.plugins.db.failed().len();
    if failed > 0 {
        let line = tf("host.manager.failed", &[("n", &failed)]);
        controls::line(ui, &s, &line, s.t.text_dim, width);
    }
}

/// A section band with a title already in words.
fn band_title_text(ui: &mut egui::Ui, s: &Style, title: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, band::BAND), Sense::hover());
    crate::kit::with_brush(ui.painter(), |b| {
        band::section(
            b,
            rect,
            s,
            crate::kit::State::REST,
            Src::Str(title),
            None,
            false,
        )
    });
}

// ---- screenshots ------------------------------------------------------------------------

/// `shoot plugin` / `shoot plugins`: the plug-in at `library` (the test plug-in, built from
/// source) read in this process into the database, beside a blocked file and a folder of
/// the user's; with `open_editor`, put on the first strip with a few values moved and its
/// editor open. The database is not written anywhere.
pub fn demo_setup(app: &mut CantusApp, library: &std::path::Path, open_editor: bool) {
    use cantus_host::{Backend, DbEntry, ScanStatus};
    app.plugins.db_path = None;
    let folder = std::env::temp_dir().join("cantus-shoot-plugins");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Gain.clap");
    let _ = std::fs::copy(library, &file);
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .unwrap_or_default();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    app.plugins.db.put(DbEntry {
        path: file.clone(),
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    app.plugins.db.put(DbEntry {
        path: folder.join("Old Reverb 2009.clap"),
        format: cantus_host::Format::Clap,
        modified: 0,
        size: 0,
        status: ScanStatus::Blocked("crashed (exit code 0xc0000409)".into()),
        plugins: Vec::new(),
    });
    app.plugins
        .db
        .folders
        .push(std::path::PathBuf::from(r"D:\Audio\Plug-ins"));
    if !open_editor {
        return;
    }
    let Some(info) = app.plugins.db.effects().first().map(|p| (*p).clone()) else {
        return;
    };
    let Some(strip) = app.project().strips.first().map(|s| s.id) else {
        return;
    };
    app.add_plugin(strip, &info);
    app.sync_engine();
    let Some(effect) = app
        .project()
        .effects_of(strip)
        .and_then(|s| s.iter().rev().find(|e| e.spec.external.is_some()))
        .map(|e| e.id)
    else {
        return;
    };
    app.set_plugin_param(strip, effect, 0, -4.5, false);
    app.set_plugin_param(strip, effect, 2, 1.0, false);
    if let Some(key) = app.project().strips.get(1).map(|s| s.id) {
        app.exec(Command::SetEffectSidechain {
            strip,
            effect,
            source: Some(key),
        });
    }
    app.sync_engine();
    app.open_effect_editor(strip, effect);
}

/// The card's key for the plug-in's own window (round 27, ADR 0051 §3): *Open plug-in
/// window*; open already — perhaps minimised or under other windows — *Show plug-in
/// window*, which brings it back and to the front (it closes with its own close box); and
/// *Close plug-in window* for a plug-in that floats a window of its own, which Cantus
/// cannot bring forward.
pub fn window_key(open: bool, floating: bool) -> &'static str {
    match (open, floating) {
        (false, _) => "host.editor.open_window",
        (true, false) => "host.editor.show_window",
        (true, true) => "host.editor.close_window",
    }
}

impl CantusApp {
    /// The card's key: open the plug-in's own window, or bring it back and to the front —
    /// or close it, for a plug-in that floats its own (round 27, ADR 0051 §3).
    pub fn show_plugin_window(&mut self, effect: EffectId) {
        if self.window_of_stopped(crate::views::plugin_notice::Target::Insert(effect)) {
            return;
        }
        let floating = self
            .plugins
            .hosted(effect)
            .is_some_and(|h| h.window_open() && h.window_floating());
        if floating || !self.plugins.hosted(effect).is_some_and(|h| h.window_open()) {
            self.toggle_plugin_window(effect);
        } else {
            let owner = self.plugins.owner_window;
            // Open already: `open_window` only brings it forward (the title is not used).
            if let Err(e) = self.plugins.open_window(effect, "", owner) {
                let name = crate::views::plugin_notice::effect_name(self, effect);
                crate::views::plugin_notice::window_failed(self, &name, &e);
            }
        }
    }
}
