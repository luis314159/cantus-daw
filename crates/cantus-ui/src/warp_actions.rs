//! The warp verbs (ADR 0061): every one a `SetClipWarp` (or a batch holding one), one undo
//! step — merged per drag where the edit is a drag (`WarpOp::merges`). A verb that lays the
//! warp again, or turns a clip to follow the tempo or back, keeps every window sharing the
//! warp (the clip, its linked copies) on the audio it plays at its start (round 29's
//! reviews, [`Keep`]).

use super::{warp_from, QuantiseSettings};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use cantus_core::warp::{Detected, Warp, WarpMode, WarpOp, WarpTail};
use cantus_core::{Command, MergeKey, Ticks, PPQ};
use std::path::PathBuf;

/// A bounce of a warped clip being written on its own thread.
#[derive(Debug)]
pub struct Bounce {
    pub r: ClipRef,
    pub name: String,
    pub(crate) rx: std::sync::mpsc::Receiver<Result<Written, String>>,
    /// What it was made from: the project's epoch and the clip as it was (round 29's second
    /// review — a bounce never lands on a clip edited since, or on another project's).
    pub(crate) epoch: u64,
    pub(crate) clip: cantus_core::Clip,
    /// The tempo it was rendered under (round 29's third review: a tempo edit meanwhile
    /// leaves a following clip as it was, yet the render is laid for the old map).
    pub(crate) tempo: cantus_core::TempoMap,
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

/// Where a warping gesture on a clip starts (a marker, a stretch, *Quantise audio*, its
/// tempo, *Follow*): [`CantusApp::warp_start`].
#[derive(Clone, Debug)]
pub struct WarpStart {
    /// The warp it starts from, following the tempo.
    pub warp: Warp,
    /// Added to a content tick counted from the clip's offset now, it is counted from the
    /// offset the clip takes with [`WarpStart::warp`] (zero unless the warp turns to follow
    /// from markers it kept, which keeps the window's audio by its offset).
    pub shift: Ticks,
    /// `Some` while the clip does not follow yet: the warp its windows are kept on as the
    /// gesture's first step turns it to follow.
    pub base: Option<Warp>,
}

/// What a warp written keeps each window sharing it on (round 29's reviews).
#[derive(Clone, Copy, Debug)]
pub enum Keep<'a> {
    /// Nothing moves but what the edit moves (a marker, the content under one, a mode).
    No,
    /// The clip turns to follow from [`WarpStart::base`]: each window's start kept on that
    /// warp (on the gesture's first step; once the clip follows, nothing more).
    Base(&'a Warp),
    /// The warp laid again (detected, from here, its tempo, *Follow* off, reset): each
    /// window's start kept on the new one.
    Next,
}

/// Each window's source frame at its start, the clip's own first.
type Frames = Vec<(ClipRef, f64)>;

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

    /// What the analysis found in clip `r`'s source, as a warp keeps it.
    fn detected_of(&self, r: ClipRef) -> Option<Detected> {
        let an = self.warp_analysis(r)?;
        let downbeat = an.grid.first_downbeat()?;
        Some(Detected {
            bpm: an.bpm(),
            confidence: an.grid.confidence,
            downbeat,
            bars: an.grid.bars,
        })
    }

    /// The warp an edit of clip `r` that does not warp it starts from — a transposition,
    /// the formants, the mode, the slice ends: its own, or — not warped — one that does not
    /// follow the tempo and holds **no markers** (round 29's second review: markers there
    /// play nothing and went stale with every tempo edit), at its source's tempo — what the
    /// analysis found, else the tempo where the clip starts — with what the analysis found.
    pub fn warp_to_edit(&self, r: ClipRef) -> Option<Warp> {
        if let Some(w) = self.clip_warp(r) {
            return Some(w.clone());
        }
        let c = self.project().track(r.0)?.clip(r.1)?;
        let a = c.audio.as_ref()?;
        let s = self.project().audio_source(a.source)?;
        let detected = self.detected_of(r);
        let bpm = detected
            .map(|d| d.bpm)
            .filter(|b| *b > 0.0)
            .unwrap_or_else(|| self.project().tempo_map().bpm_at(c.start.0 as f64));
        let mut w = Warp::new(s.sample_rate.max(1), cantus_core::time::clamp_bpm(bpm));
        w.follow = false;
        w.detected = detected;
        Some(w)
    }

    /// Where a gesture that warps clip `r` starts ([`WarpStart`]): its own warp when it
    /// follows; else — not following — the markers it kept when it followed before
    /// (*Follow* off then on keeps them, round 29's second review), or, never warped to
    /// follow, markers that play it exactly as it plays now ([`Warp::natural_of`]) — its
    /// other settings kept, so nothing moves until a marker is dragged.
    pub fn warp_start(&self, r: ClipRef) -> Option<WarpStart> {
        let own = self.clip_warp(r).cloned();
        if let Some(w) = own.as_ref().filter(|w| w.follow) {
            return Some(WarpStart {
                warp: w.clone(),
                shift: Ticks::ZERO,
                base: None,
            });
        }
        let base = match own {
            Some(mut w) if !w.markers.is_empty() => {
                w.follow = true;
                w
            }
            Some(w) => {
                let mut n = self.natural_warp(r)?;
                n.mode = w.mode;
                n.cents = w.cents;
                n.formants = w.formants;
                n.tail = w.tail;
                n.detected = w.detected.or_else(|| self.detected_of(r));
                n
            }
            None => {
                let mut n = self.natural_warp(r)?;
                n.detected = self.detected_of(r);
                n
            }
        };
        let c = self.project().track(r.0)?.clip(r.1)?;
        let shift = self
            .start_frames(r, true)
            .first()
            .map_or(Ticks::ZERO, |&(_, f)| {
                Ticks(base.tick_at(f).round() as i64).max(Ticks::ZERO) - c.offset
            });
        Some(WarpStart {
            warp: base.clone(),
            shift,
            base: Some(base),
        })
    }

    /// The warp a gesture that warps clip `r` starts from ([`Self::warp_start`]).
    pub fn warp_to_warp(&self, r: ClipRef) -> Option<Warp> {
        self.warp_start(r).map(|s| s.warp)
    }

    /// The warp that plays clip `r` as it plays unwarped, following the tempo.
    fn natural_warp(&self, r: ClipRef) -> Option<Warp> {
        let c = self.project().track(r.0)?.clip(r.1)?;
        let a = c.audio.as_ref()?;
        let s = self.project().audio_source(a.source)?;
        Some(Warp::natural_of(
            &self.project().tempo_map(),
            c.start,
            c.offset,
            c.length,
            a.slip_frames,
            s.sample_rate.max(1),
        ))
    }

    /// The source frame each window sharing clip `r`'s warp hears at its start now (the
    /// clip's own first), or the clip's alone.
    fn start_frames(&self, r: ClipRef, alone: bool) -> Frames {
        let p = self.project();
        let map = p.tempo_map();
        let Some(c) = p.track(r.0).and_then(|t| t.clip(r.1)) else {
            return Vec::new();
        };
        let mut members = vec![r];
        if let Some(link) = c.link.filter(|_| !alone) {
            members.extend(
                cantus_core::links::members(p, link)
                    .into_iter()
                    .filter(|m| *m != r),
            );
        }
        members
            .into_iter()
            .filter_map(|m| {
                let mc = p.track(m.0)?.clip(m.1)?;
                let a = mc.audio.as_ref()?;
                let rate = p.audio_source(a.source)?.sample_rate.max(1) as f64;
                let start = mc.start.0 as f64;
                let f = cantus_core::audio_time::clip_frame_at(
                    &map, mc.start, mc.offset, a, rate, start,
                );
                Some((m, f))
            })
            .collect()
    }

    /// The edits that put each window of `frames` back on its frame once `next` is the
    /// warp they share and `against` maps them (`None`: at natural speed) — the offset
    /// (whole content ticks under a warp that follows), and for a window whose region is
    /// its own (`alone`, not shared with linked copies) the rest in its fine offset.
    fn kept_commands(
        &self,
        frames: &[(ClipRef, f64)],
        against: Option<&Warp>,
        next: Option<&Warp>,
        alone: bool,
    ) -> Vec<Command> {
        let p = self.project();
        let map = p.tempo_map();
        let mut out = Vec::new();
        for &(m, frame) in frames {
            let Some(mc) = p.track(m.0).and_then(|t| t.clip(m.1)) else {
                continue;
            };
            let Some(a) = mc.audio.as_ref() else {
                continue;
            };
            let rate = p
                .audio_source(a.source)
                .map_or(48_000.0, |s| s.sample_rate.max(1) as f64);
            let offset = match against.filter(|w| w.follow) {
                Some(w) => Ticks(w.tick_at(frame).round() as i64).max(Ticks::ZERO),
                None => {
                    let secs = (frame - a.slip_frames as f64) / rate;
                    if mc.link.is_some() && !alone {
                        // The fine offset is the group's: the offset alone.
                        cantus_core::audio_time::offset_for(&map, mc.start, secs)
                    } else {
                        let (o, slip) = cantus_core::audio_time::anchor(
                            &map,
                            mc.start,
                            secs,
                            a.slip_frames,
                            rate,
                        );
                        if slip != a.slip_frames {
                            let mut region = a.clone();
                            region.warp = next.cloned().map(Box::new);
                            region.slip_frames = slip;
                            out.push(Command::SetClipAudio {
                                track: m.0,
                                clip: m.1,
                                audio: Some(region),
                            });
                        }
                        o
                    }
                }
            };
            if offset != mc.offset {
                out.push(Command::SetClipOffset {
                    track: m.0,
                    clip: m.1,
                    offset,
                });
            }
        }
        out
    }

    /// Whether a linked copy of clip `r` sits under another tempo function than `r` does:
    /// its content read at natural speed (each tick through the map from its own content
    /// origin) is not `r`'s — so a first warping edit, which starts both from one warp
    /// laid at `r`'s place, would change how the copy plays (round 29's second review).
    /// Within half a tick (and a frame) they are one: the tempo edits' re-anchoring keeps a
    /// linked window to that (third review: a Follow on after Follow off unlinked a loop
    /// pair under one tempo, comparing the kept markers with natural speed).
    fn copies_differ(&self, r: ClipRef) -> bool {
        let p = self.project();
        let map = p.tempo_map();
        let Some(rc) = p.track(r.0).and_then(|t| t.clip(r.1)) else {
            return false;
        };
        let Some(link) = rc.link else {
            return false;
        };
        let reading = |c: &cantus_core::Clip, t: f64| {
            let a = c.audio.as_ref()?;
            let rate = p.audio_source(a.source)?.sample_rate.max(1) as f64;
            let origin = (c.start - c.offset).0 as f64;
            Some(cantus_core::audio_time::frame_at(
                &map,
                c.start,
                c.offset,
                a.slip_frames as f64,
                rate,
                origin + t,
            ))
        };
        for m in cantus_core::links::members(p, link) {
            if m == r {
                continue;
            }
            let Some(mc) = p.track(m.0).and_then(|t| t.clip(m.1)) else {
                continue;
            };
            let Some(rate) = mc
                .audio
                .as_ref()
                .and_then(|a| p.audio_source(a.source))
                .map(|s| s.sample_rate.max(1) as f64)
            else {
                continue;
            };
            let s = mc.start.0 as f64;
            let room = 0.5 * map.seconds_between_f64(s, s + 1.0) * rate + 1.0;
            const PROBES: usize = 64;
            for k in 0..=PROBES {
                let t = mc.offset.0 as f64 + mc.length.0 as f64 * k as f64 / PROBES as f64;
                match (reading(mc, t), reading(rc, t)) {
                    (Some(x), Some(y)) if (x - y).abs() <= room => {}
                    _ => return true,
                }
            }
        }
        false
    }

    /// The commands that write `next` as clip `r`'s warp, keeping windows as `keep` says
    /// (from `frames`, each window's frame before a drag that keeps them, when given). A
    /// linked copy whose first warping edit would change how its copies play is made unique
    /// first (round 29's second review). `None` when nothing changes.
    fn warp_commands(
        &self,
        r: ClipRef,
        next: Option<Warp>,
        op: WarpOp,
        keep: Keep,
        frames: Option<Frames>,
    ) -> Option<Vec<Command>> {
        let cur = self.clip_warp(r);
        if cur == next.as_ref() {
            return None;
        }
        let follows = cur.is_some_and(|w| w.follow);
        let natural = |w: Option<&Warp>| w.is_none_or(|w| !w.follow);
        let against: Option<Option<&Warp>> = match keep {
            Keep::No => None,
            Keep::Base(b) if !follows => Some(Some(b)),
            Keep::Base(_) => None,
            Keep::Next if natural(cur) && natural(next.as_ref()) => None,
            Keep::Next => Some(next.as_ref()),
        };
        let linked = self
            .project()
            .track(r.0)
            .and_then(|t| t.clip(r.1))
            .is_some_and(|c| c.link.is_some());
        let turning = !follows && next.as_ref().is_some_and(|w| w.follow);
        let unlink = turning && linked && self.copies_differ(r);
        let mut out = Vec::new();
        if unlink {
            out.push(Command::SetClipLink {
                track: r.0,
                clip: r.1,
                link: None,
            });
        }
        out.push(Command::SetClipWarp {
            track: r.0,
            clip: r.1,
            warp: next.clone().map(Box::new),
            op,
        });
        if let Some(against) = against {
            let frames = frames.unwrap_or_else(|| self.start_frames(r, unlink));
            let alone = unlink || !linked;
            out.extend(self.kept_commands(&frames, against, next.as_ref(), alone));
        }
        Some(out)
    }

    /// One undo step (or merged into the gesture's) of `commands`.
    fn exec_warp_step(
        &mut self,
        r: ClipRef,
        op: WarpOp,
        merge: bool,
        mut commands: Vec<Command>,
    ) -> bool {
        let cmd = if commands.len() == 1 {
            commands.pop().expect("one")
        } else {
            Command::Batch {
                label_key: op.label_key().into(),
                commands,
            }
        };
        if merge {
            self.exec_merging(cmd, merge_key(r, op))
        } else {
            self.exec(cmd)
        }
    }

    /// Write clip `r`'s warp, nothing else kept: one undo step, or merged into the
    /// gesture's (`merge`).
    pub fn commit_warp(&mut self, r: ClipRef, warp: Option<Warp>, op: WarpOp, merge: bool) -> bool {
        self.commit_warp_keeping(r, warp, op, merge, Keep::No)
    }

    /// Write clip `r`'s warp, keeping the windows that share it as `keep` says.
    pub fn commit_warp_keeping(
        &mut self,
        r: ClipRef,
        warp: Option<Warp>,
        op: WarpOp,
        merge: bool,
        keep: Keep,
    ) -> bool {
        match self.warp_commands(r, warp, op, keep, None) {
            Some(commands) => self.exec_warp_step(r, op, merge, commands),
            None => false,
        }
    }

    /// Whether *Bounce warped audio* has something to write for clip `r`: a warp that
    /// renders (it follows the tempo, or transposes) — one test for the menu and the
    /// inspector.
    pub fn can_bounce(&self, r: ClipRef) -> bool {
        self.clip_warp(r).is_some_and(|w| !w.natural())
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

    /// *Follow the project tempo*: on, the clip is warped — its markers hold it to the beat
    /// (the ones it kept, else ones that play it as it plays now); off, it plays at its
    /// natural speed and keeps only its transposition, its markers kept for the next time.
    /// Either way every window sharing the warp keeps the audio it plays at its start
    /// (round 29's second review).
    pub fn set_warp_follow(&mut self, r: ClipRef, on: bool) -> bool {
        if on {
            let Some(s) = self.warp_start(r) else {
                return false;
            };
            let base = s.base;
            let keep = base.as_ref().map_or(Keep::No, Keep::Base);
            return self.commit_warp_keeping(r, Some(s.warp), WarpOp::Follow, false, keep);
        }
        let Some(mut w) = self.warp_to_edit(r) else {
            return false;
        };
        w.follow = false;
        self.commit_warp_keeping(r, Some(w), WarpOp::Follow, false, Keep::Next)
    }

    /// The tempo clip `r`'s source plays at, as the inspector shows it and *Set the project
    /// tempo from this clip* takes it: a following warp's own; else what the analysis found
    /// (kept with the warp, or in the cache) — never the markers of a warp that does not
    /// follow (round 29's second review).
    pub fn clip_source_tempo(&self, r: ClipRef) -> Option<f64> {
        match self.clip_warp(r) {
            Some(w) if w.follow => Some(w.tempo()),
            w => w
                .and_then(|w| w.detected)
                .map(|d| d.bpm)
                .filter(|b| *b > 0.0)
                .or_else(|| self.warp_analysis(r).map(|a| a.bpm()).filter(|b| *b > 0.0)),
        }
    }

    /// The source's tempo (a drag merges): the clip plays that much faster or slower, every
    /// window sharing its warp kept on its audio at its start. A drag is laid from the warp
    /// and the windows' frames at its start, so nothing drifts however long it runs.
    pub fn set_warp_tempo(&mut self, r: ClipRef, bpm: f64, merge: bool) -> bool {
        let key = merge_key(r, WarpOp::Tempo);
        let memo = self
            .warp
            .relay
            .clone()
            .filter(|(m, _, _)| merge && *m == r && self.bus.merging(key));
        let (w0, frames) = match memo {
            Some((_, w0, frames)) => (w0, frames),
            None => {
                let Some(s) = self.warp_start(r) else {
                    return false;
                };
                let alone = s.base.is_some() && self.copies_differ(r);
                let frames = self.start_frames(r, alone);
                self.warp.relay = Some((r, s.warp.clone(), frames.clone()));
                (s.warp, frames)
            }
        };
        let next = w0.with_tempo(bpm);
        match self.warp_commands(r, Some(next), WarpOp::Tempo, Keep::Next, Some(frames)) {
            Some(commands) => self.exec_warp_step(r, WarpOp::Tempo, merge, commands),
            None => false,
        }
    }

    /// A marker on content tick `tick` (counted from the clip's offset now; on the
    /// transient there, when `onset` names one).
    pub fn add_warp_marker(&mut self, r: ClipRef, tick: Ticks, onset: Option<f64>) -> bool {
        let Some(s) = self.warp_start(r) else {
            return false;
        };
        let w = &s.warp;
        let next = match onset {
            Some(f) => w.add_marker_at_frame(f),
            None => w.add_marker(tick + s.shift),
        };
        let Some(n) = next else {
            return false;
        };
        let i = n.markers.iter().position(|m| !w.markers.contains(m));
        let base = s.base.clone();
        let keep = base.as_ref().map_or(Keep::No, Keep::Base);
        let ok = self.commit_warp_keeping(r, Some(n), WarpOp::AddMarker, false, keep);
        if ok {
            self.warp.selected = i.map(|i| (r, i));
        }
        ok
    }

    /// The user's marker `i` taken away (the warp's own are never offered).
    pub fn remove_warp_marker(&mut self, r: ClipRef, i: usize) -> bool {
        let Some(w) = self.clip_warp(r).cloned() else {
            return false;
        };
        self.warp.selected = None;
        if w.markers.get(i).is_none_or(|m| m.auto) {
            return false;
        }
        self.commit_warp(r, Some(w.remove_marker(i)), WarpOp::RemoveMarker, false)
    }

    /// *Detect tempo* / warp by the analysis: markers on every bar of the grid it found,
    /// every window sharing the warp kept on its audio at its start.
    pub fn warp_by_analysis(&mut self, r: ClipRef) -> bool {
        let Some(a) = self.warp_analysis(r) else {
            self.set_status(t("warp.status.analysing"));
            return false;
        };
        let mode = self
            .clip_warp(r)
            .map_or_else(|| super::mode_for(&a), |w| w.mode);
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
        let ok = self.commit_warp_keeping(r, Some(w), WarpOp::Set, false, Keep::Next);
        if ok {
            self.set_status(tf(
                "warp.status.detected",
                &[("bpm", &format!("{:.2}", a.bpm()))],
            ));
        }
        ok
    }

    /// *Reset warp*: no warp at all — the clip plays its source at natural speed again,
    /// every window sharing the warp kept on its audio at its start.
    pub fn reset_warp(&mut self, r: ClipRef) -> bool {
        self.warp.selected = None;
        self.commit_warp_keeping(r, None, WarpOp::Reset, false, Keep::Next)
    }

    /// *Set the project tempo from this clip*: the tempo its source plays at
    /// ([`Self::clip_source_tempo`]), in force where the clip starts.
    /// Whether *Set the project tempo from this clip* can do anything: a tempo the clip
    /// has, or an analysis still on its way (it says so) — never once the analysis is in
    /// without one (round 29's third review).
    pub fn can_take_tempo_from(&self, r: ClipRef) -> bool {
        self.clip_source_tempo(r).is_some()
            || (self.warp_analysis(r).is_none()
                && (self.clip_warp(r).is_some() || self.warp_analysing(r)))
    }

    pub fn project_tempo_from_clip(&mut self, r: ClipRef) -> bool {
        let Some(bpm) = self.clip_source_tempo(r) else {
            // Analysed and no tempo found, or still being analysed (round 29's third
            // review: it said "still being analysed" however long one waited).
            self.set_status(t(if self.warp_analysis(r).is_some() {
                "warp.status.no_tempo"
            } else {
                "warp.status.analysing"
            }));
            return false;
        };
        let bpm = cantus_core::time::clamp_bpm((bpm * 100.0).round() / 100.0);
        // The tempo in force where the clip starts (round 29's review: the song's first
        // tempo did nothing to a clip under a later one).
        let at = self
            .project()
            .track(r.0)
            .and_then(|t| t.clip(r.1))
            .map_or(Ticks::ZERO, |c| c.start);
        let cmd = cantus_core::tempo::edit::set_tempo_at(self.project(), at, bpm);
        self.exec(cmd)
    }

    /// *Warp from here*: from the beat at or after timeline tick `at` (where the clip was
    /// right-clicked, or the playhead), the clip's markers laid again on the analysis's
    /// bars (that beat kept where it is heard now); the markers before it stay. A place
    /// outside the clip is said, never taken as its start or end.
    pub fn warp_from_here(&mut self, r: ClipRef, at: Ticks) -> bool {
        let (Some(a), Some(s)) = (self.warp_analysis(r), self.warp_start(r)) else {
            self.set_status(t("warp.status.analysing"));
            return false;
        };
        let w = s.warp;
        let Some(c) = self.project().track(r.0).and_then(|t| t.clip(r.1)) else {
            return false;
        };
        if at < c.start || at >= c.start + c.length {
            self.set_status(t("warp.status.outside"));
            return false;
        }
        let content = (at - c.start + c.offset + s.shift).0 as f64;
        let here = w.frame_at(content);
        let Some(first) = a.grid.beats.iter().position(|b| *b >= here - 1.0) else {
            self.set_status(t("warp.status.no_tempo"));
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
        self.commit_warp_keeping(r, Some(next), WarpOp::Set, false, Keep::Next)
    }

    /// Quantise audio: each clip's transients (those at least `threshold` of its
    /// strongest) pulled to the grid by `strength` — one undo step for all of them. Only
    /// what is heard is taken (a loop's first period, round 29's second review), on the
    /// lines where it is heard (a reversed clip's run right to left).
    pub fn quantise_audio(&mut self, clips: &[ClipRef], q: QuantiseSettings) -> usize {
        let mut commands = Vec::new();
        let mut changed = 0;
        let mut waiting = false;
        for &r in clips {
            let (Some(a), Some(s)) = (self.warp_analysis(r), self.warp_start(r)) else {
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
            let offset = c.offset + s.shift;
            // What is heard: the window, or a loop's period (its repeats play it again).
            let span = c
                .content_length
                .filter(|p| *p < c.length)
                .unwrap_or(c.length);
            let reverse = c.audio.as_ref().is_some_and(|a| a.reverse);
            // The lines the user sees (counted through the metres, `views/bars.rs`), where
            // each content tick is heard: forward from the content origin, or — reversed —
            // mirrored through the map inside what is heard, as the engine plays it (round
            // 29's third review: mirrored in ticks, hits were pulled off the grid across a
            // tempo change).
            let project = self.project();
            let grid = q.grid;
            let origin = (c.start - offset).0 as f64;
            let rev = reverse.then(|| {
                crate::views::arrangement::Reversal::new(
                    crate::views::bars::map(project),
                    c.start,
                    c.start + span,
                )
            });
            let line = |t: f64| {
                let heard = rev.as_ref().map_or(origin + t, |r| r.heard(origin + t));
                let snapped = crate::views::bars::snap(project, Ticks(heard.round() as i64), grid);
                let s = snapped.0 as f64;
                rev.as_ref().map_or(s - origin, |r| r.heard(s) - origin)
            };
            let next = s
                .warp
                .quantise_to(&hits, &line, q.strength, offset, offset + span);
            let keep = s.base.as_ref().map_or(Keep::No, Keep::Base);
            if let Some(cmds) = self.warp_commands(r, Some(next), WarpOp::Quantise, keep, None) {
                commands.extend(cmds);
                changed += 1;
            }
        }
        if waiting {
            self.set_status(t("warp.status.analysing"));
        }
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.warp.quantise".into(),
                commands,
            });
        }
        changed
    }
}

impl CantusApp {
    /// The keys a selected warp marker takes before anything else: Del removes it (the
    /// clip stays), Esc lets it go. Returns whether the key was taken.
    pub fn warp_action(&mut self, action: crate::keymap::Action) -> bool {
        use crate::keymap::Action;
        // In the warp editor, Q opens *Quantise audio* on its clip.
        if action == Action::Quantize
            && self.warp.view.open
            && self.focus == crate::app::EditFocus::PianoRoll
        {
            if let Some(r) = self.selected_track.zip(self.selected_clip) {
                if self.clip_warp(r).is_some() || self.warp_analysis(r).is_some() {
                    self.open_quantise(vec![r]);
                    return true;
                }
            }
        }
        let Some((r, i)) = self.warp.selected else {
            return false;
        };
        // Only while its clip is what is being edited — alone in the arrangement's
        // selection, or in the warp editor — else it is let go (round 29's review: a marker
        // selected earlier took Del from the notes, the cable or the clips selected since).
        let editing = match self.focus {
            crate::app::EditFocus::Arrangement => self.clip_selection() == vec![r],
            crate::app::EditFocus::PianoRoll => {
                self.warp.view.open && self.selected_clip == Some(r.1)
            }
            crate::app::EditFocus::Mixer => false,
        };
        if !editing || self.clip_warp(r).is_none_or(|w| i >= w.markers.len()) {
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

impl CantusApp {
    /// A finished right-edge drag with Alt on an audio clip: the clip stretched to its new
    /// length — its window's content played that much slower or faster (its warp scaled
    /// about the window's start), one undo step. Returns whether it was one.
    pub fn warp_stretch_drag(
        &mut self,
        anchor: ClipRef,
        result: crate::views::arrangement::ClipDragResult,
    ) -> bool {
        use crate::views::arrangement::ClipDragResult as R;
        let R::TrimEnd { delta } = result else {
            return false;
        };
        if !crate::views::arrangement::warp_api::stretching(self) || delta == Ticks::ZERO {
            return false;
        }
        let Some(c) = self
            .project()
            .track(anchor.0)
            .and_then(|t| t.clip(anchor.1))
            .cloned()
        else {
            return false;
        };
        if c.audio.is_none() {
            return false;
        }
        let Some(s) = self.warp_start(anchor) else {
            return false;
        };
        let length = (c.length + delta).max(Ticks::SIXTEENTH);
        let next = s.warp.stretched(
            c.offset + s.shift,
            length.0 as f64 / c.length.0.max(1) as f64,
        );
        if !next.is_valid() {
            return false;
        }
        // A linked copy is made unique first: its content stretches, its copies' windows
        // do not (round 29's review).
        let mut commands = Vec::new();
        if c.link.is_some() {
            commands.push(Command::SetClipLink {
                track: anchor.0,
                clip: anchor.1,
                link: None,
            });
        }
        commands.push(Command::SetClipWarp {
            track: anchor.0,
            clip: anchor.1,
            warp: Some(Box::new(next.clone())),
            op: WarpOp::Set,
        });
        // Turned to follow from markers it kept: its window kept on its audio first.
        if let Some(b) = s.base.as_ref() {
            let frames = self.start_frames(anchor, true);
            commands.extend(self.kept_commands(&frames, Some(b), Some(&next), true));
        }
        // A loop's period stretches with its content.
        if let Some(p) = c.content_length {
            let k = length.0 as f64 / c.length.0.max(1) as f64;
            commands.push(Command::SetClipLoop {
                track: anchor.0,
                clip: anchor.1,
                content_length: Some(Ticks(((p.0 as f64) * k).round().max(1.0) as i64)),
            });
        }
        commands.push(Command::SetClipBounds {
            track: anchor.0,
            clip: anchor.1,
            start: c.start,
            length,
        });
        self.exec(Command::Batch {
            label_key: "cmd.warp.stretch".into(),
            commands,
        });
        true
    }
}
