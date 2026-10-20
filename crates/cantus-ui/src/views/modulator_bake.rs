//! *Bake to automation* in the application (ADR 0057): a modulator's route into one
//! parameter becomes an automation clip — over the time selection, else the loop, else the
//! whole song — and the route goes, in **one undo step**. The curve is the parameter's
//! position, its own automation and this modulator together (`cantus_core::modulator::
//! bake`); other modulators on it keep modulating on top.
//!
//! The new clip starts at the stretch's start, so it plays over any clip of the parameter
//! that began before it (the latest start wins, ADR 0027 §4) and gives way to it again
//! after; a clip of the parameter that starts inside the stretch is removed, or trimmed to
//! begin where the stretch ends, so the baked curve is what plays there.
//!
//! A source that follows audio (a follower, an envelope opened by a level) cannot be known
//! ahead: the stretch is rendered offline on a worker, the engine tracing the modulator,
//! and the bake is made when the trace comes back (if the project has not moved since).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::modulator::bake::bake;
use cantus_core::{AutoTarget, Automation, Command, ModulatorId, Ticks};
use std::sync::mpsc;

/// A bake waiting for its trace.
#[derive(Debug)]
pub struct Pending {
    id: ModulatorId,
    target: AutoTarget,
    span: (Ticks, Ticks),
    revision: u64,
    rx: mpsc::Receiver<Vec<(f64, f32)>>,
}

impl CantusApp {
    /// Where a bake reaches: the time selection, else the loop, else the whole song.
    pub fn bake_span(&self) -> (Ticks, Ticks) {
        if let Some((a, b)) = self.time_selection.filter(|(a, b)| b > a) {
            return (a, b);
        }
        let p = self.project();
        if let Some(l) = p
            .loop_region
            .as_ref()
            .filter(|l| p.loop_enabled && l.end > l.start)
        {
            return (l.start, l.end);
        }
        let bar = crate::views::bars::bar_len(p, Ticks::ZERO);
        (Ticks::ZERO, p.content_end().max(bar))
    }

    /// Bake modulator `id`'s route into `target` (one undo step); a source that follows
    /// audio is traced first, on a worker.
    pub fn bake_route(&mut self, id: ModulatorId, target: &AutoTarget) -> bool {
        if self.project().modulator(id).is_none() {
            return false;
        }
        let span = self.bake_span();
        if !cantus_core::modulator::bake::needs_trace(self.project(), id) {
            return self.finish_bake(id, target, span, None);
        }
        // One bake at a time: a second one while the first traces, or waits for its
        // samples, would throw the first away and end its render instances under it
        // (round 29, review isolation-4 #5, #21).
        if self.modulators.pending_bake.is_some() || self.warp.pending_bake.is_some() {
            let job = t("isolation.render.bake");
            self.set_error(tf("isolation.render.busy", &[("job", &job)]));
            return false;
        }
        // While renders are on their way (a warp, a pitch edit, a file loading), the bake
        // waits for them beside the window, as an export does — it never blocks the frame
        // (round 29's review); started again once they are in.
        if self.engine.samples.loading() {
            self.warp.pending_bake = Some((id, target.clone(), self.epoch));
            self.set_status(crate::i18n::t("warp.status.bake_waiting"));
            return true;
        }
        // A freeze or a bounce rendering holds the hosted plug-ins' render instances: one
        // set at a time (round 29, review 1 — whichever ended first let the other's go).
        if !crate::freeze::yield_refresh(self) {
            self.set_error(t("freeze.bake_waits"));
            return false;
        }
        // An export renders through the same plug-in instances: one render at a time
        // (round 29, review isolation-3 #5, #10, #13, #48).
        if self.export.busy() {
            let job = t("isolation.render.export");
            self.set_error(tf("isolation.render.busy", &[("job", &job)]));
            return false;
        }
        // Rendered offline over the stretch, the engine tracing this modulator.
        let rate = self.engine.sample_rate.round() as u32;
        let late = self.engine.renders_late();
        let snapshot = self
            .engine
            .compile_for_render(self.bus.project(), self.bus.revision());
        self.say_late(late);
        let opts = cantus_engine::RenderOptions {
            sample_rate: rate,
            start: span.0,
            end: Some(span.1),
            tail_seconds: 0.0,
            ..Default::default()
        };
        let units = self
            .plugins
            .render_units(self.bus.project(), f64::from(rate));
        let instruments = self
            .plugins
            .render_instruments(self.bus.project(), f64::from(rate));
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("cantus-bake".into())
            .spawn(move || {
                let trace =
                    cantus_engine::modulator::trace(snapshot, &opts, units, instruments, id);
                let _ = tx.send(trace);
            });
        if spawned.is_err() {
            self.plugins.finish_render();
            self.set_error(t("mods.status.bake_failed"));
            return false;
        }
        self.modulators.pending_bake = Some(Pending {
            id,
            target: target.clone(),
            span,
            revision: self.bus.revision(),
            rx,
        });
        self.set_status(t("mods.status.baking"));
        true
    }

    /// A trace that came back: make its bake, unless the project moved meanwhile.
    pub fn poll_bake(&mut self) {
        let Some(p) = self.modulators.pending_bake.as_ref() else {
            return;
        };
        let trace = match p.rx.try_recv() {
            Ok(t) => t,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
        };
        let Some(p) = self.modulators.pending_bake.take() else {
            return;
        };
        // A plug-in that stopped working while the modulator was traced: the trace lacks
        // it, so nothing is baked (round 29, review #6).
        let lost = self.plugins.render_failures();
        self.plugins.finish_render();
        let (losses, warnings) = crate::views::plugin_notice::render_words(&lost, "bake", "");
        if let Some(text) = losses {
            self.set_error(text);
            return;
        }
        if let Some(w) = warnings {
            self.set_error(w);
        }
        if trace.is_empty() || p.revision != self.bus.revision() {
            self.set_error(t("mods.status.bake_moved"));
            return;
        }
        self.finish_bake(p.id, &p.target, p.span, Some(&trace));
    }

    /// The bake itself: the curve, the clips it plays over, the route gone — one step.
    fn finish_bake(
        &mut self,
        id: ModulatorId,
        target: &AutoTarget,
        (from, to): (Ticks, Ticks),
        trace: Option<&[(f64, f32)]>,
    ) -> bool {
        let base = target
            .is_plugin()
            .then(|| self.plugin_pos(target))
            .flatten();
        let selected = self.selected_track;
        let project = self.bus.project();
        let Some(points) = bake(project, id, target, from, to, from, base, trace) else {
            self.set_error(t("mods.status.bake_failed"));
            return false;
        };
        let Some((_, _, m)) = project.modulator(id) else {
            return false;
        };
        let Some(track) = crate::automation::host_track(project, target, selected) else {
            self.set_error(t("auto.status.no_track"));
            return false;
        };
        let mut edited = m.clone();
        edited.routes.retain(|r| &r.target != target);
        let mut commands = vec![Command::SetModulator {
            modulator: Box::new(edited),
        }];
        // Clips of the parameter that start inside the stretch would play over the bake.
        for tr in &project.tracks {
            for c in &tr.clips {
                let mine = c.automation.as_ref().is_some_and(|a| &a.target == target);
                if !mine || c.start < from || c.start >= to {
                    continue;
                }
                if c.end() <= to {
                    commands.push(Command::RemoveClip {
                        track: tr.id,
                        clip: c.id,
                    });
                } else {
                    commands.push(Command::SetClipOffset {
                        track: tr.id,
                        clip: c.id,
                        offset: c.offset + (to - c.start),
                    });
                    commands.push(Command::SetClipBounds {
                        track: tr.id,
                        clip: c.id,
                        start: to,
                        length: c.end() - to,
                    });
                }
            }
        }
        let name = tf(
            "mods.bake.clip",
            &[(
                "name",
                &crate::views::modulator::mod_name(project, id).unwrap_or_default(),
            )],
        );
        let open = project.track(track).is_some_and(|t| t.automation_open);
        let mut clip = self.bus.new_clip(name, from, to - from);
        clip.automation = Some(Automation::new(target.clone(), points));
        let clip_id = clip.id;
        commands.push(Command::InsertClip {
            track,
            clip: Box::new(clip),
        });
        if !open {
            commands.push(Command::SetTrackAutomationOpen {
                id: track,
                open: true,
            });
        }
        let ok = self.exec(Command::Batch {
            label_key: "mods.cmd.bake".into(),
            commands,
        });
        if ok {
            self.select_clip(track, clip_id);
            self.set_status(t("mods.status.baked"));
        }
        ok
    }
}
