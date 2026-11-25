//! The families of editors (round 42, ADR 0161): what each parked editor keeps of its own,
//! and how it is swapped with its family's slot (`editor_windows`).
//!
//! What stays shared, never parked: what is per track or per effect already (A/B's other
//! sides, the preset each track was set from, the scenes), the user's choices for every
//! editor (the shell's collapsed bands, the keyboard card), test and demo injections, and
//! where the next effect editor opens (`fx.pos`).

use crate::app::CantusApp;
use crate::editor_windows::Family;
use crate::views::fx::{Editor, FxEditors};
use crate::views::inst::InstEditors;
use crate::views::workspace_device::DeviceKey;
use cantus_core::{EffectId, EffectKind, StripId, TrackId};
use std::mem::swap;

// ---- the native instruments ---------------------------------------------------------------

/// A native instrument editor's own state.
#[derive(Default)]
pub struct InstSlot {
    open: Option<TrackId>,
    view: crate::views::inst::View,
    held: Vec<u8>,
    popup: crate::views::inst::Popup,
    popup_pass: u64,
    popup_from: Option<egui::Rect>,
    dragging: Option<&'static str>,
    chorda_source_open_previous: bool,
    wavetable: crate::views::inst::wavetable::State,
    analog: crate::views::inst::analog::State,
    fm: crate::views::inst::fm::State,
    sampler: crate::views::inst::sampler::State,
    bass: crate::views::inst::bass::State,
    epiano: crate::views::inst::epiano::State,
    organ: crate::views::inst::organ::State,
    dancer: crate::views::inst::dancer::State,
}

impl InstSlot {
    fn swap(&mut self, i: &mut InstEditors) {
        swap(&mut self.open, &mut i.open);
        swap(&mut self.view, &mut i.view);
        swap(&mut self.held, &mut i.held);
        swap(&mut self.popup, &mut i.popup);
        swap(&mut self.popup_pass, &mut i.popup_pass);
        swap(&mut self.popup_from, &mut i.popup_from);
        swap(&mut self.dragging, &mut i.dragging);
        swap(
            &mut self.chorda_source_open_previous,
            &mut i.chorda_source_open_previous,
        );
        swap(&mut self.wavetable, &mut i.wavetable);
        swap(&mut self.analog, &mut i.analog);
        swap(&mut self.fm, &mut i.fm);
        swap(&mut self.sampler, &mut i.sampler);
        swap(&mut self.bass, &mut i.bass);
        swap(&mut self.epiano, &mut i.epiano);
        swap(&mut self.organ, &mut i.organ);
        swap(&mut self.dancer, &mut i.dancer);
    }
}

pub(crate) struct Inst;

impl Family for Inst {
    type Key = TrackId;
    type Slot = InstSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(TrackId, InstSlot)> {
        &mut app.editors.inst
    }
    fn keys(app: &CantusApp) -> Vec<TrackId> {
        app.editors.inst.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<TrackId> {
        app.inst.open
    }
    fn swap(app: &mut CantusApp, slot: &mut InstSlot) {
        slot.swap(&mut app.inst);
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::inst::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        app.close_inst_editor();
    }
    fn device(key: TrackId) -> DeviceKey {
        crate::views::inst::editor_id(key).value()
    }
    fn exists(app: &CantusApp, key: TrackId) -> bool {
        // Without cloning the spec: this runs after every command.
        app.project()
            .track(key)
            .is_some_and(|t| t.instrument.native().is_some())
    }
}

// ---- Pulsus ---------------------------------------------------------------------------------

/// A Pulsus editor's own state.
#[derive(Default)]
pub struct PulsusSlot {
    open: Option<TrackId>,
    view: crate::views::pulsus_editor::View,
    selected: Option<cantus_core::PadId>,
    popup: crate::views::pulsus_editor::Popup,
}

pub(crate) struct Pulsus;

impl Family for Pulsus {
    type Key = TrackId;
    type Slot = PulsusSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(TrackId, PulsusSlot)> {
        &mut app.editors.pulsus
    }
    fn keys(app: &CantusApp) -> Vec<TrackId> {
        app.editors.pulsus.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<TrackId> {
        app.pulsus.open
    }
    fn swap(app: &mut CantusApp, slot: &mut PulsusSlot) {
        let p = &mut app.pulsus;
        swap(&mut slot.open, &mut p.open);
        swap(&mut slot.view, &mut p.view);
        swap(&mut slot.selected, &mut p.selected);
        swap(&mut slot.popup, &mut p.popup);
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::pulsus_editor::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        crate::views::pulsus_editor::close(app);
    }
    fn device(key: TrackId) -> DeviceKey {
        crate::views::pulsus_editor::editor_id(key).value()
    }
    fn exists(app: &CantusApp, key: TrackId) -> bool {
        crate::views::pulsus_editor::kit_of(app, key).is_some()
    }
}

// ---- a hosted instrument's Cantus view -----------------------------------------------------

/// A hosted instrument's Cantus view: its own state.
#[derive(Default)]
pub struct HostedSlot {
    open: Option<TrackId>,
    view: crate::views::plugin_view_inst::InstView,
}

pub(crate) struct Hosted;

impl Family for Hosted {
    type Key = TrackId;
    type Slot = HostedSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(TrackId, HostedSlot)> {
        &mut app.editors.hosted
    }
    fn keys(app: &CantusApp) -> Vec<TrackId> {
        app.editors.hosted.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<TrackId> {
        app.fx.hosted.inst_open
    }
    fn swap(app: &mut CantusApp, slot: &mut HostedSlot) {
        swap(&mut slot.open, &mut app.fx.hosted.inst_open);
        swap(&mut slot.view, &mut app.fx.hosted.inst);
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::plugin_view_inst::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        if let Some(t) = app.fx.hosted.inst_open {
            app.close_cantus_view(crate::views::plugin_notice::Target::Instrument(t));
        }
    }
    fn device(key: TrackId) -> DeviceKey {
        crate::views::plugin_view_inst::inst_device_id(key).value()
    }
    fn exists(app: &CantusApp, key: TrackId) -> bool {
        app.is_hosted_instrument(key)
    }
}

// ---- the effects: the plug-in family's slot, the EQ's and the compressor's ----------------

/// An effect editor's own state: its frame, its pairs, its kind's state.
#[derive(Default)]
pub struct FxSlot {
    open: Option<(StripId, EffectId, EffectKind)>,
    pairs: std::collections::VecDeque<(f32, f32)>,
    chrome: crate::views::fx::frame::Chrome,
    chips: crate::views::fx::frame::ChipText,
    spec_cache: Option<(u64, EffectId, cantus_core::EffectSpec)>,
    redo_mark: Option<(EffectId, u64)>,
    /// The kind's own state (`views::fx::Editor`), made fresh the first time it is swapped.
    kind: Option<Editor>,
    hosted: crate::views::plugins::Editor,
    filter: String,
}

impl FxSlot {
    /// The kind of the effect this parked editor edits.
    pub(crate) fn kind(&self) -> Option<EffectKind> {
        self.open.map(|o| o.2)
    }
}

/// Exchange the state of `kind` in `fx` with `ed`'s (both directions).
fn swap_kind(fx: &mut FxEditors, ed: &mut Editor) {
    match ed {
        Editor::Gate(s) => swap(&mut fx.gate, s),
        Editor::Stereo(s) => swap(&mut fx.stereo, s),
        Editor::Multiband(s) => swap(&mut fx.multiband, s),
        Editor::Space(s) => swap(&mut fx.space, s),
        Editor::Limiter(s) => swap(&mut fx.limiter, s),
        Editor::DeEsser(s) => swap(&mut fx.deesser, s),
        Editor::Maximizer(s) => swap(&mut fx.maximizer, s),
        Editor::Saturator(s) => swap(&mut fx.saturator, s),
        Editor::Concentus(s) => swap(&mut fx.concentus, s),
        Editor::Flexus(s) => swap(&mut fx.flexus, s),
        Editor::Phasis(s) => swap(&mut fx.phasis, s),
        Editor::Tremor(s) => swap(&mut fx.tremor, s),
        Editor::Vibrans(s) => swap(&mut fx.vibrans, s),
        Editor::Anulus(s) => swap(&mut fx.anulus, s),
        Editor::Translatio(s) => swap(&mut fx.translatio, s),
        Editor::Fluctus(s) => swap(&mut fx.fluctus, s),
        Editor::Tonsor(s) => swap(&mut fx.tonsor, s),
        Editor::Impetus(s) => swap(&mut fx.impetus, s),
        Editor::Discrimen(s) => swap(&mut fx.discrimen, s),
        Editor::Speculum(s) => swap(&mut fx.speculum, s),
        Editor::Tonus(s) => swap(&mut fx.tonus, s),
        Editor::Aula(s) => swap(&mut fx.aula, s),
        Editor::Cribrum(s) => swap(&mut fx.cribrum, s),
        Editor::Fragor(s) => swap(&mut fx.fragor, s),
        Editor::Lapsus(s) => swap(&mut fx.lapsus, s),
        Editor::Vetus(s) => swap(&mut fx.vetus, s),
        Editor::Libra(s) => swap(&mut fx.libra, s),
        Editor::Tabula(s) => swap(&mut fx.tabula, s),
        Editor::Saltus(s) => swap(&mut fx.saltus, s),
        Editor::Rota(s) => swap(&mut fx.rota, s),
        Editor::Arca(s) => swap(&mut fx.arca, s),
        Editor::Loquax(s) => swap(&mut fx.loquax, s),
        Editor::Fractus(s) => swap(&mut fx.fractus, s),
        Editor::Caelum(s) => swap(&mut fx.caelum, s),
        Editor::Vinculum(s) => swap(&mut fx.vinculum, s),
        Editor::Lenis(s) => swap(&mut fx.lenis, s),
        Editor::Cedo(s) => swap(&mut fx.cedo, s),
        Editor::Purus(s) => swap(&mut fx.purus, s),
        Editor::Nitor(s) => swap(&mut fx.nitor, s),
        Editor::Comparatio(s) => swap(&mut fx.comparatio, s),
        Editor::Delay(s) => swap(&mut fx.delay, s),
        // Their windows are their own slots (`Eq`, `Comp`), never this family's.
        Editor::Comp(_) | Editor::Eq(_) => {}
    }
}

pub(crate) struct Fx;

impl Family for Fx {
    type Key = EffectId;
    type Slot = FxSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(EffectId, FxSlot)> {
        &mut app.editors.fx
    }
    fn keys(app: &CantusApp) -> Vec<EffectId> {
        app.editors.fx.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<EffectId> {
        app.fx.open.map(|o| o.1)
    }
    fn swap(app: &mut CantusApp, slot: &mut FxSlot) {
        let fx = &mut *app.fx;
        // The kind of whichever editor is being put in the slot or taken out of it: both
        // are the same editor, so its kind's state goes with it.
        let kind = slot.open.or(fx.open).map(|o| o.2);
        swap(&mut slot.open, &mut fx.open);
        swap(&mut slot.pairs, &mut fx.pairs);
        swap(&mut slot.chrome, &mut fx.chrome);
        swap(&mut slot.chips, &mut fx.chips);
        swap(&mut slot.spec_cache, &mut fx.spec_cache);
        swap(&mut slot.redo_mark, &mut fx.redo_mark);
        swap(&mut slot.hosted, &mut fx.hosted.editor);
        swap(&mut slot.filter, &mut fx.hosted.filter);
        if slot.kind.is_none() {
            slot.kind = kind.and_then(Editor::of);
        }
        if let Some(ed) = slot.kind.as_mut() {
            swap_kind(fx, ed);
        }
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::fx::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        app.close_fx_editor();
    }
    fn device(key: EffectId) -> DeviceKey {
        crate::views::fx::device_id(key).value()
    }
    fn exists(app: &CantusApp, key: EffectId) -> bool {
        app.project().effect_host(key).is_some()
    }
}

/// An EQ editor's whole state (the window's state is all its own).
#[derive(Default)]
pub struct EqSlot(crate::views::eq_editor::EqEditor);

pub(crate) struct Eq;

impl Family for Eq {
    type Key = EffectId;
    type Slot = EqSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(EffectId, EqSlot)> {
        &mut app.editors.eq
    }
    fn keys(app: &CantusApp) -> Vec<EffectId> {
        app.editors.eq.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<EffectId> {
        app.eq_editor.open.map(|o| o.1)
    }
    fn swap(app: &mut CantusApp, slot: &mut EqSlot) {
        swap(&mut slot.0, &mut app.eq_editor);
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::eq_editor::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        app.close_eq_editor();
    }
    fn device(key: EffectId) -> DeviceKey {
        crate::views::fx::device_id(key).value()
    }
    fn exists(app: &CantusApp, key: EffectId) -> bool {
        app.project().effect_host(key).is_some()
    }
}

/// A compressor editor's whole state.
#[derive(Default)]
pub struct CompSlot(crate::views::comp_editor::CompEditor);

pub(crate) struct Comp;

impl Family for Comp {
    type Key = EffectId;
    type Slot = CompSlot;
    fn list(app: &mut CantusApp) -> &mut Vec<(EffectId, CompSlot)> {
        &mut app.editors.comp
    }
    fn keys(app: &CantusApp) -> Vec<EffectId> {
        app.editors.comp.iter().map(|p| p.0).collect()
    }
    fn resident(app: &CantusApp) -> Option<EffectId> {
        app.comp_editor.open.map(|o| o.1)
    }
    fn swap(app: &mut CantusApp, slot: &mut CompSlot) {
        swap(&mut slot.0, &mut app.comp_editor);
    }
    fn draw(app: &mut CantusApp, ctx: &egui::Context) {
        crate::views::comp_editor::window(app, ctx);
    }
    fn close(app: &mut CantusApp) {
        app.close_comp_editor();
    }
    fn device(key: EffectId) -> DeviceKey {
        crate::views::fx::device_id(key).value()
    }
    fn exists(app: &CantusApp, key: EffectId) -> bool {
        app.project().effect_host(key).is_some()
    }
}
