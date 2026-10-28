//! A hosted instrument's **Cantus view** (ADR 0073): the device frame of the effects — its
//! name, the track, the plug-in's saved states as presets, A/B of whole states, the swap —
//! around the view's switch, its search and its parameters (`plugin_view`), in a floating
//! window of the workspace like an effect's editor.

use crate::app::CantusApp;
use crate::i18n::tf;
use crate::kit::band;
use crate::kit::shell::{self, Grid};
use crate::kit::text::Src;
use crate::kit::Style;
use crate::views::fx::controls;
use crate::views::fx::frame::{self, tr};
use crate::views::fx::host::{FxHost, Transport};
use crate::views::plugin_notice::{self as notice, Target};
use crate::views::plugin_view::{self, Subject};
use cantus_core::{Command, EffectKind, EffectSpec, ExternalPlugin, PluginState, TrackId};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// The view's own state.
#[derive(Debug, Default)]
pub struct InstView {
    pub(crate) params: plugin_view::Params,
    /// What is typed in its search field.
    pub search: String,
    chrome: frame::Chrome,
    chips: frame::ChipText,
    /// Where the window floats.
    pos: Option<egui::Pos2>,
    /// The frame's stand-in spec (the header reads a spec): the plug-in's name and id.
    spec: Option<(TrackId, EffectSpec)>,
    /// "CLAP · Vendor", for the plug-in (`hosted_identity`) it was made for.
    badge: Option<(u64, String)>,
    /// The other side of A/B per track — state and values — and whether B is showing, for
    /// the plug-in it was taken from (`hosted_identity`): never another's state.
    ab: HashMap<TrackId, Side>,
    /// The height the window wants at 100 %.
    pub(crate) wanted: Option<f32>,
}

/// One track's other side of A/B: the plug-in it is for, its state and values, and whether
/// B is the side showing.
type Side = (u64, PluginState, BTreeMap<u32, f64>, bool);

/// Whether B is the side showing of `track`'s hosted instrument (A/B of its Cantus view).
pub fn on_b(app: &CantusApp, track: TrackId) -> bool {
    side_on_b(&app.fx.hosted.inst.ab, app, track)
}

fn side_on_b(ab: &HashMap<TrackId, Side>, app: &CantusApp, track: TrackId) -> bool {
    let id = app.hosted_identity(Target::Instrument(track));
    ab.get(&track).is_some_and(|x| x.0 == id && x.3)
}

/// Switch A and B of `track`'s hosted instrument: the side showing is kept, the other one
/// applied — one command. A side taken from another plug-in (or another load of it) is
/// forgotten, never applied.
pub fn toggle_ab(app: &mut CantusApp, track: TrackId) {
    let mut ab = std::mem::take(&mut app.fx.hosted.inst.ab);
    toggle_ab_in(app, &mut ab, track);
    app.fx.hosted.inst.ab = ab;
}

fn toggle_ab_in(app: &mut CantusApp, ab: &mut HashMap<TrackId, Side>, track: TrackId) {
    let Some(current) = app.plugins.capture_instrument(track) else {
        return;
    };
    let id = app.hosted_identity(Target::Instrument(track));
    let (state, params, on_b) = match ab.remove(&track) {
        Some((of, state, params, on_b)) if of == id => (state, params, on_b),
        _ => (current.0.clone(), current.1.clone(), false),
    };
    app.exec(Command::SetInstrumentPluginState {
        track,
        state,
        params,
    });
    // Applying a state is no other load: the identity stays.
    ab.insert(track, (id, current.0, current.1, !on_b));
}

/// The device id of a hosted instrument's Cantus view: its key among the workspace's.
pub fn inst_device_id(track: TrackId) -> egui::Id {
    egui::Id::new(("hosted-inst-view", track.0))
}

/// The window, drawn every frame while one is open (`CantusApp::dialogs`).
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(track) = app.fx.hosted.inst_open else {
        return;
    };
    if !app.is_hosted_instrument(track) {
        app.fx.hosted.inst_open = None;
        return;
    }
    let mut pos = app.fx.hosted.inst.pos;
    let mut designed = frame::designed_size(EffectKind::External);
    if let Some(h) = app.fx.hosted.inst.wanted {
        designed.y = h;
    }
    let open = crate::views::fx::open_window_sized(
        ctx,
        inst_device_id(track),
        &mut pos,
        EffectKind::External,
        designed,
        |ui| framed(app, ui, track),
    );
    app.fx.hosted.inst.pos = pos;
    if !open {
        // Closed by hand: a plug-in waiting in it no longer opens its own window later.
        app.close_cantus_view(Target::Instrument(track));
    }
}

/// The frame around the view: the header wired to [`InstHost`], the footer's chips.
pub fn framed(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.hosted.inst);
    // The frame's preset and lists are this plug-in's: another one on the track (or
    // another load of it) starts them again.
    let identity = app.hosted_identity(Target::Instrument(track));
    st.chrome.reset_for(identity);
    let record = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.external())
        .map(|p| (p.format, p.id.clone(), p.name.clone(), p.vendor.clone()));
    let Some((format, id, name, vendor)) = record else {
        app.fx.hosted.inst = st;
        return frame::Out::default();
    };
    if st.spec.as_ref().is_none_or(|(t, sp)| {
        *t != track
            || sp
                .external
                .as_deref()
                .is_none_or(|p| p.id != id || p.name != name)
    }) {
        st.spec = Some((
            track,
            EffectSpec::external(ExternalPlugin::new(format, id.clone(), name.clone())),
        ));
    }
    if st.badge.as_ref().is_none_or(|b| b.0 != identity) {
        let words = if vendor.is_empty() {
            format.badge().to_string()
        } else {
            format!("{} \u{b7} {vendor}", format.badge())
        };
        st.badge = Some((identity, words));
    }
    let latency = app
        .plugins
        .hosted_instrument(track)
        .map_or(0, |h| h.latency()) as f32;
    let sr = (app.engine.sample_rate as f32).max(1.0);
    let InstView {
        params,
        search,
        chrome,
        chips,
        spec,
        badge,
        ab,
        wanted,
        ..
    } = &mut st;
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
    let spec = &spec.as_ref().expect("made above").1;
    let out = {
        let mut host = InstHost {
            app,
            track,
            spec,
            ab,
        };
        frame::show(ui, &mut host, chrome, &look, |ui, s, body, host| {
            if let Some((app, track)) = host.daw_inst() {
                let h = body_ui(app, params, search, ui, s, body, track);
                // Kept while a search narrows the cards: the window does not jump.
                if wanted.is_none() || search.trim().is_empty() {
                    *wanted = Some(h);
                }
            }
        })
    };
    app.fx.hosted.inst = st;
    out
}

/// The body: the view's card (the switch, the default, the search, the count), then what
/// stopped it or the parameters. Returns the height the window wants at 100 %.
fn body_ui(
    app: &mut CantusApp,
    ps: &mut plugin_view::Params,
    search: &mut String,
    ui: &mut egui::Ui,
    s: &Style,
    body: egui::Rect,
    track: TrackId,
) -> f32 {
    let k = s.k;
    let target = Target::Instrument(track);
    let mut g = Grid::new(body, s);
    let card_h = band::TITLE + 8.0 + controls::ROW + 8.0;
    let top = g.full(card_h * k);
    controls::card(ui, s, top, Src::Key("fx.hosted.card"), None, |ui, cr| {
        let w = cr.body.width() / k;
        ui.spacing_mut().item_spacing.x = 4.0 * k;
        plugin_view::switch(app, ui, s, target);
        let count_w = 128.0;
        let used = plugin_view::SWITCH_W + plugin_view::DEFAULT_W + count_w + 16.0;
        plugin_view::search_field(
            ui,
            s,
            ui.id().with(("hosted-inst-search", track.0)),
            search,
            (w - used).max(120.0),
        );
        let words = plugin_view::count_words(
            app,
            ps,
            Subject::Instrument(track),
            !search.trim().is_empty(),
        );
        controls::readout(ui, s, &words, false, "", count_w);
    });
    let base = shell::HEADER + shell::FOOTER + 12.0 + card_h + 4.0;
    if let Some(line) = stopped_line(app, track) {
        let rest = g.rest();
        let reload = app.plugins.instrument_crashed(track).is_some()
            || app.plugins.instrument_load_error(track).is_some();
        controls::card(ui, s, rest, Src::Key(line.0), None, |ui, cr| {
            let w = cr.body.width() / k;
            controls::column(ui, s, w, |ui| {
                controls::line(ui, s, &line.1, line.2, w);
                if let Some(kept) = line.3 {
                    controls::line(ui, s, tr(kept), s.t.text_dim, w);
                }
                if reload
                    && controls::key(
                        ui,
                        s,
                        Src::Key("isolation.slot.reload"),
                        tr("isolation.slot.reload"),
                        170.0,
                        false,
                    )
                    .clicked()
                {
                    let name = app.hosted_instrument_name(track);
                    notice::reload(app, target, &name);
                }
            });
        });
        return base + 140.0;
    }
    let area = g.rest();
    plugin_view::area(app, ps, ui, s, area, Subject::Instrument(track), search);
    (base + ps.content_h.unwrap_or(120.0) + 12.0).clamp(300.0, 780.0)
}

/// What keeps the plug-in from showing its parameters, if anything: the card's title, its
/// line, the line's colour, and the line under it.
fn stopped_line(
    app: &CantusApp,
    track: TrackId,
) -> Option<(&'static str, String, egui::Color32, Option<&'static str>)> {
    let s = Style::current();
    let name = app.hosted_instrument_name(track);
    if let Some(why) = app.plugins.instrument_crashed(track) {
        let why = notice::stopped_words(why);
        let line = tf(
            "isolation.editor.stopped",
            &[("name", &name), ("reason", &why)],
        );
        return Some((
            "isolation.stopped_card",
            line,
            s.t.danger,
            Some("isolation.editor.kept"),
        ));
    }
    if app.plugins.instrument_loading(track) {
        let line = tf("isolation.editor.loading", &[("name", &name)]);
        return Some(("isolation.loading_card", line, s.t.text_dim, None));
    }
    if let Some(why) = app.plugins.instrument_missing(track) {
        let why = notice::load_words(why);
        let (title, key, kept) = if app.plugins.instrument_load_error(track).is_some() {
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
        let line = tf(key, &[("name", &name), ("why", &why)]);
        return Some((title, line, s.t.danger, Some(kept)));
    }
    None
}

/// The frame's host for a hosted instrument: the header's words, its saved states as
/// presets, A/B of whole states (one command each), the swap — and the app for the body.
/// It keeps no parameters of its own kind: the body sets the plug-in's.
struct InstHost<'a> {
    app: &'a mut CantusApp,
    track: TrackId,
    spec: &'a EffectSpec,
    ab: &'a mut HashMap<TrackId, Side>,
}

/// What the frame reads for the analyser: nothing, for an instrument.
static NO_PAIRS: VecDeque<(f32, f32)> = VecDeque::new();

impl InstHost<'_> {
    fn plugin_id(&self) -> Option<String> {
        self.spec.external.as_deref().map(|p| p.id.clone())
    }

    fn apply_state(&mut self, state: PluginState, params: BTreeMap<u32, f64>) {
        self.app.exec(Command::SetInstrumentPluginState {
            track: self.track,
            state,
            params,
        });
    }
}

impl FxHost for InstHost<'_> {
    fn spec(&self) -> &EffectSpec {
        self.spec
    }
    fn set(&mut self, _key: &str, _value: f32, _gesture: Option<&str>) {}
    fn set_many(&mut self, _values: &[(&str, f32)], _label_key: &str) {}
    fn end_gesture(&mut self) {
        self.app.end_gesture();
    }
    fn telemetry(&self) -> cantus_engine::Telemetry {
        [0.0; 64]
    }
    fn pairs(&self) -> &VecDeque<(f32, f32)> {
        &NO_PAIRS
    }
    fn transport(&self) -> Transport {
        Transport::default()
    }
    fn load(&mut self, _params: &BTreeMap<String, f32>, _label_key: &str) {}
    fn context(&self) -> String {
        self.app
            .project()
            .track(self.track)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }
    fn name(&self) -> Option<String> {
        self.spec.external.as_deref().map(|p| p.name.clone())
    }
    fn revision(&self) -> u64 {
        self.app.bus.revision()
    }
    fn device_key(&self) -> Option<u64> {
        Some(inst_device_id(self.track).value())
    }
    fn swap(&mut self) {
        self.app
            .open_picker(crate::picker::PickPurpose::ReplaceInstrument { track: self.track });
    }
    fn swap_tip(&self) -> &'static str {
        "swap.tip.instrument"
    }
    fn ab(&self) -> Option<bool> {
        Some(side_on_b(self.ab, self.app, self.track))
    }
    fn toggle_ab(&mut self) {
        toggle_ab_in(self.app, self.ab, self.track);
    }
    fn copy_ab(&mut self) {
        let on_b = self.ab() == Some(true);
        let id = self.app.hosted_identity(Target::Instrument(self.track));
        if let Some((state, params)) = self.app.plugins.capture_instrument(self.track) {
            self.ab.insert(self.track, (id, state, params, on_b));
        }
    }
    fn user_presets(&self) -> bool {
        true
    }
    /// The plug-in's saved states, shared with its inserts' (one folder per plug-in id).
    fn presets(&self) -> Vec<frame::Entry> {
        let Some(id) = self.plugin_id() else {
            return Vec::new();
        };
        crate::views::plugins::presets(&id)
            .into_iter()
            .map(|(name, path)| frame::Entry {
                name,
                params: Default::default(),
                factory: false,
                file: Some(path),
            })
            .collect()
    }
    fn load_preset(&mut self, preset: &frame::Entry) {
        let file = preset.file.as_ref().and_then(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            serde_json::from_str::<crate::views::plugins::PresetFile>(&text).ok()
        });
        if let Some(f) = file {
            self.apply_state(f.state, f.params);
        }
    }
    fn save_preset(&mut self, name: &str) -> Result<(), String> {
        let id = self
            .plugin_id()
            .ok_or_else(|| crate::i18n::t("host.preview_missing"))?;
        let (state, params) = self
            .app
            .plugins
            .capture_instrument(self.track)
            .ok_or_else(|| crate::i18n::t("host.preview_missing"))?;
        crate::views::plugins::write_preset(&id, name, state, params)
    }
    fn daw_inst(&mut self) -> Option<(&mut CantusApp, TrackId)> {
        Some((&mut *self.app, self.track))
    }
}
