//! **Editor windows that behave** (round 42, ADR 0161): pinned editors, several at once.
//!
//! The owner, 2026-11-25: closing an instrument's or a plug-in's editor should bring back
//! the screen that was there before; and he wants to *pin* an editor, or have several on
//! screen, so a click elsewhere does not make them go. The first is the workspace's
//! (`Workspace::device_leaves`); the rest is here.
//!
//! **Families and the slot.** Every editor's code was written for one open editor of its
//! family — `app.inst.open`, `app.fx.open`, `app.eq_editor.open`, `app.comp_editor.open`,
//! `app.pulsus.open`, `app.fx.hosted.inst_open` — and keeps its per-editor state beside it
//! (held keys, the view, the popup, the analyser's pairs, the frame's preset, each kind's own
//! state). Rather than thread an id through forty editors, each family keeps **one editor in
//! its slot** (the *resident*: what the existing code sees) and the others **parked** here,
//! each with its own copy of that state ([`Family::Slot`]). Every frame each parked editor is
//! swapped into the slot, drawn by the family's own `window`, and swapped out again, so each
//! one always sees exactly its own state — a second Unda never shows the first one's
//! oscillator page, two keyboards never share their held keys.
//!
//! **The rules** ([`opening_closes`], tested below):
//!
//! 1. Opening an editor that is not open makes room in its family: in *one at a time* (the
//!    default, today's behaviour) the family's **unpinned** editors close and the pinned
//!    ones stay; in *several at once* (Settings ▸ Editing) nothing closes. The editor that
//!    was in the slot is parked if it stays.
//! 2. The effects (Aequalis, Comprimo and the others) are one family across their three
//!    slots, as they were one at a time before; instruments keep today's three families
//!    (native, Pulsus, a hosted instrument's Cantus view).
//! 3. Opening an editor already open (parked included) brings it to the front.
//! 4. A press on a parked editor moves it into its slot: the slot's editor is the one the
//!    engine reports on — the instrument's telemetry probe and the effects' analyser follow
//!    the editor touched last ([`CantusApp::sync_editor_engine`]). The others keep their
//!    controls live and show their meters at rest until touched; nothing more is asked of
//!    the audio thread (ADR 0005: one probe, one analyser, as before).
//! 5. An editor whose track or effect is gone closes at once, parked or not, after every
//!    command, undo and redo ([`CantusApp::prune_editors`]) — so undoing the deletion brings
//!    the track back, never a dangling editor.
//! 6. While a parked editor draws, an *open* it asks for (Pulsus's chain opening an
//!    effect, for one) waits until the parked editors are drawn ([`Deferred`]).
//!
//! Window layout stays outside the project (ADR 0161 §6): which editors are open and pinned
//! lives for the session, as the dock's device tabs always did.

use crate::app::CantusApp;
use crate::views::workspace_device::{self as device, DeviceKey};
use cantus_core::{EffectId, StripId, TrackId};
use serde::{Deserialize, Serialize};

/// Settings ▸ Editing: how many editors of a family stay open (Ableton's *Multiple Plug-in
/// Windows*, FL's default of many floating windows).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorsOpen {
    /// Opening an editor closes the others of its family that are not pinned.
    #[default]
    OneAtATime,
    /// Every editor stays until it is closed.
    Several,
}

impl EditorsOpen {
    pub fn label_key(self) -> &'static str {
        match self {
            EditorsOpen::OneAtATime => "editors.open.one",
            EditorsOpen::Several => "editors.open.several",
        }
    }
}

/// An open that waits until the parked editors are drawn (rule 6).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Deferred {
    Instrument(TrackId),
    NativeInstrument(TrackId),
    Effect(StripId, EffectId),
    Pulsus(TrackId),
    Hosted(crate::views::plugin_notice::Target),
}

/// The parked editors of every family, and what the engine was last asked to report on.
#[derive(Default)]
pub struct Editors {
    pub(crate) inst: Vec<(TrackId, crate::editor_windows_park::InstSlot)>,
    pub(crate) pulsus: Vec<(TrackId, crate::editor_windows_park::PulsusSlot)>,
    pub(crate) hosted: Vec<(TrackId, crate::editor_windows_park::HostedSlot)>,
    pub(crate) fx: Vec<(EffectId, crate::editor_windows_park::FxSlot)>,
    pub(crate) eq: Vec<(EffectId, crate::editor_windows_park::EqSlot)>,
    pub(crate) comp: Vec<(EffectId, crate::editor_windows_park::CompSlot)>,
    /// The effect editor touched or opened last: the analyser's.
    pub live_effect: Option<EffectId>,
    /// What the engine was last asked to analyse, and whose instrument to probe.
    pub analysed: Option<EffectId>,
    pub probed: Option<TrackId>,
    /// A parked editor is being drawn: opens wait (rule 6), the engine is not re-aimed.
    pub(crate) drawing: bool,
    pub(crate) deferred: Vec<Deferred>,
    /// `prune_editors` is running (a close it makes can run a command).
    pub(crate) pruning: bool,
}

impl Editors {
    /// How many editors are parked, all families.
    pub fn parked_count(&self) -> usize {
        self.inst.len()
            + self.pulsus.len()
            + self.hosted.len()
            + self.fx.len()
            + self.eq.len()
            + self.comp.len()
    }

    /// The native instruments' parked editors, by track.
    pub fn parked_instruments(&self) -> Vec<TrackId> {
        self.inst.iter().map(|p| p.0).collect()
    }

    /// The effects' parked editors (every slot), by effect.
    pub fn parked_effects(&self) -> Vec<EffectId> {
        let fx = self.fx.iter().map(|p| p.0);
        let eq = self.eq.iter().map(|p| p.0);
        fx.chain(eq).chain(self.comp.iter().map(|p| p.0)).collect()
    }

    /// Another project is in place: its ids mean other things (r41:ids). The parked editors
    /// go, unclosed (their tracks are the old project's).
    pub(crate) fn forget_project(&mut self) {
        self.inst.clear();
        self.pulsus.clear();
        self.hosted.clear();
        self.fx.clear();
        self.eq.clear();
        self.comp.clear();
        self.live_effect = None;
        self.deferred.clear();
    }
}

/// Rule 1, pure: whether opening another editor in a family closes an open one that is not
/// that editor. `several`: the setting; `pinned`: the open one's pin.
pub fn opening_closes(several: bool, pinned: bool) -> bool {
    !several && !pinned
}

/// Rule 4, pure: which open effect editor the analyser follows — the one touched last if it
/// is still open, else the first open one in `open` (the slots' order: effects, EQ,
/// compressor).
pub fn live_of(touched: Option<EffectId>, open: &[Option<EffectId>]) -> Option<EffectId> {
    match touched {
        Some(t) if open.contains(&Some(t)) => Some(t),
        _ => open.iter().flatten().next().copied(),
    }
}

/// One family of editors: its slot in the application, its parked list, how to swap one
/// editor's state with the slot's, how to draw and close the slot's editor.
pub(crate) trait Family {
    type Key: Copy + PartialEq + std::fmt::Debug;
    type Slot: Default;
    fn list(app: &mut CantusApp) -> &mut Vec<(Self::Key, Self::Slot)>;
    fn keys(app: &CantusApp) -> Vec<Self::Key>;
    fn resident(app: &CantusApp) -> Option<Self::Key>;
    /// Exchange `slot` with the application's slot of the family (both directions).
    fn swap(app: &mut CantusApp, slot: &mut Self::Slot);
    fn draw(app: &mut CantusApp, ctx: &egui::Context);
    /// Close the slot's editor, as its own ✕ does.
    fn close(app: &mut CantusApp);
    fn device(key: Self::Key) -> DeviceKey;
    fn exists(app: &CantusApp, key: Self::Key) -> bool;
}

/// Move the slot's editor (if any) to the parked list. The slot is left empty.
pub(crate) fn park_resident<F: Family>(app: &mut CantusApp) {
    let Some(key) = F::resident(app) else {
        return;
    };
    let mut slot = F::Slot::default();
    F::swap(app, &mut slot);
    F::list(app).push((key, slot));
}

/// Put parked editor `i` in the slot; the slot's editor is parked.
pub(crate) fn promote<F: Family>(app: &mut CantusApp, i: usize) {
    if i >= F::list(app).len() {
        return;
    }
    let (_, mut slot) = F::list(app).remove(i);
    park_resident::<F>(app);
    F::swap(app, &mut slot);
}

/// Run `f` with parked editor `i` in the slot, then park it again — unless `f` closed it.
/// Returns whether it is still open.
pub(crate) fn with_parked<F: Family>(
    app: &mut CantusApp,
    i: usize,
    f: impl FnOnce(&mut CantusApp),
) -> bool {
    let mut slot = std::mem::take(&mut F::list(app)[i].1);
    F::swap(app, &mut slot);
    let was = app.editors.drawing;
    app.editors.drawing = true;
    f(app);
    app.editors.drawing = was;
    let open = F::resident(app).is_some();
    F::swap(app, &mut slot);
    if open {
        F::list(app)[i].1 = slot;
    } else {
        F::list(app).remove(i);
    }
    open
}

/// Draw every parked editor of a family, each with its own state.
pub(crate) fn draw_parked<F: Family>(app: &mut CantusApp, ctx: &egui::Context) {
    let mut i = 0;
    while i < F::list(app).len() {
        if with_parked::<F>(app, i, |app| F::draw(app, ctx)) {
            i += 1;
        }
    }
}

/// Close the editor of `key`, parked or in the slot. True when one was open.
pub(crate) fn close_key<F: Family>(app: &mut CantusApp, key: F::Key) -> bool {
    if F::resident(app) == Some(key) {
        F::close(app);
        return true;
    }
    match F::list(app).iter().position(|p| p.0 == key) {
        Some(i) => {
            if with_parked::<F>(app, i, F::close) {
                // A close that left it open (it refused): forget it here anyway.
                F::list(app).remove(i);
            }
            true
        }
        None => false,
    }
}

/// Whether the editor of `key` is open, parked or in the slot.
pub(crate) fn is_open<F: Family>(app: &CantusApp, key: F::Key) -> bool {
    F::resident(app) == Some(key) || F::keys(app).contains(&key)
}

/// Close the family's editors that are not pinned (rule 1), the slot's and the parked ones,
/// but `keep`.
pub(crate) fn close_unpinned<F: Family>(app: &mut CantusApp, keep: Option<F::Key>) {
    let ctx = app.theme_ctx.clone();
    let pinned = |k: F::Key| ctx.as_ref().is_some_and(|c| device::is_pinned(c, F::device(k)));
    if let Some(r) = F::resident(app).filter(|r| Some(*r) != keep && !pinned(*r)) {
        let _ = r;
        F::close(app);
    }
    for k in F::keys(app) {
        if Some(k) != keep && !pinned(k) {
            close_key::<F>(app, k);
        }
    }
}

/// Make room in a family's slot for a new editor: the slot's editor is parked if it stays
/// (pinned, or several at once), else closed; in one at a time the parked unpinned ones
/// close too.
pub(crate) fn make_room<F: Family>(app: &mut CantusApp) {
    let several = app.settings.editors_open == EditorsOpen::Several;
    if !several {
        close_unpinned::<F>(app, None);
    }
    park_resident::<F>(app);
}

/// A press landed on `focused`: if it is a parked editor of the family, it takes the slot.
/// Returns its key when it is the family's (parked or in the slot).
pub(crate) fn touch<F: Family>(app: &mut CantusApp, focused: DeviceKey) -> Option<F::Key> {
    if let Some(r) = F::resident(app).filter(|r| F::device(*r) == focused) {
        return Some(r);
    }
    let i = F::list(app)
        .iter()
        .position(|p| F::device(p.0) == focused)?;
    let key = F::list(app)[i].0;
    promote::<F>(app, i);
    Some(key)
}

/// An empty slot takes the most recently parked editor, so the engine has one to report on.
pub(crate) fn refill<F: Family>(app: &mut CantusApp) {
    if F::resident(app).is_none() {
        let n = F::list(app).len();
        if n > 0 {
            promote::<F>(app, n - 1);
        }
    }
}

/// Editors whose track or effect is gone close (rule 5).
pub(crate) fn prune<F: Family>(app: &mut CantusApp) {
    if let Some(r) = F::resident(app) {
        if !F::exists(app, r) {
            F::close(app);
        }
    }
    for k in F::keys(app) {
        if !F::exists(app, k) {
            close_key::<F>(app, k);
        }
    }
}

// ---- the application's side -----------------------------------------------------------------

use crate::editor_windows_park::{Comp, Eq, Fx, Hosted, Inst, Pulsus};

impl CantusApp {
    /// The pin key of a device window (ADR 0161): pin it, or unpin it.
    pub fn toggle_editor_pin(&mut self, ctx: &egui::Context, key: DeviceKey) {
        let on = device::toggle_pin(ctx, key);
        let name = device::with_registry(ctx, |r| {
            r.get(key).map(|e| e.title.clone()).unwrap_or_default()
        });
        let words = if on {
            "editors.status.pinned"
        } else {
            "editors.status.unpinned"
        };
        self.set_status(crate::i18n::tf(words, &[("name", &name)]));
    }

    /// Whether an open asked for now has to wait for the parked editors to be drawn (rule
    /// 6): it is kept, and run once they are.
    pub(crate) fn defer_open(&mut self, what: Deferred) -> bool {
        if !self.editors.drawing {
            return false;
        }
        if !self.editors.deferred.contains(&what) {
            self.editors.deferred.push(what);
        }
        true
    }

    /// Make room among the native instruments for an editor that is not open yet.
    pub(crate) fn room_for_instrument(&mut self) {
        make_room::<Inst>(self);
    }

    pub(crate) fn room_for_pulsus(&mut self) {
        make_room::<Pulsus>(self);
    }

    pub(crate) fn room_for_hosted(&mut self) {
        make_room::<Hosted>(self);
    }

    /// Make room for an effect editor in `slot`'s slot: the effects are one family across
    /// the three slots (rule 2), so the other slots' unpinned editors close too in one at a
    /// time; only `slot`'s own editor has to leave its slot.
    pub(crate) fn room_for_effect(&mut self, slot: EffectSlot) {
        if self.settings.editors_open == EditorsOpen::OneAtATime {
            close_unpinned::<Fx>(self, None);
            close_unpinned::<Eq>(self, None);
            close_unpinned::<Comp>(self, None);
        }
        match slot {
            EffectSlot::Fx => park_resident::<Fx>(self),
            EffectSlot::Eq => park_resident::<Eq>(self),
            EffectSlot::Comp => park_resident::<Comp>(self),
        }
    }

    /// A parked native instrument editor of `track` comes into the slot. True if one did.
    pub(crate) fn unpark_instrument(&mut self, track: TrackId) -> bool {
        match self.editors.inst.iter().position(|p| p.0 == track) {
            Some(i) => {
                promote::<Inst>(self, i);
                self.sync_editor_engine();
                true
            }
            None => false,
        }
    }

    pub(crate) fn unpark_pulsus(&mut self, track: TrackId) -> bool {
        match self.editors.pulsus.iter().position(|p| p.0 == track) {
            Some(i) => {
                promote::<Pulsus>(self, i);
                true
            }
            None => false,
        }
    }

    pub(crate) fn unpark_hosted(&mut self, track: TrackId) -> bool {
        match self.editors.hosted.iter().position(|p| p.0 == track) {
            Some(i) => {
                promote::<Hosted>(self, i);
                true
            }
            None => false,
        }
    }

    /// Whether an effect's editor is open in any slot, parked or not.
    pub fn effect_editor_open(&self, effect: EffectId) -> bool {
        is_open::<Fx>(self, effect) || is_open::<Eq>(self, effect) || is_open::<Comp>(self, effect)
    }

    /// A parked effect editor comes into its slot and is the live one. True if one did.
    pub(crate) fn unpark_effect(&mut self, effect: EffectId) -> bool {
        let key = crate::views::fx::device_id(effect).value();
        let hit = touch::<Fx>(self, key).is_some()
            || touch::<Eq>(self, key).is_some()
            || touch::<Comp>(self, key).is_some();
        if hit {
            self.editors.live_effect = Some(effect);
            self.sync_editor_engine();
        }
        hit
    }

    /// Whether `track`'s native instrument, Pulsus or hosted view editor is parked.
    pub(crate) fn instrument_parked(&self, track: TrackId) -> bool {
        self.editors.inst.iter().any(|p| p.0 == track)
            || self.editors.pulsus.iter().any(|p| p.0 == track)
            || self.editors.hosted.iter().any(|p| p.0 == track)
    }

    /// Close `track`'s parked instrument editors (its native, Pulsus or hosted view).
    pub(crate) fn close_parked_instrument(&mut self, track: TrackId) {
        if self.inst.open != Some(track) {
            close_key::<Inst>(self, track);
        }
        if self.pulsus.open != Some(track) {
            close_key::<Pulsus>(self, track);
        }
        if self.fx.hosted.inst_open != Some(track) {
            close_key::<Hosted>(self, track);
        }
        self.sync_editor_engine();
    }

    /// Close an effect's editor wherever it is, parked or in its slot.
    pub fn close_effect_editor(&mut self, effect: EffectId) {
        close_key::<Fx>(self, effect);
        close_key::<Eq>(self, effect);
        close_key::<Comp>(self, effect);
        self.sync_editor_engine();
    }

    /// Aim the engine's one instrument probe and one analyser (rule 4): the native
    /// instrument in the slot, and the live effect editor — the one touched or opened last
    /// if it is still open in its slot, else the first open slot's. Sent only on a change.
    pub fn sync_editor_engine(&mut self) {
        if self.editors.drawing {
            return;
        }
        let probe = self.inst.open;
        if probe != self.editors.probed {
            self.engine.set_instrument_probe(probe);
            self.editors.probed = probe;
        }
        let open = [
            self.fx.open.map(|o| o.1),
            self.eq_editor.open.map(|o| o.1),
            self.comp_editor.open.map(|o| o.1),
        ];
        let live = live_of(self.editors.live_effect, &open);
        self.editors.live_effect = live;
        if live != self.editors.analysed {
            self.engine.set_analyser(live);
            self.editors.analysed = live;
        }
    }

    /// Whether `effect`'s editor reads the engine's live numbers: it is the analysed one, or
    /// a test or a demo injected its telemetry.
    pub(crate) fn effect_reads_engine(&self, effect: EffectId) -> bool {
        self.fx.injected.is_some() || self.editors.analysed == Some(effect)
    }

    /// Rule 5: editors whose track or effect is gone close, parked or not. After every
    /// command, undo and redo (and once a frame).
    pub(crate) fn prune_editors(&mut self) {
        if self.editors.drawing || self.editors.pruning {
            return;
        }
        if self.editors.parked_count() == 0
            && self.inst.open.is_none()
            && self.pulsus.open.is_none()
            && self.fx.hosted.inst_open.is_none()
            && self.fx.open.is_none()
            && self.eq_editor.open.is_none()
            && self.comp_editor.open.is_none()
        {
            return;
        }
        self.editors.pruning = true;
        prune::<Inst>(self);
        prune::<Pulsus>(self);
        prune::<Hosted>(self);
        prune::<Fx>(self);
        prune::<Eq>(self);
        prune::<Comp>(self);
        self.editors.pruning = false;
        self.sync_editor_engine();
    }

    /// Every device editor of this pass (`CantusApp::device_editors`): a press on a parked
    /// editor brings it into its slot; each family's slot is drawn, then its parked editors;
    /// the opens they asked for run; empty slots take a parked editor; the engine follows;
    /// pinned windows stay above the others.
    pub(crate) fn draw_editor_windows(&mut self, ctx: &egui::Context) {
        if let Some(f) = device::focused(ctx) {
            if let Some(e) = touch::<Fx>(self, f)
                .or_else(|| touch::<Eq>(self, f))
                .or_else(|| touch::<Comp>(self, f))
            {
                self.editors.live_effect = Some(e);
            }
            let _ = touch::<Inst>(self, f);
            let _ = touch::<Pulsus>(self, f);
            let _ = touch::<Hosted>(self, f);
            self.sync_editor_engine();
        }
        crate::views::eq_editor::window(self, ctx);
        draw_parked::<Eq>(self, ctx);
        crate::views::comp_editor::window(self, ctx);
        draw_parked::<Comp>(self, ctx);
        crate::views::fx::window(self, ctx);
        draw_parked::<Fx>(self, ctx);
        crate::views::fx::fxd_window::window(self, ctx); // fxd: the controllers' windows
        crate::views::inst::window(self, ctx);
        draw_parked::<Inst>(self, ctx);
        crate::views::plugin_view_inst::window(self, ctx);
        draw_parked::<Hosted>(self, ctx);
        crate::views::pulsus_editor::window(self, ctx);
        draw_parked::<Pulsus>(self, ctx);
        for d in std::mem::take(&mut self.editors.deferred) {
            match d {
                Deferred::Instrument(t) => self.open_instrument_window(t),
                Deferred::NativeInstrument(t) => self.open_inst_editor(t),
                Deferred::Effect(s, e) => self.open_effect_editor(s, e),
                Deferred::Pulsus(t) => crate::views::pulsus_editor::open(self, t),
                Deferred::Hosted(target) => self.open_cantus_view(target),
            }
        }
        refill::<Inst>(self);
        refill::<Pulsus>(self);
        refill::<Hosted>(self);
        refill::<Fx>(self);
        refill::<Eq>(self);
        refill::<Comp>(self);
        self.prune_editors();
        self.sync_editor_engine();
        device::keep_pinned_above(ctx);
    }
}

/// Which of the effects' three slots an editor goes in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectSlot {
    Fx,
    Eq,
    Comp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_at_a_time_closes_only_what_is_not_pinned() {
        assert!(opening_closes(false, false), "today's single editor");
        assert!(!opening_closes(false, true), "a pinned editor stays");
        assert!(!opening_closes(true, false), "several at once: nothing closes");
        assert!(!opening_closes(true, true));
    }

    #[test]
    fn the_analyser_follows_the_editor_touched_last_while_it_is_open() {
        let (a, b, c) = (EffectId(1), EffectId(2), EffectId(3));
        assert_eq!(live_of(Some(b), &[Some(a), Some(b), None]), Some(b));
        // The one touched closed: the first open slot's.
        assert_eq!(live_of(Some(c), &[None, Some(b), Some(a)]), Some(b));
        assert_eq!(live_of(None, &[None, None, Some(a)]), Some(a));
        assert_eq!(live_of(Some(a), &[None, None, None]), None);
    }
}
