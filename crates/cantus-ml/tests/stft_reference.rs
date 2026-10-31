//! Cantus's HT-Demucs spectrogram and its inverse against PyTorch's own
//! (`HTDemucs._spec`, `_ispec`), on a signal both compute from the same formula
//! (`tools/reference_stft.py` wrote `data/stft_reference.json`).

use cantus_ml::stft::{self, Stft, BINS};

fn signal(n: usize) -> (Vec<f32>, Vec<f32>) {
    let s = |x: f64| cantus_dsp::math::sin(x);
    let c = |x: f64| cantus_dsp::math::cos(x);
    (0..n)
        .map(|i| {
            let i = i as f64;
            let l = 0.5 * s(i * 0.01) + 0.25 * s(i * 0.173 + 1.0) + 0.1 * s(i * 1.9);
            let r = 0.4 * c(i * 0.0037) + 0.2 * s(i * 0.61 + 0.5) + 0.05 * s(i * 2.9);
            (l as f32, r as f32)
        })
        .unzip()
}

fn reference() -> serde_json::Value {
    let text = include_str!("data/stft_reference.json");
    serde_json::from_str(text).unwrap()
}

#[test]
fn the_spectrogram_is_pytorchs() {
    let r = reference();
    let n = r["n"].as_u64().unwrap() as usize;
    let le = r["frames"].as_u64().unwrap() as usize;
    assert_eq!(stft::frames(n), le);
    let (l, rr) = signal(n);
    let z = Stft::new().spec(&l, &rr);
    let mut worst = 0f32;
    for p in r["spec"].as_array().unwrap() {
        let (c, f, t) = (
            p[0].as_u64().unwrap() as usize,
            p[1].as_u64().unwrap() as usize,
            p[2].as_u64().unwrap() as usize,
        );
        let want = p[3].as_f64().unwrap() as f32;
        let got = z[(c * BINS + f) * le + t];
        worst = worst.max((got - want).abs());
    }
    // The values reach about 30; float32 FFTs of 4 096 points agree to a few 1e-5.
    assert!(worst < 2e-4, "worst difference {worst}");
}

#[test]
fn the_inverse_is_pytorchs_edges_included() {
    let r = reference();
    let n = r["n"].as_u64().unwrap() as usize;
    let le = stft::frames(n);
    let (l, rr) = signal(n);
    let st = Stft::new();
    let mut z = st.spec(&l, &rr);
    // The same change bin by bin as the reference made: 1 + 0.5·sin(0.01·f).
    for c in 0..4 {
        for f in 0..BINS {
            let g = 1.0 + 0.5 * cantus_dsp::math::sin(f as f64 * 0.01) as f32;
            for t in 0..le {
                z[(c * BINS + f) * le + t] *= g;
            }
        }
    }
    let (mut yl, mut yr) = (vec![0f32; n], vec![0f32; n]);
    st.ispec(&z, le, n, &mut yl, &mut yr);
    let mut worst = 0f32;
    for p in r["ispec"].as_array().unwrap() {
        let (c, i) = (
            p[0].as_u64().unwrap() as usize,
            p[1].as_u64().unwrap() as usize,
        );
        let want = p[2].as_f64().unwrap() as f32;
        let got = if c == 0 { yl[i] } else { yr[i] };
        worst = worst.max((got - want).abs());
    }
    assert!(worst < 1e-5, "worst difference {worst}");
}
