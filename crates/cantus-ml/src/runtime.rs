//! The inference runtime: ONNX Runtime through its C API (`ort`), the library loaded at run
//! time from the file the content manager installed (ADR 0085). One environment per helper
//! process, telemetry off; the last model's session kept between jobs (a second separation
//! does not load the model again).
//!
//! Any model runs through [`Model::run`]: named `f32` tensors in, named `f32` tensors out.
//! That is all a job needs (HT-Demucs here, Basic Pitch for audio to MIDI), so a new model
//! adds no code here.

use crate::jobs::Stop;
use crate::proto::JobError;
use crate::spec::{Device, RuntimeSpec};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::{RunOptions, Session};
use ort::value::TensorRef;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A named input: its shape and its values, in row-major order.
pub struct Input<'a> {
    pub name: &'a str,
    pub shape: &'a [usize],
    pub data: &'a [f32],
}

/// A named output.
#[derive(Debug, Clone)]
pub struct Output {
    pub name: String,
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

struct Loaded {
    graph: PathBuf,
    threads: usize,
    device: Device,
    ran_on: &'static str,
    session: Session,
}

#[derive(Default)]
pub struct Runtime {
    library: Option<PathBuf>,
    env: Option<ort::environment::Environment>,
    model: Option<Loaded>,
    /// Files whose hash was checked, with their size and time then: not hashed again
    /// while they stay the same.
    checked: Vec<(PathBuf, u64, Option<std::time::SystemTime>)>,
}

fn stamp(p: &Path) -> Option<(u64, Option<std::time::SystemTime>)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()))
}

/// The SHA-256 of a file, hex.
pub fn sha256_file(p: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// What a library that will not load means to a user (review 1 #1): on Windows, error
/// 126 is a library it needs that is not there — the Visual C++ runtime, which Cantus
/// ships beside the helper.
fn load_error(e: impl std::fmt::Display) -> JobError {
    let m = e.to_string();
    if cfg!(windows) && (m.contains("126") || m.contains("could not be found")) {
        return JobError::Runtime(format!(
            "the inference runtime needs a Microsoft Visual C++ runtime library that is not \
             on this computer (vcruntime140.dll, msvcp140.dll); reinstalling Cantus puts it \
             back ({m})"
        ));
    }
    JobError::Runtime(m)
}

fn runtime_err(e: impl std::fmt::Display) -> JobError {
    JobError::Runtime(e.to_string())
}

impl Runtime {
    pub fn new() -> Runtime {
        Runtime::default()
    }

    /// Check `p`'s hash (once while it stays the same).
    fn verify(&mut self, p: &Path, sha256: &str) -> Result<(), JobError> {
        let st =
            stamp(p).ok_or_else(|| JobError::Model(format!("{} is not there", p.display())))?;
        if self
            .checked
            .iter()
            .any(|(q, n, t)| q == p && (*n, *t) == st)
        {
            return Ok(());
        }
        let got = sha256_file(p).map_err(|e| JobError::Model(format!("{}: {e}", p.display())))?;
        if got != sha256 {
            return Err(JobError::Model(format!(
                "{} is not the file Cantus installed (its hash is {got}); remove the model and \
                 install it again",
                p.display()
            )));
        }
        self.checked.push((p.to_path_buf(), st.0, st.1));
        Ok(())
    }

    fn environment(&mut self, spec: &RuntimeSpec) -> Result<(), JobError> {
        if let Some(lib) = &self.library {
            if *lib != spec.library {
                return Err(JobError::Runtime(format!(
                    "this helper already runs {}; another library needs another helper",
                    lib.display()
                )));
            }
            return Ok(());
        }
        if !spec.library.is_file() {
            return Err(JobError::Runtime(format!(
                "{} is not there: install the runtime first",
                spec.library.display()
            )));
        }
        if let Some(sha) = &spec.sha256 {
            self.verify(&spec.library, sha)
                .map_err(|e| JobError::Runtime(e.to_string()))?;
        }
        // Telemetry: the official builds send usage events to Microsoft unless told not
        // to. The variable stops it before the library starts (the client sets it in this
        // process's environment too); the API call stops what the variable does not cover.
        std::env::set_var("ORT_DISABLE_TELEMETRY", "1");
        let env = ort::init_from(&spec.library)
            .map_err(load_error)?
            .with_name("cantus-ml")
            .with_telemetry(false)
            .build()
            .map_err(runtime_err)?;
        self.library = Some(spec.library.clone());
        self.env = Some(env);
        Ok(())
    }

    /// The model at `graph`, loaded (or the one already loaded, if it is the same).
    /// Returns it and the device it runs on.
    /// The model at `graph`, its files checked against `check` first (or the one already
    /// loaded, if it is the same). Returns it and the device it runs on: `cpu`, or the
    /// graphics runtime's name when `spec.device` is `Auto` and one takes the model.
    pub fn model(
        &mut self,
        spec: &RuntimeSpec,
        graph: &Path,
        check: &[crate::spec::FileCheck],
        log: &mut dyn FnMut(&str, &str),
    ) -> Result<(Model<'_>, &'static str), JobError> {
        // The model's files first: nothing loads before everything is as installed.
        for f in check {
            self.verify(&f.path, &f.sha256)?;
        }
        self.environment(spec)?;
        let same = self.model.as_ref().is_some_and(|m| {
            m.graph == graph && m.threads == spec.threads && m.device == spec.device
        });
        if !same {
            // The old session goes first: two models at once would double the memory.
            self.model = None;
            if !graph.is_file() {
                return Err(JobError::Model(format!("{} is not there", graph.display())));
            }
            let env = self.env.as_ref().expect("environment made above");
            let builder = || -> Result<ort::session::builder::SessionBuilder, JobError> {
                let mut b = Session::builder(env)
                    .map_err(runtime_err)?
                    .with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(runtime_err)?
                    // The memory pattern planner keeps every intermediate of the largest run
                    // alive at once: off, HT-Demucs peaks at 2.8 GB instead of 5.2 GB, at
                    // the same speed (ADR 0085's table). DirectML asks for it off too.
                    .with_memory_pattern(false)
                    .map_err(runtime_err)?;
                if spec.threads > 0 {
                    b = b.with_intra_threads(spec.threads).map_err(runtime_err)?;
                }
                Ok(b)
            };
            let mut ran_on = "cpu";
            let mut session = None;
            if spec.device == Device::Auto {
                match gpu(builder()?) {
                    Some((name, mut b)) => match b.commit_from_file(graph) {
                        Ok(s) => {
                            session = Some(s);
                            ran_on = name;
                        }
                        Err(e) => log(
                            "info",
                            &format!("the graphics runtime could not take the model ({e}): the processor runs it"),
                        ),
                    },
                    None => log(
                        "info",
                        "no graphics runtime for this system: the model runs on the processor",
                    ),
                }
            }
            let session = match session {
                Some(s) => s,
                None => builder()?
                    .commit_from_file(graph)
                    .map_err(|e| JobError::Model(format!("{}: {e}", graph.display())))?,
            };
            self.model = Some(Loaded {
                graph: graph.to_path_buf(),
                threads: spec.threads,
                device: spec.device,
                ran_on,
                session,
            });
        }
        let loaded = self.model.as_mut().expect("loaded above");
        Ok((
            Model {
                session: &mut loaded.session,
            },
            loaded.ran_on,
        ))
    }
}

/// The builder with this system's graphics runtime, if it has one: DirectML on Windows,
/// CoreML on macOS. A registration that fails (no DirectML in this library, no device) is
/// `None`: the processor runs the model.
#[allow(unused_mut, unused_variables)]
fn gpu(
    b: ort::session::builder::SessionBuilder,
) -> Option<(&'static str, ort::session::builder::SessionBuilder)> {
    #[cfg(windows)]
    {
        let ep = ort::ep::DirectML::default().build().error_on_failure();
        return b
            .with_execution_providers([ep])
            .ok()
            .map(|b| ("directml", b));
    }
    #[cfg(target_os = "macos")]
    {
        let ep = ort::ep::CoreML::default().build().error_on_failure();
        return b.with_execution_providers([ep]).ok().map(|b| ("coreml", b));
    }
    #[allow(unreachable_code)]
    None
}

/// A loaded model.
pub struct Model<'a> {
    session: &'a mut Session,
}

impl Model<'_> {
    /// The names of the model's inputs and outputs, in its order.
    pub fn signature(&self) -> (Vec<String>, Vec<String>) {
        (
            self.session
                .inputs()
                .iter()
                .map(|i| i.name().to_string())
                .collect(),
            self.session
                .outputs()
                .iter()
                .map(|o| o.name().to_string())
                .collect(),
        )
    }

    /// One inference. `cancel` is watched while it runs: set, the runtime is told to stop
    /// and the call ends with [`Stop::Cancelled`] within a few tens of milliseconds.
    pub fn run(&mut self, inputs: &[Input<'_>], cancel: &AtomicBool) -> Result<Vec<Output>, Stop> {
        let model_err = |e: ort::Error| Stop::Failed(JobError::Model(e.to_string()));
        let mut values = Vec::with_capacity(inputs.len());
        for i in inputs {
            let t = TensorRef::from_array_view((i.shape.to_vec(), i.data)).map_err(model_err)?;
            values.push((i.name.to_string(), ort::session::SessionInputValue::from(t)));
        }
        let options = RunOptions::new().map_err(model_err)?;
        let finished = AtomicBool::new(false);
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !finished.load(Ordering::Acquire) {
                    if cancel.load(Ordering::Relaxed) {
                        let _ = options.terminate();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            let r = self
                .session
                .run_with_options(values, &options)
                .and_then(|outs| {
                    outs.iter()
                        .map(|(name, v)| {
                            let (shape, data) = v.try_extract_tensor::<f32>()?;
                            Ok(Output {
                                name: name.to_string(),
                                shape: shape.iter().map(|&d| d.max(0) as usize).collect(),
                                data: data.to_vec(),
                            })
                        })
                        .collect::<Result<Vec<_>, ort::Error>>()
                });
            finished.store(true, Ordering::Release);
            r
        });
        match result {
            Ok(outs) => Ok(outs),
            Err(_) if cancel.load(Ordering::Relaxed) => Err(Stop::Cancelled),
            Err(e) => Err(model_err(e)),
        }
    }
}
