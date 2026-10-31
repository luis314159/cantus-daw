//! The `separate` job: a WAV file into one WAV per source (`spec::SeparateArgs`).

use crate::audio::{Input, Output};
use crate::demucs::{self, Fixture, Network, Shape};
use crate::jobs::{self, Ctx, Stop};
use crate::proto::JobError;
use crate::runtime;
use crate::spec::{Family, SeparateArgs, SeparateResult, StemFile};
use crate::stft;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

/// HT-Demucs's graph in the runtime: inputs `mix` and `mag`, outputs `spec` and `wave`
/// (`tools/export_htdemucs.py`).
struct Graph<'a> {
    model: runtime::Model<'a>,
    segment: usize,
    cancel: Arc<AtomicBool>,
}

impl Network for Graph<'_> {
    fn run(&mut self, mix: &[f32], mag: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Stop> {
        let le = stft::frames(self.segment);
        let outs = self.model.run(
            &[
                runtime::Input {
                    name: "mix",
                    shape: &[1, 2, self.segment],
                    data: mix,
                },
                runtime::Input {
                    name: "mag",
                    shape: &[1, 4, stft::BINS, le],
                    data: mag,
                },
            ],
            &self.cancel,
        )?;
        let mut spec = None;
        let mut wave = None;
        for o in outs {
            match o.name.as_str() {
                "spec" => spec = Some(o.data),
                "wave" => wave = Some(o.data),
                _ => {}
            }
        }
        match (spec, wave) {
            (Some(s), Some(w)) => Ok((s, w)),
            _ => Err(Stop::Failed(JobError::Model(
                "the graph has no `spec` and `wave` outputs".into(),
            ))),
        }
    }
}

pub fn job(args: serde_json::Value, ctx: &mut Ctx) -> Result<serde_json::Value, Stop> {
    let a: SeparateArgs = jobs::args(args)?;
    let t0 = Instant::now();
    if !(0.0..0.9).contains(&a.overlap) {
        return Err(JobError::BadArgs(format!("overlap {} is not in [0, 0.9)", a.overlap)).into());
    }
    let sources = a.model.sources.len();
    if sources == 0 || a.model.segment < 2 * stft::NFFT {
        return Err(JobError::BadArgs("the model names no sources or no segment".into()).into());
    }
    let mut input = Input::open(&a.input)?;
    if input.sample_rate != a.model.sample_rate {
        return Err(JobError::Input(format!(
            "{} is at {} Hz; the model wants {} Hz",
            a.input.display(),
            input.sample_rate,
            a.model.sample_rate
        ))
        .into());
    }
    std::fs::create_dir_all(&a.out_dir)
        .map_err(|e| JobError::Output(format!("{}: {e}", a.out_dir.display())))?;
    let stems: Vec<StemFile> = a
        .model
        .sources
        .iter()
        .map(|name| StemFile {
            name: name.clone(),
            path: a.out_dir.join(format!("{name}.wav")),
        })
        .collect();
    let paths: Vec<_> = stems.iter().map(|s| s.path.clone()).collect();
    let shape = Shape {
        sources,
        segment: a.model.segment,
        overlap: a.overlap,
        delay: 0,
    };
    if a.shifts > 16 {
        return Err(JobError::BadArgs(format!("{} shifts: at most 16", a.shifts)).into());
    }
    let report = &mut ctx.report;
    report.progress(0.0, "load");
    let cancel = report.cancel_flag();
    let mut fixture;
    let mut graph;
    let device;
    let net: &mut dyn Network = match &a.model.family {
        Family::Fixture {
            spec,
            wave,
            delay_ms,
            crash_at,
            panic_at,
            hang_at,
        } => {
            if !ctx.test_hooks {
                return Err(JobError::BadArgs("fixture models are for tests".into()).into());
            }
            if spec.len() != sources || wave.len() != sources {
                return Err(JobError::BadArgs("one gain per source".into()).into());
            }
            device = "fixture";
            fixture = Fixture {
                spec: spec.clone(),
                wave: wave.clone(),
                delay_ms: *delay_ms,
                crash_at: *crash_at,
                panic_at: *panic_at,
                hang_at: *hang_at,
                runs: 0,
            };
            &mut fixture
        }
        Family::HtDemucs => {
            let (model, dev) =
                ctx.runtime
                    .model(&a.runtime, &a.model.graph, &a.model.check, &mut |l, m| {
                        report.log(l, m)
                    })?;
            device = dev;
            graph = Graph {
                model,
                segment: a.model.segment,
                cancel: cancel.clone(),
            };
            &mut graph
        }
    };
    let cancelled = || cancel.load(std::sync::atomic::Ordering::Relaxed);
    // One pass, or one per shift (`apply_model(shifts=n)`: the track moved later by a
    // fraction of half a second, separated, moved back; the passes averaged).
    let passes = a.shifts.max(1) as usize;
    let max_shift = (a.model.sample_rate / 2) as u64;
    let pass_paths = |k: usize| -> Vec<std::path::PathBuf> {
        if passes == 1 {
            paths.clone()
        } else {
            a.model
                .sources
                .iter()
                .map(|n| a.out_dir.join(format!(".pass{k}-{n}.wav")))
                .collect()
        }
    };
    let clean = |extra: &[std::path::PathBuf]| {
        for p in paths.iter().chain(extra) {
            let _ = std::fs::remove_file(p);
        }
    };
    let mut done = demucs::Separated {
        inferences: 0,
        network_seconds: 0.0,
    };
    let mut temps = Vec::new();
    // The load is the first 5 %, the passes the rest (and the average, if any, the last).
    report.progress(0.05, "separate");
    for k in 0..passes {
        let these = pass_paths(k);
        if passes > 1 {
            temps.extend(these.iter().cloned());
        }
        let mut out = Output::create(&these, input.sample_rate)?;
        // Offsets k/n of the half second: the delay is what the shift leaves.
        let delay = if passes == 1 {
            0
        } else {
            max_shift - (k as u64 * max_shift) / passes as u64
        };
        // With passes to average, the average is the last 5 %.
        let span = if passes == 1 { 0.95 } else { 0.9 } / passes as f32;
        let from = 0.05 + span * k as f32;
        let result = demucs::separate(
            &mut input,
            net,
            Shape { delay, ..shape },
            &mut out,
            &cancelled,
            &mut |i, of| report.progress(from + span * i as f32 / of as f32, "separate"),
        );
        match result {
            Ok(d) => {
                done.inferences += d.inferences;
                done.network_seconds += d.network_seconds;
            }
            Err(e) => {
                // Half-written stems are not left behind.
                drop(out);
                clean(&temps);
                return Err(e);
            }
        }
        out.finish()?;
    }
    if passes > 1 {
        report.progress(0.95, "write");
        // A block at a time, each one said and the cancel asked: a long song's average is
        // never silent long enough to look hung (review 2 #7).
        let r = average(
            &temps,
            sources,
            passes,
            &paths,
            input.sample_rate,
            input.frames,
            &cancelled,
            &mut |d| report.progress(0.95 + 0.05 * d, "write"),
        );
        for t in &temps {
            let _ = std::fs::remove_file(t);
        }
        if let Err(e) = r {
            clean(&[]);
            return Err(e);
        }
    }
    report.progress(1.0, "write");
    jobs::result(&SeparateResult {
        stems,
        frames: input.frames,
        sample_rate: input.sample_rate,
        inferences: done.inferences,
        network_seconds: done.network_seconds,
        seconds: t0.elapsed().as_secs_f64(),
        device: device.to_string(),
    })
}

/// The passes' stems averaged into the final ones, a block at a time: `progress` 0…1 after
/// each block, `cancelled` asked before it.
#[allow(clippy::too_many_arguments)]
fn average(
    temps: &[std::path::PathBuf],
    sources: usize,
    passes: usize,
    paths: &[std::path::PathBuf],
    rate: u32,
    frames: u64,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(f32),
) -> Result<(), Stop> {
    const BLOCK: usize = 1 << 16;
    let mut out = Output::create(paths, rate)?;
    for s in 0..sources {
        let mut ins: Vec<Input> = (0..passes)
            .map(|k| Input::open(&temps[k * sources + s]))
            .collect::<Result<_, _>>()?;
        let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
        let (mut al, mut ar) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
        let mut at = 0u64;
        while at < frames {
            if cancelled() {
                return Err(Stop::Cancelled);
            }
            let n = (BLOCK as u64).min(frames - at) as usize;
            al[..n].iter_mut().for_each(|v| *v = 0.0);
            ar[..n].iter_mut().for_each(|v| *v = 0.0);
            for i in ins.iter_mut() {
                i.read(at, &mut l[..n], &mut r[..n])?;
                for j in 0..n {
                    al[j] += l[j];
                    ar[j] += r[j];
                }
            }
            let inv = 1.0 / passes as f32;
            for j in 0..n {
                al[j] *= inv;
                ar[j] *= inv;
            }
            out.write(s, &al[..n], &ar[..n])?;
            at += n as u64;
            let whole = (sources as u64 * frames).max(1);
            progress((s as u64 * frames + at) as f32 / whole as f32);
        }
    }
    Ok(out.finish()?)
}
