//! Linked clips in the engine (ADR 0037): a linked copy renders exactly as an unlinked copy
//! of the same notes would — to the bit, at any block size — including two copies that
//! overlap on one track, whose notes share their ids in the model.

use cantus_core::links::Linker;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// A lead with a riff of long notes (each rings past the next copy's start).
fn base() -> Project {
    let mut p = Project::new("links");
    p.tempo_bpm = 140.0;
    let mut lead = p.make_track("Lead");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [60u8, 64, 67, 72].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::beats(3), pitch, 90 + i as u8);
        clip.notes.push(n);
    }
    lead.clips.push(clip);
    p.tracks.push(lead);
    p
}

/// Copies of the riff at `starts`: linked through the commands a duplicate runs, or
/// independent (fresh clip and note ids) as before linking existed.
fn with_copies(linked: bool, starts: &[Ticks]) -> Project {
    let mut bus = CommandBus::new(base());
    let track = bus.project().tracks[0].id;
    let src = bus.project().tracks[0].clips[0].clone();
    let mut next = bus.project().next_id;
    let mut ids = || {
        let id = next;
        next += 1;
        id
    };
    let mut linker = Linker::new();
    let mut commands = Vec::new();
    for at in starts {
        let copy = if linked {
            let (pre, c) = linker.copy(track, &src, *at, &mut ids);
            commands.extend(pre);
            c
        } else {
            clipboard::copy_of(&src, *at, &mut ids)
        };
        commands.push(Command::InsertClip {
            track,
            clip: Box::new(copy),
        });
    }
    bus.reserve_ids(next);
    bus.execute(
        Command::Batch {
            label_key: "cmd.duplicate_clips".into(),
            commands,
        },
        Origin::User,
    )
    .unwrap();
    bus.project().clone()
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let opts = RenderOptions {
        block_frames: block,
        tail_seconds: 1.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn a_linked_copy_renders_as_an_unlinked_copy_to_the_bit() {
    let starts = [Ticks::beats(8), Ticks::beats(16)];
    let linked = with_copies(true, &starts);
    let unlinked = with_copies(false, &starts);
    assert!(linked.tracks[0].clips.iter().all(|c| c.link.is_some()));
    let a = render_at(&linked, 512);
    assert!(a.iter().any(|s| s.abs() > 1e-3), "the riff plays");
    assert!(same_bits(&a, &render_at(&unlinked, 512)));
    // Block size independence, at tolerance 0.
    assert!(same_bits(&a, &render_at(&linked, 64)));
    assert!(same_bits(&a, &render_at(&linked, 997)));
}

/// Two linked copies a beat apart on one track: the first copy's note-offs must not cut
/// the second copy's notes, although the model gives both the same note ids.
#[test]
fn overlapping_linked_copies_on_one_track_do_not_cut_each_other() {
    let starts = [Ticks::beats(1)];
    let linked = with_copies(true, &starts);
    let unlinked = with_copies(false, &starts);
    assert!(same_bits(
        &render_at(&linked, 512),
        &render_at(&unlinked, 512)
    ));
}

/// An edit through one copy is heard in every copy: the linked project after the edit
/// renders as the unlinked one with the same edit made by hand in each copy.
#[test]
fn an_edit_through_one_copy_is_heard_in_all() {
    let starts = [Ticks::beats(8)];
    let mut bus = CommandBus::new(with_copies(true, &starts));
    let track = bus.project().tracks[0].id;
    let copy = bus.project().tracks[0].clips[1].clone();
    let mut n = copy.notes[2].clone();
    n.pitch = 55;
    bus.execute(
        Command::SetNotes {
            track,
            clip: copy.id,
            notes: vec![n],
        },
        Origin::User,
    )
    .unwrap();
    let mut by_hand = with_copies(false, &starts);
    for c in &mut by_hand.tracks[0].clips {
        let mut notes = c.notes.clone();
        notes[2].pitch = 55;
        notes.sort_by_key(|n| (n.start, n.pitch));
        c.notes = notes;
    }
    assert!(same_bits(
        &render_at(bus.project(), 512),
        &render_at(&by_hand, 512)
    ));
}

/// `p` with the instrument of its one track replaced by `spec`.
fn on(spec: InstrumentSpec, mut p: Project) -> Project {
    p.tracks[0].instrument = spec;
    p
}

/// A bar of drums for a Pulsus II kit: kick, snare, closed and open hats, clap — the pads
/// whose noise and variations are seeded from the note.
fn drums() -> Project {
    let mut p = Project::new("drums");
    p.tempo_bpm = 140.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(4));
    for s in 0..16i64 {
        let at = Ticks::SIXTEENTH * s;
        let pitch = [36u8, 42, 38, 42, 39, 42, 46, 42][(s % 8) as usize];
        clip.notes
            .push(p.make_note(at, Ticks::SIXTEENTH, pitch, 70 + s as u8 * 3));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// Every clip of `p` with `link` cleared: what *Make unique* on all of them leaves.
fn unlinked(p: &Project) -> Project {
    let mut p = p.clone();
    for t in &mut p.tracks {
        for c in &mut t.clips {
            c.link = None;
        }
    }
    p
}

/// A linked copy of the first clip at `at`, through the commands a duplicate runs.
fn linked_copy_at(p: &Project, at: Ticks) -> Project {
    let mut bus = CommandBus::new(p.clone());
    let track = p.tracks[0].id;
    let src = p.tracks[0].clips[0].clone();
    let mut next = p.next_id;
    let mut ids = || {
        let id = next;
        next += 1;
        id
    };
    let (mut commands, copy) = Linker::new().copy(track, &src, at, &mut ids);
    commands.push(Command::InsertClip {
        track,
        clip: Box::new(copy),
    });
    bus.reserve_ids(next);
    bus.execute(
        Command::Batch {
            label_key: "cmd.duplicate_clips".into(),
            commands,
        },
        Origin::User,
    )
    .unwrap();
    bus.project().clone()
}

/// The null test of the link: on instruments that seed a note's randomness from its tag
/// (Pulsus II's noise and variations, Calor's analogue drift), a linked duplicate leaves
/// every sample of the clip it was copied from as it was, and *Make unique* changes no
/// sample at all. Copies after the original and before it (an Alt-drag to the left).
#[test]
fn linking_and_unlinking_change_no_sample_of_an_untouched_clip() {
    let analog = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog));
    for (name, p) in [("pulsus", drums()), ("calor", on(analog, base()))] {
        let alone = render_at(&p, 512);
        assert!(alone.iter().any(|s| s.abs() > 1e-3), "{name} plays");
        // A copy far after the original: everything before it is the original alone.
        let after = linked_copy_at(&p, Ticks::beats(16));
        let a = render_at(&after, 512);
        // The original's render, tail included, ends long before the copy starts.
        let spb = 48_000.0 * 60.0 / p.tempo_bpm;
        assert!(alone.len() < (16.0 * spb) as usize * 2);
        assert!(
            same_bits(&a[..alone.len()], &alone),
            "{name}: the original changed when a copy was linked to it"
        );
        assert!(
            same_bits(&a, &render_at(&unlinked(&after), 512)),
            "{name}: Make unique changed a sample"
        );
        // A copy laid over the original, from a beat before it (the original moved on to
        // beat 2 first): the original keeps its own tags, the copy is told apart.
        let mut moved = p.clone();
        moved.tracks[0].clips[0].start = Ticks::beats(2);
        let over = linked_copy_at(&moved, Ticks::beats(1));
        let b = render_at(&over, 512);
        assert!(
            same_bits(&b, &render_at(&unlinked(&over), 512)),
            "{name}: Make unique changed a sample of overlapping copies"
        );
    }
}

/// Two overlapping copies made unique still share their note ids; they must still not cut
/// each other's notes.
#[test]
fn overlapping_copies_made_unique_do_not_cut_each_other() {
    let starts = [Ticks::beats(1)];
    let linked = with_copies(true, &starts);
    let unique = unlinked(&linked);
    assert!(same_bits(
        &render_at(&unique, 512),
        &render_at(&with_copies(false, &starts), 512)
    ));
}
