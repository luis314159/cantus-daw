//! Chorda's own tuning, from inside Chorda (round 42; the owner, 2026-11-22: "Can Chorda by
//! itself change the tuning of its strings and use presets other than equal temperament? I
//! want to change only Chorda and keep the whole song as it is"). Chorda's Edit view gains a
//! *Temperament* card: **Plays in** the project's tuning or its own (the track's
//! `TrackTuning`, `SetTrackTuning`), **the tuning presets** of
//! `research/instruments/microtuning.md` (Chorda, item 7) — equal ones, the historical
//! temperaments rotated to any key, just intonation, character ones, and the user's own
//! Scala files — the key, the purest and roughest thirds and a small circle of fifths, the
//! cadence; and the **A4 rule**: Chorda's own A4 knob is the diapason only while the track
//! plays no tuning; with the project's it shows the project's, greyed; with its own it edits
//! that tuning's A4. A preset is one undo step: the track's own tuning and the Chorda
//! parameters it needs (stretch, unisons) in one `Batch`. r42:tuning-face

use super::{words, Target};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{menu_field, menu_list, segmented};
use crate::kit::{with_brush, Style};
use cantus_core::tuning::{
    ScalaScale, ScaleDegree, Temperament, TrackTuning, Tuning, TuningSystem,
};
use cantus_core::{Command, NativeSpec, TrackId};
use egui::{Align2, Id, Pos2, Rect, Ui, Vec2};

/// The card's body height at 100 % (its controls' rows).
pub const BODY: f32 = 176.0;

/// What a preset's tuning is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Kind {
    /// Equal temperament at this A4.
    Equal(f64),
    /// A temperament rotated to the card's key, at this A4.
    Temper(Temperament, f64),
    /// Equal temperament with each key a few cents off, seeded (an untouched old piano).
    SlightlyOut,
}

/// A Chorda tuning preset: its words, its group, its tuning, and the Chorda parameters it
/// sets (stretch, unisons); the rest stay as they are.
#[derive(Copy, Clone, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    pub params: &'static [(&'static str, f32)],
}

const fn p(
    name: &'static str,
    group: &'static str,
    kind: Kind,
    params: &'static [(&'static str, f32)],
) -> Preset {
    Preset {
        name,
        group,
        kind,
        params,
    }
}

const EQ: &str = "tuning.chorda.group.equal";
const HIST: &str = "tuning.chorda.group.historical";
const JUST: &str = "tuning.chorda.group.just";
const CHAR: &str = "tuning.chorda.group.character";

/// Every built-in preset, in the list's order (the design note's item 7).
pub const PRESETS: [Preset; 24] = [
    p(
        "tuning.chorda.p.concert",
        EQ,
        Kind::Equal(440.0),
        &[("stretch", 1.0), ("unison", 1.0)],
    ),
    p(
        "tuning.chorda.p.studio",
        EQ,
        Kind::Equal(440.0),
        &[("stretch", 0.85), ("unison", 0.4)],
    ),
    p(
        "tuning.chorda.p.bright",
        EQ,
        Kind::Equal(440.0),
        &[("stretch", 1.5), ("unison", 1.6)],
    ),
    p(
        "tuning.temper.pythagorean",
        HIST,
        Kind::Temper(Temperament::Pythagorean, 440.0),
        &[],
    ),
    p(
        "tuning.temper.meantone_quarter",
        HIST,
        Kind::Temper(Temperament::MeantoneQuarter, 440.0),
        &[],
    ),
    p(
        "tuning.temper.meantone_sixth",
        HIST,
        Kind::Temper(Temperament::MeantoneSixth, 440.0),
        &[],
    ),
    p(
        "tuning.temper.werckmeister_iii",
        HIST,
        Kind::Temper(Temperament::WerckmeisterIii, 440.0),
        &[],
    ),
    p(
        "tuning.temper.werckmeister_iv",
        HIST,
        Kind::Temper(Temperament::WerckmeisterIv, 440.0),
        &[],
    ),
    p(
        "tuning.temper.kirnberger_ii",
        HIST,
        Kind::Temper(Temperament::KirnbergerIi, 440.0),
        &[],
    ),
    p(
        "tuning.temper.kirnberger_iii",
        HIST,
        Kind::Temper(Temperament::KirnbergerIii, 440.0),
        &[],
    ),
    p(
        "tuning.temper.vallotti",
        HIST,
        Kind::Temper(Temperament::Vallotti, 440.0),
        &[],
    ),
    p(
        "tuning.temper.young_1799",
        HIST,
        Kind::Temper(Temperament::Young1799, 440.0),
        &[],
    ),
    p(
        "tuning.temper.kellner",
        HIST,
        Kind::Temper(Temperament::Kellner, 440.0),
        &[],
    ),
    p(
        "tuning.temper.neidhardt",
        HIST,
        Kind::Temper(Temperament::Neidhardt, 440.0),
        &[],
    ),
    p(
        "tuning.temper.bach_lehman",
        HIST,
        Kind::Temper(Temperament::BachLehman, 440.0),
        &[],
    ),
    p(
        "tuning.temper.just_major",
        JUST,
        Kind::Temper(Temperament::JustMajor, 440.0),
        &[],
    ),
    p(
        "tuning.temper.just_minor",
        JUST,
        Kind::Temper(Temperament::JustMinor, 440.0),
        &[],
    ),
    p(
        "tuning.temper.barbershop_7",
        JUST,
        Kind::Temper(Temperament::Barbershop7, 440.0),
        &[],
    ),
    p(
        "tuning.chorda.p.honky",
        CHAR,
        Kind::Equal(440.0),
        &[("stretch", 1.0), ("unison", 7.0)],
    ),
    p(
        "tuning.chorda.p.parlour",
        CHAR,
        Kind::Equal(440.0),
        &[("stretch", 0.6), ("unison", 3.5)],
    ),
    p(
        "tuning.chorda.p.saloon",
        CHAR,
        Kind::SlightlyOut,
        &[("stretch", 0.8), ("unison", 5.0)],
    ),
    p(
        "tuning.chorda.p.slightly_out",
        CHAR,
        Kind::SlightlyOut,
        &[("unison", 2.0)],
    ),
    p(
        "tuning.chorda.p.baroque",
        CHAR,
        Kind::Temper(Temperament::WerckmeisterIii, 415.0),
        &[],
    ),
    p("tuning.chorda.p.verdi", CHAR, Kind::Equal(432.0), &[]),
];

/// "Slightly out": each of the twelve a few cents off equal, the same every time (a seeded
/// sequence), as a Scala scale (degree 0 on C4, A4 at the diapason — Scala's template).
pub fn slightly_out() -> ScalaScale {
    let mut seed: u32 = 0x5EED_C0DA;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f64 / u32::MAX as f64) * 2.0 - 1.0
    };
    let mut degrees: Vec<ScaleDegree> = (1..12)
        .map(|i| ScaleDegree::Cents(100.0 * i as f64 + (4.0 * next() * 1000.0).round() / 1000.0))
        .collect();
    degrees.push(ScaleDegree::Ratio(2, 1));
    ScalaScale {
        description: t("tuning.chorda.p.slightly_out"),
        degrees,
    }
}

/// The tuning preset `p` makes in `key`.
pub fn tuning_of(p: &Preset, key: u8) -> Tuning {
    match p.kind {
        Kind::Equal(a4) => Tuning {
            a4_hz: a4,
            ..Tuning::default()
        },
        Kind::Temper(id, a4) => Tuning {
            a4_hz: a4,
            system: TuningSystem::Temperament {
                id,
                key: key % 12,
                follow_song_key: false,
            },
            map: None,
        },
        Kind::SlightlyOut => Tuning {
            a4_hz: 440.0,
            system: TuningSystem::Scale(slightly_out()),
            map: None,
        },
    }
}

/// The one undo step a preset is: the track's own tuning, and Chorda's parameters it sets
/// that differ from the track's.
pub fn preset_command(spec: &NativeSpec, track: TrackId, p: &Preset, key: u8) -> Command {
    let mut commands = vec![Command::SetTrackTuning {
        track,
        tuning: TrackTuning::Own {
            tuning: tuning_of(p, key),
        },
    }];
    for (k, v) in p.params {
        if spec.get(k) != *v {
            commands.push(Command::SetInstrumentValue {
                track,
                key: (*k).into(),
                value: *v,
            });
        }
    }
    Command::Batch {
        label_key: "cmd.set_track_tuning".into(),
        commands,
    }
}

/// Which preset the track plays now (its own tuning and the parameters a preset sets).
pub fn current(app: &CantusApp, track: TrackId, spec: &NativeSpec) -> Option<&'static Preset> {
    let tr = app.project().track(track)?;
    let TrackTuning::Own { tuning } = &tr.tuning else {
        return None;
    };
    let key = match tuning.system {
        TuningSystem::Temperament { key, .. } => key,
        _ => 0,
    };
    PRESETS
        .iter()
        .find(|p| tuning_of(p, key) == *tuning && p.params.iter().all(|(k, v)| spec.get(k) == *v))
}

/// How Chorda's A4 knob reads (ADR 0155; the owner's rule of 2026-11-22: one diapason).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum A4 {
    /// The track plays no tuning: the knob is the diapason, as in every older song.
    Knob,
    /// The project's tuning: its diapason, shown greyed.
    Project(f64),
    /// The track's own tuning: the knob edits its A4.
    Own(f64),
}

/// [`A4`] for `track`. r42:tuning-face interim — at the merge with `tuning` this asks
/// `cantus_core::inst::chorda::follows_tuning(&table)` (the table not 12-TET A440 to the
/// bit) and `effective_a4(spec, &resolved)`; the same decision, made here until then.
pub fn a4_of(app: &CantusApp, track: TrackId) -> A4 {
    let p = app.project();
    let Some(tr) = p.track(track) else {
        return A4::Knob;
    };
    let resolved = super::effective(&tr.tuning.resolve(&p.tuning), p.key);
    if super::table_of(app, Target::Track(track)).is_equal() {
        return A4::Knob;
    }
    match tr.tuning {
        TrackTuning::Own { .. } => A4::Own(resolved.a4_hz),
        _ => A4::Project(resolved.a4_hz),
    }
}

/// The key the card's presets rotate to: the track's temperament's, else the song's, else C.
pub fn key_of(app: &CantusApp, track: TrackId) -> u8 {
    let p = app.project();
    let own = p.track(track).and_then(|t| match &t.tuning {
        TrackTuning::Own { tuning } => match tuning.system {
            TuningSystem::Temperament { key, .. } => Some(key),
            _ => None,
        },
        _ => None,
    });
    own.or(app.tuning.chorda_key)
        .or(p.key.map(|k| k.root))
        .unwrap_or(0)
        % 12
}

/// The user's own tunings: the Scala files of the data folder's `Tuning`, by name.
pub fn user_dir() -> std::path::PathBuf {
    crate::app::data_dir().join("Tuning")
}

/// The user's `.scl` files, sorted by name (never the folder's order).
pub fn user_files() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = std::fs::read_dir(user_dir())
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("scl"))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// *Save as mine*: the track's tuning written as a Scala file of the user's (named after it,
/// numbered when the name is taken). Returns the file.
pub fn save_mine(app: &CantusApp, track: TrackId) -> Result<std::path::PathBuf, String> {
    let dir = user_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tu = super::tuning_of(app, Target::Track(track));
    let base: String = words::summary_of(&tu, app.project().key)
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let base = if base.is_empty() {
        t("tuning.chorda.mine")
    } else {
        base
    };
    let mut path = dir.join(format!("{base}.scl"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} {n}.scl"));
        n += 1;
    }
    super::io::export(app, Target::Track(track), &path)?;
    Ok(path)
}

/// A control row: its caption (a fixed column) and the control's rect.
fn row_rects(r: Rect, y: f32, s: &Style) -> (Rect, Rect) {
    let k = s.k;
    let cap = Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(96.0 * k, 24.0 * k));
    let ctl = Rect::from_min_max(
        Pos2::new(cap.right() + 6.0 * k, y),
        Pos2::new(r.right(), y + 24.0 * k),
    );
    (cap, ctl)
}

fn caption(ui: &Ui, r: Rect, key: &str, s: &Style) {
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            r,
            &Text::key(key, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER),
            s.tk(),
        );
    });
}

/// A kit control placed in `r` (the kit's widgets allocate in a layout).
fn place<R>(ui: &mut Ui, r: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    add(&mut child)
}

/// What a click in the preset list chose.
#[derive(Clone, Debug)]
enum Pick {
    Preset(usize),
    User(std::path::PathBuf),
}

/// The card's body: the controls at the left, the circle in the middle, the keys that sound
/// purest at the right. Published to the tests as `chorda.tuning.<name>`.
pub fn body(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    r: Rect,
    s: &Style,
) {
    let k = s.k;
    let left = Rect::from_min_max(r.min, Pos2::new(r.left() + r.width() * 0.5, r.bottom()));
    let mid = Rect::from_min_max(
        Pos2::new(left.right() + 12.0 * k, r.top()),
        Pos2::new(left.right() + 12.0 * k + r.height(), r.bottom()),
    );
    let right = Rect::from_min_max(Pos2::new(mid.right() + 12.0 * k, r.top()), r.max);
    let own = app
        .project()
        .track(track)
        .is_some_and(|t| matches!(t.tuning, TrackTuning::Own { .. }));
    let mut y = left.top();
    let step = 30.0 * k;

    // Plays in: the project's tuning, or its own.
    let (cap, ctl) = row_rects(left, y, s);
    caption(ui, cap, "tuning.chorda.plays_in", s);
    let labels = [
        Src::Key("tuning.chorda.plays_project"),
        Src::Key("tuning.chorda.plays_own"),
    ];
    let mut at = usize::from(own);
    let resp = place(ui, ctl, |ui| {
        segmented(
            ui,
            &mut at,
            &labels,
            &t("tuning.chorda.plays_in"),
            ctl.width() / k,
        )
    });
    publish(ui, "plays", resp.rect);
    if resp.changed() {
        let mode = if at == 1 {
            TrackTuning::Own {
                tuning: super::tuning_of(app, Target::Track(track)),
            }
        } else {
            TrackTuning::Follow
        };
        super::set_track_mode(app, track, mode);
    }
    y += step;

    // The preset: its name, or the project's, or the track's own words.
    let (cap, ctl) = row_rects(left, y, s);
    caption(ui, cap, "tuning.chorda.preset", s);
    let key = key_of(app, track);
    let now = current(app, track, spec);
    let shown = match (now, own) {
        (Some(p), _) => match p.kind {
            Kind::Temper(..) => tf(
                "tuning.summary.temper",
                &[("name", &t(p.name)), ("key", &words::key_name(key))],
            ),
            _ => t(p.name),
        },
        (None, true) => words::track_mode_text(
            &app.project()
                .track(track)
                .map(|t| t.tuning.clone())
                .unwrap_or_default(),
            app.project().key,
        ),
        (None, false) => words::project_summary(app.project())
            .map_or_else(|| t("tuning.chorda.project_equal_short"), |s| s),
    };
    let (f, _) = place(ui, ctl, |ui| {
        menu_field(
            ui,
            &shown,
            false,
            false,
            &t("tuning.chorda.preset"),
            ctl.width() / k,
        )
    });
    publish(ui, "preset", f.rect);
    if let Some(pick) = preset_list(&f, now) {
        match pick {
            Pick::Preset(i) => {
                let cmd = preset_command(spec, track, &PRESETS[i], key);
                app.exec(cmd);
            }
            Pick::User(path) => {
                app.tuning.last = Some(super::io::import(app, Target::Track(track), &path, None));
            }
        }
    }
    y += step;

    // The key a temperament is rotated to.
    let (cap, ctl) = row_rects(left, y, s);
    caption(ui, cap, "tuning.row.key", s);
    let (kf, clicks) = place(ui, ctl, |ui| {
        menu_field(
            ui,
            words::key_name(key),
            true,
            false,
            &t("tuning.row.key"),
            ctl.width() / k,
        )
    });
    publish(ui, "key", kf.rect);
    let options: Vec<(u8, String)> = (0..12u8)
        .map(|n| (n, words::key_name(n).to_string()))
        .collect();
    let mut next = key_list(&kf, &options, key);
    if clicks.prev {
        next = Some((key + 11) % 12);
    }
    if clicks.next {
        next = Some((key + 1) % 12);
    }
    if let Some(n) = next {
        app.tuning.chorda_key = Some(n);
        let tu = super::tuning_of(app, Target::Track(track));
        if let (true, TuningSystem::Temperament { id, .. }) = (own, &tu.system) {
            let system = TuningSystem::Temperament {
                id: *id,
                key: n,
                follow_song_key: false,
            };
            super::set_merging(
                app,
                Target::Track(track),
                Tuning {
                    system,
                    ..tu.clone()
                },
                "chorda-key",
            );
        }
    }
    y += step;

    // Files and the cadence.
    let (cap, ctl) = row_rects(left, y, s);
    caption(ui, cap, "tuning.chorda.yours", s);
    place(ui, ctl, |ui| {
        let b =
            crate::views::audio_parts::button(ui, &t("tuning.chorda.save_mine"), false, true, 0.0);
        publish(ui, "save", b.rect);
        if b.clicked() {
            app.tuning.last = Some(save_mine(app, track).map(|p| {
                tf(
                    "tuning.io.exported",
                    &[(
                        "files",
                        &p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )],
                )
            }));
        }
        ui.add_space(6.0 * k);
        let b = crate::views::audio_parts::button(ui, &t("tuning.import"), false, true, 0.0);
        publish(ui, "import", b.rect);
        if b.clicked() {
            super::io::import_dialog(app, Target::Track(track));
        }
        ui.add_space(6.0 * k);
        let b = crate::views::audio_parts::button(ui, &t("tuning.export"), false, true, 0.0);
        publish(ui, "export", b.rect);
        if b.clicked() {
            super::io::export_dialog(app, Target::Track(track));
        }
    });
    y += step;
    let (cap, ctl) = row_rects(left, y, s);
    caption(ui, cap, "tuning.row.audition", s);
    let playing = app.tuning.audition.is_some();
    let (tonic, minor) = super::audition_key(app, Target::Track(track));
    let label = if playing {
        t("tuning.audition.stop")
    } else {
        let mode = if minor {
            "tuning.audition.minor"
        } else {
            "tuning.audition.major"
        };
        tf(
            "tuning.audition.play",
            &[("key", &words::key_name(tonic)), ("mode", &t(mode))],
        )
    };
    place(ui, ctl, |ui| {
        let b = crate::views::audio_parts::button(ui, &label, true, true, 0.0);
        publish(ui, "audition", b.rect);
        if b.clicked() {
            if playing {
                super::stop_audition(app);
            } else {
                let now = ui.input(|i| i.time);
                super::start_audition(app, Target::Track(track), now);
            }
        }
    });
    y += step;
    if let Some(last) = &app.tuning.last {
        let (words, warn) = match last {
            Ok(w) => (w.as_str(), false),
            Err(w) => (w.as_str(), true),
        };
        let line =
            Rect::from_min_size(Pos2::new(left.left(), y), Vec2::new(left.width(), 18.0 * k));
        with_brush(ui.painter(), |b| {
            let ink = if warn { s.t.danger } else { s.t.text_dim };
            let _ = text::paint(
                b,
                line,
                &Text::new(words, Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
    }

    // The circle and the keys that sound purest, of what the piano plays now.
    let table = super::table_of(app, Target::Track(track));
    if super::circle::twelve_to_the_octave(&table) {
        let tonic = key_of(app, track);
        let centres = with_brush(ui.painter(), |b| {
            super::circle::paint(b, mid, s, &table, tonic)
        });
        for (pc, c) in centres.iter().enumerate() {
            let hit = Rect::from_center_size(*c, Vec2::splat(22.0 * k));
            if ui
                .interact(
                    hit,
                    Id::new(("chorda-tuning-note", track.0, pc)),
                    egui::Sense::hover(),
                )
                .hovered()
            {
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &super::circle::hover_words(&table, pc as u8),
                    "",
                    crate::kit::hint::Caps::NONE,
                );
            }
        }
        purest(ui, right, &table, s);
    }
}

/// The keys whose major thirds beat least and most, in words.
fn purest(ui: &Ui, r: Rect, table: &cantus_core::tuning::TuningTable, s: &Style) {
    let k = s.k;
    let mut order: Vec<u8> = (0..12).collect();
    order.sort_by(|a, b| {
        super::circle::third_beats(table, *a)
            .partial_cmp(&super::circle::third_beats(table, *b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });
    let names = |v: &[u8]| {
        v.iter()
            .map(|p| words::key_name(*p))
            .collect::<Vec<_>>()
            .join(" \u{b7} ")
    };
    let lines = [
        ("tuning.chorda.purest", names(&order[..4]), 0.0),
        ("tuning.chorda.roughest", names(&order[8..]), 1.0),
    ];
    let mut y = r.top() + 8.0 * k;
    for (key, list, x) in lines {
        let head = Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(r.width(), 18.0 * k));
        let body = Rect::from_min_size(
            Pos2::new(r.left() + 14.0 * k, y + 18.0 * k),
            Vec2::new(r.width() - 14.0 * k, 20.0 * k),
        );
        with_brush(ui.painter(), |b| {
            b.circle(
                Pos2::new(r.left() + 4.0 * k, body.center().y),
                4.0 * k,
                crate::kit::Fill::Solid(super::circle::beat_colour(s, x)),
            );
            let _ = text::paint(
                b,
                head,
                &Text::key(key, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER),
                s.tk(),
            );
            let _ = text::paint(
                b,
                body,
                &Text::new(&list, Role::Control, s.t.text).align(Align2::LEFT_CENTER),
                s.tk(),
            );
        });
        y += 48.0 * k;
    }
}

/// The preset list: the built-in ones by group, then the user's files.
fn preset_list(field: &egui::Response, now: Option<&'static Preset>) -> Option<Pick> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            let files = user_files();
            let names: Vec<String> = files
                .iter()
                .map(|p| {
                    p.file_stem()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                })
                .collect();
            let mut items: Vec<Item<'_>> = Vec::new();
            let mut picks: Vec<Option<Pick>> = Vec::new();
            let mut group = "";
            for (i, p) in PRESETS.iter().enumerate() {
                if p.group != group {
                    group = p.group;
                    items.push(Item::Band(Src::Key(p.group)));
                    picks.push(None);
                }
                items.push(Item::Action {
                    label: Src::Key(p.name),
                    check: Check::Radio(now.is_some_and(|n| std::ptr::eq(n, p))),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                });
                picks.push(Some(Pick::Preset(i)));
            }
            items.push(Item::Band(Src::Key("tuning.chorda.group.yours")));
            picks.push(None);
            if files.is_empty() {
                items.push(Item::Action {
                    label: Src::Key("tuning.chorda.none_yet"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: false,
                });
                picks.push(None);
            }
            for (path, name) in files.iter().zip(&names) {
                items.push(Item::Action {
                    label: Src::Str(name),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                });
                picks.push(Some(Pick::User(path.clone())));
            }
            let width = field.rect.width().max(220.0);
            let (_, clicks) = menu_list(ui, &items, width);
            if let Some(i) = clicks.clicked {
                chosen = picks[i].clone();
                ui.close();
            }
        });
    chosen
}

/// The key list under the key field.
fn key_list(field: &egui::Response, options: &[(u8, String)], current: u8) -> Option<u8> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            let items: Vec<Item<'_>> = options
                .iter()
                .map(|(v, l)| Item::Action {
                    label: Src::Str(l),
                    check: Check::Radio(*v == current),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            let (_, clicks) = menu_list(ui, &items, field.rect.width().max(120.0));
            if let Some(i) = clicks.clicked {
                chosen = Some(options[i].0);
                ui.close();
            }
        });
    chosen
}

/// Where the card drew `name` this frame (the tests click through it).
fn publish(ui: &Ui, name: &str, r: Rect) {
    let id = Id::new("cantus-chorda-tuning-geo");
    let name = format!("chorda.tuning.{name}");
    ui.ctx().data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<(String, Rect)>>(id);
        v.retain(|(n, _)| *n != name);
        v.push((name, r));
    });
}

/// Where the card drew `name` (`plays`, `preset`, `key`, `save`, `import`, `export`,
/// `audition`) in its last frame.
pub fn geo(ctx: &egui::Context, name: &str) -> Option<Rect> {
    let id = Id::new("cantus-chorda-tuning-geo");
    let name = format!("chorda.tuning.{name}");
    ctx.data(|d| d.get_temp::<Vec<(String, Rect)>>(id))
        .unwrap_or_default()
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, r)| r)
}

/// Chorda's A4 cell when it is not its own knob: the project's diapason greyed, or the
/// track's own tuning's A4 with ‹ › a hertz.
pub fn a4_cell(app: &mut CantusApp, ui: &mut Ui, track: TrackId, r: Rect, a4: A4, s: &Style) {
    let k = s.k;
    let head = Rect::from_min_size(
        r.min + Vec2::new(0.0, 8.0 * k),
        Vec2::new(r.width(), 16.0 * k),
    );
    let value = Rect::from_center_size(
        Pos2::new(r.center().x, head.bottom() + 22.0 * k),
        Vec2::new((r.width() - 12.0 * k).min(120.0 * k), 24.0 * k),
    );
    let foot = Rect::from_min_size(
        Pos2::new(r.left(), value.bottom() + 6.0 * k),
        Vec2::new(r.width(), 16.0 * k),
    );
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            head,
            &Text::key("chorda.a4", Role::Label, s.t.text_dim).align(Align2::CENTER_CENTER),
            s.tk(),
        );
    });
    match a4 {
        A4::Knob => {}
        A4::Project(hz) => {
            let words = tf("tuning.hz", &[("hz", &words::hz_text(hz))]);
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    value,
                    &Text::new(&words, Role::Value, s.t.disabled).align(Align2::CENTER_CENTER),
                    s.tk(),
                );
                let _ = text::paint(
                    b,
                    foot,
                    &Text::key("tuning.chorda.a4_project", Role::Label, s.t.text_dim)
                        .align(Align2::CENTER_CENTER),
                    s.tk(),
                );
            });
            publish(ui, "a4", value);
        }
        A4::Own(hz) => {
            let words = tf("tuning.hz", &[("hz", &words::hz_text(hz))]);
            let (resp, clicks) = place(ui, value, |ui| {
                menu_field(ui, &words, true, false, &t("chorda.a4"), value.width() / k)
            });
            publish(ui, "a4", resp.rect);
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    foot,
                    &Text::key("tuning.chorda.a4_own", Role::Label, s.t.text_dim)
                        .align(Align2::CENTER_CENTER),
                    s.tk(),
                );
            });
            let d = if clicks.prev {
                -1.0
            } else if clicks.next {
                1.0
            } else {
                0.0
            };
            if d != 0.0 {
                let mut tu = super::tuning_of(app, Target::Track(track));
                tu.a4_hz = (tu.a4_hz + d).clamp(
                    cantus_core::tuning::A4_MIN_HZ,
                    cantus_core::tuning::A4_MAX_HZ,
                );
                super::set_merging(app, Target::Track(track), tu, "chorda-a4");
            }
        }
    }
}
