//! Note effects, keys and grooves through the application (round 31, ADR 0069): a chain
//! built, reordered and edited one undo step each; *Bake to notes* and *Commit* change no
//! sample of what plays; a groove extracted from a played part swings another clip; a
//! MIDI file as played holds the arpeggio and as written the clip's notes; the piano roll
//! keeps notes in the key; the inspector, the lists and the piano roll render.

use cantus_core::groove::{ClipGroove, GrooveSource, SwingGrid};
use cantus_core::notefx::{ArpMode, NoteFxKind, NoteFxType};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_project::midi;
use cantus_ui::CantusApp;

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

/// A Firmus track holding a C major chord for a bar, then an A minor one.
fn song() -> (Project, TrackId, ClipId) {
    let mut p = Project::new("notefx");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Keys");
    let mut c = p.make_clip("Chords", Ticks::ZERO, Ticks::beats(8));
    for (bar, chord) in [[60u8, 64, 67], [57, 60, 64]].iter().enumerate() {
        for pitch in chord {
            let n = p.make_note(Ticks::beats(4 * bar as i64), Ticks::beats(4), *pitch, 96);
            c.notes.push(n);
        }
    }
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    (p, tid, cid)
}

fn render(p: &Project) -> Vec<f32> {
    let snap = std::sync::Arc::new(Snapshot::compile(p, 1));
    let opts = cantus_engine::RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    };
    cantus_engine::render(snap, &opts).unwrap().samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs");
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn a_chain_is_built_reordered_and_edited_one_step_each() {
    let (p, t, _) = song();
    let mut a = app(p);
    assert!(a.add_note_fx(t, NoteFxType::Chord));
    assert!(a.add_note_fx(t, NoteFxType::Arp));
    let chain = a.project().track(t).unwrap().note_fx.clone();
    assert_eq!(chain.len(), 2);
    let arp = chain[1].id;
    assert!(a.move_note_fx(t, arp, -1));
    assert_eq!(a.project().track(t).unwrap().note_fx[0].id, arp);
    assert!(!a.move_note_fx(t, arp, -1), "already first");
    // A drag on a parameter is one step, whatever its frames.
    let key = MergeKey::of(&("test-drag", 1));
    for v in [2.0, 3.0, 4.0] {
        a.set_note_fx_param(t, arp, "octaves", v, Some(key));
    }
    a.end_gesture();
    match &a.project().track(t).unwrap().note_fx[0].kind {
        NoteFxKind::Arp(x) => assert_eq!(x.octaves, 4),
        k => panic!("{k:?}"),
    }
    a.undo();
    match &a.project().track(t).unwrap().note_fx[0].kind {
        NoteFxKind::Arp(x) => assert_eq!(x.octaves, 1, "the drag was one step"),
        k => panic!("{k:?}"),
    }
    assert!(a.toggle_note_fx(t, arp));
    assert!(!a.project().track(t).unwrap().note_fx[0].enabled);
    assert!(a.remove_note_fx(t, arp));
    assert_eq!(a.project().track(t).unwrap().note_fx.len(), 1);
}

#[test]
fn bake_to_notes_and_commit_change_no_sample() {
    let (p, t, c) = song();
    let mut a = app(p);
    a.add_note_fx(t, NoteFxType::Chord);
    a.add_note_fx(t, NoteFxType::Arp);
    let arp = a.project().track(t).unwrap().note_fx[1].id;
    a.set_note_fx_param(t, arp, "mode", 2.0, None);
    a.set_note_fx_param(t, arp, "octaves", 2.0, None);
    a.set_clip_groove(
        t,
        c,
        Some(ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Sixteenths,
            percent: 50,
        })),
        None,
    );
    let before = render(a.project());
    assert!(a.bake_note_fx(t, c));
    let baked = a.project().track(t).unwrap().clips[0].clone();
    assert!(baked.skip_note_fx && baked.groove.is_none());
    assert!(baked.notes.len() > 6, "the arpeggio written out");
    same_bits(&render(a.project()), &before, "baked");
    a.undo();
    assert_eq!(
        a.project().track(t).unwrap().clips[0].id,
        c,
        "undo gives the clip back"
    );
    // A swung clip committed plays what it played (no chain).
    let (p, t, c) = song();
    let mut a = app(p);
    a.set_clip_groove(
        t,
        c,
        Some(ClipGroove {
            velocity: 1.0,
            ..ClipGroove::new(GrooveSource::Swing {
                grid: SwingGrid::Eighths,
                percent: 66,
            })
        }),
        None,
    );
    // Off-beat notes, so the swing moves something.
    let off = a.bus.new_note(Ticks::EIGHTH * 3, Ticks::EIGHTH, 72, 80);
    a.exec(Command::InsertNotes {
        track: t,
        clip: c,
        notes: vec![off],
    });
    let before = render(a.project());
    assert!(a.commit_groove(t, c));
    assert!(a.project().track(t).unwrap().clips[0].groove.is_none());
    same_bits(&render(a.project()), &before, "committed");
}

#[test]
fn an_extracted_groove_swings_another_clip_as_the_played_part_was() {
    let (mut p, t, _) = song();
    let mut played = p.make_clip("Played", Ticks::beats(16), Ticks::beats(4));
    for k in 0..16 {
        let late = if k % 2 == 1 { 40 } else { 0 };
        let n = p.make_note(Ticks(k * 240 + late), Ticks(100), 48, 100);
        played.notes.push(n);
    }
    let pc = played.id;
    p.tracks[0].clips.push(played);
    let mut a = app(p);
    let id = a.extract_groove(t, pc).expect("a groove");
    let g = a.project().grooves.iter().find(|g| g.id == id).unwrap();
    assert_eq!(g.points[1].shift, 40.0);
    assert_eq!(g.name, "Played");
}

#[test]
fn a_midi_file_holds_the_arpeggio_as_played_and_the_notes_as_written() {
    let (p, t, c) = song();
    let mut a = app(p);
    a.add_note_fx(t, NoteFxType::Arp);
    let dir = std::env::temp_dir().join("cantus-notefx-midi");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clip.mid");
    a.selected_track = Some(t);
    a.selected_clip = Some(c);
    let count = |path: &std::path::Path| {
        let bytes = std::fs::read(path).unwrap();
        let read = midi::read(&bytes, &midi::ImportOptions::default()).unwrap();
        read.tracks.iter().map(|t| t.notes.len()).sum::<usize>()
    };
    use cantus_ui::views::interop::{export_midi_notes, MidiScope};
    assert!(export_midi_notes(&mut a, &path, MidiScope::Clip, 960, true));
    let played = cantus_ui::notefx_ui::played_notes(a.project(), t, c).unwrap();
    assert_eq!(count(&path), played.len(), "every step of the arpeggio");
    assert!(played.len() > 6);
    assert!(export_midi_notes(
        &mut a,
        &path,
        MidiScope::Clip,
        960,
        false
    ));
    assert_eq!(count(&path), 6, "the two chords as written");
}

#[test]
fn the_piano_roll_keeps_notes_in_the_key() {
    let (p, _, _) = song();
    let mut a = app(p);
    a.set_key(Some(Key::new(0, ScaleKind::Major)));
    a.notefx.snap = true;
    use cantus_ui::notefx_ui::transpose;
    assert_eq!(
        transpose(a.project(), &a.notefx, 64, 1),
        65,
        "E up a degree is F"
    );
    assert_eq!(
        transpose(a.project(), &a.notefx, 65, 1),
        67,
        "F up a degree is G"
    );
    assert_eq!(transpose(a.project(), &a.notefx, 60, -1), 59);
    assert_eq!(
        transpose(a.project(), &a.notefx, 61, 12),
        72,
        "C# snaps, then an octave"
    );
    a.notefx.snap = false;
    assert_eq!(transpose(a.project(), &a.notefx, 64, 1), 65);
    assert_eq!(
        transpose(a.project(), &a.notefx, 65, 1),
        66,
        "semitones without snap"
    );
}

#[test]
fn the_views_render_with_a_chain_a_key_and_a_groove() {
    let (p, t, c) = song();
    let mut a = app(p);
    a.add_note_fx(t, NoteFxType::Chord);
    a.add_note_fx(t, NoteFxType::Arp);
    a.add_note_fx(t, NoteFxType::Scale);
    a.set_key(Some(Key::new(2, ScaleKind::Dorian)));
    a.set_clip_groove(
        t,
        c,
        Some(ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Sixteenths,
            percent: 58,
        })),
        None,
    );
    // A pooled groove too: its name row and the pool's removal band are drawn.
    let gid = a.extract_groove(t, c).unwrap();
    a.extract_groove(t, c).unwrap();
    a.set_clip_groove(
        t,
        c,
        Some(ClipGroove::new(GrooveSource::Pool { id: gid })),
        None,
    );
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    a.attach(&ctx);
    let arp = a.project().track(t).unwrap().note_fx[1].id;
    for (step, fold) in [(0, false), (1, true), (2, true)] {
        a.select_clip(t, c);
        cantus_ui::views::piano_roll::scale::set_fold(&mut a, fold);
        let r = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(120.0, 22.0));
        match step {
            0 => a.notefx.key_menu = Some(r),
            1 => a.notefx.groove_menu = Some((t, c, r)),
            _ => a.notefx.choice_menu = Some((t, arp, "rate", r)),
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        for _ in 0..3 {
            let mut out = ctx.run_ui(input.clone(), |ui| {
                egui::Panel::left(egui::Id::new("insp"))
                    .exact_size(360.0)
                    .show(ui, |ui| cantus_ui::views::inspector::show(&mut a, ui));
                egui::CentralPanel::default()
                    .show(ui, |ui| cantus_ui::views::piano_roll::show(&mut a, ui));
                let ctx = ui.ctx().clone();
                cantus_ui::notefx_menus::frame(&mut a, &ctx);
            });
            out.textures_delta.clear();
        }
        a.select_track(t);
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .show(ui, |ui| cantus_ui::views::inspector::show(&mut a, ui));
        });
        out.textures_delta.clear();
    }
    match &a.project().track(t).unwrap().note_fx[1].kind {
        NoteFxKind::Arp(x) => assert_eq!(x.mode, ArpMode::Up),
        k => panic!("{k:?}"),
    }
}

#[test]
fn a_bake_keeps_what_rings_past_the_clip_a_long_gate_and_a_latch() {
    for hold in [false, true] {
        let (mut p, t, c) = song();
        // The next clip on the track: where a latch is replaced.
        let mut next = p.make_clip("Next", Ticks::beats(12), Ticks::beats(4));
        next.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 72, 90));
        p.tracks[0].clips.push(next);
        let mut a = app(p);
        a.add_note_fx(t, NoteFxType::Arp);
        let arp = a.project().track(t).unwrap().note_fx[0].id;
        a.set_note_fx_param(t, arp, "gate", 2.0, None);
        a.set_note_fx_param(t, arp, "swing", 0.75, None);
        a.set_note_fx_param(t, arp, "hold", if hold { 1.0 } else { 0.0 }, None);
        let before = render(a.project());
        assert!(a.bake_note_fx(t, c));
        let baked = a
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .find(|x| x.skip_note_fx)
            .unwrap()
            .clone();
        assert!(
            baked.length > Ticks::beats(8),
            "longer than the source: {:?}",
            baked.length
        );
        if hold {
            let last = baked.notes.iter().map(|n| n.start).max().unwrap();
            assert!(
                last >= Ticks::beats(11),
                "the latch steps on to the next note"
            );
        }
        same_bits(
            &render(a.project()),
            &before,
            &format!("baked, hold {hold}"),
        );
    }
}

#[test]
fn the_pool_names_each_groove_once_and_lets_go_of_unused_ones() {
    let (mut p, t, c) = song();
    let mut played = p.make_clip("Played", Ticks::beats(16), Ticks::beats(4));
    for k in 0..8 {
        let n = p.make_note(Ticks(k * 480 + 30 * (k % 2)), Ticks(100), 48, 100);
        played.notes.push(n);
    }
    let pc = played.id;
    p.tracks[0].clips.push(played);
    let mut a = app(p);
    let first = a.extract_groove(t, pc).unwrap();
    let second = a.extract_groove(t, pc).unwrap();
    let names: Vec<&str> = a
        .project()
        .grooves
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(names, ["Played", "Played 2"]);
    // One played by a clip, one not: only the free one can go.
    a.set_clip_groove(
        t,
        c,
        Some(ClipGroove::new(GrooveSource::Pool { id: first })),
        None,
    );
    assert!(cantus_core::groove::in_use(a.project(), first));
    assert!(!cantus_core::groove::in_use(a.project(), second));
    assert!(!a.exec(Command::RemoveGroove { id: first }));
    assert!(a.exec(Command::RemoveGroove { id: second }));
    assert_eq!(a.project().grooves.len(), 1);
}

#[test]
fn moving_in_the_key_never_puts_two_notes_on_one() {
    let (mut p, _, _) = song();
    let c = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90);
    let cs = p.make_note(Ticks::QUARTER, Ticks::QUARTER, 61, 90);
    let e = p.make_note(Ticks::ZERO, Ticks::QUARTER, 64, 90);
    let mut a = app(p);
    a.set_key(Some(Key::new(0, ScaleKind::Major)));
    a.notefx.snap = true;
    use cantus_ui::notefx_ui::squashes;
    // C and C# both to D: refused. C and E move apart, one degree each.
    assert!(squashes(
        a.project(),
        &a.notefx,
        &[c.clone(), cs.clone()],
        1
    ));
    assert!(!squashes(
        a.project(),
        &a.notefx,
        &[c.clone(), e.clone()],
        1
    ));
    // Snap off, semitones never squash.
    a.notefx.snap = false;
    assert!(!squashes(
        a.project(),
        &a.notefx,
        &[c.clone(), cs.clone()],
        1
    ));
    // A drag: the same rule, the pitches kept.
    a.notefx.snap = true;
    let axis = cantus_ui::views::piano_roll::Axis::default();
    use cantus_ui::views::piano_roll::scale::moved_with;
    let both = [c.clone(), cs.clone()];
    assert_eq!(moved_with(&a, &axis, &both, 60, 1), 60);
    assert_eq!(moved_with(&a, &axis, &both, 61, 1), 61);
    assert_eq!(moved_with(&a, &axis, &[c, e], 64, 1), 65);
}

#[test]
fn glue_never_changes_how_the_pieces_play() {
    let (mut p, t, a_id) = song();
    // A second clip after the first; the track arpeggiated.
    let mut b = p.make_clip("B", Ticks::beats(8), Ticks::beats(4));
    b.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 62, 90));
    let b_id = b.id;
    p.tracks[0].clips.push(b);
    let mut a = app(p);
    a.add_note_fx(t, NoteFxType::Arp);
    // B baked, A not: a glue would send B's arpeggio through the arpeggiator again.
    assert!(a.bake_note_fx(t, b_id));
    let baked = a.selected_clip.unwrap();
    let before = a.project().clone();
    a.selected_clips = vec![(t, a_id), (t, baked)];
    a.glue_clips();
    assert_eq!(a.project(), &before, "not glued");
    assert!(a.status.text.contains("Not glued"), "{}", a.status.text);
    // Two grooves: not glued either; the same groove: glued.
    let (mut p, t, a_id) = song();
    let mut b = p.make_clip("B", Ticks::beats(8), Ticks::beats(4));
    b.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 62, 90));
    let b_id = b.id;
    let swing = |pct| {
        Some(ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Sixteenths,
            percent: pct,
        }))
    };
    b.groove = swing(66);
    p.tracks[0].clips.push(b);
    p.tracks[0].clips[0].groove = swing(58);
    let mut a = app(p);
    a.selected_clips = vec![(t, a_id), (t, b_id)];
    let before = a.project().clone();
    a.glue_clips();
    assert_eq!(a.project(), &before);
    a.set_clip_groove(t, b_id, swing(58), None);
    a.selected_clips = vec![(t, a_id), (t, b_id)];
    a.glue_clips();
    assert_eq!(a.project().track(t).unwrap().clips.len(), 1, "glued");
    // A groove or a bypass changes a linked clip's content.
    use cantus_ui::clips::changes_content;
    assert!(changes_content(
        &Command::SetClipGroove {
            track: t,
            clip: a_id,
            groove: None
        },
        a_id
    ));
    assert!(changes_content(
        &Command::SetClipSkipNoteFx {
            track: t,
            clip: a_id,
            skip: true
        },
        a_id
    ));
}

#[test]
fn the_palette_adds_each_note_effect_to_the_selected_track() {
    let (p, t, _) = song();
    let mut a = app(p);
    a.select_track(t);
    let entries = cantus_ui::views::palette::entries(&mut a);
    let mine: Vec<_> = entries
        .iter()
        .filter(|e| e.kind == cantus_ui::palette_index::Kind::NoteEffect)
        .cloned()
        .collect();
    assert_eq!(mine.len(), 3);
    assert!(
        mine.iter().any(|e| e.name == "Add Arpeggiator to Keys"),
        "{mine:?}"
    );
    let arp = mine
        .iter()
        .find(|e| {
            matches!(
                e.target,
                cantus_ui::palette_index::Target::NoteFx(_, NoteFxType::Arp)
            )
        })
        .unwrap();
    cantus_ui::views::palette::run(&mut a, arp);
    let chain = &a.project().track(t).unwrap().note_fx;
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].kind.kind(), NoteFxType::Arp);
    a.undo();
    assert!(a.project().track(t).unwrap().note_fx.is_empty(), "one step");
}
