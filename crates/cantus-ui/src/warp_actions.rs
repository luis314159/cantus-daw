//! The warp verbs (ADR 0061): every one a `SetClipWarp` (or a batch of them), one undo
//! step — merged per drag where the edit is a drag (`WarpOp::merges`).

use super::{warp_from, QuantiseSettings};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use cantus_core::warp::{Warp, WarpMode, WarpOp, WarpTail};
use cantus_core::{Command, MergeKey, Ticks, PPQ};
use std::path::PathBuf;

/// A bounce of a warped clip being written on its own thread.
#[derive(Debug)]
pub struct Bounce {
    pub r: ClipRef,
    pub name: String,
    pub(crate) rx: std::sync::mpsc::Receiver<Result<Written, String>>,
}

/// A file a bounce wrote.
#[derive(Clone, Debug)]
pub struct Written {
    pub path: PathBuf,
    pub rate: u32,
    pub channels: u8,
    pub frames: u64,
}

/// The merge key of a drag on clip `r`'s warp (one undo step per gesture).
pub fn merge_key(r: ClipRef, op: WarpOp) -> MergeKey {
    MergeKey::of(&("warp", r.0 .0, r.1 .0, op.label_key()))
}

impl CantusApp {
    /// Clip `r`'s warp, if it has one.
    pub fn clip_warp(&self, r: ClipRef) -> Option<&Warp> {
        self.project()
            .track(r.0)?
            .clip(r.1)?
            .audio
            .as_ref()?
            .warp
            .as_deref()
    }

    /// The warp an edit of clip `r` starts from: its own, or the one that plays exactly
    /// as it does now (its fine offset in a marker on content tick 0), with what the
    /// analysis found when it is in.
    pub fn warp_to_edit(&self, r: ClipRef) -> Option<Warp> {
        if let Some(w) = self.clip_warp(r) {
            return Some(w.clone());
        }
        let c = self.project().track(r.0)?.clip(r.1)?;
        let a = c.audio.as_ref()?;
        let s = self.project().audio_source(a.source)?;
        let mut w = Warp::identity(s.sample_rate.max(1), self.project().tempo_bpm, a.slip_frames);
        if let Some(an) = self.warp_analysis(r) {
            if let Some(d) = an.grid.first_downbeat() {
                w.detected = Some(cantus_core::warp::Detected {
                    bpm: an.bpm(),
                    confidence: an.grid.confidence,
                    downbeat: d,
                    bars: an.grid.bars,
                });
            }
        }
        Some(w)
    }

    /// Write clip `r`'s warp: one undo step, or merged into the gesture's (`merge`).
    pub fn commit_warp(&mut self, r: ClipRef, warp: Option<Warp>, op: WarpOp, merge: bool) -> bool {
        if self.clip_warp(r) == warp.as_ref() {
            return false;
        }
        let cmd = Command::SetClipWarp {
            track: r.0,
            clip: r.1,
            warp: warp.map(Box::new),
            op,
        };
        if merge {
            self.exec_merging(cmd, merge_key(r, op))
        } else {
            self.exec(cmd)
        }
    }

    /// Edit clip `r`'s warp (or start one) with `f`, as one step.
    pub fn edit_warp(&mut self, r: ClipRef, op: WarpOp, f: impl FnOnce(&mut Warp)) -> bool {
        let Some(mut w) = self.warp_to_edit(r) else {
            return false;
        };
        f(&mut w);
        self.commit_warp(r, Some(w), op, false)
    }

    pub fn set_warp_mode(&mut self, r: ClipRef, mode: WarpMode) -> bool {
        self.edit_warp(r, WarpOp::Mode, |w| w.mode = mode)
    }

    /// The transposition, cents (a drag merges).
    pub fn set_warp_cents(&mut self, r: ClipRef, cents: i32, merge: bool) -> bool {
        let Some(mut w) = self.warp_to_edit(r) else {
            return false;
        };
        w.cents = cents.clamp(-cantus_core::warp::MAX_CENTS, cantus_core::warp::MAX_CENTS);
        self.commit_warp(r, Some(w), WarpOp::Transpose, merge)
    }

    pub fn set_warp_formants(&mut self, r: ClipRef, on: bool) -> bool {
        self.edit_warp(r, WarpOp::Formants, |w| w.formants = on)
    }

    pub fn set_warp_tail(&mut self, r: ClipRef, tail: WarpTail) -> bool {
        self.edit_warp(r, WarpOp::Tail, |w| w.tail = tail)
    }

    /// *Follow the project tempo*: on, the clip is warped (its markers hold it to the beat);
    /// off, it plays at its natural speed and keeps only its transposition.
    pub fn set_warp_follow(&mut self, r: ClipRef, on: bool) -> bool {
        self.edit_warp(r, WarpOp::Follow, |w| w.follow = on)
    }

    /// The source's tempo (a drag merges): the clip plays that much faster or slower.
    pub fn set_warp_tempo(&mut self, r: ClipRef, bpm: f64, merge: bool) -> bool {
        let Some(w) = self.warp_to_edit(r) else {
            return false;
        };
        let next = w.with_tempo(bpm);
        self.commit_warp(r, Some(next), WarpOp::Tempo, merge)
    }

    /// A marker on content tick `tick` (on the transient there, when `onset` names one).
    pub fn add_warp_marker(&mut self, r: ClipRef, tick: Ticks, onset: Option<f64>) -> bool {
        let Some(w) = self.warp_to_edit(r) else {
            return false;
        };
        let next = match onset {
            Some(f) => w.add_marker_at_frame(f),
            None => w.add_marker(tick),
        };
        match next {
            Some(n) => {
                let i = n.markers.iter().position(|m| !w.markers.contains(m));
                let ok = self.commit_warp(r, Some(n), WarpOp::AddMarker, false);
                if ok {
                    self.warp.selected = i.map(|i| (r, i));
                }
                ok
            }
            None => false,
        }
    }

    pub fn remove_warp_marker(&mut self, r: ClipRef, i: usize) -> bool {
        let Some(w) = self.clip_warp(r).cloned() else {
            return false;
        };
        self.warp.selected = None;
        self.commit_warp(r, Some(w.remove_marker(i)), WarpOp::RemoveMarker, false)
    }

    /// *Detect tempo* / warp by the analysis: markers on every bar of the grid it found.
    pub fn warp_by_analysis(&mut self, r: ClipRef) -> bool {
        let Some(a) = self.warp_analysis(r) else {
            self.set_status(t("warp.status.analysing"));
            return false;
        };
        let mode = self.clip_warp(r).map_or_else(|| super::mode_for(&a), |w| w.mode);
        let bpb = self.project().time_signature.numerator.max(1) as u32;
        let Some(mut w) = warp_from(&a, bpb, mode) else {
            self.set_status(t("warp.status.no_tempo"));
            return false;
        };
        if let Some(old) = self.clip_warp(r) {
            w.cents = old.cents;
            w.formants = old.formants;
            w.tail = old.tail;
        }
        let ok = self.commit_warp(r, Some(w), WarpOp::Set, false);
        if ok {
            self.set_status(tf("warp.status.detected", &[("bpm", &format!("{:.2}", a.bpm()))]));
        }
        ok
    }

    /// *Reset warp*: no warp at all — the clip plays its source at natural speed again.
    pub fn reset_warp(&mut self, r: ClipRef) -> bool {
        self.warp.selected = None;
        self.commit_warp(r, None, WarpOp::Reset, false)
    }

    /// *Set the project tempo from this clip*: the tempo the clip's source plays at.
    pub fn project_tempo_from_clip(&mut self, r: ClipRef) -> bool {
        let bpm = match self.clip_warp(r) {
            Some(w) => w.tempo(),
            None => match self.warp_analysis(r) {
                Some(a) if a.bpm() > 0.0 => a.bpm(),
                _ => {
                    self.set_status(t("warp.status.analysing"));
                    return false;
                }
            },
        };
        let bpm = cantus_core::time::clamp_bpm((bpm * 100.0).round() / 100.0);
        self.exec(Command::SetTempo { bpm })
    }

    /// *Warp from here*: from the beat at or after timeline tick `at`, the clip's markers
    /// laid again on the analysis's bars (that beat kept where it is heard now); the
    /// markers before it stay.
    pub fn warp_from_here(&mut self, r: ClipRef, at: Ticks) -> bool {
        let (Some(a), Some(w)) = (self.warp_analysis(r), self.warp_to_edit(r)) else {
            self.set_status(t("warp.status.analysing"));
            return false;
        };
        let Some(c) = self.project().track(r.0).and_then(|t| t.clip(r.1)) else {
            return false;
        };
        let content = (at - c.start + c.offset).0 as f64;
        let here = w.frame_at(content);
        let Some(first) = a.grid.beats.iter().position(|b| *b >= here - 1.0) else {
            return false;
        };
        let t0 = w.tick_at(a.grid.beats[first]).round() as i64;
        let bpb = self.project().time_signature.numerator.max(1) as usize;
        let mut next = w.clone();
        next.markers.retain(|m| m.tick.0 < t0);
        for (k, &f) in a.grid.beats.iter().enumerate().skip(first).step_by(bpb) {
            let tick = Ticks(t0 + ((k - first) as i64) * PPQ);
            if let Some(n) = next.with_marker(f, tick) {
                next = n;
            }
        }
        self.commit_warp(r, Some(next), WarpOp::Set, false)
    }

    /// Quantise audio: each clip's transients (those at least `threshold` of its
    /// strongest) pulled to the grid by `strength` — one undo step for all of them.
    pub fn quantise_audio(&mut self, clips: &[ClipRef], q: QuantiseSettings) -> usize {
        let mut commands = Vec::new();
        let mut waiting = false;
        for &r in clips {
            let (Some(a), Some(w)) = (self.warp_analysis(r), self.warp_to_edit(r)) else {
                waiting |= self.warp_analysing(r);
                continue;
            };
            let Some(c) = self.project().track(r.0).and_then(|t| t.clip(r.1)) else {
                continue;
            };
            let top = a.strengths.iter().fold(0.0f32, |m, v| m.max(*v));
            let hits: Vec<f64> = a
                .onsets
                .iter()
                .zip(&a.strengths)
                .filter(|(_, s)| **s >= q.threshold * top)
                .map(|(f, _)| *f)
                .collect();
            let next = w.quantise_from(
                &hits,
                q.grid,
                c.start - c.offset,
                q.strength,
                c.offset,
                c.offset + c.length,
            );
            if self.clip_warp(r) != Some(&next) {
                commands.push(Command::SetClipWarp {
                    track: r.0,
                    clip: r.1,
                    warp: Some(Box::new(next)),
                    op: WarpOp::Quantise,
                });
            }
        }
        if waiting {
            self.set_status(t("warp.status.analysing"));
        }
        let n = commands.len();
        if n > 0 {
            self.exec(Command::Batch {
                label_key: "cmd.warp.quantise".into(),
                commands,
            });
        }
        n
    }
}

impl CantusApp {
    /// The keys a selected warp marker takes before anything else: Del removes it (the
    /// clip stays), Esc lets it go. Returns whether the key was taken.
    pub fn warp_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        let Some((r, i)) = self.warp.selected else {
            return false;
        };
        if self.clip_warp(r).is_none_or(|w| i >= w.markers.len()) {
            self.warp.selected = None;
            return false;
        }
        match action {
            Action::Delete => {
                self.remove_warp_marker(r, i);
                true
            }
            Action::ClearSelection => {
                self.warp.selected = None;
                false
            }
            _ => false,
        }
    }
}
