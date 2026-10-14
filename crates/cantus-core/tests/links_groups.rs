//! The "Link to…" list (ADR 0037, round 26): `links::groups` names every link group once —
//! its shared name, its kind, how many clips share it, the member a link copies — and
//! every clip of its own after them, in an order that does not depend on how the clips
//! happen to be stored; `links::would_link` says, before anything runs, exactly what
//! `links::link_to` would do with a target, and why not when it would do nothing.

use cantus_core::links::{self, Group, Kind, Refusal};
use cantus_core::*;

/// Drums: "Beat" at bars 1 and 3 (one group), "Fill" alone at bar 4, an automation clip
/// of the track's level. Bass: "Beat" again (a linked copy, bar 2), "Riff" alone.
/// Vocal: "Take" twice (a linked audio pair), "Take" once more of its own.
struct Scene {
    p: Project,
    drums: TrackId,
    bass: TrackId,
    vocal: TrackId,
    beat: [ClipId; 3],
    fill: ClipId,
    auto: ClipId,
    riff: ClipId,
    takes: [ClipId; 3],
}

fn note_clip(p: &mut Project, name: &str, bar: i64, pitch: u8) -> Clip {
    let mut c = p.make_clip(name, Ticks::beats(4 * bar), Ticks::beats(4));
    let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, pitch, 100);
    c.notes.push(n);
    c
}

fn scene() -> Scene {
    let mut p = Project::default();
    let mut drums = p.make_track("Drums");
    let mut bass = p.make_track("Bass");
    let mut vocal = p.make_track("Vocal");
    let beat_link = LinkId(p.alloc_id());
    let mut beat0 = note_clip(&mut p, "Beat", 0, 36);
    beat0.link = Some(beat_link);
    let mut beat2 = beat0.clone();
    beat2.id = ClipId(p.alloc_id());
    beat2.start = Ticks::beats(8);
    let mut beat1 = beat0.clone();
    beat1.id = ClipId(p.alloc_id());
    beat1.start = Ticks::beats(4);
    let fill = note_clip(&mut p, "Fill", 3, 38);
    let mut auto = p.make_clip("Level", Ticks::ZERO, Ticks::beats(8));
    auto.automation = Some(Automation::new(AutoTarget::TrackGain(drums.id), Vec::new()));
    let riff = note_clip(&mut p, "Riff", 0, 40);
    let src = AudioSourceId(p.alloc_id());
    let take_link = LinkId(p.alloc_id());
    let mut take0 = p.make_clip("Take", Ticks::ZERO, Ticks::beats(4));
    take0.audio = Some(AudioRegion::of(src));
    let mut lone_take = take0.clone();
    lone_take.id = ClipId(p.alloc_id());
    lone_take.start = Ticks::beats(16);
    take0.link = Some(take_link);
    let mut take1 = take0.clone();
    take1.id = ClipId(p.alloc_id());
    take1.start = Ticks::beats(8);
    let s = Scene {
        drums: drums.id,
        bass: bass.id,
        vocal: vocal.id,
        beat: [beat0.id, beat1.id, beat2.id],
        fill: fill.id,
        auto: auto.id,
        riff: riff.id,
        takes: [take0.id, take1.id, lone_take.id],
        p: Project::default(),
    };
    // Stored out of time order on purpose: the list must not care.
    drums.clips = vec![fill, beat2, auto, beat0];
    bass.clips = vec![riff, beat1];
    vocal.clips = vec![lone_take, take1, take0];
    p.tracks = vec![drums, bass, vocal];
    assert!(links::check(&p).is_ok());
    Scene { p, ..s }
}

fn entry(list: &[Group], first: (TrackId, ClipId)) -> &Group {
    list.iter()
        .find(|g| g.first == first)
        .unwrap_or_else(|| panic!("{first:?} is not listed: {list:#?}"))
}

#[test]
fn every_group_is_listed_once_with_its_name_kind_count_and_first_member() {
    let s = scene();
    let list = links::groups(&s.p);
    // Two groups and four clips of their own: every clip is in exactly one entry.
    assert_eq!(list.len(), 6, "{list:#?}");
    assert_eq!(list.iter().map(|g| g.count).sum::<usize>(), 9);
    let beat = entry(&list, (s.drums, s.beat[0]));
    assert_eq!(beat.name, "Beat");
    assert_eq!(beat.kind, Kind::Notes);
    assert_eq!(beat.count, 3);
    assert!(beat.link.is_some());
    assert_eq!((beat.track_index, beat.start), (0, Ticks::ZERO));
    let take = entry(&list, (s.vocal, s.takes[0]));
    assert_eq!((take.kind, take.count), (Kind::Audio, 2));
    let lone = entry(&list, (s.vocal, s.takes[2]));
    assert_eq!((lone.kind, lone.count, lone.link), (Kind::Audio, 1, None));
    assert_eq!(lone.name, take.name, "two entries may share a name");
    assert_ne!(lone.first, take.first, "…and are still told apart");
    let auto = entry(&list, (s.drums, s.auto));
    assert_eq!((auto.kind, auto.count), (Kind::Automation, 1));
    assert_eq!(entry(&list, (s.bass, s.riff)).track_index, 1);
    assert_eq!(entry(&list, (s.drums, s.fill)).start, Ticks::beats(12));
}

#[test]
fn groups_come_first_then_clips_of_their_own_each_by_name_then_place() {
    let s = scene();
    let firsts: Vec<_> = links::groups(&s.p).iter().map(|g| g.first).collect();
    assert_eq!(
        firsts,
        vec![
            (s.drums, s.beat[0]),
            (s.vocal, s.takes[0]),
            (s.drums, s.fill),
            (s.drums, s.auto),
            (s.bass, s.riff),
            (s.vocal, s.takes[2]),
        ]
    );
}

#[test]
fn the_order_does_not_depend_on_how_the_clips_are_stored() {
    let s = scene();
    let before = links::groups(&s.p);
    let mut p = s.p.clone();
    for t in &mut p.tracks {
        t.clips.reverse();
    }
    assert_eq!(links::groups(&p), before);
    // Names compare without case: "beat" sorts with "Beat", not after "Take".
    let mut p = s.p.clone();
    let i = p.tracks[1]
        .clips
        .iter()
        .position(|c| c.id == s.riff)
        .unwrap();
    p.tracks[1].clips[i].name = "alpha".into();
    let names: Vec<_> = links::groups(&p).into_iter().map(|g| g.name).collect();
    assert_eq!(
        names,
        vec!["Beat", "Take", "alpha", "Fill", "Level", "Take"]
    );
}

#[test]
fn a_group_of_one_is_a_clip_of_its_own_and_an_empty_project_lists_nothing() {
    let mut s = scene();
    // Two of the three beats leave their group: the last member is alone in it.
    for (t, c) in [(s.drums, s.beat[2]), (s.bass, s.beat[1])] {
        s.p.track_mut(t).unwrap().clip_mut(c).unwrap().link = None;
    }
    let list = links::groups(&s.p);
    let beat = entry(&list, (s.drums, s.beat[0]));
    assert_eq!(beat.count, 1);
    assert_eq!(list.len(), 8);
    assert_eq!(
        list[0].first,
        (s.vocal, s.takes[0]),
        "the only real group leads"
    );
    assert!(links::groups(&Project::default()).is_empty());
}

/// What `link_to` would join: the selected clips it gives a `SetClipLink` into a group.
fn joins(p: &Project, sel: &[(TrackId, ClipId)], target: (TrackId, ClipId)) -> usize {
    let mut next = p.next_id + 1000;
    links::link_to(p, sel, target, &mut || {
        next += 1;
        next
    })
    .iter()
    .filter(|c| {
        matches!(c, Command::SetClipLink { track, clip, link: Some(_) }
            if (*track, *clip) != target)
    })
    .count()
}

#[test]
fn would_link_says_exactly_what_link_to_would_do() {
    let s = scene();
    let all: Vec<(TrackId, ClipId)> =
        s.p.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.id)))
            .collect();
    // Every target against every selection of one or two clips.
    let mut seen_ok = 0;
    for &target in &all {
        for (i, &a) in all.iter().enumerate() {
            for &b in &all[i..] {
                let sel = if a == b { vec![a] } else { vec![a, b] };
                let n = joins(&s.p, &sel, target);
                match links::would_link(&s.p, &sel, target) {
                    Ok(k) => {
                        assert_eq!(k, n, "{sel:?} → {target:?}");
                        assert!(k > 0);
                        seen_ok += 1;
                    }
                    Err(why) => assert_eq!(n, 0, "{sel:?} → {target:?} refused as {why:?}"),
                }
            }
        }
    }
    assert!(seen_ok > 20, "the scene offers real links ({seen_ok})");
}

#[test]
fn a_refusal_names_its_reason() {
    let s = scene();
    let (d, b, v) = (s.drums, s.bass, s.vocal);
    // Notes onto audio, and audio onto notes: another kind.
    assert_eq!(
        links::would_link(&s.p, &[(b, s.riff)], (v, s.takes[0])),
        Err(Refusal::Kind)
    );
    assert_eq!(
        links::would_link(&s.p, &[(v, s.takes[2])], (d, s.fill)),
        Err(Refusal::Kind)
    );
    // Already in the group, or the target itself.
    assert_eq!(
        links::would_link(&s.p, &[(b, s.beat[1])], (d, s.beat[0])),
        Err(Refusal::Already)
    );
    assert_eq!(
        links::would_link(&s.p, &[(d, s.fill)], (d, s.fill)),
        Err(Refusal::Already)
    );
    // An automation clip of another parameter.
    let mut p = s.p.clone();
    let mut pan = p.make_clip("Pan", Ticks::beats(8), Ticks::beats(4));
    pan.automation = Some(Automation::new(AutoTarget::TrackPan(d), Vec::new()));
    let pan_id = pan.id;
    p.track_mut(d).unwrap().clips.push(pan);
    assert_eq!(
        links::would_link(&p, &[(d, pan_id)], (d, s.auto)),
        Err(Refusal::Target)
    );
    // A mixed selection links what it can: the lone take joins the takes, the riff not.
    assert_eq!(
        links::would_link(&s.p, &[(b, s.riff), (v, s.takes[2])], (v, s.takes[0])),
        Ok(1)
    );
    // No selection, or a target that is gone: nothing to link.
    assert_eq!(
        links::would_link(&s.p, &[], (d, s.fill)),
        Err(Refusal::Already)
    );
    assert_eq!(
        links::would_link(&s.p, &[(d, s.fill)], (d, ClipId(999_999))),
        Err(Refusal::Gone)
    );
}

#[test]
fn a_rename_through_any_member_renames_the_group_in_the_list() {
    let s = scene();
    let mut bus = CommandBus::new(s.p.clone());
    bus.execute(
        Command::SetClipName {
            track: s.bass,
            clip: s.beat[1],
            name: "Groove".into(),
        },
        Origin::User,
    )
    .unwrap();
    let list = links::groups(bus.project());
    assert_eq!(entry(&list, (s.drums, s.beat[0])).name, "Groove");
    assert!(!list.iter().any(|g| g.name == "Beat"));
    bus.undo();
    assert_eq!(
        entry(&links::groups(bus.project()), (s.drums, s.beat[0])).name,
        "Beat"
    );
}

/// A selection of mixed kinds that holds the target's own group (or the target) is refused
/// as *already the same clip*, not as *another kind*: that is the reason that applies to
/// the clip the user can see is the same.
#[test]
fn already_wins_over_another_kind_in_a_mixed_selection() {
    let s = scene();
    let (d, b, v) = (s.drums, s.bass, s.vocal);
    // A beat (in the beat group) and a take (audio): onto the beat group.
    assert_eq!(
        links::would_link(&s.p, &[(b, s.beat[1]), (v, s.takes[0])], (d, s.beat[0])),
        Err(Refusal::Already)
    );
    // The fill itself and an audio take: onto the fill.
    assert_eq!(
        links::would_link(&s.p, &[(d, s.fill), (v, s.takes[2])], (d, s.fill)),
        Err(Refusal::Already)
    );
    // The take group's own member and the riff (notes): onto the takes.
    assert_eq!(
        links::would_link(&s.p, &[(v, s.takes[1]), (b, s.riff)], (v, s.takes[0])),
        Err(Refusal::Already)
    );
}

/// Names with numbers sort as a musician numbers them: "Groove 2" before "Groove 10";
/// equal names keep their order by place.
#[test]
fn numbered_names_sort_naturally() {
    let mut p = Project::default();
    let mut t = p.make_track("T");
    for (i, name) in [
        "Groove 10",
        "Groove 2",
        "groove 1",
        "Groove 02",
        "Groove 2",
        "Groove",
    ]
    .into_iter()
    .enumerate()
    {
        let c = note_clip(&mut p, name, i as i64, 60);
        t.clips.push(c);
    }
    p.tracks.push(t);
    let names: Vec<_> = links::groups(&p)
        .into_iter()
        .map(|g| (g.name, g.start))
        .collect();
    let bar = |b: i64| Ticks::beats(4 * b);
    assert_eq!(
        names,
        vec![
            ("Groove".to_string(), bar(5)),
            ("groove 1".to_string(), bar(2)),
            ("Groove 02".to_string(), bar(3)),
            ("Groove 2".to_string(), bar(1)),
            ("Groove 2".to_string(), bar(4)),
            ("Groove 10".to_string(), bar(0)),
        ]
    );
}

/// The selection summary the picker answers every row from (once per rebuild, not once
/// per row) says exactly what `would_link` says, for every entry and many selections —
/// automation of two targets, a target in the selection, clips missing, duplicates.
#[test]
fn the_selection_summary_agrees_with_would_link_for_every_entry() {
    let s = scene();
    let mut p = s.p.clone();
    let mut pan = p.make_clip("Pan", Ticks::beats(8), Ticks::beats(4));
    pan.automation = Some(Automation::new(AutoTarget::TrackPan(s.drums), Vec::new()));
    p.track_mut(s.drums).unwrap().clips.push(pan);
    let mut all: Vec<(TrackId, ClipId)> = p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.id)))
        .collect();
    all.push((s.drums, ClipId(999_999)));
    let groups = links::groups(&p);
    let mut checked = 0;
    for (i, &a) in all.iter().enumerate() {
        for (j, &b) in all.iter().enumerate().skip(i) {
            for &c in &all[j..] {
                for sel in [vec![a], vec![a, b], vec![a, b, c], vec![a, a, b]] {
                    let summary = links::Selection::of(&p, &sel);
                    for g in &groups {
                        assert_eq!(
                            summary.would_link(g),
                            links::would_link(&p, &sel, g.first),
                            "{sel:?} → {g:?}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 5_000, "{checked}");
    assert_eq!(
        links::Selection::of(&p, &[]).would_link(&groups[0]),
        links::would_link(&p, &[], groups[0].first)
    );
}
