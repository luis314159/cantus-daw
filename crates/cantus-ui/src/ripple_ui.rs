//! r40:ripple — insert and delete time, the section drag and the ripple mode, from the app
//! (ADR 0116). The model's work is `cantus_core::ripple*`; here it is aimed (the time
//! selection, the playhead's bar, the section dropped), run as one undo step, and said.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::ripple_bars::{self, Landing};
use cantus_core::{Project, SectionId, Ticks};

/// What a section dragged with no key held does (Settings ▸ Editing, ADR 0116 §3).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionDrag {
    /// Its bars go in where it lands and the song moves on — nothing is overwritten (Logic's
    /// arrangement markers, Studio One's arranger). The default.
    #[default]
    Insert,
    /// Its bars replace what is where it lands; where they were is left silent.
    Replace,
}

/// How a section drop lands for the keys held: Ctrl copies; Shift does what a plain drag,
/// as `plain` has it, does not.
pub fn landing_for(mods: egui::Modifiers, plain: SectionDrag) -> Landing {
    Landing {
        copy: mods.command,
        insert: mods.shift != (plain == SectionDrag::Insert),
    }
}

/// The help line's keys for a section drag under `plain`.
pub fn drag_keys_key(plain: SectionDrag) -> &'static str {
    match plain {
        SectionDrag::Insert => "ripple.drag.keys_replace",
        SectionDrag::Replace => "ripple.drag.keys",
    }
}

impl CantusApp {
    /// Whether the keys of the time edits (Insert, Ctrl+Del, Ctrl+Backspace) act now: only
    /// while the arrangement is what is being edited (the keys' focus, the area on screen)
    /// and the pointer is not over something else that has keys of its own — an editor's
    /// window, a menu, a dialog, another area of the workspace (FL's insert and delete space
    /// are the playlist's). Review mid-round 40, C-3: an EQ's Ctrl+Del took its band and the
    /// song's time at once. Review 2, R2-B1: the arrangement floating in a window of its own
    /// or maximised is the arrangement, wherever on it the pointer is (its ruler, its
    /// sections lane, its tool row); and when the keyboard brought the focus there
    /// (Ctrl+F6), the keys act wherever the mouse happens to rest.
    pub(crate) fn time_keys(&self, ctx: &egui::Context) -> Result<(), TimeKeysAway> {
        use crate::views::workspace::Pane;
        use crate::views::workspace_fold::PaneAt;
        if !self.workspace.is_visible(Pane::Arrangement) {
            return Err(TimeKeysAway::Hidden);
        }
        if self.focus != crate::app::EditFocus::Arrangement {
            return Err(TimeKeysAway::Focus);
        }
        if self.focus_by_keys {
            return Ok(());
        }
        let Some(pos) = ctx.pointer_hover_pos() else {
            return Ok(());
        };
        match self.workspace.pane_at(ctx, pos) {
            PaneAt::Area(Pane::Arrangement) | PaneAt::Bare => Ok(()),
            PaneAt::Area(p) => Err(TimeKeysAway::Pointer(p)),
            PaneAt::Above => Err(TimeKeysAway::Above),
        }
    }

    /// The status line's word when a time key was refused: why, and how to get there. Over
    /// a layer with keys of its own (an editor, a menu) the key was that layer's: nothing
    /// said.
    pub(crate) fn say_time_keys_away(&mut self, why: TimeKeysAway) {
        // Ctrl+F6, or whatever the keymap has it as; unbound, the mouse's way only.
        let key = self.keymap.display_for(crate::keymap::Action::NextArea);
        let keyed = !key.is_empty();
        let text = match why {
            TimeKeysAway::Above => return,
            TimeKeysAway::Hidden => t("ripple.keys.hidden"),
            TimeKeysAway::Focus if keyed => tf("ripple.keys.focus", &[("key", &key)]),
            TimeKeysAway::Focus => t("ripple.keys.focus_click"),
            TimeKeysAway::Pointer(p) => {
                let area = t(p.title_key());
                if keyed {
                    tf("ripple.keys.pointer", &[("area", &area), ("key", &key)])
                } else {
                    tf("ripple.keys.pointer_click", &[("area", &area)])
                }
            }
        };
        self.set_status(text);
    }

    /// Settings ▸ Editing's *Section drag*, stored.
    pub fn set_section_drag(&mut self, plain: SectionDrag) {
        self.settings.section_drag = plain;
        self.store_settings();
    }
}

/// Why the time edits' keys did not act (R2-B1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimeKeysAway {
    /// The arrangement is not on screen.
    Hidden,
    /// Another view is what is being edited.
    Focus,
    /// The pointer rests on another area of the workspace.
    Pointer(crate::views::workspace::Pane),
    /// The pointer is over a layer with keys of its own: an editor, a menu, a dialog.
    Above,
}

/// A section being dragged in the sections lane: its bars, where they would land and how.
#[derive(Clone, Debug, PartialEq)]
pub struct BarsDrop {
    pub section: SectionId,
    pub from: Ticks,
    pub to: Ticks,
    /// Where the bars would begin, in the song as it is (before the drop).
    pub dest: Ticks,
    pub how: Landing,
}

impl BarsDrop {
    /// Whether dropping here changes nothing.
    pub fn is_still(&self) -> bool {
        self.how.is_still(self.from, self.to, self.dest)
    }
}

/// A length in the song's words: "2 bars", "1 bar", "3 beats", "2.5 bars" ("2,5 compases").
pub fn length_words(p: &Project, from: Ticks, to: Ticks) -> String {
    use crate::views::bars::bar_position;
    let bars = bar_position(p, to) - bar_position(p, from);
    if (bars - bars.round()).abs() < 1e-6 {
        let n = bars.round() as i64;
        return if n == 1 {
            t("ripple.one_bar")
        } else {
            tf("ripple.bars", &[("n", &n)])
        };
    }
    let beats = (to - from).as_beats_f64();
    if (beats - beats.round()).abs() < 1e-6 && bars < 1.0 {
        let n = beats.round() as i64;
        return if n == 1 {
            t("ripple.one_beat")
        } else {
            tf("ripple.beats", &[("n", &n)])
        };
    }
    tf("ripple.bars", &[("n", &decimal_words(bars))])
}

/// A length's number as prose writes it: at most two decimals, no trailing zeros, and the
/// language's decimal mark — "2.5 bars", "2,5 compases" (review mid-round 40, C-4). The
/// readouts of values keep the point; this is a sentence.
pub fn decimal_words(x: f64) -> String {
    let mut s = format!("{x:.2}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if crate::i18n::locale() == crate::i18n::Locale::Es {
        s = s.replace('.', ",");
    }
    s
}

/// Bar and beat of a tick, as the ruler reads it ("33.1").
pub fn place_words(p: &Project, at: Ticks) -> String {
    let (bar, beat, _) = crate::views::bars::bbt(p, at);
    format!("{bar}.{beat}")
}

impl CantusApp {
    /// *Insert time*: the time selection's length of empty time at its start — or, with no
    /// selection, one bar at the bar the playhead is in — on every track, with the tempo,
    /// the metre, the markers and the sections. The selection then holds the new time.
    pub fn insert_time(&mut self) -> bool {
        let (at, len) = match self.time_selection {
            Some((a, b)) => (a, b - a),
            None => {
                let p = self.project();
                let pos = self.engine.position().max(Ticks::ZERO);
                (
                    crate::views::bars::bar_start(p, pos),
                    crate::views::bars::bar_len(p, pos),
                )
            }
        };
        let cmd = self.with_ids(|p, ids| cantus_core::ripple::insert_time(p, at, len, ids));
        let Some(cmd) = cmd else {
            return false;
        };
        if !self.exec(cmd) {
            return false;
        }
        self.validate_selection();
        self.set_time_selection(at, at + len);
        let words = length_words(self.project(), at, at + len);
        let place = place_words(self.project(), at);
        self.set_status(tf(
            "ripple.status.inserted",
            &[("len", &words), ("at", &place)],
        ));
        true
    }

    /// *Delete time*: the time selection taken out of the whole song, the gap closed.
    /// Without a selection it says so and does nothing (deleting the playhead's bar unasked
    /// would take music nobody chose).
    pub fn delete_time(&mut self) -> bool {
        let Some((from, to)) = self.time_selection else {
            self.set_status(t("ripple.status.select_first"));
            return false;
        };
        let words = length_words(self.project(), from, to);
        let place = place_words(self.project(), from);
        let cmd = self.with_ids(|p, ids| cantus_core::ripple::delete_time(p, from, to, ids));
        let Some(cmd) = cmd else {
            return false;
        };
        if !self.exec(cmd) {
            return false;
        }
        self.clear_time_selection();
        self.validate_selection();
        self.set_status(tf(
            "ripple.status.deleted",
            &[("len", &words), ("at", &place)],
        ));
        true
    }

    /// Drop a dragged section's bars where `d` says (one undo step); the section, or its
    /// copy, is selected where it landed.
    pub fn drop_section_bars(&mut self, d: &BarsDrop) -> bool {
        if d.is_still() {
            return false;
        }
        let cmd =
            self.with_ids(|p, ids| ripple_bars::move_bars(p, d.from, d.to, d.dest, d.how, ids));
        let Some(cmd) = cmd else {
            return false;
        };
        let name = self
            .project()
            .section(d.section)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        if !self.exec(cmd) {
            return false;
        }
        self.validate_selection();
        let land = d.how.lands_at(d.from, d.to, d.dest);
        let landed = self
            .project()
            .sections
            .iter()
            .find(|s| s.start == land && s.name == name)
            .map(|s| s.id);
        self.arrangement.sections.selected = landed.or(Some(d.section));
        let words = length_words(self.project(), land, land + (d.to - d.from));
        let place = place_words(self.project(), land);
        let key = match (d.how.copy, d.how.insert) {
            (false, false) => "ripple.status.moved",
            (true, false) => "ripple.status.copied",
            (false, true) => "ripple.status.moved_in",
            (true, true) => "ripple.status.copied_in",
        };
        self.set_status(tf(key, &[("name", &name), ("len", &words), ("at", &place)]));
        true
    }
}

impl CantusApp {
    /// The ripple mode's switch: off → this track → every track → off, said in the status
    /// line (the arrangement's tool row shows it while it is on).
    pub fn cycle_ripple(&mut self) {
        let mode = self.arrangement.ripple.next();
        self.set_ripple(mode);
    }

    /// The ripple mode set (the panel menu's entries), said in the status line.
    pub fn set_ripple(&mut self, mode: cantus_core::ripple_mode::RippleMode) {
        use cantus_core::ripple_mode::RippleMode;
        self.arrangement.ripple = mode;
        self.set_status(t(match mode {
            RippleMode::Off => "ripple.status.off",
            RippleMode::Track => "ripple.status.track",
            RippleMode::All => "ripple.status.all",
        }));
    }
}

impl CantusApp {
    /// The removal of `sel` (its `commands`) as one step, with what followed each clip moved
    /// back over its gap when the ripple mode is on (on its track, or on every track with the
    /// song's marks and tempo).
    pub(crate) fn rippled_delete(
        &mut self,
        sel: &[(cantus_core::TrackId, cantus_core::ClipId)],
        commands: Vec<cantus_core::Command>,
    ) -> cantus_core::Command {
        use cantus_core::ripple_mode::{rippled, Follow};
        let plain = cantus_core::Command::Batch {
            label_key: "cmd.remove_clip".into(),
            commands: commands.clone(),
        };
        let mode = self.arrangement.ripple;
        if !mode.is_on() {
            return plain;
        }
        let follows: Vec<Follow> = sel
            .iter()
            .filter_map(|r| crate::clips::clip_of(self.project(), *r).map(|c| (r.0, c)))
            .map(|(track, c)| Follow {
                track,
                at: c.end(),
                delta: Ticks::ZERO - c.length,
                exclude: Vec::new(),
            })
            .collect();
        // In every-track mode the song closes each stretch the clips left once, however
        // many tracks they were on: their spans merged.
        let follows = if mode == cantus_core::ripple_mode::RippleMode::All {
            let mut spans: Vec<(Ticks, Ticks, cantus_core::TrackId)> = follows
                .iter()
                .map(|f| (f.at + f.delta, f.at, f.track))
                .collect();
            spans.sort_by_key(|s| (s.0, s.1));
            let mut merged: Vec<(Ticks, Ticks, cantus_core::TrackId)> = Vec::new();
            for s in spans {
                match merged.last_mut() {
                    Some(m) if s.0 <= m.1 => m.1 = m.1.max(s.1),
                    _ => merged.push(s),
                }
            }
            merged
                .into_iter()
                .map(|(a, b, track)| Follow {
                    track,
                    at: b,
                    delta: a - b,
                    exclude: Vec::new(),
                })
                .collect()
        } else {
            follows
        };
        self.with_ids(|p, ids| rippled(p, commands, "cmd.ripple.delete_clips", mode, &follows, ids))
            .unwrap_or(plain)
    }
}

impl CantusApp {
    /// A clip drag's `commands` with the ripple mode's follow, when the mode is on and the
    /// drag is one it ripples: a move along the track (not a copy, not to another track) —
    /// what follows the moved clips on their track moves by as much — or a right edge
    /// drawn out or in, what follows the clip's end moving with it. `None`: the drag alone.
    pub(crate) fn rippled_drag(
        &mut self,
        sel: &[(cantus_core::TrackId, cantus_core::ClipId)],
        anchor: (cantus_core::TrackId, cantus_core::ClipId),
        result: crate::views::arrangement::ClipDragResult,
        commands: &[cantus_core::Command],
        label: &str,
    ) -> Option<cantus_core::Command> {
        use crate::views::arrangement::ClipDragResult as R;
        use cantus_core::ripple_mode::{rippled, Follow};
        let mode = self.arrangement.ripple;
        if !mode.is_on() || commands.is_empty() {
            return None;
        }
        let mut clips: Vec<(cantus_core::TrackId, cantus_core::Clip)> = sel
            .iter()
            .chain(std::iter::once(&anchor))
            .filter_map(|r| crate::clips::clip_of(self.project(), *r).map(|c| (r.0, c.clone())))
            .collect();
        clips.sort_by_key(|(t, c)| (t.0, c.id));
        clips.dedup_by_key(|(t, c)| (t.0, c.id));
        let follows: Vec<Follow> = match result {
            R::Move {
                delta,
                rows: 0,
                copy: false,
            } => {
                let mut tracks: Vec<cantus_core::TrackId> = clips.iter().map(|c| c.0).collect();
                tracks.dedup();
                tracks
                    .into_iter()
                    .filter_map(|t| {
                        let mine: Vec<&cantus_core::Clip> =
                            clips.iter().filter(|c| c.0 == t).map(|c| &c.1).collect();
                        let at = mine.iter().map(|c| c.start).min()?;
                        Some(Follow {
                            track: t,
                            at,
                            delta,
                            exclude: mine.iter().map(|c| c.id).collect(),
                        })
                    })
                    .collect()
            }
            R::TrimEnd { delta } => clips
                .iter()
                .map(|(t, c)| Follow {
                    track: *t,
                    at: c.end(),
                    delta,
                    exclude: vec![c.id],
                })
                .collect(),
            _ => return None,
        };
        // In every-track mode one follow is the song's: the earliest point, once.
        let follows = if mode == cantus_core::ripple_mode::RippleMode::All {
            follows
                .into_iter()
                .min_by_key(|f| f.at)
                .map(|mut f| {
                    f.exclude = clips.iter().map(|c| c.1.id).collect();
                    vec![f]
                })
                .unwrap_or_default()
        } else {
            follows
        };
        let commands = commands.to_vec();
        self.with_ids(|p, ids| rippled(p, commands, label, mode, &follows, ids))
    }
}

/// The ripple mode's word in the tool row while it is on ("Ripple · this track").
pub fn mode_word(mode: cantus_core::ripple_mode::RippleMode) -> Option<String> {
    use cantus_core::ripple_mode::RippleMode;
    match mode {
        RippleMode::Off => None,
        RippleMode::Track => Some(t("ripple.mode.track")),
        RippleMode::All => Some(t("ripple.mode.all")),
    }
}

impl CantusApp {
    /// A paste of clips at `at` (`commands`, making `made`) as one step with the ripple
    /// mode's follow when it is on: what started at or after `at` on the tracks pasted to (or
    /// on every track) moves on by the pasted block's length — the paste goes in (REAPER's
    /// and Studio One's ripple paste). `None`: the paste alone.
    pub(crate) fn rippled_paste(
        &mut self,
        at: Ticks,
        commands: &[cantus_core::Command],
        made: &[(cantus_core::TrackId, cantus_core::ClipId)],
    ) -> Option<cantus_core::Command> {
        use cantus_core::ripple_mode::{rippled, Follow, RippleMode};
        use cantus_core::Command;
        let mode = self.arrangement.ripple;
        if !mode.is_on() || commands.is_empty() {
            return None;
        }
        let laid: Vec<(cantus_core::TrackId, cantus_core::ClipId, Ticks)> = commands
            .iter()
            .filter_map(|c| match c {
                Command::InsertClip { track, clip } if made.contains(&(*track, clip.id)) => {
                    Some((*track, clip.id, clip.end()))
                }
                _ => None,
            })
            .collect();
        let end = laid.iter().map(|l| l.2).max()?;
        let delta = end - at;
        if delta <= Ticks::ZERO {
            return None;
        }
        let exclude: Vec<cantus_core::ClipId> = laid.iter().map(|l| l.1).collect();
        let mut tracks: Vec<cantus_core::TrackId> = laid.iter().map(|l| l.0).collect();
        tracks.sort_by_key(|t| t.0);
        tracks.dedup();
        if mode == RippleMode::All {
            tracks.truncate(1);
        }
        let follows: Vec<Follow> = tracks
            .into_iter()
            .map(|track| Follow {
                track,
                at,
                delta,
                exclude: exclude.clone(),
            })
            .collect();
        let commands = commands.to_vec();
        self.with_ids(|p, ids| rippled(p, commands, "cmd.ripple.paste", mode, &follows, ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn a_settings_file_from_before_the_section_drag_opens_with_insert_and_replace_is_kept() {
        let old: Settings = serde_json::from_str("{}").expect("an old settings file reads");
        assert_eq!(old.section_drag, SectionDrag::Insert);
        let mut s = Settings::default();
        assert_eq!(s.section_drag, SectionDrag::Insert, "the default");
        s.section_drag = SectionDrag::Replace;
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"section_drag\":\"replace\""), "{text}");
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.section_drag, SectionDrag::Replace);
    }

    // Review mid-round 40, C-4: a length reads as prose writes it, in each language, and the
    // Spanish lines agree with any length.
    #[test]
    fn lengths_read_as_prose_and_the_spanish_lines_agree() {
        use crate::i18n::{set_thread_locale, Locale};
        set_thread_locale(Locale::Es);
        assert_eq!(decimal_words(2.5), "2,5");
        assert_eq!(decimal_words(2.25), "2,25");
        assert_eq!(decimal_words(2.0), "2");
        assert_eq!(decimal_words(0.126), "0,13");
        let one = tf(
            "ripple.status.inserted",
            &[("len", &t("ripple.one_bar")), ("at", &"5.1")],
        );
        assert_eq!(
            one,
            "Tiempo insertado: 1 compás en 5.1, en todas las pistas"
        );
        let moved = tf(
            "ripple.status.moved",
            &[
                ("name", &"Estribillo"),
                ("len", &t("ripple.one_bar")),
                ("at", &"9.1"),
            ],
        );
        assert_eq!(moved, "Estribillo: se movió a 9.1 (1 compás)");
        set_thread_locale(Locale::En);
        assert_eq!(decimal_words(2.5), "2.5");
        assert_eq!(decimal_words(3.10), "3.1");
    }

    #[test]
    fn shift_gives_the_other_landing_and_ctrl_copies() {
        use egui::Modifiers as M;
        let l = |m, plain| landing_for(m, plain);
        assert!(l(M::NONE, SectionDrag::Insert).insert);
        assert!(!l(M::SHIFT, SectionDrag::Insert).insert);
        assert!(!l(M::NONE, SectionDrag::Replace).insert);
        assert!(l(M::SHIFT, SectionDrag::Replace).insert);
        let c = l(M::COMMAND | M::SHIFT, SectionDrag::Insert);
        assert!(c.copy && !c.insert);
        assert!(
            !l(M::ALT, SectionDrag::Insert).copy,
            "Alt only leaves the bars"
        );
    }
}
