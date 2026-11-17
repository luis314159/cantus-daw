//! A board's pedal as an effect of its own (ADR 0134 §5): [`PedalHost`] wraps the board's
//! [`FxHost`] so the unit's own full editor (`Editor::of(kind)`) runs on one pedal — every
//! key it reads or writes is the pedal's (`p{id}.{key}` on the board), its undo the board's,
//! its automation and modulation the board's lanes on those keys, a Lapsus pattern the
//! pedal's bank, a Cedo shape the pedal's (fxb:cedo).

use super::host::{FxHost, Transport};
use cantus_core::fx::tabula::{pedal_key, PedalId};
use cantus_core::EffectSpec;
use cantus_engine::Telemetry;
use std::collections::VecDeque;

/// One pedal of the board `inner` hosts, as a host of its own.
pub struct PedalHost<'a> {
    inner: &'a mut dyn FxHost,
    pedal: PedalId,
    /// The pedal's spec as it is this frame (the board's copy, kept in step with each set).
    spec: EffectSpec,
    /// What the pedal's own editor draws (`Meters::fx_pedal`, or the screenshots' feed).
    telemetry: Telemetry,
}

impl<'a> PedalHost<'a> {
    /// `None` when the board holds no such pedal.
    pub fn new(inner: &'a mut dyn FxHost, pedal: PedalId, telemetry: Telemetry) -> Option<Self> {
        let spec = inner
            .spec()
            .pedals()
            .iter()
            .find(|p| p.id == pedal)?
            .spec
            .clone();
        Some(PedalHost {
            inner,
            pedal,
            spec,
            telemetry,
        })
    }

    pub fn pedal(&self) -> PedalId {
        self.pedal
    }

    fn key(&self, key: &str) -> String {
        pedal_key(self.pedal, key)
    }
}

impl FxHost for PedalHost<'_> {
    fn spec(&self) -> &EffectSpec {
        &self.spec
    }

    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>) {
        self.spec.set(key, value);
        let k = self.key(key);
        let g = gesture.map(|g| self.key(g));
        self.inner.set(&k, value, g.as_deref());
    }

    fn set_many(&mut self, values: &[(&str, f32)], label_key: &str) {
        for (k, v) in values {
            self.spec.set(k, *v);
        }
        let keys: Vec<String> = values.iter().map(|(k, _)| self.key(k)).collect();
        let pairs: Vec<(&str, f32)> = keys
            .iter()
            .zip(values)
            .map(|(k, (_, v))| (k.as_str(), *v))
            .collect();
        self.inner.set_many(&pairs, label_key);
    }

    fn end_gesture(&mut self) {
        self.inner.end_gesture();
    }

    fn telemetry(&self) -> Telemetry {
        self.telemetry
    }

    fn pairs(&self) -> &VecDeque<(f32, f32)> {
        self.inner.pairs()
    }

    fn transport(&self) -> Transport {
        self.inner.transport()
    }

    fn can_automate(&self) -> bool {
        self.inner.can_automate()
    }

    fn automated(&self, key: &str) -> bool {
        self.inner.automated(&self.key(key))
    }

    fn automate(&mut self, key: &str) {
        let k = self.key(key);
        self.inner.automate(&k);
    }

    fn modulation(
        &self,
        key: &str,
        base: f32,
        s: &crate::kit::Style,
        rings: &mut [(f32, egui::Color32); crate::views::modulator::MAX_RINGS],
    ) -> Option<(crate::kit::automate::ModOffer, usize)> {
        self.inner.modulation(&self.key(key), base, s, rings)
    }

    fn modulate(&mut self, key: &str, choice: crate::kit::automate::Choice, rect: egui::Rect) {
        let k = self.key(key);
        self.inner.modulate(&k, choice, rect);
    }

    fn learn_idle(&self, key: &str) -> bool {
        self.inner.learn_idle(&self.key(key))
    }

    fn set_learn_range(&mut self, key: &str, from: f32, to: f32) {
        let k = self.key(key);
        self.inner.set_learn_range(&k, from, to);
    }

    fn mapping_reaches(&self, key: &str, value: f32) -> bool {
        self.inner.mapping_reaches(&self.key(key), value)
    }

    fn apply(&mut self, edits: &[(&str, Option<f32>)], label_key: &str, gesture: Option<u64>) {
        for (k, v) in edits {
            match v {
                Some(v) => {
                    self.spec.set(k, *v);
                }
                None => {
                    self.spec.clear(k);
                }
            }
        }
        let keys: Vec<String> = edits.iter().map(|(k, _)| self.key(k)).collect();
        let pairs: Vec<(&str, Option<f32>)> = keys
            .iter()
            .zip(edits)
            .map(|(k, (_, v))| (k.as_str(), *v))
            .collect();
        self.inner.apply(&pairs, label_key, gesture);
    }

    fn set_lapsus_pattern(
        &mut self,
        lane: cantus_core::fx::lapsus::Lane,
        slot: u8,
        pattern: Option<cantus_core::fx::lapsus::Pattern>,
        gesture: Option<u64>,
    ) {
        let _ = gesture;
        let pedal = Some(self.pedal);
        if let Some((app, strip, effect)) = self.inner.daw() {
            let cmd = cantus_core::Command::SetLapsusPattern {
                strip,
                effect,
                lane,
                slot,
                pattern: pattern.map(Box::new),
                pedal,
            };
            app.exec(cmd);
        }
    }

    // fxb:cedo — a Cedo pedal's drawn shape: the pedal's own, one step a gesture in Cantus;
    // in the exported board, through the host's board (one step there).
    fn set_cedo_shape(
        &mut self,
        shape: Option<cantus_core::fx::cedo::Shape>,
        gesture: Option<u64>,
    ) {
        let pedal = self.pedal;
        let shape = shape.map(Box::new);
        self.spec.cedo = shape.clone();
        if let Some((app, strip, effect)) = self.inner.daw() {
            let cmd = cantus_core::Command::SetCedoShape {
                strip,
                effect,
                shape,
                pedal: Some(pedal),
            };
            match gesture {
                Some(g) => {
                    app.exec_merging(
                        cmd,
                        cantus_core::MergeKey::of(&("tabula-cedo", effect.0, pedal.0, g)),
                    );
                }
                None => {
                    app.exec(cmd);
                }
            }
            return;
        }
        self.inner
            .board_commands("cmd.cedo_shape", &|strip, effect| {
                vec![cantus_core::Command::SetCedoShape {
                    strip,
                    effect,
                    shape: shape.clone(),
                    pedal: Some(pedal),
                }]
            });
    }

    fn sample_rate(&self) -> f32 {
        self.inner.sample_rate()
    }

    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    fn context(&self) -> String {
        let n = self
            .inner
            .spec()
            .pedals()
            .iter()
            .position(|p| p.id == self.pedal)
            .map_or(1, |i| i + 1);
        crate::i18n::tf(
            "tabula.context",
            &[("n", &n), ("where", &self.inner.context())],
        )
    }

    fn history(&self) -> Option<(bool, bool)> {
        self.inner.history()
    }

    fn undo(&mut self) {
        self.inner.undo();
    }

    fn redo(&mut self) {
        self.inner.redo();
    }
}
