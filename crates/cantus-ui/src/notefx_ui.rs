//! Note effects, the song's key and grooves in the interface (round 31, ADR 0069): the
//! piano roll's scale view (highlight, fold, snap), and what the inspector's rows, the
//! piano roll's toolbar and the channel rack ask the application to do — each one command,
//! a drag merged into one step.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::groove::{ClipGroove, Groove, GrooveId, GrooveSource};
use cantus_core::notefx::{NoteEffect, NoteFxId, NoteFxKind, NoteFxType, MAX_NOTE_FX};
use cantus_core::{ClipId, Command, Key, MergeKey, Project, Ticks, TrackId};
use std::collections::HashSet;

/// What the interface keeps about scales and note effects between frames.
#[derive(Debug)]
pub struct NoteFxUi {
    /// The piano roll lights the key's rows and its root (FL's scale highlighting).
    pub highlight: bool,
    /// The piano roll shows the key's rows only (Ableton 12's *fold to scale*).
    pub fold: bool,
    /// Drawing, moving and transposing notes keep them in the key.
    pub snap: bool,
    /// The + of a track's Note effects band: the kinds, under the rect.
    pub add_menu: Option<(TrackId, egui::Rect)>,
    /// A note clip's groove list, under the rect.
    pub groove_menu: Option<(TrackId, ClipId, egui::Rect)>,
    /// The groove list was opened by the channel rack: its choice goes to the whole pattern.
    pub groove_pattern: bool,
    /// A choice parameter's list (an arpeggio's order, a rate, a chord …), under the rect.
    pub choice_menu: Option<(TrackId, NoteFxId, &'static str, egui::Rect)>,
    /// The key's menu (roots and scales), under the rect.
    pub key_menu: Option<egui::Rect>,
    /// Note effects folded to their title row in the inspector.
    pub folded: HashSet<NoteFxId>,
}

impl Default for NoteFxUi {
    fn default() -> Self {
        NoteFxUi {
            highlight: true,
            fold: false,
            snap: false,
            add_menu: None,
            groove_menu: None,
            groove_pattern: false,
            key_menu: None,
            choice_menu: None,
            folded: HashSet::new(),
        }
    }
}

/// The key the piano roll shows and snaps to: the project's.
pub fn key_of(project: &Project) -> Option<Key> {
    project.key
}

/// "D minor", in the interface's language.
pub fn key_name(key: Option<Key>) -> String {
    match key {
        Some(k) => tf(
            "notefx.key.name",
            &[("root", &k.root_name()), ("scale", &t(k.scale.label_key()))],
        ),
        None => t("notefx.key.none"),
    }
}

/// A note effect's name: its kind's, numbered when the chain holds two of the kind.
pub fn effect_name(chain: &[NoteEffect], id: NoteFxId) -> String {
    let Some(e) = chain.iter().find(|e| e.id == id) else {
        return t("auto.target.missing");
    };
    let kind = e.kind.kind();
    let same: Vec<NoteFxId> = chain
        .iter()
        .filter(|x| x.kind.kind() == kind)
        .map(|x| x.id)
        .collect();
    let name = t(kind.label_key());
    if same.len() > 1 {
        let n = same.iter().position(|x| *x == id).unwrap_or(0) + 1;
        format!("{name} {n}")
    } else {
        name
    }
}

/// A note effect's value as its row reads: "Up-down", "1/16T", "80 %", "Off", "C".
pub fn write_value(kind: NoteFxType, key: &str, v: f32, out: &mut String) {
    use cantus_core::notefx::NoteFxUnit as U;
    use std::fmt::Write as _;
    let Some(p) = NoteFxKind::params_of(kind).iter().find(|p| p.key == key) else {
        return;
    };
    match p.unit {
        U::Choice => match cantus_core::notefx::choice_text(kind, key, v) {
            Ok(k) => out.push_str(&t(k)),
            Err(s) => out.push_str(s),
        },
        U::Percent => {
            let _ = write!(out, "{:.0} %", v * 100.0);
        }
        U::Switch => out.push_str(&t(if v >= 0.5 { "notefx.on" } else { "notefx.off" })),
        U::Count | U::Velocity => {
            let _ = write!(out, "{}", v.round() as i64);
        }
    }
}

/// An automation target's words for a note effect's parameter: the place ("Lead ·
/// Arpeggiator") and the parameter.
pub fn target_parts(
    project: &Project,
    track: TrackId,
    effect: NoteFxId,
    key: &str,
) -> (String, String) {
    let Some(tr) = project.track(track) else {
        return (t("auto.target.missing"), t("auto.target.missing"));
    };
    let place = format!("{} \u{b7} {}", tr.name, effect_name(&tr.note_fx, effect));
    let what = tr
        .note_fx
        .iter()
        .find(|e| e.id == effect)
        .and_then(|e| e.kind.param(key))
        .map(|p| t(p.label_key))
        .unwrap_or_else(|| t("auto.target.missing"));
    (place, what)
}

impl CantusApp {
    /// The track's chain, cloned (what a chain edit starts from).
    fn chain_of(&self, track: TrackId) -> Option<Vec<NoteEffect>> {
        self.project().track(track).map(|t| t.note_fx.clone())
    }

    /// Put a note effect of `kind` at the end of `track`'s chain (at its defaults).
    pub fn add_note_fx(&mut self, track: TrackId, kind: NoteFxType) -> bool {
        let Some(mut chain) = self.chain_of(track) else {
            return false;
        };
        if chain.len() >= MAX_NOTE_FX {
            self.set_error(t("notefx.full"));
            return false;
        }
        let id = NoteFxId(self.bus.alloc_id());
        chain.push(NoteEffect::new(id, kind));
        self.exec(Command::SetTrackNoteFx { track, chain })
    }

    /// Take a note effect out of its chain.
    pub fn remove_note_fx(&mut self, track: TrackId, id: NoteFxId) -> bool {
        let Some(mut chain) = self.chain_of(track) else {
            return false;
        };
        chain.retain(|e| e.id != id);
        self.exec(Command::SetTrackNoteFx { track, chain })
    }

    /// Switch a note effect on or off.
    pub fn toggle_note_fx(&mut self, track: TrackId, id: NoteFxId) -> bool {
        let Some(mut chain) = self.chain_of(track) else {
            return false;
        };
        if let Some(e) = chain.iter_mut().find(|e| e.id == id) {
            e.enabled = !e.enabled;
        }
        self.exec(Command::SetTrackNoteFx { track, chain })
    }

    /// Move a note effect `by` places in its chain (earlier: before the others).
    pub fn move_note_fx(&mut self, track: TrackId, id: NoteFxId, by: i32) -> bool {
        let Some(mut chain) = self.chain_of(track) else {
            return false;
        };
        let Some(i) = chain.iter().position(|e| e.id == id) else {
            return false;
        };
        let j = (i as i32 + by).clamp(0, chain.len() as i32 - 1) as usize;
        if i == j {
            return false;
        }
        let e = chain.remove(i);
        chain.insert(j, e);
        self.exec(Command::SetTrackNoteFx { track, chain })
    }

    /// One parameter of a note effect; a drag merges under `merge`.
    pub fn set_note_fx_param(
        &mut self,
        track: TrackId,
        effect: NoteFxId,
        key: &str,
        value: f32,
        merge: Option<MergeKey>,
    ) -> bool {
        let cmd = Command::SetNoteFxParam {
            track,
            effect,
            key: key.to_string(),
            value,
        };
        match merge {
            Some(k) => self.exec_merging(cmd, k),
            None => self.exec(cmd),
        }
    }

    /// The song's key (or none).
    pub fn set_key(&mut self, key: Option<Key>) -> bool {
        if self.project().key == key {
            return false;
        }
        self.exec(Command::SetProjectKey { key })
    }

    /// A clip's groove and amounts (or none); a drag on an amount merges under `merge`.
    pub fn set_clip_groove(
        &mut self,
        track: TrackId,
        clip: ClipId,
        groove: Option<ClipGroove>,
        merge: Option<MergeKey>,
    ) -> bool {
        let cmd = Command::SetClipGroove {
            track,
            clip,
            groove,
        };
        match merge {
            Some(k) => self.exec_merging(cmd, k),
            None => self.exec(cmd),
        }
    }

    /// *Commit*: the clip's groove written into its notes, and taken off — one step.
    pub fn commit_groove(&mut self, track: TrackId, clip: ClipId) -> bool {
        let Some(c) = self.project().track(track).and_then(|t| t.clip(clip)) else {
            return false;
        };
        if c.groove.is_none() {
            return false;
        }
        // A loop that is not a whole number of the groove's periods plays each pass with
        // another feel: one set of notes cannot hold them (ADR 0069 §4).
        if !cantus_core::groove::commit_is_exact(c, &self.project().grooves) {
            self.set_error(t("notefx.groove.commit_loop"));
            return false;
        }
        let notes = cantus_core::groove::committed(c, &self.project().grooves);
        self.exec(Command::Batch {
            label_key: "cmd.notefx.commit".into(),
            commands: vec![
                Command::SetNotes { track, clip, notes },
                Command::SetClipGroove {
                    track,
                    clip,
                    groove: None,
                },
            ],
        })
    }

    /// *Extract groove*: the feel of a note clip's notes (or an audio clip's transients,
    /// when its analysis is ready) into the pool, a bar long on a 16th grid. Returns the
    /// new groove's id.
    pub fn extract_groove(&mut self, track: TrackId, clip: ClipId) -> Option<GrooveId> {
        let id = GrooveId(self.bus.alloc_id());
        let p = self.project();
        let c = p.track(track)?.clip(clip)?;
        let bar = p.time_signature.ticks_per_bar();
        let mut groove = if c.audio.is_some() {
            let hits = self.transient_hits(track, clip)?;
            let mut g = Groove::extract(id, c.name.clone(), &hits, bar, Ticks::SIXTEENTH)?;
            g.name = tf("notefx.groove.from_audio", &[("name", &c.name)]);
            g
        } else {
            Groove::from_clip(id, c, bar, Ticks::SIXTEENTH)?
        };
        // A name of its own in the pool (extracting twice gives "Riff", "Riff 2").
        groove.name = cantus_core::groove::unique_name(&p.grooves, &groove.name);
        let index = p.grooves.len();
        self.exec(Command::InsertGroove {
            index,
            groove: Box::new(groove),
        })
        .then_some(id)
    }

    /// An audio clip's transients as groove hits — `(tick on the timeline, strength,
    /// length to the next)` — from its warp analysis (round 29's onsets); `None` until the
    /// analysis is ready (it is asked for here).
    fn transient_hits(&self, track: TrackId, clip: ClipId) -> Option<Vec<(Ticks, f32, Ticks)>> {
        let a = self.warp_analysis((track, clip))?;
        let p = self.project();
        let c = p.track(track)?.clip(clip)?;
        let region = c.audio.as_ref()?;
        // The content tick a source frame is heard at: through its warp, or the unwarped
        // rule (seconds at the song's tempo, less the fine offset).
        let content = |f: f64| match region.warp.as_deref() {
            Some(w) => w.tick_at(f),
            None => {
                (f - region.slip_frames as f64)
                    / cantus_core::warp::frames_per_tick(a.rate, p.tempo_bpm)
            }
        };
        let zero = (c.start - c.offset).0 as f64;
        let mut hits: Vec<(Ticks, f32, Ticks)> = a
            .onsets
            .iter()
            .zip(a.strengths.iter())
            .map(|(f, s)| {
                (
                    Ticks((zero + content(*f)).round() as i64),
                    *s,
                    Ticks::SIXTEENTH,
                )
            })
            .filter(|(t, _, _)| *t >= c.start && *t < c.end())
            .collect();
        hits.sort_by_key(|h| h.0);
        (!hits.is_empty()).then_some(hits)
    }

    /// *Bake to notes*: the clip replaced by a new one holding what its track's note
    /// effects play from it (its groove included), which plays past them — one step; undo
    /// gives the clip back. Selects the new clip.
    pub fn bake_note_fx(&mut self, track: TrackId, clip: ClipId) -> bool {
        let Some(notes) = played_notes(self.project(), track, clip) else {
            return false;
        };
        let Some(c) = self
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .cloned()
        else {
            return false;
        };
        // A clip of its own (no link group, no loop, no groove: they are in the notes),
        // where the source was.
        let mut baked = cantus_core::Clip {
            id: ClipId(self.bus.alloc_id()),
            name: tf("notefx.baked_name", &[("name", &c.name)]),
            offset: Ticks::ZERO,
            content_length: None,
            notes: Vec::new(),
            // Its controllers as heard, laid out with the notes (round 31's integrated review).
            lanes: cantus_core::clip_lanes::edit::heard(&c, c.start, c.end()),
            link: None,
            comp: false,
            groove: None,
            skip_note_fx: true,
            ..c.clone()
        };
        // Long enough for what rings past the source's end (a gate's tail, a latch).
        let last = notes
            .iter()
            .map(|n| n.start + n.length)
            .max()
            .unwrap_or(c.end());
        baked.length = baked.length.max(last - c.start);
        for n in notes {
            let mut note = self
                .bus
                .new_note(n.start - c.start, n.length, n.pitch, n.velocity);
            note.release = n.release;
            baked.notes.push(note);
        }
        let id = baked.id;
        let ok = self.exec(Command::Batch {
            label_key: "cmd.notefx.bake".into(),
            commands: vec![
                Command::RemoveClip { track, clip },
                Command::InsertClip {
                    track,
                    clip: Box::new(baked),
                },
            ],
        });
        if ok {
            self.selected_clip = Some(id);
            self.selected_notes.clear();
        }
        ok
    }
}

/// What `clip` of `track` plays — its groove applied, through its track's note effects
/// (unless it already plays past them) — as notes on the timeline, cut at the clip's end.
/// What *Bake to notes* writes and what MIDI export writes when it writes the played notes.
pub fn played_notes(
    project: &Project,
    track: TrackId,
    clip: ClipId,
) -> Option<Vec<cantus_engine::notefx::bake::BakedNote>> {
    let t = project.track(track)?;
    let c = t.clip(clip)?;
    if c.audio.is_some() || c.automation.is_some() {
        return None;
    }
    let mut alone = Project::empty(project.name.clone());
    alone.tempo_bpm = project.tempo_bpm;
    alone.time_signature = project.time_signature;
    alone.tempo_points = project.tempo_points.clone();
    alone.metre_changes = project.metre_changes.clone();
    alone.key = project.key;
    alone.grooves = project.grooves.clone();
    let mut c1 = c.clone();
    c1.muted = false;
    c1.lane = 0;
    alone.tracks.push(cantus_core::Track {
        clips: vec![c1],
        frozen: None,
        ..t.clone()
    });
    let snap = cantus_core::Snapshot::compile(&alone, 0);
    let st = &snap.tracks[0];
    let chain: &[NoteEffect] = if c.skip_note_fx { &[] } else { &st.note_fx };
    // A latch steps on past the clip until the track's next note replaces it (or the song
    // ends), as playback does: the full track's next note-on after the clip.
    let full = cantus_core::Snapshot::compile(project, 0);
    let hold_until = project
        .track_index(track)
        .and_then(|i| full.tracks.get(i))
        .and_then(|ft| ft.events.iter().find(|e| e.is_on() && e.tick >= c.end()))
        .map(|e| e.tick)
        .unwrap_or_else(|| project.content_end().max(c.end()));
    Some(cantus_engine::notefx::bake_until(
        chain,
        snap.key,
        &st.events,
        &st.direct,
        c.end(),
        hold_until,
    ))
}

/// The built-in swings and the pool's grooves, as the groove list offers them: the source
/// and its name.
pub fn groove_choices(project: &Project) -> Vec<(GrooveSource, String)> {
    let mut v: Vec<(GrooveSource, String)> = GrooveSource::builtins()
        .map(|s| (s, groove_name(project, &s)))
        .collect();
    for g in &project.grooves {
        v.push((GrooveSource::Pool { id: g.id }, g.name.clone()));
    }
    v
}

/// A groove source's name: "Swing 16 · 66 %", or the pooled groove's.
pub fn groove_name(project: &Project, source: &GrooveSource) -> String {
    match source {
        GrooveSource::Swing { grid, percent } => tf(
            match grid {
                cantus_core::SwingGrid::Eighths => "notefx.groove.swing8",
                cantus_core::SwingGrid::Sixteenths => "notefx.groove.swing16",
            },
            &[("pct", &percent.to_string())],
        ),
        GrooveSource::Pool { id } => project
            .grooves
            .iter()
            .find(|g| g.id == *id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| t("auto.target.missing")),
    }
}

/// A keyboard transposition of `pitch` by `semitones`: in the key's degrees while *Snap to
/// scale* is on and the song has a key (±1 the next note of the key, ±12 an octave);
/// otherwise semitones, as always. The caller has checked the plain move stays in range.
pub fn transpose(project: &Project, ui: &NoteFxUi, pitch: u8, semitones: i32) -> u8 {
    let plain = (pitch as i32 + semitones).clamp(0, 127) as u8;
    let Some(k) = project.key.filter(|_| ui.snap) else {
        return plain;
    };
    if semitones.abs() >= 12 {
        let p = k.snap(pitch) as i32 + 12 * (semitones / 12);
        return if (0..=127).contains(&p) {
            p as u8
        } else {
            plain
        };
    }
    k.step(pitch, semitones).unwrap_or(plain)
}

/// The channel rack's groove (FL's channel-rack swing): the current pattern's groove, as
/// its selected clip holds it; a click opens the groove list, whose choice goes to every
/// clip of the pattern at once (one step).
pub fn rack_groove(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    bar: &mut crate::views::clips_chrome::Toolbar,
    id: egui::Id,
    track: TrackId,
    clip: ClipId,
) {
    bar.divider(ui);
    let caption = t("notefx.groove");
    let p = app.project();
    let name = match p
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| c.groove)
    {
        Some(g) => groove_name(p, &g.source),
        None => t("notefx.groove.none"),
    };
    let field = bar.field(ui, id.with("groove"), Some(&caption), &name, 88.0, || {
        t("notefx.groove.rack_tip")
    });
    if let Some(r) = field.filter(|r| r.clicked()) {
        app.notefx.groove_pattern = true;
        app.notefx.groove_menu = Some((track, clip, r.rect));
    }
}

impl CantusApp {
    /// A groove for every note clip of the pattern around `clip` (the channel rack's
    /// window across the tracks), one step.
    pub fn set_pattern_groove(
        &mut self,
        track: TrackId,
        clip: ClipId,
        groove: Option<ClipGroove>,
    ) -> bool {
        let p = self.project();
        let Some(region) = cantus_core::pattern::PatternRegion::of(p, track, clip) else {
            return self.set_clip_groove(track, clip, groove, None);
        };
        let commands: Vec<Command> = region
            .clips
            .iter()
            .filter(|(t, c)| {
                p.track(*t)
                    .and_then(|x| x.clip(*c))
                    .is_some_and(|c| c.audio.is_none() && c.automation.is_none())
            })
            .map(|(t, c)| Command::SetClipGroove {
                track: *t,
                clip: *c,
                groove,
            })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.notefx.groove".into(),
            commands,
        })
    }
}

/// MIDI export's note-effects hook ([`cantus_project::midi::NoteFxPlay`]): what a track's
/// chain plays from its compiled events, as the bake writes it.
pub fn midi_hook(
    st: &cantus_core::TrackSnapshot,
    key: Option<Key>,
    end: Ticks,
) -> Vec<(Ticks, Ticks, u8, u8, u8)> {
    cantus_engine::notefx::bake(&st.note_fx, key, &st.events, &st.direct, end)
        .into_iter()
        .map(|n| (n.start, n.length, n.pitch, n.velocity, n.release))
        .collect()
}

/// Whether moving `notes` by `semitones` in the key (*In key* on, a key set) would put two
/// different pitches on one — C and C# both to D in C major. Such a move is refused, as a
/// kit's rows refuse one that piles pads up: transposing back could not undo it.
pub fn squashes(
    project: &Project,
    ui: &NoteFxUi,
    notes: &[cantus_core::Note],
    semitones: i32,
) -> bool {
    if project.key.is_none() || !ui.snap {
        return false;
    }
    injective_fails(notes.iter().map(|n| n.pitch), |p| {
        transpose(project, ui, p, semitones)
    })
}

/// Whether `f` sends two of the distinct `pitches` to one.
pub fn injective_fails(pitches: impl Iterator<Item = u8>, f: impl Fn(u8) -> u8) -> bool {
    let mut from = [u8::MAX; 128];
    let mut seen = [false; 128];
    for p in pitches {
        let p = p.min(127);
        if seen[p as usize] {
            continue;
        }
        seen[p as usize] = true;
        let q = f(p).min(127) as usize;
        if from[q] != u8::MAX && from[q] != p {
            return true;
        }
        from[q] = p;
    }
    false
}

/// The command palette's note effects (round 31): "Add Arpeggiator to Keys", … — one per
/// kind for the selected track, in the interface's language and in English, each adding it
/// at the end of the chain in one step (`CantusApp::add_note_fx`).
pub fn palette_entries(app: &CantusApp) -> Vec<crate::palette_index::Entry> {
    use crate::i18n::{t_in, tf_in, Locale};
    use crate::palette_index::{Entry, Kind, Target};
    let Some(track) = app.selected_track.and_then(|t| app.project().track(t)) else {
        return Vec::new();
    };
    if track.note_fx.len() >= MAX_NOTE_FX {
        return Vec::new();
    }
    NoteFxType::ALL
        .iter()
        .map(|kind| {
            let name = tf(
                "notefx.palette.add",
                &[("kind", &t(kind.label_key())), ("track", &track.name)],
            );
            let en = tf_in(
                Locale::En,
                "notefx.palette.add",
                &[
                    ("kind", &t_in(Locale::En, kind.label_key())),
                    ("track", &track.name),
                ],
            );
            Entry::new(
                Target::NoteFx(track.id, *kind),
                Kind::NoteEffect,
                name,
                en,
                t("notefx.palette.detail"),
                format!("notefx:{}", kind.label_key()),
            )
        })
        .collect()
}
