//! Delay's tests: the free time as it always was (round 39 and before), and the synced time
//! (round 40, ADR 0119).

use super::*;

#[test]
fn echo_lands_at_the_right_sample() {
    let mut d = StereoDelay::new(48_000.0);
    d.set(&DelaySettings {
        time_ms: 10.0,
        feedback: 0.0,
        tone_hz: 20_000.0,
        ping_pong: false,
        mix: 1.0,
        ..Default::default()
    });
    d.reset();
    let n = 2_000;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    l[0] = 1.0;
    r[0] = 1.0;
    d.process(&mut l, &mut r);
    let peak_at = l
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    assert_eq!(peak_at, 480);
    assert!(l[480] > 0.9);
}

#[test]
fn ping_pong_alternates_channels_and_decays() {
    let mut d = StereoDelay::new(48_000.0);
    d.set(&DelaySettings {
        time_ms: 5.0,
        feedback: 0.5,
        tone_hz: 20_000.0,
        ping_pong: true,
        mix: 1.0,
        ..Default::default()
    });
    d.reset();
    let n = 2_000;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    l[0] = 1.0;
    d.process(&mut l, &mut r);
    // First echo on the left (input goes into the left line), second on the right.
    assert!(
        l[240].abs() > 0.4 && r[240].abs() < 1e-3,
        "l {} r {}",
        l[240],
        r[240]
    );
    assert!(
        r[480].abs() > 0.2 && l[480].abs() < 1e-3,
        "l {} r {}",
        l[480],
        r[480]
    );
    assert!(l[720].abs() < l[240].abs());
    assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()));
}

#[test]
fn deterministic_and_silent_on_silence() {
    let run = || {
        let mut d = StereoDelay::new(44_100.0);
        let mut l: Vec<f32> = (0..4_000).map(|i| math::sin((i as f32) * 0.01)).collect();
        let mut r = l.clone();
        d.process(&mut l, &mut r);
        (l, r)
    };
    assert_eq!(run(), run());
    let mut d = StereoDelay::new(44_100.0);
    let mut l = vec![0.0; 1_000];
    let mut r = vec![0.0; 1_000];
    d.process(&mut l, &mut r);
    assert!(l.iter().chain(r.iter()).all(|x| *x == 0.0));
}

// ---- round 40: the synced time (ADR 0119) ------------------------------------------------

const SR: f32 = 48_000.0;

/// The unit as it was before round 40, line for line (`git show 878fd4cc:…/delay.rs`): what
/// the sync switched off must still be, to the bit.
struct Before {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    time: Smoother,
    mix: Smoother,
    lp_l: f32,
    lp_r: f32,
    lp_coef: f32,
    s: DelaySettings,
}

impl Before {
    fn new(sample_rate: f32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + 2;
        let s = DelaySettings::default();
        let mut d = Before {
            sample_rate,
            buf_l: vec![0.0; len],
            buf_r: vec![0.0; len],
            write: 0,
            time: Smoother::new(s.time_ms * 0.001 * sample_rate, 0.05, sample_rate),
            mix: Smoother::new(s.mix, 0.02, sample_rate),
            lp_l: 0.0,
            lp_r: 0.0,
            lp_coef: 1.0,
            s: s.clone(),
        };
        d.set(&s);
        d
    }

    fn set(&mut self, s: &DelaySettings) {
        let max_samples = (self.buf_l.len() - 2) as f32;
        let samples = (s.time_ms.max(0.0) * 0.001 * self.sample_rate).clamp(1.0, max_samples);
        self.time.set_target(samples);
        self.mix.set_target(s.mix.clamp(0.0, 1.0));
        let fc = s.tone_hz.clamp(20.0, self.sample_rate * 0.45);
        self.lp_coef = 1.0 - math::exp(-std::f32::consts::TAU * fc / self.sample_rate);
        self.s = s.clone();
    }

    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fb = self.s.feedback.clamp(0.0, 0.98);
        let ping_pong = self.s.ping_pong;
        let len = self.buf_l.len();
        for i in 0..l.len().min(r.len()) {
            let d = self.time.next();
            let mix = self.mix.next();
            let (in_l, in_r) = (l[i], r[i]);
            let rd_l = StereoDelay::read(&self.buf_l, self.write, d);
            let rd_r = StereoDelay::read(&self.buf_r, self.write, d);
            self.lp_l += self.lp_coef * (rd_l - self.lp_l);
            self.lp_r += self.lp_coef * (rd_r - self.lp_r);
            flush(&mut self.lp_l);
            flush(&mut self.lp_r);
            let (wr_l, wr_r) = if ping_pong {
                ((in_l + in_r) * 0.5 + fb * self.lp_r, fb * self.lp_l)
            } else {
                (in_l + fb * self.lp_l, in_r + fb * self.lp_r)
            };
            self.buf_l[self.write] = wr_l;
            self.buf_r[self.write] = wr_r;
            self.write = (self.write + 1) % len;
            l[i] = in_l + (rd_l - in_l) * mix;
            r[i] = in_r + (rd_r - in_r) * mix;
        }
    }
}

/// A test signal: a decaying chirp, then silence, then a burst.
fn signal(n: usize) -> (Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            if i < n / 3 || (i > n / 2 && i < n / 2 + 2_000) {
                0.6 * math::sin(t * (300.0 + 900.0 * t) * std::f32::consts::TAU)
                    * math::exp(-2.0 * t)
            } else {
                0.0
            }
        })
        .collect();
    let r = l
        .iter()
        .enumerate()
        .map(|(i, x)| x * if i % 2 == 0 { 0.8 } else { -0.3 })
        .collect();
    (l, r)
}

/// A synced unit with the line its settings want (as `EffectUnit::new` builds it).
fn synced(note: usize, bpm: f64, s: DelaySettings) -> StereoDelay {
    let mut d = StereoDelay::new(SR);
    d.set(&DelaySettings {
        sync: true,
        note,
        ..s
    });
    if let Some(len) = d.need() {
        assert!(d.supply(Box::new(Lines::new(len))).is_some());
    }
    d.set_tempo(bpm, false);
    d.reset();
    d
}

#[test]
fn the_sync_off_is_the_unit_before_round_40_to_the_bit() {
    let settings = [
        DelaySettings::default(),
        DelaySettings {
            time_ms: 123.4,
            feedback: 0.9,
            tone_hz: 2_500.0,
            ping_pong: false,
            mix: 0.7,
            // The note and the tempo mean nothing while the sync is off.
            note: 25,
            ..Default::default()
        },
    ];
    for s in settings {
        let (mut l0, mut r0) = signal(60_000);
        let (mut l1, mut r1) = (l0.clone(), r0.clone());
        let mut before = Before::new(SR);
        let mut now = StereoDelay::new(SR);
        before.set(&s);
        now.set(&s);
        assert_eq!(now.need(), None, "the free line is the line it always was");
        for (k, at) in (0..60_000).step_by(997).enumerate() {
            let end = (at + 997).min(60_000);
            now.set_tempo(90.0 + k as f64, k % 2 == 0);
            if k == 20 {
                // A time change halfway, gliding the same in both.
                let moved = DelaySettings {
                    time_ms: 640.0,
                    ..s.clone()
                };
                before.set(&moved);
                now.set(&moved);
            }
            before.process(&mut l0[at..end], &mut r0[at..end]);
            now.process(&mut l1[at..end], &mut r1[at..end]);
        }
        let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&l0), bits(&l1), "left, {s:?}");
        assert_eq!(bits(&r0), bits(&r1), "right, {s:?}");
    }
}

#[test]
fn the_note_values_are_the_ones_promised() {
    // 1/64 … 4/1, straight, dotted, triplet: in quarter notes.
    assert_eq!(sync::beats(0), 1.0 / 16.0);
    assert_eq!(sync::beats(1), 1.5 / 16.0);
    assert_eq!(sync::beats(2), 1.0 / 24.0);
    assert_eq!(sync::beats(12), 1.0, "a quarter");
    assert_eq!(sync::beats(9), 0.5, "an eighth");
    assert_eq!(sync::beats(sync::DEFAULT_NOTE), 0.75, "a dotted eighth");
    assert_eq!(sync::beats(24), 16.0, "4/1: four bars of 4/4");
    assert_eq!(sync::beats(26), 16.0 * 2.0 / 3.0);
    assert_eq!(sync::NOTES, 27);
    // The default note at 120 BPM is the free time's default: switching on moves nothing.
    let d = DelaySettings::default();
    assert_eq!(sync::time_ms(d.note, 120.0) as f32, d.time_ms);
}

#[test]
fn the_line_holds_the_note_at_the_slowest_tempo() {
    assert_eq!(sync::line_seconds(false, 24), 2.0, "the free line");
    assert_eq!(
        sync::line_seconds(true, 9),
        2.0,
        "an eighth at 20 BPM is 1.5 s"
    );
    assert_eq!(sync::line_seconds(true, 12), 3.0, "a quarter at 20 BPM");
    assert_eq!(sync::line_seconds(true, 18), 12.0, "a whole note");
    assert_eq!(sync::line_seconds(true, 24), 48.0, "4/1 at 20 BPM");
    assert_eq!(
        sync::line_seconds(true, 25),
        48.0,
        "a dotted 4/1: held to the longest"
    );
    let mut d = StereoDelay::new(SR);
    d.set(&DelaySettings {
        sync: true,
        note: 18,
        ..Default::default()
    });
    assert_eq!(d.need(), Some(sync::line_len(12.0, SR)));
    assert!(!d.at_rest(), "not as new while its line is on its way");
    let wrong = d.supply(Box::new(Lines::new(10))).unwrap();
    assert_eq!(
        wrong.len(),
        10,
        "a line of another length is handed back unused"
    );
    let old = d
        .supply(Box::new(Lines::new(sync::line_len(12.0, SR))))
        .unwrap();
    assert_eq!(
        old.len(),
        sync::line_len(2.0, SR),
        "the old line goes back to be freed"
    );
    assert_eq!(d.need(), None);
    d.set(&DelaySettings::default());
    assert_eq!(
        d.need(),
        Some(sync::line_len(2.0, SR)),
        "off again: the free line back"
    );
}

/// Where the loudest sample of `x` is.
fn peak(x: &[f32]) -> usize {
    x.iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .map(|(i, _)| i)
        .unwrap()
}

fn one_echo() -> DelaySettings {
    DelaySettings {
        feedback: 0.0,
        tone_hz: 20_000.0,
        ping_pong: false,
        mix: 1.0,
        ..Default::default()
    }
}

#[test]
fn each_note_lands_at_its_time_at_the_tempo() {
    for (note, bpm) in [
        (10, 120.0),
        (12, 90.0),
        (9, 140.0),
        (14, 77.0),
        (21, 60.0),
        (2, 200.0),
    ] {
        let mut d = synced(note, bpm, one_echo());
        let want = sync::time_samples(note, bpm, SR);
        let n = want.ceil() as usize + 100;
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        l[0] = 1.0;
        r[0] = 1.0;
        d.process(&mut l, &mut r);
        let at = peak(&l);
        assert!(
            (at as f64 - want).abs() <= 1.0,
            "note {note} at {bpm} BPM: at {at}, wanted {want}"
        );
        assert!(l[at] > 0.45, "note {note}: the echo whole ({})", l[at]);
    }
}

/// A run through tempo changes the way the engine tells them: before each pass, the tempo
/// of the pass; a change only ever at the start of a pass (`changes`: sample, BPM, whether
/// it is a ramp's step).
fn run_tempo(
    d: &mut StereoDelay,
    l: &mut [f32],
    r: &mut [f32],
    blocks: &[usize],
    changes: &[(usize, f64, bool)],
) {
    let mut at = 0;
    let mut k = 0;
    while at < l.len() {
        let mut end = (at + blocks[k % blocks.len()]).min(l.len());
        k += 1;
        // A pass never crosses a tempo step (ADR 0060 §4): it is cut there.
        if let Some((c, _, _)) = changes.iter().find(|(c, _, _)| *c > at && *c < end) {
            end = *c;
        }
        let (bpm, ramp) = changes
            .iter()
            .rev()
            .find(|(c, _, _)| *c <= at)
            .map_or((120.0, false), |(_, b, r)| (*b, *r));
        d.set_tempo(bpm, ramp);
        d.process(&mut l[at..end], &mut r[at..end]);
        at = end;
    }
}

/// A ramp from 120 down to 114.25 BPM in steps of a sixteenth of a beat (as the engine
/// tells one), then a jump up to 150 (a tempo marker).
fn ramp() -> Vec<(usize, f64, bool)> {
    let mut v: Vec<(usize, f64, bool)> = (0..24)
        .map(|i| (20_000 + i * 1_500, 120.0 - 0.25 * i as f64, i < 23))
        .collect();
    v.push((70_000, 150.0, false));
    v
}

#[test]
fn through_tempo_changes_any_block_size_is_the_same_to_the_bit() {
    let s = DelaySettings {
        feedback: 0.6,
        ..Default::default()
    };
    let render = |blocks: &[usize]| {
        let mut d = synced(12, 120.0, s.clone());
        let (mut l, mut r) = signal(96_000);
        run_tempo(&mut d, &mut l, &mut r, blocks, &ramp());
        l.extend_from_slice(&r);
        l.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    };
    let reference = render(&[96_000]);
    for blocks in [&[1][..], &[7], &[64], &[256], &[1000, 3, 512, 17, 4096]] {
        assert!(render(blocks) == reference, "blocks of {blocks:?}");
    }
}

#[test]
fn a_tempo_change_glides_the_echo_without_a_click() {
    // A steady sine through a quarter-note echo, fully wet, the tempo ramping down in steps
    // and jumping to 150: the output's steepest step stays near the sine's own (no click,
    // no zipper, no sweep), and after the jump the echo is at the new time.
    let s = DelaySettings {
        feedback: 0.5,
        tone_hz: 20_000.0,
        ping_pong: false,
        mix: 1.0,
        ..Default::default()
    };
    let n = 110_000;
    let x: Vec<f32> = (0..n)
        .map(|i| 0.5 * math::sin(i as f32 * 220.0 * std::f32::consts::TAU / SR))
        .collect();
    let mut d = synced(12, 120.0, s.clone());
    let (mut l, mut r) = (x.clone(), x.clone());
    run_tempo(&mut d, &mut l, &mut r, &[128], &ramp());
    let steepest = |v: &[f32]| {
        v.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    // The sine's steepest step, times the feedback's sum (1 / (1 - 0.5)), with room for
    // the pitch the ramp bends (a tape's, a few per cent).
    let bound = steepest(&x) * 2.0 * 1.15;
    let got = steepest(&l[30_000..]);
    assert!(
        got <= bound,
        "steepest step {got}, the sine's {}",
        steepest(&x)
    );
    assert!(l.iter().all(|v| v.is_finite()));
    // Landed: at 150 BPM a quarter is 19 200 samples.
    assert_eq!(d.tempo(), 150.0);
    assert!(d.synced.is_settled());
    assert_eq!(d.synced.current, sync::time_samples(12, 150.0, SR));
    // Inside the ramp the head goes straight from step to step: half a step (750 of its
    // 1 503 samples at 119.75 BPM) after the second step, it is half way there.
    let mut d = synced(12, 120.0, s);
    let (mut l, mut r) = (x.clone(), x.clone());
    run_tempo(&mut d, &mut l[..22_250], &mut r[..22_250], &[128], &ramp());
    let (t0, t1) = (
        sync::time_samples(12, 120.0, SR),
        sync::time_samples(12, 119.75, SR),
    );
    assert_eq!(d.synced.target, t1);
    let want = t0 + (t1 - t0) * 750.0 / 1_503.0;
    assert!(
        (d.synced.current - want).abs() < 1e-6,
        "{} {want}",
        d.synced.current
    );
}

#[test]
fn a_line_that_grows_mid_song_changes_no_sample() {
    // A half note at 120 BPM is a second: the free line holds it, but the sync asks for the
    // line a half note needs at 20 BPM. Played on the old line until the new one arrives,
    // then on the new: the same, to the bit, as with the new line from the start.
    let s = DelaySettings {
        feedback: 0.7,
        ..Default::default()
    };
    let full = || {
        let mut d = synced(15, 120.0, s.clone());
        let (mut l, mut r) = signal(150_000);
        d.process(&mut l, &mut r);
        (l, r)
    };
    let (want_l, want_r) = full();
    let mut d = StereoDelay::new(SR);
    d.set(&DelaySettings {
        sync: true,
        note: 15,
        ..s.clone()
    });
    d.set_tempo(120.0, false);
    d.reset();
    let len = d.need().expect("a longer line is asked for");
    let (mut l, mut r) = signal(150_000);
    d.process(&mut l[..70_001], &mut r[..70_001]);
    // The history moves in over the next calls (A-M3): nothing to free yet.
    assert!(d.supply(Box::new(Lines::new(len))).is_none());
    d.process(&mut l[70_001..], &mut r[70_001..]);
    assert!(l == want_l && r == want_r);
    let old = d.take_spent().expect("the old line, to be freed");
    assert_eq!(old.len(), sync::line_len(2.0, SR));
    assert!(!d.busy());
}

#[test]
fn held_to_its_line_until_the_longer_one_comes() {
    // A whole note at 60 BPM is 4 s: on the free line it is held to 2 s; the 12 s line
    // lets it reach.
    let mut d = StereoDelay::new(SR);
    d.set(&DelaySettings {
        sync: true,
        note: 18,
        ..one_echo()
    });
    d.set_tempo(60.0, false);
    d.reset();
    assert_eq!(d.synced.current, f64::from(d.max_samples()));
    let len = d.need().unwrap();
    d.supply(Box::new(Lines::new(len)));
    d.reset();
    assert_eq!(d.synced.current, 4.0 * f64::from(SR));
}

#[test]
fn at_rest_a_new_tempo_is_taken_at_once_and_skipping_is_exact() {
    let s = DelaySettings {
        feedback: 0.5,
        ..Default::default()
    };
    let mut ran = synced(12, 120.0, s.clone());
    let mut skipped = synced(12, 120.0, s.clone());
    assert!(ran.at_rest() && skipped.at_rest());
    // Silence, a new tempo: one processes it, the other skips it.
    let (mut zl, mut zr) = (vec![0.0f32; 5_000], vec![0.0f32; 5_000]);
    ran.set_tempo(97.0, false);
    skipped.set_tempo(97.0, false);
    assert!(skipped.at_rest(), "silent lines take the tempo at once");
    ran.process(&mut zl, &mut zr);
    assert!(zl.iter().chain(&zr).all(|v| v.to_bits() == 0));
    skipped.skip(5_000);
    let (mut l0, mut r0) = signal(60_000);
    let (mut l1, mut r1) = (l0.clone(), r0.clone());
    ran.process(&mut l0, &mut r0);
    skipped.process(&mut l1, &mut r1);
    assert!(l0 == l1 && r0 == r1);
}

#[test]
fn back_to_the_free_line_it_is_as_good_as_new() {
    // On, a 12 s line, sound; off, the free line back; silence long enough to empty it:
    // then it plays as a new unit skipped as far does.
    let mut d = synced(18, 120.0, DelaySettings::default());
    let (mut l, mut r) = signal(30_000);
    d.process(&mut l, &mut r);
    d.set(&DelaySettings::default());
    let len = d.need().unwrap();
    d.supply(Box::new(Lines::new(len)));
    // The repeats fall 0.4 each, flushed under 1e-20 by the tone: some 18 s.
    let quiet = 1_200_000;
    let (mut zl, mut zr) = (vec![0.0f32; quiet], vec![0.0f32; quiet]);
    d.process(&mut zl, &mut zr);
    assert!(d.at_rest());
    let mut new = StereoDelay::new(SR);
    new.skip(30_000 + quiet);
    let (mut l0, mut r0) = signal(50_000);
    let (mut l1, mut r1) = (l0.clone(), r0.clone());
    d.process(&mut l0, &mut r0);
    new.process(&mut l1, &mut r1);
    assert!(l0 == l1 && r0 == r1);
}

#[test]
fn sync_switched_on_crossfades_once_to_the_tempo_it_is_told() {
    // fix-a (A-L1): with the sync off the engine tells the unit no tempo (it does not follow
    // the song), so the unit's last tempo is stale — 120 here, the song at 90. Turned on,
    // it waits for the pass's tempo and crossfades once, straight to the 90-BPM time.
    let s = DelaySettings {
        feedback: 0.5,
        ..Default::default()
    };
    let mut d = StereoDelay::new(SR);
    d.set(&s);
    d.reset();
    let (mut l, mut r) = signal(30_000);
    d.process(&mut l, &mut r);
    assert_eq!(d.tempo(), 120.0, "never told");
    d.set(&DelaySettings {
        sync: true,
        note: 12,
        ..s
    });
    // The engine's next pass: the unit follows the song now, and is told its tempo first.
    d.set_tempo(90.0, false);
    let t90 = sync::time_samples(12, 90.0, SR);
    assert_eq!(
        d.synced.current, t90,
        "the crossfade goes to the 90-BPM time"
    );
    assert_eq!(d.synced.goal(), t90, "and nothing waits after it");
    let (mut l, mut r) = (vec![0.0; 2_400], vec![0.0; 2_400]);
    d.process(&mut l, &mut r);
    assert!(d.synced.is_settled());
    assert_eq!(d.synced.current, t90);
    // Never told a tempo at all (a host with no transport): it goes at its first pass.
    let mut d = StereoDelay::new(SR);
    d.set(&s);
    d.reset();
    let (mut l, mut r) = signal(30_000);
    d.process(&mut l, &mut r);
    d.set(&DelaySettings {
        sync: true,
        note: 12,
        ..s
    });
    let (mut l, mut r) = (vec![0.0; 2_400], vec![0.0; 2_400]);
    d.process(&mut l, &mut r);
    assert!(d.synced.is_settled());
    assert_eq!(d.synced.current, sync::time_samples(12, 120.0, SR));
}

/// fix-a (A-M3): the most a call may copy or clear, both sides, for `frames`.
fn bound(frames: usize) -> usize {
    2 * 2 * (frames + MAINTAIN_CELLS)
}

#[test]
fn a_long_line_moves_in_a_bounded_stretch_per_call() {
    // 2/1 on its 24 s line for 27 s of sound, then 4/1 (a 48 s line): the history moves in
    // a stretch per call — never the 1.15 M cells a side at once — and the output is the
    // unit's that had the long line from the start, to the bit.
    let s = DelaySettings {
        feedback: 0.7,
        ..Default::default()
    };
    let (first, rest) = (1_300_000, 400_000);
    let (mut want_l, mut want_r) = signal(first + rest);
    let (mut l, mut r) = (want_l.clone(), want_r.clone());
    let mut long = synced(24, 120.0, s.clone());
    let at_21 = DelaySettings {
        sync: true,
        note: 21,
        ..s.clone()
    };
    long.set(&at_21);
    long.process(&mut want_l[..first], &mut want_r[..first]);
    long.set(&DelaySettings {
        note: 24,
        ..at_21.clone()
    });
    long.process(&mut want_l[first..], &mut want_r[first..]);
    let mut d = synced(21, 120.0, s);
    d.process(&mut l[..first], &mut r[..first]);
    d.set(&DelaySettings { note: 24, ..at_21 });
    let len = d.need().expect("the 48 s line");
    assert_eq!(len, sync::line_len(48.0, SR));
    assert!(d.supply(Box::new(Lines::new(len))).is_none());
    let mut calls = 0;
    let mut at = first;
    while at < first + rest {
        let end = (at + 128).min(first + rest);
        d.process(&mut l[at..end], &mut r[at..end]);
        assert!(d.work() <= bound(end - at), "{} cells", d.work());
        if d.work() > 0 {
            calls += 1;
        }
        at = end;
    }
    assert!(calls > 30, "in stretches: {calls} calls");
    assert_eq!(
        d.take_spent().map(|o| o.len()),
        Some(sync::line_len(24.0, SR))
    );
    assert!(!d.busy());
    assert!(l == want_l && r == want_r, "the same, to the bit");
}

#[test]
fn a_reset_clears_a_long_line_a_stretch_per_call() {
    // 4/1 on its 48 s line, loud, reset: nothing is cleared whole at once, and from then
    // on it plays — and skips — as a new unit does, to the bit.
    let s = DelaySettings {
        feedback: 0.7,
        ..Default::default()
    };
    let mut d = synced(24, 120.0, s.clone());
    let (mut l, mut r) = signal(1_300_000);
    d.process(&mut l, &mut r);
    d.reset();
    assert!(
        d.work() == 0 && d.busy(),
        "nothing cleared by the reset itself"
    );
    assert!(d.at_rest(), "what a reset leaves is at rest");
    let mut new = synced(24, 120.0, s);
    d.skip(5_000);
    new.skip(5_000);
    assert!(d.work() <= bound(5_000));
    let (mut l0, mut r0) = signal(300_000);
    let (mut l1, mut r1) = (l0.clone(), r0.clone());
    let mut calls = 0;
    for at in (0..l0.len()).step_by(128) {
        let end = (at + 128).min(l0.len());
        d.process(&mut l0[at..end], &mut r0[at..end]);
        assert!(d.work() <= bound(end - at), "{} cells", d.work());
        if d.work() > 0 {
            calls += 1;
        }
    }
    new.process(&mut l1, &mut r1);
    assert!(calls > 30, "in stretches: {calls} calls");
    assert!(!d.busy());
    assert!(l0 == l1 && r0 == r1, "a cleared line's output, to the bit");
}

#[test]
fn a_second_reset_never_brings_the_old_line_back() {
    // fix-c (R2-A1): a loud line reset is read as zeros while its cells are cleared a
    // stretch per call; a second reset before the clearing ends must not forget them —
    // with 0, 1 or 7 calls (of sound) between the resets, on the free 2 s line and on the
    // synced 48 s one, what follows is a new unit's, to the bit.
    let s = DelaySettings {
        feedback: 0.7,
        ..Default::default()
    };
    for long in [false, true] {
        for between in [0usize, 1, 7] {
            let make = || {
                if long {
                    synced(24, 120.0, s.clone())
                } else {
                    let mut d = StereoDelay::new(SR);
                    d.set(&s);
                    d.reset();
                    d
                }
            };
            let mut d = make();
            let (mut l, mut r) = signal(if long { 1_300_000 } else { 200_000 });
            d.process(&mut l, &mut r);
            d.reset();
            let (mut bl, mut br) = signal(128 * between.max(1));
            for at in (0..128 * between).step_by(128) {
                d.process(&mut bl[at..at + 128], &mut br[at..at + 128]);
            }
            d.reset();
            let mut new = make();
            let (mut l0, mut r0) = signal(300_000);
            let (mut l1, mut r1) = (l0.clone(), r0.clone());
            for at in (0..l0.len()).step_by(128) {
                let end = (at + 128).min(l0.len());
                d.process(&mut l0[at..end], &mut r0[at..end]);
            }
            new.process(&mut l1, &mut r1);
            assert!(
                l0 == l1 && r0 == r1,
                "long {long}, {between} calls between: a cleared line's output, to the bit"
            );
            assert!(!d.busy());
        }
    }
}

#[test]
fn the_free_time_on_a_long_line_keeps_its_fraction() {
    // fix-c (R2-A3): the sync off on a line kept 48 s long (a lane's floor): 46 s into the
    // song an impulse echoes 4 800.3 samples later, between the two cells around it in
    // the ratio of that fraction — not rounded to the quarter sample an `f32` position
    // 2.2 M cells along keeps.
    let s = DelaySettings {
        time_ms: 4_800.3 / 48.0,
        feedback: 0.0,
        mix: 1.0,
        ping_pong: false,
        ..Default::default()
    };
    let mut d = StereoDelay::new(SR);
    d.set(&s);
    d.set_line_floor(Some(24));
    let len = d.need().expect("the floor's line");
    assert_eq!(len, sync::line_len(48.0, SR));
    assert!(
        d.supply(Box::new(Lines::new(len))).is_some(),
        "swapped at once"
    );
    d.reset();
    let at = 46 * SR as usize;
    let (mut l, mut r) = (vec![0.0f32; at + 6_000], vec![0.0f32; at + 6_000]);
    l[at] = 1.0;
    r[at] = 1.0;
    for i in (0..l.len()).step_by(4_096) {
        let e = (i + 4_096).min(l.len());
        d.process(&mut l[i..e], &mut r[i..e]);
    }
    let want = f64::from(d.time.value()) - 4_800.0;
    let (a, b) = (l[at + 4_800], l[at + 4_801]);
    assert!(
        (f64::from(b) - want).abs() < 1e-3 && (f64::from(a) - (1.0 - want)).abs() < 1e-3,
        "the echo's two cells {a} and {b}: the fraction {want:.4}"
    );
}
