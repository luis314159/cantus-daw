//! Note effects on the audio thread (round 31, ADR 0069): a track's chain of MIDI effects
//! — arpeggiator, chords, scale — between the notes it is given (its clips' events, live
//! input) and its instrument.
//!
//! **One path for every note.** The track node hands each clip event to its chain at the
//! event's own sample, the control phase each live note; what comes out goes to the
//! instrument (and to the inserts that hear the track, ADR 0022) at the sample it is due.
//! An arpeggiator's steps are scheduled times the track's segment loop splits at, exactly
//! as it splits at its events, so a step lands on its sample whatever the blocks.
//!
//! **Time.** While the transport rolls, every time is a **tick** (the event's own tick, a
//! step at anchor + k × rate): the transport turns it into a sample offset exactly as it
//! does for clip events, so playback, an offline render, a freeze and a bounce hear the
//! same notes on the same samples. Stopped, a live arpeggio runs on the engine's **sample
//! clock** at the tempo at the playhead. Randomness is a hash of the seed and the step:
//! the same every time.
//!
//! **Real time.** Everything is fixed-size and built off the audio thread
//! ([`Chain::new`]); configuring, playing and resetting never allocate. An empty chain —
//! every track before schema 43 — is not consulted at all: its notes take the path they
//! always took, to the bit.

mod arp;
pub mod bake;
pub(crate) mod play;
mod table;

use cantus_core::notefx::{NoteEffect, NoteFxId, NoteFxKind, NoteFxType, MAX_NOTE_FX};
use cantus_core::scale::Key;

pub use bake::{bake, bake_until};

/// A note on its way through the chain.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Msg {
    pub on: bool,
    pub pitch: u8,
    /// A note-on's velocity (1…127).
    pub velocity: u8,
    /// A note-off's release velocity (0…127; 0 where none was given). Carried with the
    /// note each output belongs to.
    pub release: u8,
    /// The voice tag that pairs an off with its on (`NoteEvent::note`, the live tag).
    pub tag: u64,
    /// The note's ordinal (`NoteEvent::ordinal`), or an arpeggio's step.
    pub ordinal: u32,
    /// From live input (not a clip): an instrument plays it as a live note.
    pub live: bool,
}

impl Msg {
    pub fn on(pitch: u8, velocity: u8, tag: u64) -> Msg {
        Msg {
            on: true,
            pitch,
            velocity: velocity.max(1),
            release: 0,
            tag,
            ordinal: 0,
            live: false,
        }
    }

    pub fn off(pitch: u8, tag: u64) -> Msg {
        Msg {
            on: false,
            pitch,
            velocity: 0,
            release: 0,
            tag,
            ordinal: 0,
            live: false,
        }
    }
}

/// Where the chain is in time when it is called.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Now {
    /// The transport rolls: `pos` is a tick. Stopped: an engine sample.
    pub rolling: bool,
    pub pos: f64,
    /// Ticks per sample at the tempo now (what a step's length in samples is read from
    /// while stopped).
    pub tps: f64,
}

/// A scheduled time: in the domain it was scheduled in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Due {
    pub rolling: bool,
    pub pos: f64,
}

/// Outputs one call can leave before they are taken.
pub const OUT: usize = 256;

/// The voice tag of output `k` of a note tagged `tag` at stage `stage`: never the note's
/// own (bits 30 and 63 set, as `voice_tag`), different for every output.
#[inline]
pub(crate) fn derive_tag(tag: u64, stage: usize, k: u64) -> u64 {
    cantus_core::snapshot::voice_tag(tag, 0x4E46_0000_0000 ^ ((stage as u64) << 40) ^ k)
}

/// One stage of the chain.
#[derive(Debug)]
struct Stage {
    id: NoteFxId,
    kind: NoteFxType,
    enabled: bool,
    params: NoteFxKind,
    arp: arp::ArpState,
    map: table::Table,
}

/// A track's note-effect chain.
#[derive(Debug)]
pub struct Chain {
    stages: Vec<Stage>,
    /// How many stages are in use.
    n: usize,
    key: Option<Key>,
    out: [(f64, Msg); OUT],
    out_n: usize,
    /// Outputs that found no room (counted for `EngineStats::dropped_events`).
    dropped: u64,
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

impl Chain {
    /// A chain with room for [`MAX_NOTE_FX`] stages. Allocates: build it off the audio
    /// thread.
    pub fn new() -> Chain {
        Chain {
            stages: (0..MAX_NOTE_FX)
                .map(|_| Stage {
                    id: NoteFxId(0),
                    kind: NoteFxType::Scale,
                    enabled: false,
                    params: NoteFxType::Scale.make(),
                    arp: arp::ArpState::new(),
                    map: table::Table::new(),
                })
                .collect(),
            n: 0,
            key: None,
            out: [(0.0, Msg::off(0, 0)); OUT],
            out_n: 0,
            dropped: 0,
        }
    }

    /// Whether any stage is switched on: only then does a note go through the chain.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.stages[..self.n].iter().any(|s| s.enabled)
    }

    /// Take the snapshot's chain and key. Allocation-free (the settings are copied into
    /// the stages). A chain whose shape changed — an effect added, removed, moved, switched
    /// or of another kind — starts afresh: returns `true` when what it was sounding must be
    /// silenced (the caller's all-notes-off).
    pub fn configure(&mut self, chain: &[NoteEffect], key: Option<Key>) -> bool {
        self.key = key;
        let n = chain.len().min(MAX_NOTE_FX);
        let same = n == self.n
            && chain[..n]
                .iter()
                .zip(&self.stages)
                .all(|(e, s)| e.id == s.id && e.kind.kind() == s.kind && e.enabled == s.enabled);
        if same {
            for (e, s) in chain[..n].iter().zip(self.stages.iter_mut()) {
                s.params.clone_from(&e.kind);
            }
            return false;
        }
        let sounding = self.sounding();
        self.reset();
        self.n = n;
        for (e, s) in chain[..n].iter().zip(self.stages.iter_mut()) {
            s.id = e.id;
            s.kind = e.kind.kind();
            s.enabled = e.enabled;
            s.params.clone_from(&e.kind);
        }
        sounding
    }

    /// One parameter of stage `stage` (automation, a modulator): read from the next note
    /// or step on.
    pub fn set_param(&mut self, stage: usize, param: usize, v: f32) {
        if let Some(s) = self.stages[..self.n].get_mut(stage) {
            s.params.set_index(param, v);
        }
    }

    /// The stage holding effect `id`.
    pub fn stage_of(&self, id: NoteFxId) -> Option<usize> {
        self.stages[..self.n].iter().position(|s| s.id == id)
    }

    /// Forget everything held and scheduled (after the instrument's all-notes-off: a
    /// stop, a seek, a loop's wrap).
    pub fn reset(&mut self) {
        for s in self.stages.iter_mut() {
            s.arp.reset();
            s.map.clear();
        }
        self.out_n = 0;
    }

    /// Whether some output may still be sounding.
    pub fn sounding(&self) -> bool {
        self.stages[..self.n]
            .iter()
            .any(|s| s.arp.sounding() || !s.map.is_empty())
    }

    /// A note into the chain at `now`: what it sounds at once is left for [`Self::take`].
    pub fn input(&mut self, now: Now, m: Msg) {
        self.feed(0, now, now_due(now), m);
    }

    /// The earliest time something is scheduled (an arpeggio's next step or note-off).
    pub fn next_due(&self) -> Option<Due> {
        let mut best: Option<Due> = None;
        for s in self.stages[..self.n].iter().filter(|s| s.enabled) {
            if let Some(d) = s.arp.next_due() {
                best = match best {
                    Some(b) if b.rolling == d.rolling && b.pos <= d.pos => Some(b),
                    _ => Some(d),
                };
            }
        }
        best
    }

    /// Fire everything scheduled that `is_due` says is due by now, earliest first (a
    /// stage's note-offs before its next step at the same time), each at its own time.
    pub fn run_due(&mut self, now: Now, is_due: impl Fn(Due) -> bool) {
        // A transport that started or stopped since: each arpeggio moves to the new time.
        for i in 0..self.n {
            if !self.stages[i].enabled || self.stages[i].kind != NoteFxType::Arp {
                continue;
            }
            let mut buf = [Msg::off(0, 0); arp::MAX_OUT];
            let k = self.stages[i].arp.sync(now, &mut buf);
            for b in buf.iter().take(k) {
                self.feed(i + 1, now, now_due(now), *b);
            }
        }
        // Bounded: every firing moves a stage on (a step, or a note-off taken away).
        for _ in 0..4096 {
            let mut pick: Option<(usize, Due)> = None;
            for (i, s) in self.stages[..self.n].iter().enumerate() {
                if !s.enabled {
                    continue;
                }
                if let Some(d) = s.arp.next_due().filter(|d| is_due(*d)) {
                    let earlier = match pick {
                        None => true,
                        Some((_, b)) => d.pos < b.pos,
                    };
                    if earlier {
                        pick = Some((i, d));
                    }
                }
            }
            let Some((i, at)) = pick else { return };
            self.fire(i, now, at);
        }
    }

    /// Stage `i`'s earliest scheduled event, at time `at`.
    fn fire(&mut self, i: usize, now: Now, at: Due) {
        let mut buf = [Msg::off(0, 0); arp::MAX_OUT];
        let n = {
            let s = &mut self.stages[i];
            let NoteFxKind::Arp(p) = &s.params else {
                return;
            };
            s.arp.fire(p, now, i, &mut buf)
        };
        for m in buf.iter().take(n) {
            self.feed(i + 1, now, at, *m);
        }
    }

    /// `m` into stage `i` (and on), at time `at`.
    fn feed(&mut self, i: usize, now: Now, at: Due, m: Msg) {
        if i >= self.n {
            self.emit(at, m);
            return;
        }
        if !self.stages[i].enabled {
            return self.feed(i + 1, now, at, m);
        }
        let key = self.key;
        match self.stages[i].kind {
            NoteFxType::Scale | NoteFxType::Chord => {
                // What an on sounds: the key's note (or nothing), or the chord's voices.
                let mut voices = [0u8; 8];
                let n = if m.on {
                    match &self.stages[i].params {
                        NoteFxKind::Scale(p) => match p.map(m.pitch, key) {
                            Some(x) => {
                                voices[0] = x;
                                1
                            }
                            None => 0,
                        },
                        NoteFxKind::Chord(p) => p.voice(m.pitch, key, &mut voices),
                        NoteFxKind::Arp(_) => return self.feed(i + 1, now, at, m),
                    }
                } else {
                    0
                };
                // A note on again before its off, or its off: what it sounded ends.
                match self.stages[i].map.take(m.tag, m.pitch) {
                    Some(e) => {
                        let off = Msg {
                            on: false,
                            velocity: 0,
                            ..m
                        };
                        for k in 0..e.n as usize {
                            self.send_voice(i, now, at, off, e.pitches[k], k);
                        }
                    }
                    // An off it never saw (a note held before the chain changed): on.
                    None if !m.on => return self.feed(i + 1, now, at, m),
                    None => {}
                }
                if m.on {
                    if !self.stages[i].map.insert(m.tag, m.pitch, &voices[..n]) {
                        self.dropped += 1;
                    }
                    for (k, &pitch) in voices.iter().enumerate().take(n) {
                        self.send_voice(i, now, at, m, pitch, k);
                    }
                }
            }
            NoteFxType::Arp => {
                let NoteFxKind::Arp(p) = &self.stages[i].params else {
                    return self.feed(i + 1, now, at, m);
                };
                let p = p.clone();
                let mut buf = [Msg::off(0, 0); arp::MAX_OUT];
                let n = self.stages[i].arp.input(&p, now, at, m, &mut buf);
                // What it lets through (an off it never held) or lets go at once (a
                // domain change's offs).
                for b in buf.iter().take(n) {
                    self.feed(i + 1, now, at, *b);
                }
            }
        }
    }

    /// Voice `k` (pitch `pitch`) of note `m` out of stage `i`: on the note's own pitch it keeps
    /// the note's tag; a chord's other voices and a scale's moved note have their own.
    fn send_voice(&mut self, i: usize, now: Now, at: Due, m: Msg, pitch: u8, k: usize) {
        let tag = match self.stages[i].kind {
            NoteFxType::Chord | NoteFxType::Scale => chord_tag(m, pitch, i, k),
            _ => m.tag,
        };
        self.feed(i + 1, now, at, Msg { pitch, tag, ..m });
    }

    fn emit(&mut self, at: Due, m: Msg) {
        if self.out_n < OUT {
            self.out[self.out_n] = (at.pos, m);
            self.out_n += 1;
        } else {
            self.dropped += 1;
        }
    }

    /// What the chain sounded since the last call, in order, each with the time it was
    /// due (a tick rolling, a sample stopped).
    pub fn take(&mut self) -> &[(f64, Msg)] {
        let n = std::mem::replace(&mut self.out_n, 0);
        &self.out[..n]
    }

    /// Outputs that found no room since the last call.
    pub fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }
}

/// The tag of voice `k` (pitch `pitch`) of note `m` at stage `stage` — a chord's voice, a
/// scale's snapped note: the note's own
/// for the voice on its own key, so a live key's pressure still finds it.
#[inline]
fn chord_tag(m: Msg, pitch: u8, stage: usize, k: usize) -> u64 {
    if pitch == m.pitch {
        m.tag
    } else {
        // The input key and the voice both in it: two live keys (one tag for every key)
        // that sound the same pitch never share a voice.
        derive_tag(
            m.tag ^ ((m.pitch as u64) << 48) ^ ((pitch as u64) << 56),
            stage,
            k as u64,
        )
    }
}

#[inline]
fn now_due(now: Now) -> Due {
    Due {
        rolling: now.rolling,
        pos: now.pos,
    }
}

impl Chain {
    /// Leave `m` as an output at time `t` (a note that passes the chain, for the bake).
    pub(super) fn out_push(&mut self, t: f64, m: Msg) {
        self.emit(
            Due {
                rolling: true,
                pos: t,
            },
            m,
        );
    }
}

#[cfg(test)]
mod tests;

impl Chain {
    /// Every arpeggiator's *Hold* off (the bake's end: a latch lets go and rings out).
    pub(super) fn release_holds(&mut self) {
        for s in self.stages[..self.n].iter_mut() {
            if let NoteFxKind::Arp(a) = &mut s.params {
                a.hold = false;
            }
        }
    }
}
