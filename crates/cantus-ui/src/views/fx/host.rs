//! What an effect's editor needs from whatever runs it (ADR 0020).
//!
//! The editors were written against `CantusApp`. The same editor also runs inside a
//! plug-in window in another DAW (`cantus-plugins`), so what it asks of its host is this
//! trait, and each host answers it its own way:
//!
//! | Need | Cantus ([`DawHost`]) | A plug-in in another DAW |
//! |---|---|---|
//! | the parameters | the slot's `EffectSpec` | the plug-in's values |
//! | set one, with a gesture | a command merged into one undo step | CLAP gesture begin/value/end, VST3 `beginEdit`/`performEdit`/`endEdit` — the host's undo and automation record it |
//! | telemetry, analyser pairs | `Meters::fx`, the engine's tap | the plug-in's own audio thread, through the same lock-free rings |
//! | the key (sidechain) | any strip of the desk, chosen here | the host routes it; the editor offers the plug-in's *Sidechain* switch |
//! | tempo and transport | the song | what the host said in its last block, if anything |
//! | DAW-only actions (hits → a MIDI clip) | yes | `None`: the button is not shown |
//! | the theme | `theme::p()`, as everywhere | the same palette, applied to the plug-in window's context |

use cantus_core::{EffectSpec, StripId};
use cantus_engine::Telemetry;
use std::collections::VecDeque;

/// Where the song is, for displays that draw bar lines or stamp events in song time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transport {
    /// The song position in ticks, while it plays.
    pub playing_at: Option<i64>,
    pub ticks_per_second: f64,
    pub ticks_per_bar: i64,
}

impl Default for Transport {
    /// 120 BPM in 4/4, stopped: what a display assumes when the host says nothing.
    fn default() -> Self {
        let ppq = cantus_core::PPQ as f64;
        Transport {
            playing_at: None,
            ticks_per_second: 2.0 * ppq,
            ticks_per_bar: 4 * cantus_core::PPQ,
        }
    }
}

/// Where the detector's key comes from, as far as the host lets the editor choose it.
#[derive(Clone, Debug, PartialEq)]
pub enum KeySource {
    /// Any strip of the desk (Cantus): the one chosen, its label, and the choices.
    Strips {
        current: Option<StripId>,
        label: String,
        options: Vec<(StripId, String)>,
    },
    /// The host routes a sidechain input; the editor switches the detector to it.
    Switch(bool),
}

/// A change of key source.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum KeyTarget {
    Strip(Option<StripId>),
    Switch(bool),
}

/// What an editor asks of its host.
pub trait FxHost {
    /// The effect's parameters as they are this frame.
    fn spec(&self) -> &EffectSpec;
    /// Set one parameter. `gesture` names the drag or turn it belongs to: everything one
    /// gesture sets is one step of undo (Cantus) or one begin…end gesture (a plug-in
    /// host); `None` is a change of its own.
    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>);
    /// Several at once, as one step (a learned pair, a reset).
    fn set_many(&mut self, values: &[(&str, f32)], label_key: &str);
    /// The pointer was released: the gesture is over.
    fn end_gesture(&mut self);
    /// The unit's 64 numbers, refreshed each block.
    fn telemetry(&self) -> Telemetry;
    /// Recent (input, output) mono pairs from the analyser tap, oldest first.
    fn pairs(&self) -> &VecDeque<(f32, f32)>;
    fn transport(&self) -> Transport;
    /// The key's source, if the host lets the editor choose it.
    fn key_source(&self) -> Option<KeySource> {
        None
    }
    fn set_key_source(&mut self, _target: KeyTarget) {}
    /// A line for the status bar, where the host has one.
    fn status(&mut self, _text: String) {}
    /// Notes into a MIDI clip on a new drum track — something only a DAW can do. `notes`
    /// are (tick, velocity) at `pitch`. `None` when the host cannot; `Some(written)`.
    fn notes_to_clip(&mut self, _notes: &[(i64, u8)], _pitch: u8) -> Option<bool> {
        None
    }
    /// Whether [`FxHost::notes_to_clip`] is offered, so the editor can hide the button.
    fn makes_clips(&self) -> bool {
        false
    }
}

/// Cantus as an editor's host: the project, the engine and the desk.
pub struct DawHost<'a> {
    pub app: &'a mut crate::app::CantusApp,
    pub strip: StripId,
    pub effect: cantus_core::EffectId,
    spec: EffectSpec,
    /// What the editor's gestures are merged under (`MergeKey`): its kind's name.
    tag: &'static str,
}

impl<'a> DawHost<'a> {
    /// The host for one effect, its spec read from the project (a new spec of the
    /// kind if the effect is gone).
    pub fn new(
        app: &'a mut crate::app::CantusApp,
        strip: StripId,
        effect: cantus_core::EffectId,
        tag: &'static str,
    ) -> DawHost<'a> {
        let spec = app
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
            .unwrap_or_else(|| EffectSpec::new(cantus_core::EffectKind::Gate));
        DawHost::with_spec(app, strip, effect, spec, tag)
    }

    /// The host for one effect whose spec the caller already has.
    pub fn with_spec(
        app: &'a mut crate::app::CantusApp,
        strip: StripId,
        effect: cantus_core::EffectId,
        spec: EffectSpec,
        tag: &'static str,
    ) -> DawHost<'a> {
        DawHost {
            app,
            strip,
            effect,
            spec,
            tag,
        }
    }
}

impl FxHost for DawHost<'_> {
    fn spec(&self) -> &EffectSpec {
        &self.spec
    }

    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>) {
        let cmd = cantus_core::Command::SetEffectParam {
            strip: self.strip,
            effect: self.effect,
            key: key.to_string(),
            value,
        };
        match gesture {
            Some(g) => {
                self.app.exec_merging(
                    cmd,
                    cantus_core::MergeKey::of(&(self.tag, self.effect.0, g)),
                );
            }
            None => {
                self.app.exec(cmd);
            }
        }
    }

    fn set_many(&mut self, values: &[(&str, f32)], label_key: &str) {
        let commands = values
            .iter()
            .map(|(k, v)| cantus_core::Command::SetEffectParam {
                strip: self.strip,
                effect: self.effect,
                key: k.to_string(),
                value: *v,
            })
            .collect();
        self.app.exec(cantus_core::Command::Batch {
            label_key: label_key.into(),
            commands,
        });
    }

    fn end_gesture(&mut self) {
        self.app.end_gesture();
    }

    fn telemetry(&self) -> Telemetry {
        self.app.fx_telemetry()
    }

    fn pairs(&self) -> &VecDeque<(f32, f32)> {
        &self.app.fx.pairs
    }

    fn transport(&self) -> Transport {
        let project = self.app.project();
        Transport {
            playing_at: self
                .app
                .engine
                .is_playing()
                // The engine's position, not the heard one: an editor times what its unit
                // sees at its input (ADR 0022).
                .then(|| self.app.engine.engine_position().0),
            ticks_per_second: project.tempo_bpm / 60.0 * cantus_core::PPQ as f64,
            ticks_per_bar: project.time_signature.ticks_per_bar().0,
        }
    }

    fn key_source(&self) -> Option<KeySource> {
        let project = self.app.project();
        let current = project
            .strip(self.strip)
            .and_then(|s| s.effect(self.effect))
            .and_then(|e| e.sidechain);
        let label = |id: StripId| {
            self.app
                .capture_label(cantus_engine::CaptureTarget::Strip(id))
        };
        let options = project
            .strips
            .iter()
            .map(|x| x.id)
            .filter(|id| *id != self.strip && !project.routing_would_cycle(*id, Some(self.strip)))
            .map(|id| (id, label(id)))
            .collect();
        Some(KeySource::Strips {
            current,
            label: match current {
                Some(k) => label(k),
                None => crate::i18n::t("gate.key_own"),
            },
            options,
        })
    }

    fn set_key_source(&mut self, target: KeyTarget) {
        if let KeyTarget::Strip(source) = target {
            self.app.exec(cantus_core::Command::SetEffectSidechain {
                strip: self.strip,
                effect: self.effect,
                source,
            });
        }
    }

    fn status(&mut self, text: String) {
        self.app.set_status(text);
    }

    fn notes_to_clip(&mut self, notes: &[(i64, u8)], pitch: u8) -> Option<bool> {
        Some(self.app.gate_notes_to_clip(self.strip, notes, pitch))
    }

    fn makes_clips(&self) -> bool {
        true
    }
}
