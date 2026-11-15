//! r40:pads — a pad's own mixer track, chosen per pad from the rack (ADR 0117), on the
//! engine: a pad sent to a track of its own sounds there and only there — the track's stem
//! is that pad alone and the kit's strip is the kit without it, to the bit — at every block
//! size; a pad sent back renders the song as it was, to the bit.

use cantus_core::padout::set_pad_track;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// A busy two-bar groove on every pad of the studio kit, and a synth on a strip of its own.
fn song() -> Project {
    let mut p = Project::new("Pads");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        for k in 0..1 + (step % 3) {
            let pitch = pitches[((step * 5 + k * 3) as usize) % pitches.len()];
            let vel = 50 + ((step * 17 + k * 29) % 77) as u8;
            c.notes.push(p.make_note(at, Ticks(PPQ / 3), pitch, vel));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// `p` with its kit's pad `k` (by pitch) keeping only, or losing, its notes.
fn notes_of(p: &Project, k: usize, keep: bool) -> Project {
    let mut q = p.clone();
    let pitch = q.tracks[0].instrument.pulsus().unwrap().pads[k].pitch;
    for c in &mut q.tracks[0].clips {
        c.notes.retain(|n| (n.pitch == pitch) == keep);
    }
    q
}

/// `p` with pad `k` sent to mixer track `to` as the rack's well sends it.
fn routed(p: &Project, k: usize, to: Option<StripId>) -> (Project, Command) {
    let mut q = p.clone();
    let pad = q.tracks[0].instrument.pulsus().unwrap().pads[k].id;
    let cmd = set_pad_track(&q, q.tracks[0].id, pad, to).unwrap();
    let undo = cmd.apply(&mut q).unwrap();
    (q, undo)
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            block_frames: block,
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// The stems of strips `strips` (indices into the desk) of `p`.
fn strip_stems(p: &Project, strips: &[usize]) -> Vec<Vec<f32>> {
    let opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let specs: Vec<StemSpec> = strips
        .iter()
        .map(|i| StemSpec {
            source: StemSource::Strip(*i),
            file_name: format!("s{i}"),
        })
        .collect();
    render_stems(Arc::new(Snapshot::compile(p, 1)), &opts, &specs)
        .unwrap()
        .into_iter()
        .map(|s| s.samples)
        .collect()
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs: {:e} vs {:e}", a[i], b[i]);
    }
}

fn energy(a: &[f32]) -> f64 {
    a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum()
}

#[test]
fn a_pad_sent_to_its_own_track_sounds_there_and_only_there() {
    let p = song();
    let kit = p.tracks[0].strip.unwrap();
    let own = p.first_free_strip().unwrap();
    let (ki, oi) = (p.strip_index(kit).unwrap(), p.strip_index(own).unwrap());
    for k in [0usize, 1, 2] {
        let (q, _) = routed(&p, k, Some(own));
        let got = strip_stems(&q, &[oi, ki]);
        // Its own track: the pad alone, as the kit's strip played it alone.
        let alone = strip_stems(&notes_of(&p, k, true), &[ki]);
        same_bits(&got[0], &alone[0], &format!("pad {k} on its own track"));
        assert!(energy(&got[0]) > 0.0, "pad {k}: its own track sounds");
        // The kit's strip: the kit without it.
        let rest = strip_stems(&notes_of(&p, k, false), &[ki]);
        same_bits(&got[1], &rest[0], &format!("the kit without pad {k}"));
    }
}

#[test]
fn a_pad_on_its_own_track_is_block_size_independent() {
    let p = song();
    let own = p.first_free_strip().unwrap();
    let (q, _) = routed(&p, 0, Some(own));
    let base = render_at(&q, 512);
    assert!(energy(&base) > 0.0);
    for block in [1usize, 64, 333, 4096] {
        same_bits(&render_at(&q, block), &base, &format!("block {block}"));
    }
}

#[test]
fn a_pad_sent_back_renders_the_song_as_it_was_to_the_bit() {
    let p = song();
    let before = render_at(&p, 512);
    let own = p.first_free_strip().unwrap();
    let (mut q, undo) = routed(&p, 0, Some(own));
    // Sent back by its inverse (undo), and by the well's own "the kit's track".
    let mut r = q.clone();
    undo.apply(&mut r).unwrap();
    assert_eq!(r, p);
    same_bits(&render_at(&r, 512), &before, "undone");
    let pad = q.tracks[0].instrument.pulsus().unwrap().pads[0].id;
    let back = set_pad_track(&q, q.tracks[0].id, pad, None).unwrap();
    back.apply(&mut q).unwrap();
    same_bits(&render_at(&q, 512), &before, "sent back to the kit's track");
}

#[test]
fn a_pad_shares_another_channels_track_and_its_inserts_and_fader() {
    let mut p = song();
    let keys = p.make_track("Keys");
    let keys_strip = keys.strip.unwrap();
    p.tracks.push(keys);
    let (mut q, _) = routed(&p, 0, Some(keys_strip));
    let ki = q.strip_index(keys_strip).unwrap();
    let alone = strip_stems(&notes_of(&q, 0, true), &[ki])[0].clone();
    assert!(energy(&alone) > 0.0, "the kick is heard on the keys' strip");
    // The strip's fader is the pad's now: down 6 dB on the strip, the kick follows.
    q.strip_mut(keys_strip).unwrap().gain_db = -6.0;
    let down = strip_stems(&notes_of(&q, 0, true), &[ki])[0].clone();
    let ratio = (energy(&down) / energy(&alone)).sqrt();
    assert!(
        (ratio - 0.501).abs() < 0.01,
        "the strip's fader moves it: {ratio}"
    );
}
