use super::*;
use crate::model::Project;
use crate::AudioSourceId;

fn clip(p: &mut Project, start: i64, len: i64) -> Clip {
    p.make_audio_clip("c", AudioSourceId(1), Ticks(start), Ticks(len))
}

/// The earlier window `r` crosses from, through `overlaps_of_track`.
fn partner(clips: &[Clip], r: &Clip) -> Option<ClipId> {
    let mut p = Project::new("x");
    let mut t = p.make_track("T");
    t.clips = clips.to_vec();
    overlaps_of_track(&t)
        .into_iter()
        .find(|o| o.in_right(r.id))
        .map(|o| o.left_last)
}

/// The rule clip by clip, as round 31 first wrote it (no split's pieces joined).
fn naive_partner<'a>(clips: &'a [Clip], right: &Clip) -> Option<&'a Clip> {
    if !window(right) {
        return None;
    }
    clips
        .iter()
        .filter(|l| {
            l.id != right.id
                && window(l)
                && l.start < right.start
                && l.end() > right.start
                && l.end() <= right.end()
        })
        .max_by_key(|l| (l.start, l.id.0))
}

#[test]
fn equal_power_is_the_seam_law_and_the_others_sum_to_one() {
    for k in 0..=100 {
        let u = k as f32 / 100.0;
        let (i, o) = XfadeCurve::EqualPower.gains(u);
        assert_eq!(i, math::sin(std::f32::consts::FRAC_PI_2 * u));
        assert_eq!(o, math::cos(std::f32::consts::FRAC_PI_2 * u.min(1.0)));
        assert!((i * i + o * o - 1.0).abs() < 1e-5);
        for c in [XfadeCurve::Linear, XfadeCurve::SCurve] {
            let (i, o) = c.gains(u);
            assert!((i + o - 1.0).abs() < 1e-6, "{c:?} at {u}");
        }
    }
    assert_eq!(XfadeCurve::SCurve.gain_in(0.0), 0.0);
    assert_eq!(XfadeCurve::SCurve.gain_in(1.0), 1.0);
}

#[test]
fn a_partner_starts_before_and_ends_inside() {
    let mut p = Project::new("x");
    let a = clip(&mut p, 0, 100);
    let b = clip(&mut p, 60, 100);
    let inside = clip(&mut p, 10, 20);
    let abut = clip(&mut p, 160, 50);
    let clips = vec![a.clone(), b.clone(), inside.clone(), abut.clone()];
    assert_eq!(partner(&clips, &b), Some(a.id));
    assert_eq!(overlap(&a, &b), (Ticks(60), Ticks(100)));
    // Wholly inside another: layered, not crossed.
    assert!(partner(&clips, &inside).is_none());
    // Meeting at an edge is a butt join (or a comp seam), not an overlap.
    assert!(partner(&clips, &abut).is_none());
    // Muted, or on a take lane: out of it.
    let mut m = a.clone();
    m.muted = true;
    assert!(partner(&[m, b.clone()], &b).is_none());
    let mut l = a.clone();
    l.lane = 1;
    assert!(partner(&[l, b.clone()], &b).is_none());
}

#[test]
fn the_latest_starting_partner_wins() {
    let mut p = Project::new("x");
    let a = clip(&mut p, 0, 100);
    let b = clip(&mut p, 40, 80);
    let c = clip(&mut p, 90, 100);
    let clips = vec![a.clone(), b.clone(), c.clone()];
    assert_eq!(partner(&clips, &c), Some(b.id));
}

#[test]
fn an_overlap_made_by_an_edit_gets_a_crossfade_and_an_old_one_keeps_its_choice() {
    let mut p = Project::new("x");
    let mut t = p.make_track("A");
    let a = clip(&mut p, 0, 100);
    let mut b = clip(&mut p, 200, 100);
    t.clips.push(a.clone());
    b.audio.as_mut().unwrap().cross = None;
    t.clips.push(b.clone());
    let tid = t.id;
    p.tracks.push(t);
    let before = pairs(&p);
    assert!(before.is_empty());
    p.tracks[0].clips[1].start = Ticks(50);
    let fix = fresh(&before, &p, Some(XfadeCurve::EqualPower));
    assert_eq!(fix.len(), 1);
    match &fix[0] {
        Command::SetClipAudio { track, clip, audio } => {
            assert_eq!((*track, *clip), (tid, b.id));
            assert_eq!(audio.as_ref().unwrap().cross, Some(XfadeCurve::EqualPower));
        }
        other => panic!("{other:?}"),
    }
    // The same overlap before and after: nothing to do.
    let now = pairs(&p);
    assert!(fresh(&now, &p, Some(XfadeCurve::Linear)).is_empty());
}

#[test]
fn may_overlap_knows_the_clip_moves() {
    let c = Command::SetClipBounds {
        track: TrackId(1),
        clip: ClipId(2),
        start: Ticks(0),
        length: Ticks(5),
    };
    assert!(may_overlap(&c));
    assert!(may_overlap(&Command::Batch {
        label_key: "x".into(),
        commands: vec![c]
    }));
    assert!(!may_overlap(&Command::SetTempo { bpm: 100.0 }));
}

#[test]
fn a_resize_keeps_the_centre_and_each_clips_limits() {
    let mut p = Project::new("x");
    let a = clip(&mut p, 0, 100);
    let mut b = clip(&mut p, 60, 100);
    b.offset = Ticks(60);
    let tid = TrackId(9);
    // 40 → 20, centred on 80: the right starts at 70, the left ends at 90.
    let cmds = resize(tid, &a, &b, Ticks(20), (Ticks(1000), Ticks(1000))).unwrap();
    assert!(cmds.contains(&Command::SetClipBounds {
        track: tid,
        clip: a.id,
        start: Ticks(0),
        length: Ticks(90),
    }));
    assert!(cmds.contains(&Command::SetClipBounds {
        track: tid,
        clip: b.id,
        start: Ticks(70),
        length: Ticks(90),
    }));
    assert!(cmds.contains(&Command::SetClipOffset {
        track: tid,
        clip: b.id,
        offset: Ticks(70),
    }));
    // Longer than the files allow: only as far as they have audio.
    let cmds = resize(tid, &a, &b, Ticks(400), (Ticks(5), Ticks(7))).unwrap();
    assert!(cmds.contains(&Command::SetClipBounds {
        track: tid,
        clip: a.id,
        start: Ticks(0),
        length: Ticks(107),
    }));
    assert!(cmds.contains(&Command::SetClipBounds {
        track: tid,
        clip: b.id,
        start: Ticks(55),
        length: Ticks(105),
    }));
    // The length it has: nothing.
    assert!(resize(tid, &a, &b, Ticks(40), (Ticks(0), Ticks(0))).is_none());
}

/// The sweep finds what `partner` finds, clip by clip, on random layouts.
#[test]
fn the_sweep_agrees_with_partner() {
    let mut seed = 7u64;
    let mut next = |n: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) % n
    };
    for _ in 0..200 {
        let mut p = Project::new("x");
        let mut t = p.make_track("T");
        for _ in 0..12 {
            let mut c = clip(&mut p, next(40) as i64 * 10, 5 + next(60) as i64 * 5);
            c.muted = next(9) == 0;
            c.lane = u8::from(next(8) == 0);
            t.clips.push(c);
        }
        let mut sweep: Vec<(ClipId, ClipId)> = overlaps_of_track(&t)
            .into_iter()
            .map(|o| (o.left_last, o.right_first))
            .collect();
        sweep.sort_by_key(|(l, r)| (r.0, l.0));
        let mut each: Vec<(ClipId, ClipId)> = t
            .clips
            .iter()
            .filter_map(|r| naive_partner(&t.clips, r).map(|l| (l.id, r.id)))
            .collect();
        each.sort_by_key(|(l, r)| (r.0, l.0));
        assert_eq!(sweep, each);
    }
}

/// Review 1 (ADR 0070): a split's halves are one window — split inside a crossed overlap,
/// either clip still crosses over the whole overlap.
#[test]
fn a_splits_halves_cross_as_the_window_they_were() {
    let mut p = Project::new("x");
    let mut l = clip(&mut p, 0, 80);
    let mut r = clip(&mut p, 40, 80);
    r.offset = Ticks(500);
    r.audio.as_mut().unwrap().cross = Some(XfadeCurve::Linear);
    // The left split at 60: two pieces, the second going on from the first.
    let mut l2 = l.clone();
    l2.id = ClipId(p.alloc_id());
    l2.start = Ticks(60);
    l2.length = Ticks(20);
    l2.offset = Ticks(60);
    l.length = Ticks(60);
    assert!(continues(&l, &l2));
    let mut t = p.make_track("T");
    t.clips = vec![l.clone(), r.clone(), l2.clone()];
    let o = overlaps_of_track(&t);
    assert_eq!(o.len(), 1);
    assert_eq!((o[0].from, o[0].to), (Ticks(40), Ticks(80)));
    assert_eq!(o[0].left, vec![l.id, l2.id]);
    assert_eq!((o[0].left_last, o[0].right_first), (l2.id, r.id));
    assert_eq!(o[0].curve, Some(XfadeCurve::Linear));
    // The right split at 60 instead: its pieces both cross in.
    let mut r2 = r.clone();
    r2.id = ClipId(p.alloc_id());
    r2.start = Ticks(60);
    r2.length = Ticks(60);
    r2.offset = Ticks(520);
    r2.audio.as_mut().unwrap().cross = None;
    let mut r1 = r.clone();
    r1.length = Ticks(20);
    let whole_l = clip(&mut p, 0, 80);
    t.clips = vec![whole_l.clone(), r1.clone(), r2.clone()];
    let o = overlaps_of_track(&t);
    assert_eq!(o.len(), 1);
    assert_eq!((o[0].from, o[0].to), (Ticks(40), Ticks(80)));
    assert_eq!(o[0].right, vec![r1.id, r2.id]);
    assert_eq!(o[0].curve, Some(XfadeCurve::Linear));
}

/// Review 1: thousands of clips cost what sorting them does.
#[test]
fn many_clips_are_found_in_one_sweep() {
    let mut p = Project::new("x");
    let mut t = p.make_track("T");
    for k in 0..20_000i64 {
        let mut c = clip(&mut p, k * 100, 150);
        c.offset = Ticks(k * 1_000_000);
        t.clips.push(c);
    }
    let t0 = std::time::Instant::now();
    let o = overlaps_of_track(&t);
    assert_eq!(o.len(), 19_999);
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        t0.elapsed()
    );
}

/// Review 1: a clip that leaves its overlap loses its stale curve, whatever the setting;
/// a linked copy keeps it (its siblings share it).
#[test]
fn a_clip_that_leaves_its_overlap_drops_its_curve() {
    let mut p = Project::new("x");
    let mut t = p.make_track("A");
    let a = clip(&mut p, 0, 100);
    let mut b = clip(&mut p, 50, 100);
    b.audio.as_mut().unwrap().cross = Some(XfadeCurve::SCurve);
    let bid = b.id;
    t.clips.extend([a, b]);
    p.tracks.push(t);
    let before = pairs(&p);
    assert_eq!(before.len(), 1);
    p.tracks[0].clips[1].start = Ticks(500);
    let fix = fresh(&before, &p, None);
    assert!(
        matches!(&fix[..], [Command::SetClipAudio { clip, audio: Some(a), .. }]
        if *clip == bid && a.cross.is_none())
    );
    p.tracks[0].clips[1].link = Some(crate::model::LinkId(7));
    assert!(fresh(&before, &p, None).is_empty());
}
