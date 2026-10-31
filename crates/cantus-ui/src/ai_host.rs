//! The application as the tools' host (ADR 0081 §2): the bus the project lives in, the
//! project's folder, and the execution of an agent's plan with `Origin::Mcp` under the
//! same holds as the user's own edits (`CantusApp::exec`): no tempo change while audio
//! records, a pass of automation ended before another project comes in, frozen sounds
//! refused with how to give them back, the sync service told of a tempo edit, the
//! selection kept valid.

use crate::app::CantusApp;
use crate::i18n::t;
use cantus_ai::{Said, ToolHost};
use cantus_core::{Command, CommandBus, CommandError, HistoryEntry, MergeKey, Origin};
use cantus_mcp::{AgentHost, Policy};
use std::any::Any;
use std::path::PathBuf;

impl ToolHost for CantusApp {
    fn bus(&self) -> &CommandBus {
        &self.bus
    }

    fn bus_mut(&mut self) -> &mut CommandBus {
        &mut self.bus
    }

    fn project_dir(&self) -> Option<PathBuf> {
        self.path
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
    }

    fn file_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self.project_dir().into_iter().collect();
        roots.extend(self.settings.ai.folders.iter().cloned());
        roots
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl AgentHost for CantusApp {
    fn policy(&self) -> Policy {
        self.settings.ai.policy()
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn preview(&self, command: &Command) -> Option<String> {
        // The planned command in the history's words, measured on a copy of the project.
        let mut after = self.bus.project().clone();
        let inverse = command.clone().apply(&mut after).ok()?;
        Some(crate::history_text::describe(
            command.label_key(),
            command,
            &inverse,
            &after,
        ))
    }

    fn execute(&mut self, command: Command, merge: Option<MergeKey>) -> Result<(), String> {
        // A kit an agent adds speaks the interface's language, as one the user adds does
        // (`CantusApp::set_instrument`): its pad names localised in the command executed,
        // so its undo stays exact.
        let mut command = command;
        localize_kits(&mut command);
        // `CantusApp::exec`'s holds, for an agent's command (keep the two in step).
        if self.tempo_held(&command) {
            return Err(t("tempo.status.recording"));
        }
        let tempo_edit = crate::views::transport::sync_service::is_tempo_edit(&command);
        let replaces = matches!(command, Command::ReplaceProject { .. });
        if replaces {
            self.end_write_pass_here();
        }
        let done = match merge {
            Some(key) => self.bus.execute_merging(command, Origin::Mcp, key),
            None => self.bus.execute(command, Origin::Mcp),
        };
        match done {
            Ok(()) => {
                if replaces {
                    self.epoch += 1;
                }
                crate::views::transport::sync_service::tempo_edited(self, tempo_edit);
                self.validate_selection();
                Ok(())
            }
            Err(CommandError::Frozen(target)) => {
                crate::freeze::refused(self, target);
                Err(self.status.text.clone())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn undo_step(&mut self) -> Result<(), String> {
        let before = self.bus.revision();
        self.undo();
        if self.bus.revision() == before {
            // A hold refused it, and said why in the status bar.
            Err(self.status.text.clone())
        } else {
            Ok(())
        }
    }

    fn say(&self, said: &Said) -> String {
        // Every nested phrase, bar range and quoted name in the interface's language too.
        said.render(&word)
    }

    fn describe_step(&self, entry: &HistoryEntry) -> String {
        crate::history_text::describe(
            &entry.label_key,
            &entry.command,
            &entry.inverse,
            self.bus.project(),
        )
    }
}

/// Every Pulsus kit `command` brings (a new track's, a swapped instrument's, in a batch
/// too) with its shipped pad names in the interface's language — `cantus-ai` stays free of
/// any locale (the integrated review: an agent's kit kept English names on a Spanish
/// screen).
fn localize_kits(command: &mut Command) {
    use cantus_core::InstrumentSpec;
    let localize = |spec: &mut InstrumentSpec| {
        if let InstrumentSpec::Pulsus(kit) = spec {
            for pad in &mut kit.pads {
                pad.name = crate::app::localized_pad_name(pad);
            }
        }
    };
    match command {
        Command::InsertTrack { track, .. } => localize(&mut track.instrument),
        Command::SetInstrument { spec, .. } => localize(spec),
        Command::Batch { commands, .. } => commands.iter_mut().for_each(localize_kits),
        _ => {}
    }
}

/// The interface's localiser for a [`Said`] (review 4 #10): the key's text — English when
/// this language lacks it — filled in one pass ([`cantus_ai::fill`]), so nothing an
/// argument brings in is read as a placeholder. A key no language has (a tool's mistake)
/// still says its arguments rather than a bare key.
pub(crate) fn word(key: &str, args: &[(&str, String)]) -> String {
    let frame = t(key);
    if frame == key {
        log::warn!("no text for the agent sentence {key}");
        if args.is_empty() {
            return key.to_string();
        }
        let listed: Vec<String> = args.iter().map(|(n, v)| format!("{n}: {v}")).collect();
        return format!("{key} ({})", listed.join(", "));
    }
    cantus_ai::fill(&frame, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{set_thread_locale, Locale};

    #[test]
    fn a_sentence_is_worded_in_one_pass_with_english_behind_it() {
        set_thread_locale(Locale::Es);
        // One pass: the bars' {from} is filled, what `from` brings in is not.
        assert_eq!(
            word("ai.bars", &[("from", "{to}".into()), ("to", "4".into())]),
            "compases {to}–4"
        );
        // A key Spanish lacks reads in English; one nobody has, with its arguments.
        assert_eq!(
            word("ai.no.such.key", &[("db", "-6".into())]),
            "ai.no.such.key (db: -6)"
        );
        set_thread_locale(Locale::En);
        assert_eq!(word("ai.bar", &[("n", "3".into())]), "bar 3");
    }
}
