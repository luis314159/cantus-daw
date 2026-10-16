//! A demo song rendered as a mastering engineer would check it, and written out
//! (`cargo run --release -p cantus-ui --example demo_master -- <out-dir> [demo-id]`,
//! `heart` by default).
//!
//! The whole song goes through the engine's offline render (latency compensated, as the
//! export does) into a 24-bit WAV and a 320 kbps MP3; the first chorus (bars 24–48 with the
//! pre-chorus that leads into it) also as an MP3 and an Ogg Vorbis. Then the numbers to
//! listen by without ears, read off the render:
//!
//! - **loudness**: integrated, its range (LRA) and the largest true peak, by Speculum's
//!   meter (EBU Tech 3341/3342), and the loudest short-term second;
//! - **spectrum**: the long-term average per octave against a reference tilt of commercial
//!   pop masters (relative to the 1 kHz octave);
//! - **stereo**: the correlation per 400 ms window — its mean, its lowest, how many
//!   windows go negative;
//! - **clicks**: the sharpest sample-to-sample change (second difference) around each
//!   section change, against the same measure on the downbeats inside the sections.

use std::path::PathBuf;
use std::sync::Arc;

use cantus_core::{Snapshot, Ticks};
use cantus_dsp::math;
use cantus_engine::{render, RenderOptions, Rendered};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, Depth, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::demos;

const REFERENCE: [(f32, f32); 10] = [
    // (octave centre, dB against the 1 kHz octave): a gentle tilt, the low end a few dB up
    // — the long-term average of commercial pop and dance masters.
    (31.5, 1.0),
    (63.0, 5.0),
    (125.0, 4.0),
    (250.0, 2.0),
    (500.0, 1.0),
    (1_000.0, 0.0),
    (2_000.0, -2.5),
    (4_000.0, -5.0),
    (8_000.0, -8.0),
    (16_000.0, -16.0),
];

fn audio(r: &Rendered) -> Audio {
    Audio {
        rate: r.sample_rate,
        channels: r.channels,
        samples: r.samples.clone(),
    }
}

fn write(out: &std::path::Path, name: &str, a: Audio, s: ExportSettings, title: &str) {
    let tags = Tags {
        title: title.to_string(),
        artist: "Cantus".into(),
        album: "Demo songs".into(),
        comment: "Made and rendered in Cantus, every sound from its own instruments.".into(),
        ..Default::default()
    };
    let path = out.join(name);
    let done = export_rendered(a, &s, &tags, &path, &Control::new()).expect("written");
    println!(
        "  wrote {} ({:.1} s, peak {:.2} dBFS)",
        path.display(),
        done.seconds,
        done.peak_db
    );
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let out: PathBuf = args
        .next()
        .map(PathBuf::from)
        .expect("usage: demo_master <out-dir> [demo-id]");
    let id = args
        .next()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "heart".into());
    std::fs::create_dir_all(&out).expect("create the output folder");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let d = demos::find(&id).expect("a demo by that id");
    let p = (d.build)();
    let title = cantus_ui::i18n::t(d.name_key);
    let t0 = std::time::Instant::now();
    let r = render(
        Arc::new(Snapshot::compile(&p, 0)),
        &RenderOptions {
            start: Ticks::ZERO,
            end: None,
            tail_seconds: 3.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("the demo renders");
    println!(
        "{title}: {:.1} s rendered in {:.1} s",
        r.frames() as f64 / r.sample_rate as f64,
        t0.elapsed().as_secs_f64()
    );
    report(&p, &r);

    let wav = ExportSettings {
        format: Format::Wav,
        depth: Depth::Int24,
        rate: Rate::Session,
        ..Default::default()
    };
    let mp3 = ExportSettings {
        format: Format::Mp3,
        mp3: Mp3Mode::Cbr(320),
        rate: Rate::Hz(44_100),
        ..Default::default()
    };
    let ogg = ExportSettings {
        format: Format::Ogg,
        ogg_quality: 7,
        rate: Rate::Hz(44_100),
        ..Default::default()
    };
    write(&out, &format!("{id}.wav"), audio(&r), wav, &title);
    write(&out, &format!("{id}.mp3"), audio(&r), mp3.clone(), &title);
    // The short one: the pre-chorus into the first chorus, faded over two seconds.
    let spb = 60.0 / p.tempo_bpm * 4.0;
    let at = |bar: f64| (bar * spb * r.sample_rate as f64) as usize;
    let short_title = format!("{title} (excerpt)");
    let excerpt = || excerpt(&r, at(24.0), at(48.0), r.sample_rate as usize * 2);
    write(
        &out,
        &format!("{id}-short.mp3"),
        excerpt(),
        mp3,
        &short_title,
    );
    write(
        &out,
        &format!("{id}-short.ogg"),
        excerpt(),
        ogg,
        &short_title,
    );
}

/// Frames `a..b` of a stereo render and `fade` more, faded out over those.
fn excerpt(r: &Rendered, a: usize, b: usize, fade: usize) -> Audio {
    let end = (b + fade).min(r.frames());
    let mut s = r.samples[a * 2..end * 2].to_vec();
    let n = s.len() / 2;
    let fade = fade.min(n);
    for i in 0..fade {
        let g = 1.0 - i as f32 / fade as f32;
        let f = n - fade + i;
        s[f * 2] *= g * g;
        s[f * 2 + 1] *= g * g;
    }
    Audio {
        rate: r.sample_rate,
        channels: 2,
        samples: s,
    }
}

/// The bars where a clip starts on the track with the most clips (each section, and each
/// eight-bar phrase of a long one): where a cut between clips could click.
fn section_starts(p: &cantus_core::Project) -> Vec<f64> {
    let bar = p.time_signature.ticks_per_bar();
    let notes = |t: &&cantus_core::Track| t.clips.iter().filter(|c| c.automation.is_none()).count();
    let Some(t) = p.tracks.iter().max_by_key(notes) else {
        return Vec::new();
    };
    let mut out: Vec<f64> = t
        .clips
        .iter()
        .filter(|c| c.automation.is_none() && c.start.0 > 0)
        .map(|c| (c.start.0 / bar.0) as f64)
        .collect();
    out.dedup();
    out
}

fn report(p: &cantus_core::Project, r: &Rendered) {
    let sr = r.sample_rate as f32;
    let n = r.frames();
    let (l, rr): (Vec<f32>, Vec<f32>) = (0..n)
        .map(|i| (r.samples[2 * i], r.samples[2 * i + 1]))
        .unzip();

    // Loudness, by Speculum's own meter; the loudest short-term second by sliding it.
    let mut a = cantus_dsp::speculum::Analysis::new(sr);
    let mut max_short = f64::NEG_INFINITY;
    for (x, y) in l.chunks(4_800).zip(rr.chunks(4_800)) {
        a.push(x, y, None);
        max_short = max_short.max(a.loudness().short_term);
    }
    let loud = a.loudness();
    println!(
        "  loudness: I {:.2} LUFS, LRA {:.1} LU, true peak {:.2} dBTP, loudest short-term {:.1} LUFS, sample peak {:.2} dBFS",
        loud.integrated.unwrap_or(f64::NAN),
        loud.range.unwrap_or(f64::NAN),
        loud.true_peak_db,
        max_short,
        20.0 * math::log10(r.peak.max(1e-9) as f64)
    );

    // Each section's own loudness: the song's dynamics, intro to outro.
    let spb = 60.0 / p.tempo_bpm * 4.0;
    let mut edges = vec![0.0];
    edges.extend(section_starts(p));
    edges.push(p.content_end().as_beats_f64() / 4.0);
    print!("  sections (bar: integrated LUFS):");
    for w in edges.windows(2) {
        let (i0, i1) = (
            (w[0] * spb * sr as f64) as usize,
            ((w[1] * spb * sr as f64) as usize).min(n),
        );
        let mut a = cantus_dsp::speculum::Analysis::new(sr);
        for (x, y) in l[i0..i1].chunks(4_800).zip(rr[i0..i1].chunks(4_800)) {
            a.push(x, y, None);
        }
        print!(
            " {:.0}: {:.1}",
            w[0],
            a.loudness().integrated.unwrap_or(f64::NAN)
        );
    }
    println!();

    // The long-term average spectrum per octave (Welch: 8192-point Hann frames, half
    // overlapped, the mid signal).
    const N: usize = 8_192;
    let win: Vec<f32> = (0..N)
        .map(|i| 0.5 - 0.5 * math::cos(2.0 * std::f32::consts::PI * i as f32 / N as f32))
        .collect();
    let mut power = vec![0f64; N / 2];
    let mut frames = 0usize;
    let mut at = 0;
    let (mut re, mut im) = (vec![0f32; N], vec![0f32; N]);
    while at + N <= n {
        for (i, (x, y)) in re.iter_mut().zip(im.iter_mut()).enumerate() {
            *x = 0.5 * (l[at + i] + rr[at + i]) * win[i];
            *y = 0.0;
        }
        cantus_dsp::spectrum::fft(&mut re, &mut im);
        for (p, (x, y)) in power.iter_mut().zip(re.iter().zip(&im)) {
            *p += (x * x + y * y) as f64;
        }
        frames += 1;
        at += N / 2;
    }
    let band = |c: f32| {
        let (lo, hi) = (c / std::f32::consts::SQRT_2, c * std::f32::consts::SQRT_2);
        let (k0, k1) = ((lo / sr * N as f32) as usize, (hi / sr * N as f32) as usize);
        let e: f64 = power[k0.max(1)..k1.min(N / 2)].iter().sum();
        10.0 * math::log10((e / frames.max(1) as f64).max(1e-30))
    };
    let k1 = band(1_000.0);
    let mut worst = 0f64;
    print!("  spectrum (octave, dB vs 1 kHz, reference in brackets):");
    for (c, want) in REFERENCE {
        let got = band(c) - k1;
        if c > 40.0 {
            worst = worst.max((got - want as f64).abs());
        }
        print!(" {c:.0} Hz {got:+.1} [{want:+.0}]");
    }
    println!("\n  spectrum: largest distance from the reference above 40 Hz {worst:.1} dB");

    // Stereo correlation per 400 ms.
    let w = (0.4 * sr) as usize;
    let mut cors = Vec::new();
    for (x, y) in l.chunks(w).zip(rr.chunks(w)) {
        let (mut lr, mut ll, mut rr2) = (0f64, 0f64, 0f64);
        for (a, b) in x.iter().zip(y) {
            lr += (*a as f64) * (*b as f64);
            ll += (*a as f64) * (*a as f64);
            rr2 += (*b as f64) * (*b as f64);
        }
        if ll > 1e-6 && rr2 > 1e-6 {
            cors.push(lr / (ll * rr2).sqrt());
        }
    }
    let negative: Vec<String> = cors
        .iter()
        .enumerate()
        .filter(|(_, c)| **c < 0.0)
        .map(|(i, c)| format!("{:.1}s:{c:.2}", i as f64 * 0.4))
        .collect();
    if !negative.is_empty() {
        println!("  stereo: windows below 0 at {}", negative.join(" "));
    }
    let mean = cors.iter().sum::<f64>() / cors.len().max(1) as f64;
    let low = cors.iter().copied().fold(1.0f64, f64::min);
    let neg = cors.iter().filter(|c| **c < 0.0).count();
    println!(
        "  stereo: correlation mean {mean:.2}, lowest {low:.2}, {neg} of {} windows below 0",
        cors.len()
    );

    // Clicks: the largest second difference within 30 ms of each section change, against
    // the median of the same on the other bars' downbeats.
    let sharp = |bar: f64| {
        let c = (bar * spb * sr as f64) as usize;
        let (i0, i1) = (
            c.saturating_sub((0.03 * sr) as usize).max(2),
            (c + (0.03 * sr) as usize).min(n),
        );
        (i0..i1)
            .map(|i| {
                let d = |x: &[f32]| (x[i] - 2.0 * x[i - 1] + x[i - 2]).abs();
                d(&l).max(d(&rr))
            })
            .fold(0f32, f32::max)
    };
    let bars = (p.content_end().as_beats_f64() / 4.0) as i64;
    let changes: Vec<f64> = section_starts(p);
    let mut inside: Vec<f32> = (1..bars)
        .filter(|b| !changes.contains(&(*b as f64)))
        .map(|b| sharp(b as f64))
        .collect();
    inside.sort_by(f32::total_cmp);
    let median = inside[inside.len() / 2].max(1e-6);
    let worst_in = inside.last().copied().unwrap_or(0.0);
    print!("  clicks: downbeats inside sections median {median:.3}, largest {worst_in:.3}; section changes:");
    for b in &changes {
        print!(" bar {b:.0} {:.3}", sharp(*b));
    }
    println!();
}
