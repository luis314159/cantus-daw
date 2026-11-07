//! Round 37's fixes to Pulsus III's per-pad outputs and chains (ADR 0100), each reached the
//! way a user reaches it — through `CantusApp::assemble` (the boot path) and the manual
//! engine's processor, the app's own actions, the export's own stem list:
//!
//! 1. a kit routed to another strip takes its pads' chains with it (nothing of the kit is
//!    left playing on the old strip, which a new track may then take);
//! 2. a kit's track template keeps its chains and pad strips, in the new project's ids;
//! 3. exploding a kit's takes does not hand the copies the kit's chains;
//! 4. stems: a chained pad of a kit on the master is in the Direct stem (the stems add up
//!    to the master), and a kit's track stem holds its pads' chains;
//! 5. a sample kit loaded onto a split one gives each strip to the pad of its name;
//! 6. a split the engine has no room for is refused, said in the interface's language.
//!
//! Its own binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render_stems, EngineHandle, Processor, RenderOptions, StemOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// A two-bar groove on every pad of `t`'s kit.
fn groove(p: &mut Project, t: &mut Track) {
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
}

/// A kit track (the studio kit) on a strip of its own, playing a groove; a second audio
/// strip on the desk.
fn kit_song() -> Project {
    let mut p = Project::new("Kit");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    groove(&mut p, &mut t);
    p.tracks.push(t);
    if p.strips.len() < 2 {
        let s = p.make_strip("Bus");
        p.strips.push(s);
    }
    p
}

fn boot_in(p: Project, locale: cantus_ui::i18n::Locale) -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(locale));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    (app, proc)
}

fn boot(p: Project) -> (CantusApp, Processor) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    boot_in(p, cantus_ui::i18n::Locale::En)
}

/// Play from the top and take `frames` of what the engine sends the device.
fn play(app: &mut CantusApp, proc: &mut Processor, frames: usize) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let mut out = Vec::with_capacity(frames * 2);
    let sizes = [512usize, 256, 333, 1024];
    let mut i = 0;
    while out.len() < frames * 2 {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
    }
    app.toggle_play();
    app.sync_engine();
    out
}

fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

fn kit(app: &CantusApp, t: TrackId) -> PulsusParams {
    app.project()
        .track(t)
        .unwrap()
        .instrument
        .pulsus()
        .unwrap()
        .clone()
}

/// The kick's id (the pad on note 36).
fn kick(app: &CantusApp, t: TrackId) -> PadId {
    kit(app, t).pads.iter().find(|p| p.pitch == 36).unwrap().id
}

/// An audio strip of the desk that is not `not`.
fn other_strip(app: &CantusApp, not: StripId) -> StripId {
    app.project()
        .strips
        .iter()
        .find(|s| s.id != not && s.kind == vca::StripKind::Audio)
        .unwrap()
        .id
}

// ---- 1. A kit routed to another strip -----------------------------------------------------

#[test]
fn a_kit_routed_to_another_strip_takes_its_chained_pad_with_it() {
    let _ = data();
    let frames = 3 * 48_000;
    let (mut app, mut proc) = boot(kit_song());
    let t = app.project().tracks[0].id;
    let one = app.project().track(t).unwrap().strip.unwrap();
    let two = other_strip(&app, one);
    let k = kick(&app, t);
    assert!(
        app.add_to_pad_chain(t, k, EffectKind::Eq),
        "a chain on the kick"
    );
    let chain = padout::chain_of(app.project(), kit(&app, t).by_id(k).unwrap()).unwrap();
    // The mixer's route, as a click on the track's output does it: one undo step.
    let steps = app.bus.history().len();
    app.route_track(t, Some(two));
    assert_eq!(app.bus.history().len(), steps + 1, "one undo step");
    let p = app.project();
    assert_eq!(
        p.strip(chain).unwrap().output,
        Some(two),
        "the chain follows the kit"
    );
    assert!(!padout::is_split(p, t), "not split: no Merge back offered");
    // *Freeze* on the kit freezes its strip — now strip 2, which holds the whole kit.
    let target = p.freeze_target_of_track(t).unwrap();
    assert_eq!(target, freeze::FreezeTarget::Strip(two));
    assert!(
        freeze::region(p, target).strips.contains(&chain),
        "the chain is in it"
    );
    assert!(freeze::freeze_commands(p, target).is_ok(), "it freezes");
    assert!(
        !p.tracks_on_strip(one).any(|x| x.id == t)
            && !p.strips.iter().any(|s| s.output == Some(one)),
        "nothing of the kit is left on strip 1"
    );
    // Heard: strip 2 muted silences the whole kit — the kick included.
    app.exec(Command::SetStripMute {
        id: two,
        muted: true,
    });
    let muted = play(&mut app, &mut proc, frames);
    assert!(peak(&muted) < 1e-6, "the kick played on: {}", peak(&muted));
    // And a new track does not get a strip the kick plays through: strip 1 is free, and
    // a kit that kept its chain there would have made it not.
    app.exec(Command::SetStripMute {
        id: two,
        muted: false,
    });
    let heard = play(&mut app, &mut proc, frames);
    assert!(peak(&heard) > 0.05, "the kit sounds on strip 2");
    // Undo puts the chain back on strip 1 with the kit.
    app.undo();
    app.undo();
    app.undo();
    let p = app.project();
    assert_eq!(p.track(t).unwrap().strip, Some(one));
    assert_eq!(p.strip(chain).unwrap().output, Some(one));
}

// ---- 6. No room for a split ---------------------------------------------------------------

#[test]
fn a_split_the_engine_has_no_room_for_is_refused_and_said() {
    let _ = data();
    let mut p = kit_song();
    // 250 tracks: the studio kit's fourteen pads would make 264 the engine plays.
    while p.tracks.len() < 250 {
        let t = p.make_track(format!("T{}", p.tracks.len()));
        p.tracks.push(t);
    }
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(locale);
        let (mut app, _proc) = boot_in(p.clone(), locale);
        let t = app.project().tracks[0].id;
        let before = app.project().clone();
        assert!(!app.split_kit(t), "refused");
        assert_eq!(app.project().tracks, before.tracks, "nothing changed");
        assert_eq!(app.project().strips, before.strips, "no strip made");
        let said = cantus_ui::i18n::tf("padout.no_room", &[("name", &"Drums")]);
        assert!(
            app.status.error && app.status.text == said,
            "{locale:?}: said {:?}",
            app.status.text
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

// ---- 2. Track templates ---------------------------------------------------------------------

/// The strips made for `t`'s pads: chains and pads' own strips.
fn kit_made(p: &Project, t: TrackId) -> Vec<StripId> {
    let k = p.track(t).unwrap().instrument.pulsus().unwrap();
    let mut out: Vec<StripId> = Vec::new();
    for pad in &k.pads {
        if let Some(o) = pad.output {
            out.push(o);
            if let Some(c) = p.strip(o).filter(|s| s.kind == vca::StripKind::Chain) {
                out.extend(
                    c.output
                        .filter(|x| p.strip(*x).is_some_and(padout::pad_made)),
                );
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn a_kits_track_template_keeps_its_chains_in_the_new_projects_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _proc) = boot(kit_song());
    app.workflow.wire_at(dir.path(), dir.path());
    let t = app.project().tracks[0].id;
    let k = kick(&app, t);
    assert!(app.add_to_pad_chain(t, k, EffectKind::Compressor));
    let snare = kit(&app, t).pads.iter().find(|p| p.pitch == 38).unwrap().id;
    assert!(app.add_to_pad_chain(t, snare, EffectKind::Eq));
    let path = app
        .workflow
        .store
        .save_track(app.project(), t, "My kit")
        .unwrap();
    let originals = kit_made(app.project(), t);
    assert_eq!(originals.len(), 2, "two chains");
    // Into the same project: the copy has chains of its own, not the original's.
    let before = app.project().clone();
    assert!(app.insert_track_template(&path, 1));
    let copy = app.project().tracks[1].id;
    let p = app.project();
    let mine = kit_made(p, copy);
    assert_eq!(mine.len(), 2, "the copy's two chains");
    assert!(
        mine.iter().all(|s| !originals.contains(s)),
        "never the original's"
    );
    let copy_strip = p.track(copy).unwrap().strip;
    for s in &mine {
        let st = p.strip(*s).unwrap();
        assert_eq!(st.kind, vca::StripKind::Chain);
        assert_eq!(st.output, copy_strip, "into the copy's own strip");
        assert_eq!(st.effects.len(), 1, "its insert came with it");
        assert!(st.effects.iter().all(|e| p
            .strips
            .iter()
            .filter(|x| x.effects.iter().any(|f| f.id == e.id))
            .count()
            == 1));
    }
    let ck = p.track(copy).unwrap().instrument.pulsus().unwrap();
    let kick_chain = ck
        .pads
        .iter()
        .find(|x| x.pitch == 36)
        .unwrap()
        .output
        .unwrap();
    assert_eq!(
        p.strip(kick_chain).unwrap().effects[0].spec.kind,
        EffectKind::Compressor,
        "the kick keeps its compressor"
    );
    assert!(padout::orphans(p).is_empty());
    // Undo takes the copy and its chains in one step.
    app.undo();
    assert_eq!(
        app.project().strips,
        before.strips,
        "undo: the strips as they were"
    );
    assert_eq!(
        app.project().tracks,
        before.tracks,
        "undo: the tracks as they were"
    );
    // Into another project: no strip id of the first one survives; the chains come along.
    let (mut other, _proc) = boot(Project::new("Other"));
    assert!(other.insert_track_template(&path, 0));
    let p = other.project();
    let t2 = p.tracks[0].id;
    let made = kit_made(p, t2);
    assert_eq!(made.len(), 2, "both chains in the other project");
    for pad in &p.track(t2).unwrap().instrument.pulsus().unwrap().pads {
        if let Some(o) = pad.output {
            assert!(p.strip(o).is_some(), "{}: no dangling output", pad.name);
        }
    }
    assert!(!padout::is_split(p, t2));
    let target = p.freeze_target_of_track(t2).unwrap();
    assert!(
        made.iter()
            .all(|s| freeze::region(p, target).strips.contains(s)),
        "its chains are in what Freeze freezes"
    );
    assert!(freeze::freeze_commands(p, target).is_ok(), "it freezes");
}

// ---- 3. Explode takes -----------------------------------------------------------------------

#[test]
fn exploding_a_kits_takes_leaves_its_chains_with_it() {
    let mut p = kit_song();
    // Two take lanes of the groove.
    let src = p.tracks[0].clips[0].clone();
    for lane in [1u8, 2] {
        let mut c = p.make_clip("Take", Ticks::ZERO, Ticks::beats(8));
        c.notes = src
            .notes
            .iter()
            .map(|n| p.make_note(n.start, n.length, n.pitch, n.velocity))
            .collect();
        c.lane = lane;
        p.tracks[0].clips.push(c);
    }
    let (mut app, _proc) = boot(p);
    let t = app.project().tracks[0].id;
    let k = kick(&app, t);
    assert!(app.add_to_pad_chain(t, k, EffectKind::Gate));
    let chain = padout::chain_of(app.project(), kit(&app, t).by_id(k).unwrap()).unwrap();
    app.explode_takes(t);
    let p = app.project();
    assert_eq!(p.tracks.len(), 3, "two take tracks");
    for copy in &p.tracks[1..] {
        let ck = copy.instrument.pulsus().unwrap();
        assert!(
            ck.pads.iter().all(|x| x.output.is_none()),
            "{}: a copy's pads play on its strip, through no chain of the kit's",
            copy.name
        );
    }
    // The chain's last insert comes out: the chain is the kit's alone.
    let e = padout::chain_effects(app.project(), kit(&app, t).by_id(k).unwrap())[0].id;
    assert!(
        app.remove_from_pad_chain(t, k, e),
        "the chain goes with its last insert"
    );
    assert!(app.project().strip(chain).is_none());
}

// ---- 4. Stems -------------------------------------------------------------------------------

fn stems_of(app: &mut CantusApp, tracks: bool) -> Vec<(cantus_engine::StemSpec, Vec<f32>)> {
    let p = app.project().clone();
    let specs = cantus_ui::views::interop::stem_specs(&p, tracks, true);
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = StemOptions {
        render: RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    render_stems(snap, &opts, &specs)
        .unwrap()
        .into_iter()
        .map(|s| (s.spec, s.samples))
        .collect()
}

fn close(a: &[f32], b: &[f32], tol: f32, what: &str) {
    let n = a.len().min(b.len());
    assert!(n > 48_000, "{what}: {n} samples");
    let worst = (0..n).map(|i| (a[i] - b[i]).abs()).fold(0.0f32, f32::max);
    assert!(worst <= tol, "{what}: off by {worst}");
}

#[test]
fn a_chained_pad_of_a_kit_on_the_master_is_in_the_strips_stems() {
    let mut p = kit_song();
    p.tracks[0].strip = None; // the kit straight to the master
    let (mut app, _proc) = boot(p);
    let t = app.project().tracks[0].id;
    let k = kick(&app, t);
    assert!(app.add_to_pad_chain(t, k, EffectKind::Eq));
    let chain = padout::chain_of(app.project(), kit(&app, t).by_id(k).unwrap()).unwrap();
    assert_eq!(
        app.project().strip(chain).unwrap().output,
        None,
        "the chain to the master"
    );
    let stems = stems_of(&mut app, false);
    let master = &stems
        .iter()
        .find(|(s, _)| s.source == cantus_engine::StemSource::Master)
        .unwrap()
        .1;
    assert!(peak(master) > 0.05);
    // What reaches the master: the Direct stem and every strip stem into the master.
    let p = app.project().clone();
    let mut sum = vec![0f32; master.len()];
    for (spec, x) in &stems {
        let into_master = match spec.source {
            cantus_engine::StemSource::Direct => true,
            cantus_engine::StemSource::Strip(i) => p.strips[i].output.is_none(),
            _ => false,
        };
        if into_master {
            for (o, v) in sum.iter_mut().zip(x) {
                *o += v;
            }
        }
    }
    close(&sum, master, 1e-5, "the stems add up to the master");
}

#[test]
fn a_kits_track_stem_holds_its_pads_chains() {
    let (mut app, _proc) = boot(kit_song());
    let t = app.project().tracks[0].id;
    let kit_strip = app.project().track(t).unwrap().strip.unwrap();
    // The kit's strip at unity under a balance law: its stem is what the track sends it.
    app.exec(Command::SetStripPanLaw {
        id: kit_strip,
        law: Some(PanLaw::ZeroDb),
    });
    let dry = stems_of(&mut app, true);
    let k = kick(&app, t);
    assert!(app.add_to_pad_chain(t, k, EffectKind::Tremolo));
    // (A tremolo at its starting preset changes the kick for certain: checked below.)
    let tracked = stems_of(&mut app, true);
    let track_stem = |stems: &[(cantus_engine::StemSpec, Vec<f32>)]| {
        stems
            .iter()
            .find(|(s, _)| s.source == cantus_engine::StemSource::Track(0))
            .unwrap()
            .1
            .clone()
    };
    let master = tracked
        .iter()
        .find(|(s, _)| s.source == cantus_engine::StemSource::Master)
        .unwrap()
        .1
        .clone();
    let wet = track_stem(&tracked);
    // The track stem is the kit as heard: with the chain, it is the master (the kit's
    // strip and the master at unity, nothing else playing).
    close(
        &wet,
        &master,
        1e-5,
        "the kit's track stem is the kit as the master hears it",
    );
    let differ = wet
        .iter()
        .zip(&track_stem(&dry))
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        differ > 1e-4,
        "the chain changes the kick ({differ}): the stem is not dry"
    );
}

// ---- 5. A sample kit onto a split one -------------------------------------------------------

fn sample_kit(names: &[(u8, &str)]) -> PulsusParams {
    let pads = names
        .iter()
        .map(|(pitch, name)| {
            let mut d = DrumPad::new(*pitch, name, PadVoice::Sample);
            d.sample = Some(format!("{name}.wav"));
            d
        })
        .collect();
    PulsusParams::of(pads)
}

#[test]
fn a_sample_kit_on_a_split_one_gives_each_strip_to_the_pad_of_its_name() {
    let mut p = kit_song();
    p.tracks[0].instrument =
        InstrumentSpec::Pulsus(sample_kit(&[(36, "Kick"), (38, "Snare"), (42, "Hat")]));
    let (mut app, _proc) = boot(p);
    let t = app.project().tracks[0].id;
    assert!(app.split_kit(t));
    let strip_of = |app: &CantusApp, name: &str| {
        let k = kit(app, t);
        let pad = k.pads.iter().find(|x| x.name == name).unwrap();
        pad.output
            .and_then(|o| app.project().strip(o))
            .map(|s| s.name.clone())
    };
    // Another sample kit, its pads on other notes and in another order: by name.
    let new = sample_kit(&[(35, "Hat"), (37, "Snare"), (39, "Kick")]);
    app.set_instrument(t, InstrumentSpec::Pulsus(new));
    for name in ["Kick", "Snare", "Hat"] {
        assert_eq!(
            strip_of(&app, name).as_deref(),
            Some(name),
            "{name} plays on {name}'s strip"
        );
    }
}

// ---- The second fixes review: a template's undo, its VCAs ---------------------------------

/// A chain made while the kit went to the master sits before the strip the kit was routed
/// to later, and before the pad strips a split put after that: the template keeps them in
/// that desk order, and its insert must still undo in one step (a pad strip a chain feeds
/// cannot be removed before the chain). A strip's VCAs are the project's: no template keeps
/// them.
#[test]
fn a_kits_template_with_a_chain_before_its_pad_strips_undoes_and_keeps_no_vca() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = kit_song();
    p.tracks[0].strip = None; // the kit to the master while its chain is made
    let mut v = p.make_strip("VCA");
    v.kind = vca::StripKind::Vca;
    let vca_id = v.id;
    p.strips.push(v);
    let (mut app, _proc) = boot(p);
    app.workflow.wire_at(dir.path(), dir.path());
    let t = app.project().tracks[0].id;
    let k = kick(&app, t);
    assert!(app.add_to_pad_chain(t, k, EffectKind::Eq));
    let chain = padout::chain_of(app.project(), kit(&app, t).by_id(k).unwrap()).unwrap();
    // A strip after the chain on the desk, the kit routed to it, a VCA over it; split.
    let late = app.bus.new_strip("Kit bus");
    let late_id = late.id;
    let n = app.project().strips.len();
    assert!(app.exec(Command::InsertStrip {
        index: n,
        strip: Box::new(late),
    }));
    app.route_track(t, Some(late_id));
    assert_eq!(app.project().strip(chain).unwrap().output, Some(late_id));
    assert!(app.exec(Command::Mix(
        cantus_core::mix_cmd::MixCommand::SetStripVcas {
            id: late_id,
            vcas: vec![vca_id],
        }
    )));
    assert!(app.split_kit(t));
    let p = app.project();
    let own = padout::heard_on(p, p.track(t).unwrap(), kit(&app, t).by_id(k).unwrap()).unwrap();
    assert!(
        p.strip_index(chain) < p.strip_index(own),
        "the chain sits before the pad strip it feeds"
    );
    let path = app
        .workflow
        .store
        .save_track(app.project(), t, "Chained kit")
        .unwrap();
    let tpl = cantus_ui::templates::load_track_template(&path).unwrap();
    assert!(
        tpl.strips.iter().all(|s| s.vcas.is_empty()),
        "no VCA id in the template"
    );
    // Inserted, then undone: one step, exactly back.
    let before = app.project().clone();
    assert!(app.insert_track_template(&path, 1));
    let p = app.project();
    let copy = p.tracks[1].id;
    assert_eq!(kit_made(p, copy).len(), kit_made(&before, t).len());
    assert!(p
        .strips
        .iter()
        .all(|s| before.strip(s.id).is_some() || s.vcas.is_empty()));
    app.undo();
    assert_eq!(
        app.project().strips,
        before.strips,
        "undo: the strips as they were"
    );
    assert_eq!(
        app.project().tracks,
        before.tracks,
        "undo: the tracks as they were"
    );
}
