//! Chorda's source and mixer, in its window (round 40 phase 3, ADR 0125; `chorda-src`):
//! the source list beside the piano (*Chorda 2*, *Previous models*, the recorded pianos,
//! *Install from a file…*) and the switch it makes — one undo step through
//! `cantus_core::inst::chorda_source::switch`.

#[path = "chorda_source_card.rs"]
pub mod card;
#[path = "chorda_source_list.rs"]
pub mod list;
#[path = "chorda_source_mixer.rs"]
pub mod mixer;
#[path = "chorda_source_read.rs"]
pub mod read;
#[path = "chorda_source_room.rs"]
pub mod room;
#[path = "chorda_source_strip.rs"]
pub mod strip;

pub use mixer::mixer;

use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_core::inst::chorda_source::{
    self as cs, ChordaSource, RecordedPiano, SourceChoice, SourceRefusal,
};
use cantus_core::inst_out::{key, OutputField, OutputId};
use cantus_core::{Command, NativeSpec, TrackId};

/// The image a set plays first when nothing says otherwise: bitKlavier's Stage Lid ORTF —
/// balanced, the hammers clear, the audience's place (ADR 0125; the owner's ABX decides
/// the default piano, not this).
pub const DEFAULT_IMAGE: OutputId = OutputId(6);

impl CantusApp {
    /// Switch the Chorda on `track` to `choice` as one undo step: a model version, or a
    /// recorded piano read from its installed pack(s). Says why on the error line when it
    /// cannot; `false` then.
    pub fn chorda_switch(&mut self, track: TrackId, choice: &SourceChoice) -> bool {
        let Some(spec) = self
            .project()
            .track(track)
            .and_then(|t| t.instrument.native())
            .cloned()
        else {
            return false;
        };
        let cmd = match choice {
            SourceChoice::Model(_) => cs::switch_choice(self.project(), track, choice, None),
            SourceChoice::Recorded(r) => self.recorded_switch(track, &spec, r, None),
        };
        match cmd {
            Ok(c) => self.exec(c),
            Err(SourceRefusal::Same) => false,
            Err(e) => {
                self.set_error(tf("chorda_source.failed", &[("why", &refusal_words(e))]));
                false
            }
        }
    }

    pub(crate) fn recorded_switch(
        &mut self,
        track: TrackId,
        spec: &NativeSpec,
        r: &RecordedPiano,
        turn: Option<(OutputId, bool)>,
    ) -> Result<Command, SourceRefusal> {
        let entry = read::entry(self.samples.content(), r).ok_or(SourceRefusal::NotInstalled)?;
        if !entry.installed() {
            return Err(SourceRefusal::NotInstalled);
        }
        // The images that are on (an output's `on` key, absent = on); the one this piano
        // played first stays first.
        let on = |o: OutputId| match turn {
            Some((t, v)) if t == o => v,
            _ => spec.get(&key(o, OutputField::On)) >= 0.5,
        };
        let preferred = match ChordaSource::of(spec) {
            ChordaSource::Recorded(now) if now == *r => cs::outputs(spec).first().map(|o| o.id),
            _ => Some(DEFAULT_IMAGE),
        };
        let (zones, left_out) =
            read::zones(&entry, preferred, &on).map_err(SourceRefusal::Unreadable)?;
        let mut cmd = cs::switch(
            self.project(),
            track,
            &ChordaSource::Recorded(r.clone()),
            Some(&zones),
        )?;
        // Images on disk that did not fit (Exemplar's zones): off, so the mixer says so and
        // one can be turned on in another's place.
        if let Command::Batch { commands, .. } = &mut cmd {
            if let Some(Command::SetInstrument { spec, .. }) = commands.first_mut() {
                if let Some(n) = spec.native_mut() {
                    for o in &left_out {
                        n.set(&key(*o, OutputField::On), 0.0);
                    }
                    // Images on disk the user turned off stay off.
                    for img in entry.images.iter().filter(|i| !on(i.output)) {
                        n.set(&key(img.output, OutputField::On), 0.0);
                    }
                    if let Some((o, true)) = turn {
                        n.clear(&key(o, OutputField::On));
                    }
                }
            }
        }
        Ok(cmd)
    }
}

/// Turn one image of the recorded piano `track` plays on or off (its samples loaded or
/// not): the piano read again with it, one undo step.
pub fn set_image_on(app: &mut CantusApp, track: TrackId, image: OutputId, on: bool) {
    let Some(spec) = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
    else {
        return;
    };
    let ChordaSource::Recorded(r) = ChordaSource::of(&spec) else {
        // The model: its outputs load nothing of their own; the key is all there is.
        app.set_inst_value(
            track,
            &key(image, OutputField::On),
            if on { 1.0 } else { 0.0 },
            false,
        );
        return;
    };
    match app.recorded_switch(track, &spec, &r, Some((image, on))) {
        Ok(cmd) => {
            app.exec(cmd);
        }
        Err(SourceRefusal::Same) => {}
        Err(e) => app.set_error(tf("chorda_source.failed", &[("why", &refusal_words(e))])),
    }
}

/// Why a source could not be switched, in the interface's language (review of 40a27bb7,
/// finding 9: the image switch's error line showed the refusal's debug text).
pub fn refusal_words(e: SourceRefusal) -> String {
    match e {
        SourceRefusal::NotInstalled => crate::i18n::t("chorda_source.not_installed"),
        SourceRefusal::Unreadable(w) => w,
        _ => crate::i18n::t("chorda_source.not_chorda"),
    }
}

/// *Install from a file…*: a file chosen, recognised (a pack's own file by size and hash,
/// else a user's instrument or archive), installed, and — a user's piano — played.
pub fn install_from_file(app: &mut CantusApp, track: TrackId) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(
            "SFZ · Decent Sampler · SF2",
            &["sfz", "dspreset", "sf2", "zip", "gz", "tgz"],
        )
        .pick_file()
    else {
        return;
    };
    install_path(app, track, &path);
}

/// What *Install from a file…* made of a file.
#[derive(Clone, Debug)]
pub enum Installed {
    /// A pack the manifest knows (its own download, recognised by size and hash): it
    /// installs in the background as that pack.
    Pack(String),
    /// A user's own instrument, archive or DrumGizmo kit: its install started on the
    /// content manager's worker under this id (r40:fix-f — never on the window's thread);
    /// the track takes it once it is in ([`CantusApp::user_installs_done`]).
    Started(String),
}

/// *Install from a file…* for any instrument that takes packs (Chorda's pianos,
/// Tympanum's kits): a pack's own file installs as that pack; any other instrument file or
/// archive becomes a user pack whose instruments play in `plays` on `track` once installed
/// (ADR 0125 §3). Shared by Chorda's source list and Tympanum's kits list. Both run in the
/// background; the track keeps playing what it plays until the new one is in.
pub fn install_any(
    app: &mut CantusApp,
    path: &std::path::Path,
    plays: cantus_content::manifest::Plays,
    track: TrackId,
) -> Result<Installed, String> {
    use cantus_content::user_pack::{self, Recognised};
    let m = app.samples.content();
    match user_pack::recognise(m.manifest(), path).map_err(|e| e.to_string())? {
        Recognised::Pack(id) => {
            m.install_from_file(&id, path);
            Ok(Installed::Pack(id))
        }
        _ => {
            let id = m
                .install_user(path, plays)
                .ok_or_else(|| crate::i18n::t("inst.install.already"))?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| id.clone());
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            app.samples
                .user_installs
                .retain(|u| u.id != id || u.track != track);
            app.samples.user_installs.push(crate::samples::UserInstall {
                id: id.clone(),
                track,
                plays,
                name,
                size,
            });
            Ok(Installed::Started(id))
        }
    }
}

/// [`install_from_file`] for a path already chosen (a drop; the tests).
pub fn install_path(app: &mut CantusApp, track: TrackId, path: &std::path::Path) {
    match install_any(app, path, cantus_content::manifest::Plays::Chorda, track) {
        Ok(Installed::Pack(id)) | Ok(Installed::Started(id)) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(id);
            app.set_status(tf("chorda_source.installing", &[("name", &name)]));
        }
        Err(why) => app.set_error(tf("chorda_source.failed", &[("why", &why)])),
    }
}

impl CantusApp {
    /// r40:fix-f — the user installs among `finished` that ended: each that is in gives its
    /// track the piano or kit (if the track still hosts that instrument), one that failed
    /// says why, one cancelled says so. Returns the other installs (the packs').
    pub fn user_installs_done(
        &mut self,
        finished: Vec<cantus_content::manager::Finished>,
    ) -> Vec<cantus_content::manager::Finished> {
        use cantus_content::manifest::Plays;
        let mut rest = Vec::new();
        for f in finished {
            let Some(at) = self.samples.user_installs.iter().position(|u| u.id == f.id) else {
                rest.push(f);
                continue;
            };
            let job = self.samples.user_installs.remove(at);
            self.samples.refresh();
            let hosts =
                |app: &CantusApp, kind| app.native_of(job.track).is_some_and(|n| n.kind == kind);
            match f.result {
                Ok(receipt) => {
                    let Some(i) = receipt.instruments.first() else {
                        continue;
                    };
                    match job.plays {
                        Plays::Tympanum
                            if hosts(self, cantus_core::InstrumentKind::AcousticDrums) =>
                        {
                            let kit = cantus_core::inst::tympanum::KitRef::Pack {
                                pack: receipt.id.clone(),
                                kit: i.id.clone(),
                            };
                            super::tympanum::kit_change::change_kit(self, job.track, &kit);
                        }
                        Plays::Chorda if hosts(self, cantus_core::InstrumentKind::Chorda) => {
                            let choice = SourceChoice::Recorded(RecordedPiano {
                                pack: receipt.id.clone(),
                                instrument: i.id.clone(),
                            });
                            self.chorda_switch(job.track, &choice);
                        }
                        _ => {}
                    }
                    self.set_status(tf("inst.install.done", &[("name", &job.name)]));
                }
                Err(cantus_content::install::InstallError::Fetch(
                    cantus_content::fetch::FetchError::Cancelled,
                )) => {
                    self.set_status(tf("inst.install.cancelled", &[("name", &job.name)]));
                }
                Err(e) => {
                    let why = crate::views::samples_content::install_error_text(&e);
                    self.set_error(tf(
                        "inst.install.failed",
                        &[("name", &job.name), ("why", &why)],
                    ));
                }
            }
        }
        rest
    }

    /// Wait for every user install to end and take each in (the tests; no frame needed).
    pub fn wait_user_installs(&mut self, timeout: std::time::Duration) {
        let finished = self.samples.content().wait_idle(timeout);
        let _ = self.user_installs_done(finished);
    }
}

/// The install under way for `plays` (Chorda's pianos, Tympanum's kits), if one is.
pub fn install_under_way(
    app: &mut CantusApp,
    plays: cantus_content::manifest::Plays,
) -> Option<(
    crate::samples::UserInstall,
    Option<cantus_content::install::Phase>,
)> {
    let job = app
        .samples
        .user_installs
        .iter()
        .find(|u| u.plays == plays)
        .cloned()?;
    match app.samples.content().status(&job.id) {
        cantus_content::manager::Status::Busy(ph) => Some((job, ph)),
        _ => Some((job, None)),
    }
}

/// *Install from a file…*'s box while a file installs: its name, a bar, how far, and a
/// click on it cancels (the help line says so). Hand-drawn as the box it stands in for.
pub fn install_progress_box(
    app: &mut CantusApp,
    ui: &egui::Ui,
    s: &crate::kit::Style,
    r: egui::Rect,
    job: &crate::samples::UserInstall,
    phase: Option<&cantus_content::install::Phase>,
) {
    use crate::kit::{with_brush, Fill};
    use crate::views::mixer::look::{Pal, W};
    use cantus_content::install::Phase;
    use egui::{pos2, vec2, Align2, Rect};
    let p = Pal::of(s);
    let resp = ui.interact(
        r,
        ui.id().with(("user-install-busy", &job.id)),
        egui::Sense::click(),
    );
    let resp = super::inst_mixer::ui::hint(resp, || {
        tf("inst.install.tip_cancel", &[("name", &job.name)])
    });
    if resp.clicked() {
        app.samples.content().cancel(&job.id);
    }
    let f = crate::views::samples_content::fraction(phase);
    let (what, done, total) = match phase {
        Some(Phase::Building { done, total }) => (
            crate::i18n::t("inst.install.building"),
            format!("{done}"),
            format!("{total}"),
        ),
        _ => (
            crate::i18n::t("inst.install.unpacking"),
            super::tympanum::kits::size_text((job.size as f64 * f64::from(f)) as u64),
            super::tympanum::kits::size_text(job.size),
        ),
    };
    let line = tf(
        "inst.install.progress",
        &[
            ("what", &what),
            ("pct", &format!("{:.0}", f * 100.0)),
            ("done", &done),
            ("total", &total),
        ],
    );
    with_brush(ui.painter(), |b| {
        let rr = b.snap_rect(r);
        let ink = if resp.hovered() { p.ink2 } else { p.ink4 };
        let pts = [
            rr.left_top(),
            rr.right_top(),
            rr.right_bottom(),
            rr.left_bottom(),
            rr.left_top(),
        ];
        b.dashed(&pts, b.px(), ink, s.s(4.0), s.s(3.0));
        let tx = rr.left() + s.s(12.0);
        let nr = Rect::from_min_max(
            pos2(tx, rr.top() + s.s(5.0)),
            pos2(rr.right() - s.s(8.0), rr.top() + s.s(21.0)),
        );
        let name = tf("inst.install.installing", &[("name", &job.name)]);
        super::inst_mixer::ui::words(
            b,
            s,
            &name,
            nr,
            Align2::LEFT_CENTER,
            12.0,
            W::Medium,
            p.ink1,
        );
        let bar = Rect::from_min_size(
            pos2(tx, nr.bottom() + s.s(3.0)),
            vec2(rr.width() - s.s(24.0), s.s(3.0)),
        );
        b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
        let fill =
            Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height()));
        b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.accent));
        let lr = Rect::from_min_max(
            pos2(tx, bar.bottom() + s.s(2.0)),
            pos2(rr.right() - s.s(8.0), rr.bottom() - s.s(2.0)),
        );
        super::inst_mixer::ui::words(
            b,
            s,
            &line,
            lr,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}
