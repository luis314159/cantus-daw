//! The video dialog's rows: the picture (format, shape, frame rate, quality), the look
//! (background, title), what of the song, and the encoders — said before anything is made,
//! with Cisco's switch where the system has no H.264 of its own.

use super::put;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::Src;
use crate::kit::widgets::{menu_field, segmented};
use crate::kit::Style;
use crate::video_export::{
    self as vx, job, Back, Format, Quality, Range, Shape, VideoSettings, SIZES,
};
use cantus_video::select::{AudioPath, VideoPath};
use egui::Ui;

fn width(ui: &Ui) -> f32 {
    let s = Style::current();
    (ui.available_width() / s.k - 1.0).max(160.0)
}

fn row<R>(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    crate::views::export_dialog::row(ui, key, add)
}

fn seg(ui: &mut Ui, selected: usize, labels: &[Src<'_>], name: &str) -> Option<usize> {
    let mut i = selected;
    let wd = width(ui);
    let r = segmented(ui, &mut i, labels, name, wd).rect;
    let n = labels.len() as f32;
    for j in 0..labels.len() {
        let w = r.width() / n;
        let x = r.left() + w * j as f32;
        let seg = egui::Rect::from_min_size(egui::pos2(x, r.top()), egui::vec2(w, r.height()));
        put(ui, format!("{name}#{j}"), seg);
    }
    (i != selected).then_some(i)
}

fn check(ui: &mut Ui, on: bool, enabled: bool, label: &str, name: &str) -> bool {
    let r = ui.vertical(|ui| super::super::interop::form::choice(ui, on, enabled, label));
    put(ui, name, r.response.rect);
    r.inner && enabled
}

fn note(ui: &mut Ui, text: &str, warn: bool) {
    super::super::workspace_settings::note(ui, text, warn);
}

/// Every row.
pub fn body(app: &mut CantusApp, ui: &mut Ui, d: &mut VideoSettings) {
    picture(ui, d);
    look(ui, d);
    what(app, ui, d);
    encoders(app, ui, d);
}

fn picture(ui: &mut Ui, d: &mut VideoSettings) {
    crate::views::export_dialog::band_row(ui, "video.band.picture");
    let labels: Vec<Src<'_>> = Format::ALL.iter().map(|f| Src::Key(f.key())).collect();
    let at = Format::ALL.iter().position(|f| *f == d.format).unwrap_or(0);
    if let Some(i) = row(ui, "video.row.format", |ui| seg(ui, at, &labels, "format")) {
        d.format = Format::ALL[i];
    }
    let labels: Vec<Src<'_>> = Shape::ALL.iter().map(|s| Src::Key(s.key())).collect();
    let at = Shape::ALL.iter().position(|s| *s == d.shape).unwrap_or(0);
    if let Some(i) = row(ui, "video.row.shape", |ui| seg(ui, at, &labels, "shape")) {
        d.shape = Shape::ALL[i];
    }
    if d.shape == Shape::Custom {
        let (w, h) = d.size();
        let value = format!("{w} × {h}");
        let resp = row(ui, "video.row.size", |ui| {
            let wd = width(ui);
            menu_field(ui, &value, false, false, "size", wd).0
        });
        put(ui, "size", resp.rect);
        let options: Vec<((u32, u32), String)> = SIZES
            .iter()
            .map(|&(w, h)| ((w, h), format!("{w} × {h}")))
            .collect();
        if let Some(v) = crate::views::export_dialog::pick(&resp, &options, (w, h)) {
            d.custom = v;
        }
    }
    let rates = [30u32, 60];
    let labels = [Src::Key("video.fps.30"), Src::Key("video.fps.60")];
    let at = rates.iter().position(|r| *r == d.fps).unwrap_or(0);
    if let Some(i) = row(ui, "video.row.fps", |ui| seg(ui, at, &labels, "fps")) {
        d.fps = rates[i];
    }
    let labels = [
        Src::Key("video.quality.good"),
        Src::Key("video.quality.high"),
        Src::Key("video.quality.best"),
    ];
    let at = Quality::ALL
        .iter()
        .position(|q| *q == d.quality)
        .unwrap_or(1);
    if let Some(i) = row(ui, "video.row.quality", |ui| {
        seg(ui, at, &labels, "quality")
    }) {
        d.quality = Quality::ALL[i];
    }
}

fn look(ui: &mut Ui, d: &mut VideoSettings) {
    crate::views::export_dialog::band_row(ui, "video.band.look");
    // Stage, transparent, a chroma key (its colour in the row below) or black.
    let labels = [
        Src::Key("video.back.stage"),
        Src::Key("video.back.transparent"),
        Src::Key("video.back.key"),
        Src::Key("video.back.black"),
    ];
    let at = match d.background {
        Back::Stage => 0,
        Back::Transparent => 1,
        Back::Green | Back::Blue | Back::Magenta => 2,
        Back::Black => 3,
    };
    if let Some(i) = row(ui, "video.row.background", |ui| {
        seg(ui, at, &labels, "background")
    }) {
        d.background = [Back::Stage, Back::Transparent, Back::Green, Back::Black][i];
    }
    if d.background.chroma().is_some() {
        let keys = [Back::Green, Back::Blue, Back::Magenta];
        let labels: Vec<Src<'_>> = keys.iter().map(|b| Src::Key(b.key())).collect();
        let at = keys.iter().position(|b| *b == d.background).unwrap_or(0);
        if let Some(i) = row(ui, "video.row.key", |ui| seg(ui, at, &labels, "key")) {
            d.background = keys[i];
        }
    }
    if d.background == Back::Transparent && !d.format.alpha() {
        note(ui, &t("video.note.no_alpha"), true);
    } else if d.background.chroma().is_some() && !d.format.alpha() {
        note(ui, &t("video.note.chroma"), false);
    } else if d.transparent() {
        note(ui, &t("video.note.alpha"), false);
    }
    row(ui, "video.row.title", |ui| {
        if check(ui, d.title, true, &t("video.title_on"), "title") {
            d.title = !d.title;
        }
    });
    row(ui, "stage.video.row", |ui| {
        // r35:stage — the light visualiser (ADR 0091 §4)
        if check(ui, d.visualiser, true, &t("stage.video.on"), "visualiser") {
            d.visualiser = !d.visualiser; // r35:stage
        } // r35:stage
    }); // r35:stage
}

fn what(app: &mut CantusApp, ui: &mut Ui, d: &mut VideoSettings) {
    crate::views::export_dialog::band_row(ui, "video.band.what");
    let labels = [
        Src::Key("export_audio.scope.song"),
        Src::Key("export_audio.scope.selection"),
        Src::Key("export_audio.scope.loop"),
    ];
    let at = Range::ALL.iter().position(|r| *r == d.range).unwrap_or(0);
    if let Some(i) = row(ui, "video.row.range", |ui| seg(ui, at, &labels, "range")) {
        d.range = Range::ALL[i];
        app.video.scrub = 0.0;
    }
    match vx::range(app, d.range) {
        Some(_) => {
            let secs = super::length(app, d);
            note(
                ui,
                &tf("video.length", &[("time", &super::clock(secs))]),
                false,
            );
        }
        None => note(ui, &refusal(app).unwrap_or_default(), true),
    }
}

/// The encoders in words, for the plan.
fn names(p: &cantus_video::Plan) -> (String, String) {
    let v = match &p.video {
        Some(VideoPath::MediaFoundation) => t("video.enc.mf_h264"),
        Some(VideoPath::OpenH264(_)) => t("video.enc.openh264"),
        Some(VideoPath::VideoToolbox) => t("video.enc.vt"),
        None => t("video.enc.none"),
    };
    let a = match &p.audio {
        AudioPath::MediaFoundation => t("video.enc.mf_aac"),
        AudioPath::AudioToolbox => t("video.enc.at_aac"),
        AudioPath::System(cantus_video::audio_alt::Tool::Ffmpeg(_)) => t("video.enc.ffmpeg"),
        AudioPath::System(cantus_video::audio_alt::Tool::GStreamer(_)) => t("video.enc.gstreamer"),
        AudioPath::Mp3 => t("video.enc.mp3"),
    };
    (v, a)
}

fn encoders(app: &mut CantusApp, ui: &mut Ui, d: &mut VideoSettings) {
    crate::views::export_dialog::band_row(ui, "video.band.encoders");
    match d.format {
        Format::Webm => {
            note(ui, &t("video.enc.webm"), false);
            note(ui, &t("video.note.webm_slow"), false);
            return;
        }
        Format::Mov => {
            note(ui, &t("video.enc.mov"), false);
            return;
        }
        Format::Png => {
            note(ui, &t("video.enc.png"), false);
            return;
        }
        Format::Mp4 => {}
    }
    let Some(plan) = vx::plan(app, d) else {
        note(ui, &t("video.enc.checking"), false);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        return;
    };
    let (v, a) = names(&plan);
    note(
        ui,
        &tf("video.enc.plan", &[("video", &v), ("audio", &a)]),
        false,
    );
    // Cisco's OpenH264 where the system has no H.264 of its own (Linux, Windows N).
    let own = matches!(
        plan.video,
        Some(VideoPath::MediaFoundation | VideoPath::VideoToolbox)
    );
    if !own && cantus_video::openh264::blob().is_some() {
        row(ui, "video.row.openh264", |ui| {
            if check(
                ui,
                d.openh264,
                true,
                &t("video.openh264.switch"),
                "openh264",
            ) {
                d.openh264 = !d.openh264;
            }
        });
        note(ui, cantus_video::openh264::CREDIT, false);
        let installed = job::openh264_install(app)
            .and_then(|i| i.library())
            .is_some();
        if let Some(f) = &app.video.openh264_fetch {
            let kb = f.bytes.load(std::sync::atomic::Ordering::Relaxed) / 1024;
            note(
                ui,
                &tf(
                    "video.openh264.fetching",
                    &[("kb", &kb), ("of", &(f.of / 1024))],
                ),
                false,
            );
        } else if d.openh264 && !installed {
            row(ui, "video.row.download", |ui| {
                let b = crate::views::audio_parts::button(
                    ui,
                    &t("video.openh264.download"),
                    true,
                    true,
                    140.0,
                );
                put(ui, "button.download", b.rect);
                if b.clicked() {
                    job::fetch_openh264(app);
                }
            });
            note(ui, &t("video.openh264.terms"), false);
        } else if !d.openh264 && installed {
            // Switched off with Cisco's library here: offer to remove it (review 1 #9).
            row(ui, "video.row.download", |ui| {
                let b = crate::views::audio_parts::button(
                    ui,
                    &t("video.openh264.remove"),
                    false,
                    true,
                    140.0,
                );
                put(ui, "button.remove", b.rect);
                if b.clicked() {
                    job::remove_openh264(app);
                }
            });
        }
        if plan.video.is_none() {
            note(ui, &t(vx::no_h264_key()), true);
        }
    }
    if !matches!(
        plan.audio,
        AudioPath::MediaFoundation | AudioPath::AudioToolbox
    ) {
        row(ui, "video.row.system_aac", |ui| {
            if check(ui, d.system_aac, true, &t("video.system_aac"), "system_aac") {
                d.system_aac = !d.system_aac;
            }
        });
    }
    if plan.audio == AudioPath::Mp3 {
        note(ui, &t("video.warn.mp3"), true);
    }
    if plan.audio == AudioPath::MediaFoundation {
        note(ui, &t("video.note.windows_player"), false);
    }
}

/// Why the export cannot start now, in words (`None`: it can).
pub fn refusal(app: &mut CantusApp) -> Option<String> {
    let d = app.video.dialog.clone()?;
    if vx::range(app, d.range).is_none() {
        return Some(t(match d.range {
            Range::Song => "video.error.empty",
            Range::Selection => "export_audio.no_selection",
            Range::Loop => "export_audio.no_loop",
        }));
    }
    if d.format == Format::Mp4 {
        match vx::plan(app, &d) {
            None => return Some(t("video.enc.checking")),
            Some(p) if p.video.is_none() => return Some(t(vx::no_h264_key())),
            Some(_) => {}
        }
    }
    None
}

/// What the export will take and make, before it starts: from this machine's measured
/// pace (ADR 0088's amendment) and the format's bytes.
pub fn estimate(app: &mut CantusApp) -> String {
    let Some(d) = app.video.dialog.clone() else {
        return String::new();
    };
    if let Some(why) = refusal(app) {
        return why;
    }
    let secs = super::length(app, &d);
    let (w, h) = d.size();
    let mpx_frames = vx::mpx_frames(&d, secs);
    // Milliseconds a megapixel-frame: this machine's, measured by its last exports of the
    // format; until one has run, the round's measurements at 1080 × 1920.
    let measured = app.settings.video.pace.get(vx::pace_key(d.format)).copied();
    let ms = measured.unwrap_or(match d.format {
        Format::Mp4 => 2.0,
        Format::Mov => 1.4,
        Format::Png => 1.3,
        Format::Webm if cantus_video::av1::ASSEMBLY => 77.0,
        Format::Webm => 220.0,
    });
    let took = mpx_frames * ms / 1000.0 + secs * 0.05;
    let bytes = match d.format {
        Format::Mp4 => d.quality.video().bitrate(w, h, d.fps) as f64 / 8.0 * secs * 0.6,
        Format::Webm => mpx_frames * 1e6 * 0.021,
        Format::Mov | Format::Png => mpx_frames * 1e6 * 0.042,
    };
    tf(
        "video.estimate",
        &[
            ("time", &vx::honest(took)),
            ("mb", &format!("{:.0}", (bytes / 1e6).max(1.0))),
        ],
    )
}
