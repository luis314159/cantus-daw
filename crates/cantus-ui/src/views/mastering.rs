//! The mastering assistant's panel (round 34, ADR 0084): opened from the master strip's
//! menu and the master chain's *Add effect* list. It listens to the mix as it reaches the
//! master (or to the loop), shows what it hears against the chosen platform and genre — or
//! a reference track — proposes a chain of Cantus's own effects with why each is there,
//! applies it as one undo step, and plays the mix and the proposed master one against the
//! other at the same loudness (A/B), through the preview voice, so nothing in the project
//! changes to compare them.

use super::listening::{self, Range};
use super::mastering::{self, Proposal, Target};
use super::run::assist_project;
use super::targets;
use crate::app::CantusApp;
use cantus_core::{Project, SampleData};
use cantus_dsp::speculum::ear::Reading;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

#[path = "mastering_draw.rs"]
mod draw;

/// What the panel heard and proposed, sent back by its worker.
pub struct Done {
    pub proposal: Proposal,
    /// The mix as it reached the master, and the proposed master (the chain run over it,
    /// the master's fader after it), interleaved stereo.
    pub mix: Arc<Vec<f32>>,
    pub mastered: Arc<Vec<f32>>,
    pub sample_rate: u32,
    /// Integrated loudness of each, for the A/B's match.
    pub mix_lufs: f64,
    pub mastered_lufs: f64,
    /// Whether it was listened before the master's chain (to replace it).
    pub replace: bool,
}

/// A worker's run.
pub struct Running {
    pub progress: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
    pub rx: mpsc::Receiver<Result<Done, String>>,
}

/// Which side the A/B plays.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    Mix,
    Master,
}

/// The panel's state (kept in `CantusApp::listen`).
#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub platform: usize,
    pub curve: usize,
    pub reference: Option<(PathBuf, Reading)>,
    /// The loop instead of the whole song.
    pub loop_only: bool,
    /// Add after the master's own chain instead of replacing it.
    pub append: bool,
    pub running: Option<Running>,
    pub done: Option<Done>,
    pub error: Option<String>,
    /// The A/B playing: which side, and when the passage started (to switch in place).
    pub ab: Option<(Side, std::time::Instant)>,
    /// A choice's list open under its field: which (the platform, the balance) and where.
    pub menu: Option<(Pick, egui::Pos2)>,
    /// The A/B was stopped without the engine at hand (another project opened): the next
    /// frame silences the preview (review 2).
    pub stop_preview: bool,
    /// Scroll the body here at the next frame (the shots' way to its end).
    pub scroll: Option<f32>,
}

/// The two fields that open a list.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Platform,
    Curve,
}

/// Open the panel.
pub fn open(app: &mut CantusApp) {
    app.listen.panel.open = true;
}

/// The target the panel's choices make.
pub fn target(p: &Panel) -> Target {
    let platform = &targets::PLATFORMS[p.platform.min(targets::PLATFORMS.len() - 1)];
    match &p.reference {
        Some((_, r)) => Target::reference(platform, r),
        None => Target::genre(
            platform,
            &targets::CURVES[p.curve.min(targets::CURVES.len() - 1)],
        ),
    }
}

/// The range the panel listens to: the loop when asked and set, else the whole song.
fn range(project: &Project, loop_only: bool) -> Range {
    match project.loop_region.clone().filter(|_| loop_only) {
        Some(l) => Range {
            start: l.start,
            end: Some(l.end),
        },
        None => Range {
            start: cantus_core::Ticks::ZERO,
            end: None,
        },
    }
}

/// Listen and propose, on a worker.
pub fn start(app: &mut CantusApp) {
    stop_ab(app);
    let replace = !(app.listen.panel.append && !app.project().master.effects.is_empty());
    let project = app.project().clone();
    let heard = assist_project(&project, replace);
    let r = range(&project, app.listen.panel.loop_only);
    // Too long for the assistant: said in the interface's words, with what to do.
    let rate = app.engine.sample_rate.round() as u32;
    if let Err(t) = listening::length_check(&project, r, true, rate) {
        // Minutes and seconds, and what to do for what was chosen: the loop to set, or to
        // shorten (review 3).
        let looped = app.listen.panel.loop_only && project.loop_region.is_some();
        app.listen.panel.error = Some(crate::i18n::tf(
            if looped {
                "listen.panel.too_long_loop"
            } else {
                "listen.panel.too_long"
            },
            &[
                ("length", &listening::min_sec(t.minutes)),
                ("most", &listening::min_sec(t.most_minutes)),
            ],
        ));
        return;
    }
    let prep = match listening::prepare(app, &heard, r, Vec::new(), true, 0.0) {
        Ok(p) => p,
        Err(e) => {
            app.listen.panel.error = Some(e.to_string());
            return;
        }
    };
    let target = target(&app.listen.panel);
    let post = mastering::Post::of(&project.master);
    let progress = Arc::new(AtomicU32::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let (p2, c2) = (progress.clone(), cancel.clone());
    let mut scratch = project;
    let spawned = std::thread::Builder::new()
        .name("cantus-mastering".into())
        .spawn(move || {
            let set = |f: f32| p2.store((f * 0.6).to_bits(), Ordering::Relaxed);
            let cancelled = || c2.load(Ordering::Relaxed);
            let cx = cantus_ai::JobCx {
                progress: &set,
                cancelled: &cancelled,
            };
            let r = (|| -> Result<Done, String> {
                let h = prep.run(&cx).map_err(|e| e.to_string())?;
                let mut new_slot = |k| scratch.make_effect(k);
                let proposal = mastering::propose(
                    &h.audio,
                    h.sample_rate,
                    &target,
                    &post,
                    &mut new_slot,
                    &cancelled,
                )
                .ok_or_else(String::new)?;
                p2.store(0.95f32.to_bits(), Ordering::Relaxed);
                let mastered = mastering::trial(&proposal.chain, &h.audio, h.sample_rate, &post);
                // The proposal read both already (the mix before, the trial after): no
                // whole-length copies to read them again, none to hand them over (review 3).
                Ok(Done {
                    mix_lufs: proposal.before.integrated_lufs.unwrap_or(-70.0),
                    mastered_lufs: proposal.predicted.integrated_lufs.unwrap_or(-70.0),
                    mix: Arc::new(h.audio),
                    mastered: Arc::new(mastered),
                    sample_rate: h.sample_rate,
                    proposal,
                    replace,
                })
            })();
            let _ = tx.send(r);
        });
    match spawned {
        Ok(_) => {
            let panel = &mut app.listen.panel;
            panel.running = Some(Running {
                progress,
                cancel,
                rx,
            });
            panel.error = None;
        }
        Err(e) => app.listen.panel.error = Some(e.to_string()),
    }
}

/// Every frame: the service's chores, the worker's answer, and the panel while open.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    poll(app, ctx);
    if app.listen.panel.open {
        draw::window(app, ctx);
    } else if app.listen.panel.ab.is_some() {
        stop_ab(app);
    }
}

/// The service's chores and the worker's answer (no drawing).
pub fn poll(app: &mut CantusApp, ctx: &egui::Context) {
    app.listen.sync_epoch(app.epoch);
    if std::mem::take(&mut app.listen.panel.stop_preview) {
        app.engine.preview(None);
    }
    app.listen.frame(&mut app.plugins);
    let mut finished = None;
    if let Some(r) = &app.listen.panel.running {
        match r.rx.try_recv() {
            Ok(done) => finished = Some(done),
            Err(mpsc::TryRecvError::Disconnected) => finished = Some(Err(String::new())),
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100))
            }
        }
    }
    if let Some(done) = finished {
        let panel = &mut app.listen.panel;
        panel.running = None;
        match done {
            Ok(d) => {
                panel.done = Some(d);
                panel.error = None;
            }
            // An empty reason is a stop.
            Err(e) if e.is_empty() => {}
            Err(e) => panel.error = Some(e),
        }
    }
}

/// Put the proposal on the master: one undo step.
pub fn apply(app: &mut CantusApp) {
    let Some(d) = app.listen.panel.done.as_ref() else {
        return;
    };
    let mut proposal = d.proposal.clone();
    for slot in &mut proposal.chain {
        slot.id = cantus_core::EffectId(app.bus.alloc_id());
    }
    let cmd = mastering::apply_command(app.project(), &proposal, d.replace);
    app.exec(cmd);
    stop_ab(app);
}

/// Play one side of the A/B from where the passage is now (from its start the first time),
/// both at the quieter one's loudness.
pub fn play_ab(app: &mut CantusApp, side: Side) {
    let Some(d) = app.listen.panel.done.as_ref() else {
        return;
    };
    let quiet = d.mix_lufs.min(d.mastered_lufs);
    let (audio, lufs) = match side {
        Side::Mix => (&d.mix, d.mix_lufs),
        Side::Master => (&d.mastered, d.mastered_lufs),
    };
    let gain = cantus_dsp::math::db_to_gain((quiet - lufs) as f32);
    let now = std::time::Instant::now();
    let started = app.listen.panel.ab.map_or(now, |(_, t)| t);
    let frames = audio.len() / 2;
    let mut from = (started.elapsed().as_secs_f64() * d.sample_rate as f64) as usize;
    if from >= frames {
        from = 0;
    }
    let started = now - std::time::Duration::from_secs_f64(from as f64 / d.sample_rate as f64);
    let sample = SampleData {
        rate: d.sample_rate,
        channels: 2,
        frames: audio[from * 2..].into(),
        stream: None,
    };
    app.engine.preview(Some((Arc::new(sample), gain)));
    app.listen.panel.ab = Some((side, started));
}

/// Stop the A/B.
pub fn stop_ab(app: &mut CantusApp) {
    if app.listen.panel.ab.take().is_some() {
        app.engine.preview(None);
    }
}
