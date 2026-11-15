//! An instrument's own mixer (round 40 phase 3, ADR 0123): its named outputs as strips —
//! Tympanum's mics, Chorda's perspectives and noises — drawn as Cantus's mixer draws its
//! strips (des-mixer's palette, law and parts, `crate::views::mixer::look`), so the two
//! read as one hand. Shared: Tympanum (`tympanum_strips`) and Chorda (`chorda-src`'s
//! mixer) describe their strips as [`Strip`]s and act on the [`Edit`]s a frame returns —
//! each a command on the instrument's keys (`o{n}.level`, `o{n}.pan`, `o{n}.invert`,
//! `o{n}.mute`, `o{n}.solo`, `o{n}.delay`; a bleed or a width of their own).
//!
//! The desk: strips 72 pt wide under **family heads** that fold (a folded family is one
//! strip with its members' meters side by side), pinned strips at both ends (Tympanum's
//! *Kit* and *Ambience*, Chorda's *Piano* and *Room*), the middle scrolling with a fade and
//! the *n more ›* pill in the heads' row, over no strip.

#[path = "inst_mixer_desk.rs"]
pub mod desk;
#[path = "inst_mixer_strip.rs"]
pub mod strip;
#[path = "inst_mixer_ui.rs"]
pub mod ui;

use cantus_core::{OutputFamily, OutputId};

/// A strip's width at 100 %.
pub const STRIP_W: f32 = 72.0;
/// The family heads' row.
pub const HEADS: f32 = 22.0;

/// What a strip's top cell shows and moves.
#[derive(Clone, Debug, PartialEq)]
pub enum Top {
    /// A share (0…1 at the bar's end; 100 % at `full`), its word ("Bleed", "Width", "Level").
    Share {
        word: String,
        value: f32,
        full: f32,
    },
    /// Words in place of a control ("hears all").
    Words(String),
    None,
}

/// What a strip's Out cell says.
#[derive(Clone, Debug, PartialEq)]
pub struct OutCell {
    /// The second line: *Main*, *Own strip*, a strip's name.
    pub label: String,
    /// The strip number at the right of the first line, if it goes to one.
    pub number: Option<usize>,
    /// The cell opens a list (a fixed one — Kit's own — does not).
    pub opens: bool,
    /// The help line's words.
    pub tip: String,
}

/// One strip as the instrument describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    /// The output it is (`None`: a strip of the instrument's own, Kit or Ambience).
    pub id: Option<OutputId>,
    pub name: String,
    pub sub: String,
    pub family: OutputFamily,
    pub top: Top,
    /// ø, when the strip has a polarity.
    pub invert: Option<bool>,
    /// Pan −1…1, when it has one.
    pub pan: Option<f32>,
    /// M and S, when the strip has them (`None`: Tympanum's Kit, whose strip on the desk
    /// has its own).
    pub mute: Option<bool>,
    pub solo: Option<bool>,
    /// The output's delay in ms (`o{n}.delay`, 0 as recorded), when it has one.
    pub delay: Option<f32>,
    pub level_db: f32,
    /// The level's range at the fader's ends.
    pub level_max: f32,
    /// The meter, dB per side; `None`: stopped.
    pub meter: Option<[f32; 2]>,
    pub out: OutCell,
    pub selected: bool,
    /// Off (`o{n}.on` = 0): dimmed, its samples unloaded.
    pub loaded: bool,
}

/// What the user did to a strip this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Edit {
    /// The top cell dragged to this share (0…1 of the bar), and released.
    Top(f32),
    Pan(f32),
    Invert,
    Mute,
    Solo,
    /// The fader dragged to this level (dB).
    Level(f32),
    /// The delay dragged or typed to this many ms.
    Delay(f32),
    /// A drag ended: the gesture's undo step closes.
    End,
    /// The strip clicked (its name, its body).
    Select,
    /// The Out cell clicked, at this rect.
    Out(egui::Rect),
}

/// Which strip an [`Edit`] is for: its index in the list the instrument gave.
pub type Edits = Vec<(usize, Edit)>;

/// The family's head word.
pub fn family_word(f: OutputFamily) -> String {
    crate::i18n::t(f.label_key())
}

/// "−4.0", "+1.5", "0.0": the fader's value (one decimal, a true minus); silence, nothing
/// (the desk's rule: no "−∞").
pub fn db_text(db: f32) -> String {
    let mut s = String::new();
    crate::views::mixer::look::write_db(&mut s, db);
    s
}

/// What an output's Out list offers (Tympanum's mics, Chorda's perspectives): *Main*, the
/// first free strip, then the desk's named strips but the instrument's own — never a VCA,
/// which plays no sound and `inst_out::set_route` refuses (review of 40a27bb7, finding 9:
/// the menu closed silently on one). Each with its label.
pub fn out_targets(
    p: &cantus_core::Project,
    track: cantus_core::TrackId,
) -> Vec<(String, Option<cantus_core::StripId>)> {
    use crate::i18n::tf;
    let inst = p.track(track).map(|t| t.name.clone()).unwrap_or_default();
    let mut out: Vec<(String, Option<cantus_core::StripId>)> =
        vec![(tf("inst_mixer.out_menu_main", &[("name", &inst)]), None)];
    if let Some(free) = p.first_free_strip().filter(|s| !p.is_vca(*s)) {
        let num = p.strip_number(free).unwrap_or(0);
        out.push((tf("pads.track_free", &[("n", &num)]), Some(free)));
    }
    let mine = p.track(track).and_then(|t| t.strip);
    for st in p
        .strips
        .iter()
        .filter(|x| x.on_desk() && Some(x.id) != mine && !p.is_vca(x.id))
    {
        if out.iter().any(|(_, s)| *s == Some(st.id)) || st.name.is_empty() {
            continue;
        }
        let num = p.strip_number(st.id).unwrap_or(0);
        out.push((format!("{num} \u{b7} {}", st.name), Some(st.id)));
    }
    out
}
