//! Exemplar — the multisample sampler: its editor (ADR 0023, `docs/instruments/exemplar.md`,
//! `research/instruments/sampler.md` §6 and §9.5).
//!
//! On the kit's device shell (ADR 0024 §8, `docs/instruments/REDESIGN.md`): the display
//! first, then a card per section in its function's hue, cells inside.
//!
//! **Edit**: the files and quality row; the key × velocity map beside the zone card (every
//! number of the selected zone as a cell); the waveform with its start, end and loop
//! handles and the loop's settings, beside the seam — both sides of the join, with the
//! measured error in dB; the instrument's cards; the zone table (TX16Wx's pairing: shape
//! in the map, exact numbers in the table, both editable, always in step). **Play**: the
//! map beside the zone's waveform, lit by what sounds, over the cards that matter live.
//!
//! Every edit is a command: a drag is one undo step, an import is one batch. Files dropped
//! from the system onto the map are mapped where they land.

#[path = "sampler_cards.rs"]
mod cards;
#[path = "sampler_ds.rs"]
pub mod ds;
#[path = "sampler_import.rs"]
pub mod import;
#[path = "sampler_map.rs"]
mod map;
#[path = "sampler_parts.rs"]
pub mod parts;
#[path = "sampler_presets.rs"]
mod presets;
#[path = "sampler_sf2.rs"]
pub mod sf2;
#[path = "sampler_sfz.rs"]
pub mod sfz;
#[path = "sampler_wave.rs"]
mod wave;
#[path = "sampler_xml.rs"]
pub mod xml;

use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_core::inst::sampler::{self as model, zf, Zone};
use cantus_core::{Command, NativeSpec, TrackId};
use cantus_engine::inst::sampler::tel;
use std::path::{Path, PathBuf};

pub use map::set_fields as set_zone_fields;

/// Whether the selected zone's file is being read for its waveform (beside the window).
pub fn wave_file_reading(app: &CantusApp) -> bool {
    matches!(
        app.inst.sampler.wave_file,
        Some((_, wave::WaveFile::Reading(_)))
    )
}
pub use presets::FACTORY_PRESETS;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// The zone the inspector, the table and the waveform show.
    pub selected: Option<usize>,
    /// The map's visible keys (inclusive); `None` fits the zones.
    pub keys: Option<(u8, u8)>,
    /// The waveform's visible frames; `None` shows the whole region.
    pub view: Option<(f64, f64)>,
    pub drag: Option<map::Drag>,
    pub marker: Option<wave::Marker>,
    /// What the last import could not carry over.
    pub log: Vec<String>,
    pub show_log: bool,
    /// A SoundFont waiting for its preset to be chosen.
    pub pending_sf2: Option<(PathBuf, sf2::SoundFont)>,
    /// The selected zone's whole file for its waveform, read beside the window when the
    /// engine holds only a stream's head (review 19): one file at a time, let go when
    /// another zone is shown.
    pub wave_file: Option<(String, wave::WaveFile)>,
    /// The sample cache's size and when it was read (Settings).
    pub cache_size: Option<(std::time::Instant, cantus_engine::inst::stream::CacheRoom)>,
    /// The status line said once this session that notes played their start only.
    pub cut_told: bool,
}

/// Every command that replaces the instrument's zones and groups with an import's: one
/// batch, so one undo step brings the old instrument back.
pub fn import_commands(track: TrackId, now: &NativeSpec, imp: &import::Import) -> Command {
    let mut commands = Vec::new();
    for k in now.params.keys() {
        let zone_or_group = model::zone_key(k).is_some() || model::group_key(k).is_some();
        // An instrument file's round robins are a strict cycle: an import plays them in
        // order (`rr.order` back to its default) on an Exemplar saved before round 28 too.
        let taken = imp.values.iter().any(|(key, _)| key == k) || k == "rr.order";
        if zone_or_group || taken {
            commands.push(Command::ClearInstrumentValue {
                track,
                key: k.clone(),
            });
        }
    }
    for k in now.text.keys() {
        if model::zone_key(k).is_some() || model::group_key(k).is_some() {
            commands.push(Command::SetInstrumentText {
                track,
                key: k.clone(),
                text: None,
            });
        }
    }
    let mut push_values = |values: Vec<(String, f32)>| {
        for (key, value) in values {
            commands.push(Command::SetInstrumentValue { track, key, value });
        }
    };
    for (m, g) in &imp.groups {
        push_values(g.values_for(*m));
    }
    for (n, z) in imp.zones.iter().enumerate() {
        push_values(z.values_for(n));
    }
    push_values(imp.values.clone());
    for (m, g) in &imp.groups {
        if let Some(name) = &g.name {
            commands.push(Command::SetInstrumentText {
                track,
                key: format!("g{m}.name"),
                text: Some(name.clone()),
            });
        }
    }
    for (n, z) in imp.zones.iter().enumerate() {
        for (key, text) in z.texts_for(n) {
            commands.push(Command::SetInstrumentText {
                track,
                key,
                text: Some(text),
            });
        }
    }
    Command::Batch {
        label_key: "cmd.smp_import".into(),
        commands,
    }
}

/// Add zones to the ones there are (files dropped on the map), in free indices.
pub fn add_zones_commands(track: TrackId, now: &NativeSpec, zones: &[Zone]) -> Command {
    let mut commands = Vec::new();
    let used: std::collections::BTreeSet<usize> = now
        .params
        .keys()
        .chain(now.text.keys())
        .filter_map(|k| model::zone_key(k).map(|(n, _)| n))
        .collect();
    let mut free = (0..model::MAX_ZONES).filter(|n| !used.contains(n));
    for z in zones {
        let Some(n) = free.next() else { break };
        for (key, value) in z.values_for(n) {
            commands.push(Command::SetInstrumentValue { track, key, value });
        }
        for (key, text) in z.texts_for(n) {
            commands.push(Command::SetInstrumentText {
                track,
                key,
                text: Some(text),
            });
        }
    }
    Command::Batch {
        label_key: "cmd.smp_add_zones".into(),
        commands,
    }
}

/// Remove zone `n`: every key it stored, one undo step.
pub fn remove_zone_command(track: TrackId, now: &NativeSpec, n: usize) -> Command {
    let (nums, texts) = model::zone_keys(now, n);
    let mut commands: Vec<Command> = nums
        .into_iter()
        .map(|key| Command::ClearInstrumentValue { track, key })
        .collect();
    commands.extend(texts.into_iter().map(|key| Command::SetInstrumentText {
        track,
        key,
        text: None,
    }));
    Command::Batch {
        label_key: "cmd.smp_remove_zone".into(),
        commands,
    }
}

/// Where a SoundFont's samples are written: the app's data folder, per bank.
pub fn sf2_dir(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "soundfont".into());
    crate::app::data_dir()
        .join("samples")
        .join("soundfont")
        .join(stem)
}

/// Read any supported file into an import: SFZ, Decent Sampler, or a SoundFont (whose
/// first preset is taken; the editor asks when there are several).
pub fn read_instrument(path: &Path) -> Result<import::Import, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let base = path.parent().unwrap_or(Path::new("."));
    let meta = |p: &str| import::meta_of(Path::new(p));
    let mut imp = match ext.as_str() {
        "sfz" => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let include = |name: &str| std::fs::read_to_string(base.join(name)).ok();
            sfz::parse(&text, base, &include, &meta)
        }
        "dspreset" => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            ds::parse(&text, base, &meta)?
        }
        "sf2" | "sf3" => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let sf = sf2::parse(&bytes)?;
            sf2::to_import(&sf, 0, &sf2_dir(path))?
        }
        _ => return Err(tf("smp.unknown_format", &[("ext", &ext)])),
    };
    if imp.name.is_none() {
        imp.name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    }
    Ok(imp)
}

/// Whether a path is audio Exemplar maps as a zone.
pub fn is_audio(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("wav" | "wave" | "aif" | "aiff" | "aifc" | "flac" | "ogg")
    )
}

/// Apply an import to the track as one undo step, keeping its log for the editor.
pub fn apply_import(app: &mut CantusApp, track: TrackId, imp: import::Import) {
    let Some(now) = app.native_of(track) else {
        return;
    };
    let n = imp.zones.len();
    app.exec(import_commands(track, &now, &imp));
    app.inst.sampler.selected = (n > 0).then_some(0);
    app.inst.sampler.keys = None;
    app.inst.sampler.view = None;
    app.inst.sampler.log = imp.log.clone();
    let name = imp.name.clone().unwrap_or_default();
    app.set_status(tf(
        "smp.imported",
        &[
            ("name", &name),
            ("n", &n.to_string()),
            ("log", &imp.log.len().to_string()),
        ],
    ));
}

/// Open an instrument file (SFZ, Decent Sampler, SoundFont). A SoundFont with several
/// presets waits for the choice.
pub fn open_instrument(app: &mut CantusApp, track: TrackId, path: &Path) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "sf2" || ext == "sf3" {
        match std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| sf2::parse(&b))
        {
            Ok(sf) if sf.presets.len() > 1 => {
                app.inst.sampler.pending_sf2 = Some((path.to_path_buf(), sf));
                return;
            }
            Ok(sf) => match sf2::to_import(&sf, 0, &sf2_dir(path)) {
                Ok(imp) => apply_import(app, track, imp),
                Err(e) => app.set_error(e),
            },
            Err(e) => app.set_error(e),
        }
        return;
    }
    match read_instrument(path) {
        Ok(imp) => apply_import(app, track, imp),
        Err(e) => app.set_error(e),
    }
}

/// Files dropped on the map at `key`: an instrument file replaces the instrument; audio
/// files (or folders of them) are added — one where it lands, several auto-mapped.
pub fn drop_files(app: &mut CantusApp, track: TrackId, paths: &[PathBuf], key: u8) {
    if let Some(inst) = paths.iter().find(|p| !is_audio(p) && p.is_file()) {
        open_instrument(app, track, inst);
        return;
    }
    let mut audio: Vec<PathBuf> = Vec::new();
    for p in paths {
        if p.is_dir() {
            if let Ok(dir) = std::fs::read_dir(p) {
                let mut inside: Vec<PathBuf> = dir
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| is_audio(p))
                    .collect();
                inside.sort();
                audio.extend(inside);
            }
        } else if is_audio(p) {
            audio.push(p.clone());
        }
    }
    if audio.is_empty() {
        return;
    }
    let zones: Vec<Zone> = if audio.len() == 1 {
        let meta = import::meta_of(&audio[0]).unwrap_or_default();
        let root = meta.root.unwrap_or(key);
        let mut z = Zone::new(
            Some(audio[0].to_string_lossy().replace('\\', "/")),
            key,
            key,
            root,
        );
        z.set(zf::TUNE, -meta.fine_ct);
        if let Some((s, e, alt)) = meta.sustain_loop {
            z.set(zf::MODE, 2.0)
                .set(zf::LS, s as f32)
                .set(zf::LE, e as f32);
            if alt {
                z.set(zf::DIR, 1.0);
            }
        }
        vec![z]
    } else {
        let files: Vec<_> = audio
            .iter()
            .map(|p| (p.clone(), import::meta_of(p)))
            .collect();
        import::map_files(&files).zones
    };
    let Some(now) = app.native_of(track) else {
        return;
    };
    let first = model::free_zone(&now);
    app.exec(add_zones_commands(track, &now, &zones));
    app.inst.sampler.selected = first;
    app.set_status(tf("smp.added", &[("n", &zones.len().to_string())]));
}

/// `path` relative to `base` when it lies under it, else as it is (with `/`).
fn relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Exemplar's own footer chips (ADR 0024 §8): the sample memory and the zone count, the
/// zones streamed from disk, and — while its samples load in the background — how far
/// (ADR 0050).
pub fn chips(
    tel: &cantus_engine::inst::Telemetry,
    loading: Option<(usize, usize)>,
    out: &mut Vec<String>,
    tips: &mut Vec<Option<String>>,
) {
    if let Some((done, total)) = loading {
        out.push(tf(
            "smp.loading_status",
            &[("done", &done.to_string()), ("total", &total.to_string())],
        ));
        tips.push(Some(crate::i18n::t("smp.loading_tip")));
    }
    // The memory: what it counts (a long sample holds only its start here).
    tips.push(Some(crate::i18n::t("smp.footer_tip")));
    out.push(tf(
        "smp.footer",
        &[
            ("mib", &format!("{:.1}", tel[tel::MEMORY_MIB])),
            ("zones", &format!("{}", tel[tel::ZONES] as u32)),
        ],
    ));
    if tel[tel::STREAMED] > 0.0 {
        out.push(tf(
            "smp.footer_streamed",
            &[("n", &format!("{}", tel[tel::STREAMED] as u32))],
        ));
        tips.push(Some(crate::i18n::t("smp.footer_tip")));
    }
}

/// The sample cache's progress while some file of this instrument is still loading.
pub fn loading(app: &CantusApp, spec: &NativeSpec) -> Option<(usize, usize)> {
    let progress = app.engine.samples.progress()?;
    let mine = model::zones(spec).iter().any(|(_, z)| {
        z.sample.as_deref().is_some_and(|p| {
            app.engine.samples.status(Path::new(p)) == cantus_engine::SampleStatus::Loading
        })
    });
    mine.then_some(progress)
}

/// Files dragged from the system: the key under the pointer when they are let go over
/// `rect` (`key_at` maps a point to a key).
fn take_drop(
    ui: &egui::Ui,
    rect: egui::Rect,
    key_at: impl Fn(egui::Pos2) -> Option<u8>,
) -> Option<(Vec<PathBuf>, u8)> {
    let (dropped, pos) = ui.ctx().input(|i| {
        (
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect::<Vec<_>>(),
            i.pointer.latest_pos(),
        )
    });
    if dropped.is_empty() {
        return None;
    }
    let pos = pos.filter(|p| rect.contains(*p))?;
    Some((dropped, key_at(pos).unwrap_or(60)))
}

/// The Edit view: every section, as tall as it needs (the frame scrolls it).
pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &crate::kit::Style,
) {
    cards::body(app, ui, track, spec, s);
}

/// The Play view: the map beside the zone's waveform, lit by what sounds, and the cards
/// of what matters live.
pub fn play(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &crate::kit::Style,
) {
    cards::play(app, ui, track, spec, s);
}

/// The inspector's small read-only picture: the zones over the 128 keys.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    map::mini(ui, spec)
}

/// `--instrument sampler`: a multisample built from the built-in sounds, the looped pad
/// selected so the waveform shows its loop and seam.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let preset = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "smp.preset.showcase")
        .unwrap_or(&FACTORY_PRESETS[0]);
    let spec = super::preset_spec(cantus_core::InstrumentKind::Sampler, preset);
    app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    let pad = model::zones(&spec)
        .into_iter()
        .find(|(_, z)| z.get(zf::MODE) >= 2.0 && z.get(zf::XF) > 0.0)
        .map(|(n, _)| n);
    app.inst.sampler.selected = pad.or(Some(0));
    app.inst.view = super::View::Edit;
}

/// For screenshots and tests with no engine: three notes sounding.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0f32; cantus_engine::inst::TELEMETRY_LEN];
    t[super::TEL_VOICES] = 3.0;
    t[super::TEL_POLYPHONY] = 32.0;
    t[tel::LAYERS] = 4.0;
    t[tel::MEMORY_MIB] = 6.4;
    t[tel::ZONES] = 6.0;
    t[tel::PLAYING] = 3.0;
    for (i, (zone, pos)) in [(1.0, 9_000.0), (4.0, 20_000.0), (5.0, 70_000.0)]
        .iter()
        .enumerate()
    {
        t[tel::FIRST_PAIR + 2 * i] = *zone;
        t[tel::FIRST_PAIR + 2 * i + 1] = *pos;
    }
    t[tel::KEYSWITCH] = -1.0;
    app.inst.injected = Some(t);
}
