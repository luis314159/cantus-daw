//! What drives a control, in words (round 39, ADR 0109).
//!
//! In round 38 the owner took some of Chorda's knobs for broken: automation clips were
//! setting them. A control bound to a parameter that something drives — an automation clip
//! (ADR 0027), a modulator (ADR 0057), a hardware controller (ADR 0062), a VCA (ADR 0093)
//! — wears the kit's signs (`kit::automate::signs`), shows the value it is driven to, and
//! its hint says what drives it and how to take it back, in the words of the menus that do
//! it.
//!
//! The kit knows nothing of projects, so the control's hint carries only a tag
//! ([`drive_tag`], set by `CantusApp::mod_offer` when something drives the target); once a
//! frame [`frame`] finds the target under that tag and puts it in words, kept in the hint's
//! slot until the project or the mappings change — a control hovered or dragged re-reads
//! them and formats nothing.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::controller::MappingTarget;
use cantus_core::{AutoTarget, Ticks};
use std::hash::{Hash, Hasher};

/// The tag a driven `target`'s hint carries: never 0 (0 is "nothing drives it").
pub fn drive_tag(target: &AutoTarget) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    target.hash(&mut h);
    h.finish().max(1)
}

/// Once a frame, after the views drew: the words for the driven control under the
/// pointer, when they are not already the ones for the project as it is.
pub fn frame(app: &CantusApp, ctx: &egui::Context) {
    let stamp = app.drive_stamp();
    let Some((tag, done)) = crate::kit::hint::wanted(ctx, stamp) else {
        return;
    };
    if done {
        return;
    }
    let words = app
        .driven_target(tag)
        .map(|target| app.drive_words(&target))
        .unwrap_or_default();
    crate::kit::hint::resolve(ctx, tag, stamp, &words);
}

/// What drives a control and how to take it back, a driver at a time: the words are what
/// drives it ("Automated by …", joined by " · "), a line break, and how to take each back.
#[derive(Default)]
struct Words {
    what: Vec<String>,
    back: Vec<String>,
}

impl Words {
    fn push(&mut self, what: String, back: String) {
        self.what.push(what);
        self.back.push(back);
    }

    fn done(self) -> String {
        if self.what.is_empty() {
            return String::new();
        }
        format!(
            "{}\n{}",
            self.what.join(" \u{b7} "),
            self.back.join(" \u{b7} ")
        )
    }
}

impl CantusApp {
    /// What the words of a driven control depend on: the project, the global controller
    /// map, the playhead's clip.
    pub fn drive_stamp(&self) -> u64 {
        let rev = self.bus.revision();
        let global = self.controllers.global.mappings.len() as u64;
        rev.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ global
    }

    /// The driven target whose tag is `tag`, among everything that drives one.
    pub fn driven_target(&self, tag: u64) -> Option<AutoTarget> {
        let is = |t: &AutoTarget| drive_tag(t) == tag;
        if let Some(l) = self.auto_lanes().iter().find(|l| is(&l.target)) {
            return Some(l.target.clone());
        }
        if let Some(t) = self.mod_routes().keys().find(|t| is(t)) {
            return Some(t.clone());
        }
        let p = self.project();
        let mapped = p
            .controllers
            .mappings
            .iter()
            .chain(self.controllers.global.mappings.iter())
            .find_map(|m| match &m.target {
                MappingTarget::Param(t) if is(t) => Some(t.clone()),
                _ => None,
            });
        if mapped.is_some() {
            return mapped;
        }
        p.strips
            .iter()
            .map(|s| AutoTarget::StripGain(s.id))
            .find(|t| is(t))
    }

    /// What drives `target` and how to take it back: what drives it on the first line, how
    /// to take it back on the second (empty when nothing does).
    pub fn drive_words(&self, target: &AutoTarget) -> String {
        let mut out = Words::default();
        if let Some(what) = self.clip_words(target) {
            let back = tf("ux.drive.clip.back", &[("menu", &t("auto.param.open"))]);
            out.push(what, back);
        }
        let p = self.project();
        let names: Vec<String> = self
            .mod_routes()
            .get(target)
            .into_iter()
            .flatten()
            .filter(|(id, ..)| !p.modulator(*id).is_some_and(|(_, _, m)| m.muted))
            .filter_map(|(id, ..)| crate::views::modulator::mod_name(p, *id))
            .collect();
        if !names.is_empty() {
            let what = tf("ux.drive.mod", &[("names", &names.join(", "))]);
            let back = tf("ux.drive.mod.back", &[("menu", &t("mods.param.modulate"))]);
            out.push(what, back);
        }
        let mt = MappingTarget::Param(target.clone());
        let source = p
            .controllers
            .of_target_in(&mt, p)
            .chain(self.controllers.global.of_target(&mt))
            .map(|m| crate::controllers::control_text(m.source.control))
            .next();
        if let Some(source) = source {
            let what = tf("ux.drive.ctl", &[("control", &source)]);
            let back = tf("ux.drive.ctl.back", &[("menu", &t("ctl.param.forget"))]);
            out.push(what, back);
        }
        if let AutoTarget::StripGain(s) = target {
            if p.has_vca() {
                let vcas = crate::views::mixer::vca_ui::names(p, *s);
                if !vcas.is_empty() {
                    let what = tf("ux.drive.vca", &[("names", &vcas)]);
                    let back = tf("ux.drive.vca.back", &[("menu", &t("mix.menu.vca"))]);
                    out.push(what, back);
                }
            }
        }
        out.done()
    }

    /// "Automated by the clip ‘Hardness’ on Piano": the clip where the song is heard (else
    /// the first in time) and its track; the number of clips when there are several.
    fn clip_words(&self, target: &AutoTarget) -> Option<String> {
        let p = self.project();
        let at = self.engine.position().max(Ticks::ZERO);
        let mut n = 0usize;
        let mut best: Option<(bool, Ticks, &str, &str)> = None;
        for tr in &p.tracks {
            for c in &tr.clips {
                if !c.automation.as_ref().is_some_and(|a| &a.target == target) {
                    continue;
                }
                n += 1;
                let here = c.start <= at && at < c.end();
                let better = match best {
                    None => true,
                    Some((h, start, ..)) => (here && !h) || (here == h && c.start < start),
                };
                if better {
                    best = Some((here, c.start, &c.name, &tr.name));
                }
            }
        }
        // The clip's name says where (a new clip is named "Chorda · Hardness"); its track is
        // said only when the name does not hold it.
        let (_, _, clip, track) = best?;
        let place = if clip.contains(track) {
            String::new()
        } else {
            format!(" {}", tf("ux.drive.on", &[("track", &track)]))
        };
        Some(if n > 1 {
            tf(
                "ux.drive.clips",
                &[("n", &n.to_string()), ("clip", &clip), ("on", &place)],
            )
        } else {
            tf("ux.drive.clip", &[("clip", &clip), ("on", &place)])
        })
    }
}
