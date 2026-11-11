//! Round 39, `lo-rest`: the channel rack and the browser as des-views drew them — the
//! rack's plates, target marks, a kit's tree and its foot ("Add a channel", the drop zone);
//! the browser's tree, its search grouped with chips, the preview (its 38-high well and
//! its star) and the Packs page's covers with their state — the whole window, in Slate
//! Night and Slate Paper at 1920 × 1080 (125 %), on the demo song the program opens on.
//!
//! `cargo run -p cantus-ui --example shoot_lo_rest [-- out_dir [case…]]`
//! Writes `<view>-<night|paper>-1920x1080.png`; a case on the command line renders only the
//! cases whose name starts with it.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

type Setup = fn(&mut CantusApp);

fn rack(a: &mut CantusApp) {
    a.show_central(CentralView::ChannelRack);
    // The arrangement folded: the rack as tall as it goes, its foot in view.
    a.workspace.toggle_fold(Pane::Arrangement);
}

/// The kit of the demo's hats chosen, its pads listed.
fn rack_kit(a: &mut CantusApp) {
    rack(a);
    let hats = a
        .project()
        .tracks
        .iter()
        .find(|t| t.name.to_lowercase().contains("hat"))
        .map(|t| (t.id, t.clips.first().map(|c| c.id)));
    if let Some((t, c)) = hats {
        a.selected_track = Some(t);
        a.selected_clip = c;
    }
}

/// The first pack open on its first folder of sounds, the first sound chosen.
fn open_a_pack(a: &mut CantusApp) {
    let user = a.settings.sample_folders.clone();
    for root in a.samples.roots(&user) {
        let Ok(rd) = std::fs::read_dir(&root.path) else {
            continue;
        };
        let mut dirs: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        let Some(dir) = dirs.into_iter().find(|d| {
            std::fs::read_dir(d).is_ok_and(|r| {
                r.flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "wav"))
            })
        }) else {
            continue;
        };
        a.samples.open.insert(root.path.clone());
        a.samples.open.insert(dir.clone());
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut wavs: Vec<_> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "wav"))
                .collect();
            wavs.sort();
            // Every row's waveform read before the shot (they are read off this thread):
            // the steady browser, not the second it opens.
            let since = std::time::Instant::now();
            while wavs
                .iter()
                .any(|w| cantus_ui::views::samples_thumbs::thumb(w).is_none())
                && since.elapsed().as_secs() < 30
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            a.samples.selected = wavs.first().cloned();
            // Starred: the Favourites section and the filled star.
            if let Some(w) = wavs.get(1) {
                a.settings.sample_favourites = vec![w.clone()];
            }
        }
        break;
    }
}

fn browser(a: &mut CantusApp) {
    a.toggle_pane(Pane::Browser);
    open_a_pack(a);
}

fn search(a: &mut CantusApp) {
    browser(a);
    a.samples.query = std::env::var("LO_REST_QUERY").unwrap_or_else(|_| "kick".into());
    // The index built before the shot (it is built on its own thread).
    let user = a.settings.sample_folders.clone();
    let roots = a.samples.roots(&user);
    let since = std::time::Instant::now();
    while a.samples.search(&roots, 10).is_none() && since.elapsed().as_secs() < 60 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn packs(a: &mut CantusApp) {
    browser(a);
    a.samples.window.open = true;
}

/// The packs not yet here in view: two installed ones kept, then every pack to get.
fn packs_get(a: &mut CantusApp) {
    browser(a);
    let root = cantus_ui::app::data_dir().join("Content");
    let mut m = a.samples.content().manifest().clone();
    let installed: Vec<String> = m
        .packs
        .iter()
        .filter(|p| a.samples.content().is_installed(&p.id))
        .map(|p| p.id.clone())
        .collect();
    m.packs
        .retain(|p| !installed.contains(&p.id) || installed.iter().take(2).any(|i| *i == p.id));
    a.samples.set_content(cantus_content::Manager::new(root, m));
    a.samples.window.open = true;
}

/// As `packs_get`, the first pack to get shown as being fetched (the window's test hook):
/// the 4-px bar, "42 % of …" and Cancel.
fn packs_getting(a: &mut CantusApp) {
    packs_get(a);
    // The first pack to get as the page shows them (by category).
    let mut packs = a.samples.content().manifest().packs.clone();
    packs.sort_by_key(|p| p.category as u8);
    let busy = packs
        .iter()
        .find(|p| !a.samples.content().is_installed(&p.id))
        .map(|p| (p.id.clone(), p.download_size()));
    if let Some((id, total)) = busy {
        a.samples.window.shown_busy = Some((
            id,
            cantus_content::Phase::Downloading {
                done: total * 47 / 100,
                total,
            },
        ));
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-lo-rest")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let views: [(&str, Setup); 7] = [
        ("rack", rack),
        ("rack-kit", rack_kit),
        ("browser", browser),
        ("search", search),
        ("packs", packs),
        ("packs-get", packs_get),
        ("packs-getting", packs_getting),
    ];
    for (view, setup) in views {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            let (size, zoom) = ((1920.0f32, 1080.0f32), 1.25f32);
            let name = format!(
                "{view}-{}-{}x{}",
                preset.trim_start_matches("slate_"),
                size.0 as u32,
                size.1 as u32
            );
            if !only.is_empty() && !only.iter().any(|o| name.starts_with(o.as_str())) {
                continue;
            }
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-lo-rest");
            let project = cantus_ui::demos::startup_project();
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
            let mode = app.appearance.mode;
            app.appearance.assign(mode, format!("preset:{preset}"));
            app.attach(&ctx);
            if let Some(strip) = app.project().tracks.get(1).and_then(|t| t.strip) {
                app.select_strip(strip);
            }
            setup(&mut app);
            let points = egui::vec2(size.0 / zoom, size.1 / zoom);
            let path = dir.join(format!("{name}.png"));
            raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
            println!("{}", path.display());
        }
    }
}
