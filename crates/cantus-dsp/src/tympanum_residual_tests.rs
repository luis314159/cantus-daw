use super::*;

const LANE: usize = 256;

/// A small residual: key 38 (piece 1) with two layers of two takes, key 46 (piece 2) with
/// a closed and an open state.
fn sample() -> Residual {
    let mut data = Vec::new();
    let mut take = |seed: i32, len: u32| {
        let start = data.len() as u32;
        for i in 0..len as i32 {
            data.push(((i * 37 + seed * 101) % 2000 - 1000) as i16);
        }
        Take {
            start,
            len,
            scale: 1.0 / 32768.0,
        }
    };
    let snare = Art {
        key: 38,
        open: 255,
        layers: vec![
            Layer {
                vel: 40,
                rho: 0.3,
                takes: vec![take(1, 700), take(2, 650)],
            },
            Layer {
                vel: 110,
                rho: 0.0,
                takes: vec![take(3, 900), take(4, 880)],
            },
        ],
    };
    let closed = Art {
        key: 46,
        open: 0,
        layers: vec![Layer {
            vel: 80,
            rho: 0.0,
            takes: vec![take(5, 300)],
        }],
    };
    let open = Art {
        key: 46,
        open: 95,
        layers: vec![Layer {
            vel: 80,
            rho: 0.0,
            takes: vec![take(6, 1200)],
        }],
    };
    let mut r = Residual {
        rate: 48_000,
        model: 7,
        arts: vec![snare, closed, open],
        data,
        soft: Vec::new(),
    };
    r.finish();
    r
}

fn player(r: &Residual) -> Player {
    let mut p = Player::new(48_000.0);
    p.set(Some(Arc::new(r.clone())), &[(38, 1), (46, 2)], 5);
    p
}

/// A groove of hits rendered in chunks of `block`: piece 1 and 2's sources concatenated.
fn render(r: &Residual, block: usize) -> Vec<f32> {
    let mut p = player(r);
    let hits: [(usize, u8, u8, f32); 6] = [
        (0, 38, 100, 0.0),
        (100, 46, 90, 0.0),
        (333, 38, 30, 0.0),
        (500, 46, 90, 0.9),
        (700, 38, 127, 0.0),
        (701, 38, 75, 0.0),
    ];
    let total = 2400;
    let mut out = vec![0.0f32; 2 * total];
    let mut src = vec![0.0f32; 4 * LANE];
    let mut touched = [false; 4];
    let mut ord = [0u32; 128];
    let mut at = 0;
    while at < total {
        for &(t, key, vel, open) in &hits {
            if t == at {
                p.strike(key, vel, Some(ord[key as usize]), open);
                ord[key as usize] += 1;
            }
        }
        let next = hits
            .iter()
            .map(|h| h.0)
            .filter(|&t| t > at)
            .min()
            .unwrap_or(total);
        let c = block.min(LANE).min(next - at).min(total - at);
        src.fill(0.0);
        p.render(c, &mut src, LANE, &mut touched);
        for t in 0..c {
            out[at + t] = src[LANE + t];
            out[total + at + t] = src[2 * LANE + t];
        }
        at += c;
    }
    out
}

#[test]
fn a_residual_round_trips_to_the_byte_and_bad_files_are_refused() {
    let r = sample();
    let bytes = r.encode();
    let back = Residual::decode(&bytes).unwrap();
    assert_eq!(back, r);
    assert_eq!(back.encode(), bytes);
    assert!(Residual::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut wrong = bytes.clone();
    wrong[0] = b'X';
    assert!(Residual::decode(&wrong).is_err());
    let mut v2 = bytes.clone();
    v2[4] = 2;
    assert!(Residual::decode(&v2).is_err());
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(Residual::decode(&extra).is_err());
    // A take that reaches past the data.
    let mut bad = r.clone();
    bad.arts[0].layers[0].takes[0].len = 1_000_000;
    assert!(Residual::decode(&bad.encode()).is_err());
}

/// A `CRS1` file as `tools/tympanum-fit/residual_pack.py` writes it (its own writer, not
/// ours): key 36, any openness, one layer at velocity 64 with rho 0.5, one take of three
/// samples [1000, −2000, 0] at scale 2⁻¹⁵, fitted on model hash 0xA1B2C3D4.
#[test]
fn a_file_the_fit_tools_writer_made_reads() {
    let bytes: &[u8] = &[
        0x43, 0x52, 0x53, 0x31, 0x01, 0x00, 0x00, 0x00, 0x80, 0xbb, 0x00, 0x00, 0xd4, 0xc3, 0xb2,
        0xa1, 0x01, 0x00, 0x00, 0x00, 0x24, 0xff, 0x01, 0x00, 0x40, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x38, 0x03,
        0x00, 0x00, 0x00, 0xe8, 0x03, 0x30, 0xf8, 0x00, 0x00,
    ];
    let r = Residual::decode(bytes).unwrap();
    assert_eq!(r.rate, 48_000);
    assert_eq!(r.model, 0xA1B2_C3D4);
    assert_eq!(r.arts.len(), 1);
    assert_eq!((r.arts[0].key, r.arts[0].open), (36, 255));
    let l = &r.arts[0].layers[0];
    assert_eq!((l.vel, l.rho), (64, 0.5));
    assert_eq!(
        l.takes[0],
        Take {
            start: 0,
            len: 3,
            scale: 1.0 / 32768.0
        }
    );
    assert_eq!(r.data, vec![1000, -2000, 0]);
}

#[test]
fn a_player_without_a_residual_touches_nothing() {
    let mut p = Player::new(48_000.0);
    p.set(None, &[(38, 1)], 0);
    p.strike(38, 100, Some(0), 0.0);
    let mut src = vec![0.0f32; 4 * LANE];
    let mut touched = [false; 4];
    p.render(LANE, &mut src, LANE, &mut touched);
    assert!(src.iter().all(|x| x.to_bits() == 0));
    assert!(!touched.iter().any(|t| *t));
    assert!(p.at_rest());
    // A key the residual has no take for plays nothing either.
    let mut p = player(&sample());
    p.strike(36, 100, Some(0), 0.0);
    p.render(LANE, &mut src, LANE, &mut touched);
    assert!(src.iter().all(|x| x.to_bits() == 0));
    assert!(p.at_rest());
}

#[test]
fn any_chunking_renders_the_same_bits() {
    let r = sample();
    let reference = render(&r, 512);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1, 37, 64, 256, 333] {
        let other = render(&r, block);
        assert!(
            reference
                .iter()
                .zip(&other)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "block {block}"
        );
    }
}

#[test]
fn a_hit_plays_its_take_exactly_and_ends() {
    let r = sample();
    let mut p = player(&r);
    // At a layer's own velocity: that layer's take alone, at its scale.
    p.strike(38, 110, Some(0), 0.0);
    let mut src = vec![0.0f32; 4 * LANE];
    let mut touched = [false; 4];
    p.render(LANE, &mut src, LANE, &mut touched);
    assert!(touched[1] && !touched[2]);
    let l = &r.arts[0].layers[1];
    let t = l.takes[take_index_for(5, 38, 0, l.takes.len())];
    for i in 0..LANE {
        let want = f32::from(r.data[t.start as usize + i]) * t.scale;
        assert_eq!(src[LANE + i].to_bits(), want.to_bits(), "sample {i}");
    }
    for _ in 0..8 {
        src.fill(0.0);
        p.render(LANE, &mut src, LANE, &mut touched);
    }
    assert!(p.at_rest());
}

fn take_index_for(seed: u32, key: u8, ordinal: u32, n: usize) -> usize {
    crate::tympanum::take_index(crate::tympanum::hit_draw(seed, key, ordinal), n, ordinal)
}

#[test]
fn two_hits_running_never_play_the_same_take() {
    let r = sample();
    let mut p = player(&r);
    let mut first = Vec::new();
    for o in 0..12u32 {
        p.reset();
        p.strike(38, 40, Some(o), 0.0);
        let mut src = vec![0.0f32; 4 * LANE];
        let mut touched = [false; 4];
        p.render(8, &mut src, LANE, &mut touched);
        first.push(src[LANE..LANE + 8].to_vec());
    }
    for w in first.windows(2) {
        assert_ne!(w[0], w[1]);
    }
}

#[test]
fn between_layers_both_play_and_the_level_follows_the_velocity() {
    let r = sample();
    let level = |vel: u8| {
        let mut p = player(&r);
        p.strike(38, vel, Some(0), 0.0);
        let mut e = 0.0f64;
        let mut src = vec![0.0f32; 4 * LANE];
        let mut touched = [false; 4];
        for _ in 0..4 {
            src.fill(0.0);
            p.render(LANE, &mut src, LANE, &mut touched);
            e += src[LANE..2 * LANE]
                .iter()
                .map(|x| f64::from(*x) * f64::from(*x))
                .sum::<f64>();
        }
        e
    };
    // Below the softest layer, quieter the softer; between, neither silent nor doubled.
    assert!(level(10) < level(25));
    assert!(level(25) < level(40));
    let (lo, mid, hi) = (level(40), level(75), level(110));
    assert!(
        mid > 0.3 * lo.min(hi) && mid < 2.0 * lo.max(hi),
        "{lo} {mid} {hi}"
    );
}

#[test]
fn a_hat_plays_the_state_nearest_its_openness() {
    let r = sample();
    let len = |open: f32| {
        let mut p = player(&r);
        p.strike(46, 80, Some(0), open);
        let mut n = 0;
        let mut src = vec![0.0f32; 4 * LANE];
        let mut touched = [false; 4];
        while !p.at_rest() {
            src.fill(0.0);
            p.render(LANE, &mut src, LANE, &mut touched);
            n += LANE;
        }
        n
    };
    assert_eq!(len(0.1), 512); // the closed take, 300 frames
    assert_eq!(len(0.8), 1280); // the open take, 1200 frames
}

#[test]
fn another_rate_is_read_at_the_engines() {
    let r = sample();
    let mut p = Player::new(96_000.0);
    p.set(Some(Arc::new(r.clone())), &[(38, 1)], 5);
    p.strike(38, 110, Some(0), 0.0);
    let mut src = vec![0.0f32; 4 * LANE];
    let mut touched = [false; 4];
    let mut n = 0;
    while !p.at_rest() {
        src.fill(0.0);
        p.render(LANE, &mut src, LANE, &mut touched);
        n += LANE;
    }
    // About twice the frames of its 880–900-frame take.
    assert!((1760..=2048).contains(&n), "{n}");
}

#[test]
fn the_models_fingerprint_is_its_blobs() {
    assert_eq!(model_hash(), fnv1a(include_bytes!("pulsus_measured.bin")));
    assert_eq!(fnv1a(b""), 0x811C_9DC5);
    assert_eq!(fnv1a(b"a"), 0xE40C_292C);
}

/// The player's own cost: a busy minute's hits (16th-note hats, kick, snare, crash, every
/// hit 200 ms of residual) into the pieces' sources — `cargo test --release -p cantus-dsp
/// --lib player_cost -- --ignored --nocapture`. Prints the share of one core.
#[test]
#[ignore]
fn player_cost() {
    let mut data = vec![0i16; 9_600 * 4];
    for (i, d) in data.iter_mut().enumerate() {
        *d = ((i * 7919) % 2000) as i16 - 1000;
    }
    let take = |k: u32| Take {
        start: k * 9_600,
        len: 9_600,
        scale: 1.0 / 32768.0,
    };
    let arts = [36u8, 38, 42, 49]
        .iter()
        .map(|&key| Art {
            key,
            open: 255,
            layers: vec![
                Layer {
                    vel: 40,
                    rho: 0.0,
                    takes: vec![take(0), take(1)],
                },
                Layer {
                    vel: 120,
                    rho: 0.0,
                    takes: vec![take(2), take(3)],
                },
            ],
        })
        .collect();
    let mut r = Residual {
        rate: 48_000,
        model: 0,
        arts,
        data,
        soft: Vec::new(),
    };
    r.finish();
    let r = Arc::new(r);
    let frames = 60 * 48_000;
    let mut best = f64::MAX;
    for _ in 0..5 {
        let mut p = Player::new(48_000.0);
        p.set(Some(r.clone()), &[(36, 0), (38, 1), (42, 2), (49, 3)], 1);
        let mut src = vec![0.0f32; 4 * LANE];
        let mut touched = [false; 4];
        let t = std::time::Instant::now();
        let mut at = 0;
        let mut n = 0u32;
        let mut next = 0;
        while at < frames {
            while next <= at {
                next += 6_000;
                p.strike(42, 80, Some(n), 0.0);
                if n.is_multiple_of(4) {
                    p.strike(36, 110, Some(n), 0.0);
                }
                if n % 8 == 4 {
                    p.strike(38, 100, Some(n), 0.0);
                }
                if n.is_multiple_of(32) {
                    p.strike(49, 120, Some(n), 0.0);
                }
                n += 1;
            }
            src.fill(0.0);
            p.render(256.min(frames - at), &mut src, LANE, &mut touched);
            at += 256;
        }
        best = best.min(t.elapsed().as_secs_f64());
        std::hint::black_box(&src);
    }
    eprintln!(
        "the hybrid's player, a busy groove: {:.2} % of a core",
        100.0 * best / (frames as f64 / 48_000.0)
    );
}
