//! What an import or an export did, and what it could not carry — said to the user, never
//! dropped in silence (ADR 0021).
//!
//! This crate has no strings of its own: an item is a locale key and its arguments, and
//! the interface translates it (`locales/areas/interop.*`). Tests assert on the keys.

/// Every key an import or export in this crate can report. The interface has words for
/// each in both languages (its `interop_flows` test), and a test here keeps the list and
/// the code in step.
pub const KEYS: &[&str] = &[
    // MIDI files
    "interop.report.midi_read",
    "interop.report.format2",
    "interop.report.smpte",
    "interop.report.tempo_changes",
    "interop.report.tempo_retimed",
    "interop.report.metre_changes",
    "interop.report.odd_metres",
    "interop.report.controllers",
    "interop.report.pitch_bends",
    "interop.report.pressure",
    "interop.report.programs",
    "interop.report.sysex",
    "interop.report.texts",
    "interop.report.key_signatures",
    "interop.report.unterminated",
    "interop.report.zero_length",
    "interop.report.tempo_set",
    "interop.report.tempo_kept",
    "interop.report.too_many_tracks",
    "interop.report.pads_full",
    // DAWproject export
    "interop.report.x_empty_strips",
    "interop.report.x_muted_notes",
    "interop.report.x_devices",
    "interop.report.x_pan",
    "interop.report.x_width",
    "interop.report.x_polarity",
    "interop.report.x_swap",
    "interop.report.x_fx_off",
    "interop.report.x_slot_mix",
    "interop.report.x_sidechain",
    "interop.report.x_master_width",
    "interop.report.x_master_polarity",
    // Audio clips (round 29, ADR 0061)
    "interop.report.x_audio_external",
    "interop.report.x_warp_transpose",
    "interop.report.x_pitch_edits",
    "interop.report.x_reverse",
    "interop.report.x_takes",
    "interop.report.x_track_input",
    "interop.report.x_clip_gain",
    "interop.report.x_fade_curve",
    "interop.report.x_warp_tail",
    "interop.report.x_warp_formants",
    "interop.report.x_warp_markers",
    "interop.report.x_audio_unread",
    "interop.report.d_audio_missing",
    // DAWproject import
    "interop.report.d_read",
    "interop.report.d_foreign",
    "interop.report.d_instrument",
    "interop.report.d_plugin",
    "interop.report.d_part_too_large",
    "interop.report.d_warp_natural",
    "interop.report.d_device",
    "interop.report.d_master_device",
    "interop.report.d_vca",
    "interop.report.d_cycle",
    "interop.report.d_send",
    "interop.report.d_too_many_strips",
    "interop.report.d_too_many_tracks",
    "interop.report.d_audio_clips",
    "interop.report.d_automation",
    "interop.report.d_markers",
    "interop.report.d_launcher",
    "interop.report.d_scenes",
    "interop.report.d_expressions",
    "interop.report.d_references",
    "interop.report.d_loop",
    "interop.report.d_seconds",
];

/// Whether an item is something that was done, or something that was left behind.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Done, and worth knowing: "the tempo was set to 96 BPM".
    Info,
    /// In the source and not in the result: "412 controller changes were not imported".
    Dropped,
}

/// One line of a report.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportItem {
    pub severity: Severity,
    /// A key in `locales/areas/interop.*`.
    pub key: &'static str,
    /// The key's `{name}` arguments, already formatted.
    pub args: Vec<(&'static str, String)>,
}

/// Everything an import or export has to say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub items: Vec<ReportItem>,
}

impl Report {
    pub fn info(&mut self, key: &'static str, args: Vec<(&'static str, String)>) {
        self.items.push(ReportItem {
            severity: Severity::Info,
            key,
            args,
        });
    }

    pub fn dropped(&mut self, key: &'static str, args: Vec<(&'static str, String)>) {
        self.items.push(ReportItem {
            severity: Severity::Dropped,
            key,
            args,
        });
    }

    /// Nothing was left behind.
    pub fn is_lossless(&self) -> bool {
        self.items.iter().all(|i| i.severity == Severity::Info)
    }

    pub fn has(&self, key: &str) -> bool {
        self.items.iter().any(|i| i.key == key)
    }

    /// The first item with this key.
    pub fn get(&self, key: &str) -> Option<&ReportItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn dropped_items(&self) -> impl Iterator<Item = &ReportItem> {
        self.items
            .iter()
            .filter(|i| i.severity == Severity::Dropped)
    }

    /// Count `n` things of one kind that were not carried, if there were any.
    pub fn count_dropped(&mut self, key: &'static str, n: usize) {
        if n > 0 {
            self.dropped(key, vec![("n", n.to_string())]);
        }
    }
}

impl ReportItem {
    /// The value of one argument, for tests and for the interface.
    pub fn arg(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    /// Every `interop.report.*` key written in this crate's sources is in [`super::KEYS`],
    /// and every key there is used — so the interface's words cannot fall behind.
    #[test]
    fn the_key_list_is_the_keys_the_code_uses() {
        let sources = [
            include_str!("midi/import.rs"),
            include_str!("midi/export.rs"),
            include_str!("dawproject/read.rs"),
            include_str!("dawproject/read_timeline.rs"),
            include_str!("dawproject/write.rs"),
            include_str!("dawproject/audio.rs"),
            include_str!("dawproject/mod.rs"),
        ];
        let mut used = std::collections::BTreeSet::new();
        for src in sources {
            for part in src.split('"').skip(1).step_by(2) {
                if part.starts_with("interop.report.") {
                    used.insert(part.to_string());
                }
            }
        }
        let listed: std::collections::BTreeSet<String> =
            super::KEYS.iter().map(|k| k.to_string()).collect();
        assert_eq!(used, listed);
    }
}
