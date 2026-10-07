//! The export dialog's rows: what, format and quality, rate, channels, dither, loudness,
//! tags. Each row is a caption and a kit control; a menu field opens a kit menu.

use super::export_dialog::{band_row, pick, row};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::Src;
use crate::kit::widgets::{menu_field, segmented};
use crate::kit::Style;
use cantus_export::{
    Channels, Depth, Dither, ExportSettings, Format, Mp3Mode, Rate, Scope, CEILINGS, LUFS_TARGETS,
    MP3_BITRATES, MP3_VBR, OGG_QUALITIES, RATES,
};
use egui::Ui;

/// The tails offered after the music, seconds.
const TAILS: [f32; 7] = [0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 15.0];

/// The control column's width at 100 %.
fn width(ui: &Ui) -> f32 {
    let s = Style::current();
    (ui.available_width() / s.k - super::export_dialog::CAPTION - 8.0).max(160.0)
}

fn seg(ui: &mut Ui, selected: usize, labels: &[Src<'_>], name: &str) -> Option<usize> {
    let mut i = selected;
    let wd = width(ui);
    segmented(ui, &mut i, labels, name, wd);
    (i != selected).then_some(i)
}

fn field(ui: &mut Ui, value: &str, name: &str) -> egui::Response {
    let wd = width(ui);
    menu_field(ui, value, false, false, name, wd).0
}

pub fn khz(hz: u32) -> String {
    if hz % 1000 == 0 {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{:.1} kHz", hz as f32 / 1000.0)
    }
}

/// Every row.
pub fn body(app: &mut CantusApp, ui: &mut Ui, d: &mut ExportSettings) {
    what(app, ui, d);
    format(app, ui, d);
    loudness(ui, d);
    tags(ui, d);
    ui.add_space(4.0);
    super::workspace_settings::note(ui, &summary(app, d), false);
}

fn what(app: &mut CantusApp, ui: &mut Ui, d: &mut ExportSettings) {
    band_row(ui, "export_audio.band.what");
    let scopes = [Scope::Song, Scope::Selection, Scope::Stems];
    let labels = [
        Src::Key("export_audio.scope.song"),
        Src::Key("export_audio.scope.selection"),
        Src::Key("export_audio.scope.stems"),
    ];
    let at = scopes.iter().position(|s| *s == d.scope).unwrap_or(0);
    if let Some(i) = row(ui, "export_audio.row.what", |ui| {
        seg(ui, at, &labels, "export_audio.row.what")
    }) {
        d.scope = scopes[i];
    }
    let sel = app.time_selection;
    if d.scope == Scope::Selection {
        match sel {
            Some((from, to)) => {
                let p = app.project();
                let secs = (to - from).to_seconds(p.tempo_bpm);
                let (a, b) = (p.time_signature.bbt(from).0, p.time_signature.bbt(to).0);
                let text = tf(
                    "export_audio.selection",
                    &[("from", &a), ("to", &b), ("seconds", &format!("{secs:.1}"))],
                );
                super::workspace_settings::note(ui, &text, false);
            }
            None => {
                super::workspace_settings::note(ui, &t("export_audio.no_selection"), true);
                d.scope = Scope::Song;
            }
        }
    }
    if d.scope == Scope::Stems {
        let at = usize::from(d.stems.tracks);
        let labels = [
            Src::Key("interop.stems.strips"),
            Src::Key("interop.stems.tracks"),
        ];
        if let Some(i) = row(ui, "export_audio.row.stems", |ui| {
            seg(ui, at, &labels, "export_audio.row.stems")
        }) {
            d.stems.tracks = i == 1;
        }
        let master_chain = !app.project().master.effects.is_empty();
        row(ui, "export_audio.row.include", |ui| {
            ui.vertical(|ui| {
                use super::interop::form::choice;
                if choice(ui, d.stems.master, true, &t("interop.stems.master")) {
                    d.stems.master = !d.stems.master;
                }
                if choice(
                    ui,
                    d.stems.keep_silent,
                    true,
                    &t("interop.stems.keep_silent"),
                ) {
                    d.stems.keep_silent = !d.stems.keep_silent;
                }
                if master_chain
                    && choice(ui, d.stems.through_master, true, &t("master.stems.through"))
                {
                    d.stems.through_master = !d.stems.through_master;
                }
            });
        });
    }
    let tail = format!("{:.1} s", d.tail_seconds);
    let f = row(ui, "export_audio.row.tail", |ui| {
        field(ui, &tail, "export_audio.row.tail")
    });
    let options: Vec<(u32, String)> = TAILS
        .iter()
        .map(|s| ((s * 10.0) as u32, format!("{s:.1} s")))
        .collect();
    if let Some(v) = pick(&f, &options, (d.tail_seconds * 10.0).round() as u32) {
        d.tail_seconds = v as f32 / 10.0;
    }
}

fn format(app: &mut CantusApp, ui: &mut Ui, d: &mut ExportSettings) {
    band_row(ui, "export_audio.band.format");
    let labels: Vec<Src<'_>> = Format::ALL.iter().map(|f| Src::Str(f.label())).collect();
    let at = Format::ALL.iter().position(|f| *f == d.format).unwrap_or(0);
    if let Some(i) = row(ui, "export_audio.row.format", |ui| {
        seg(ui, at, &labels, "export_audio.row.format")
    }) {
        d.format = Format::ALL[i];
    }
    match d.format {
        Format::Wav => {
            let depths = [Depth::Int16, Depth::Int24, Depth::Float32];
            let labels = [
                Src::Key("export_audio.depth.16"),
                Src::Key("export_audio.depth.24"),
                Src::Key("export_audio.depth.32f"),
            ];
            let at = depths.iter().position(|x| *x == d.depth).unwrap_or(1);
            if let Some(i) = row(ui, "export_audio.row.depth", |ui| {
                seg(ui, at, &labels, "export_audio.row.depth")
            }) {
                d.depth = depths[i];
            }
        }
        Format::Flac => {
            let labels = [
                Src::Key("export_audio.depth.16"),
                Src::Key("export_audio.depth.24"),
            ];
            let at = usize::from(d.flac_depth() == Depth::Int24);
            if let Some(i) = row(ui, "export_audio.row.depth", |ui| {
                seg(ui, at, &labels, "export_audio.row.depth")
            }) {
                d.depth = if i == 0 { Depth::Int16 } else { Depth::Int24 };
            }
        }
        Format::Mp3 => mp3_rows(ui, d),
        Format::Ogg => {
            let q = tf(
                "export_audio.ogg_q",
                &[("q", &d.ogg_quality), ("kbps", &ogg_kbps(d.ogg_quality))],
            );
            let f = row(ui, "export_audio.row.quality", |ui| {
                field(ui, &q, "export_audio.row.quality")
            });
            let options: Vec<(u8, String)> = OGG_QUALITIES
                .iter()
                .map(|q| {
                    (
                        *q,
                        tf("export_audio.ogg_q", &[("q", q), ("kbps", &ogg_kbps(*q))]),
                    )
                })
                .collect();
            if let Some(v) = pick(&f, &options, d.ogg_quality) {
                d.ogg_quality = v;
            }
        }
    }
    rate_row(app, ui, d);
    let at = usize::from(d.channels == Channels::Mono);
    let labels = [
        Src::Key("export_audio.stereo"),
        Src::Key("export_audio.mono"),
    ];
    if let Some(i) = row(ui, "export_audio.row.channels", |ui| {
        seg(ui, at, &labels, "export_audio.row.channels")
    }) {
        d.channels = if i == 0 {
            Channels::Stereo
        } else {
            Channels::Mono
        };
    }
    if d.dithers() {
        let modes = [Dither::Off, Dither::Tpdf, Dither::Shaped];
        let labels = [
            Src::Key("export_audio.dither.off"),
            Src::Key("export_audio.dither.tpdf"),
            Src::Key("export_audio.dither.shaped"),
        ];
        let at = modes.iter().position(|m| *m == d.dither).unwrap_or(1);
        if let Some(i) = row(ui, "export_audio.row.dither", |ui| {
            seg(ui, at, &labels, "export_audio.row.dither")
        }) {
            d.dither = modes[i];
        }
    }
}

/// libvorbis's nominal bitrate for 44.1 kHz stereo at a quality.
fn ogg_kbps(q: u8) -> u32 {
    [64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 500][q.min(10) as usize]
}

/// LAME's typical average for a VBR level.
fn vbr_kbps(v: u8) -> u32 {
    [245, 225, 190, 175, 165, 130, 115, 100, 85, 65][v.min(9) as usize]
}

fn mp3_rows(ui: &mut Ui, d: &mut ExportSettings) {
    let vbr = matches!(d.mp3, Mp3Mode::Vbr(_));
    let labels = [Src::Key("export_audio.cbr"), Src::Key("export_audio.vbr")];
    if let Some(i) = row(ui, "export_audio.row.mode", |ui| {
        seg(ui, usize::from(vbr), &labels, "export_audio.row.mode")
    }) {
        d.mp3 = if i == 0 {
            Mp3Mode::Cbr(320)
        } else {
            Mp3Mode::Vbr(0)
        };
    }
    let (value, options): (String, Vec<(Mp3Mode, String)>) = match d.mp3 {
        Mp3Mode::Cbr(k) => (
            format!("{k} kbps"),
            MP3_BITRATES
                .iter()
                .map(|k| (Mp3Mode::Cbr(*k), format!("{k} kbps")))
                .collect(),
        ),
        Mp3Mode::Vbr(v) => (
            tf(
                "export_audio.vbr_level",
                &[("v", &v), ("kbps", &vbr_kbps(v))],
            ),
            MP3_VBR
                .iter()
                .map(|v| {
                    let text = tf(
                        "export_audio.vbr_level",
                        &[("v", v), ("kbps", &vbr_kbps(*v))],
                    );
                    (Mp3Mode::Vbr(*v), text)
                })
                .collect(),
        ),
    };
    let f = row(ui, "export_audio.row.quality", |ui| {
        field(ui, &value, "export_audio.row.quality")
    });
    if let Some(m) = pick(&f, &options, d.mp3) {
        d.mp3 = m;
    }
}

fn rate_row(app: &mut CantusApp, ui: &mut Ui, d: &mut ExportSettings) {
    let session = app.engine.sample_rate.round() as u32;
    let label = |r: Rate| match r {
        Rate::Session => tf("export_audio.rate.session", &[("rate", &khz(session))]),
        Rate::Hz(hz) => khz(hz),
    };
    let value = label(d.rate);
    let f = row(ui, "export_audio.row.rate", |ui| {
        field(ui, &value, "export_audio.row.rate")
    });
    let mut options = vec![(Rate::Session, label(Rate::Session))];
    for hz in RATES {
        if d.format.rates().is_none_or(|ok| ok.contains(&hz)) {
            options.push((Rate::Hz(hz), khz(hz)));
        }
    }
    if let Some(r) = pick(&f, &options, d.rate) {
        d.rate = r;
    }
    let written = d.file_rate(session);
    let asked = match d.rate {
        Rate::Session => session,
        Rate::Hz(hz) => hz,
    };
    if written != asked {
        let text = tf("export_audio.rate.instead", &[("rate", &khz(written))]);
        super::workspace_settings::note(ui, &text, false);
    }
}

fn loudness(ui: &mut Ui, d: &mut ExportSettings) {
    band_row(ui, "export_audio.band.loudness");
    if d.scope == Scope::Stems {
        super::workspace_settings::note(ui, &t("export_audio.stems_level"), false);
        return;
    }
    let target = |v: Option<f32>| match v {
        None => t("export_audio.loudness.off"),
        Some(l) => format!("{l:.0} LUFS"),
    };
    let value = target(d.loudness.target_lufs);
    let f = row(ui, "export_audio.row.target", |ui| {
        field(ui, &value, "export_audio.row.target")
    });
    let mut options = vec![(None, target(None))];
    options.extend(LUFS_TARGETS.iter().map(|l| (Some(*l), target(Some(*l)))));
    if let Some(v) = pick(&f, &options, d.loudness.target_lufs) {
        d.loudness.target_lufs = v;
    }
    if d.loudness.target_lufs.is_none() {
        return;
    }
    let ceil = |c: f32| format!("{c:.1} dBTP");
    let value = ceil(d.loudness.ceiling_dbtp);
    let f = row(ui, "export_audio.row.ceiling", |ui| {
        field(ui, &value, "export_audio.row.ceiling")
    });
    let options: Vec<(i32, String)> = CEILINGS
        .iter()
        .map(|c| ((c * 10.0).round() as i32, ceil(*c)))
        .collect();
    if let Some(v) = pick(
        &f,
        &options,
        (d.loudness.ceiling_dbtp * 10.0).round() as i32,
    ) {
        d.loudness.ceiling_dbtp = v as f32 / 10.0;
    }
    row(ui, "export_audio.row.peaks", |ui| {
        if super::interop::form::choice(ui, d.loudness.limit, true, &t("export_audio.limit")) {
            d.loudness.limit = !d.loudness.limit;
        }
    });
}

fn tags(ui: &mut Ui, d: &mut ExportSettings) {
    band_row(ui, "export_audio.band.tags");
    let wd = |ui: &Ui| width(ui);
    for (key, text) in [
        ("export_audio.tag.title", &mut d.tags.title),
        ("export_audio.tag.artist", &mut d.tags.artist),
        ("export_audio.tag.album", &mut d.tags.album),
        ("export_audio.tag.year", &mut d.tags.year),
        ("export_audio.tag.comment", &mut d.tags.comment),
    ] {
        row(ui, key, |ui| {
            let x = wd(ui);
            w::text_field(ui, text, key, x);
        });
    }
}

/// One line: what will be written.
pub fn summary(app: &CantusApp, d: &ExportSettings) -> String {
    let session = app.engine.sample_rate.round() as u32;
    let quality = match d.format {
        Format::Wav => match d.depth {
            Depth::Float32 => t("export_audio.depth.32f"),
            dd => tf("export_audio.bits", &[("n", &dd.bits())]),
        },
        Format::Flac => tf("export_audio.bits", &[("n", &d.flac_depth().bits())]),
        Format::Mp3 => match d.mp3 {
            Mp3Mode::Cbr(k) => format!("{k} kbps CBR"),
            Mp3Mode::Vbr(v) => format!("VBR V{v}"),
        },
        Format::Ogg => format!("q{}", d.ogg_quality),
    };
    let ch = if d.channels == Channels::Mono {
        t("export_audio.mono")
    } else {
        t("export_audio.stereo")
    };
    let mut line = format!(
        "{} · {quality} · {} · {ch}",
        d.format.label(),
        khz(d.file_rate(session))
    );
    if let (true, Some(l)) = (d.normalises(), d.loudness.target_lufs) {
        line.push_str(&format!(
            " · {l:.0} LUFS · {:.1} dBTP",
            d.loudness.ceiling_dbtp
        ));
    }
    line
}
