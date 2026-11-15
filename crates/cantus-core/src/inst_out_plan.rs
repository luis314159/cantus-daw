//! The mixer's one-click actions over an instrument's named outputs (ADR 0123 §4), and
//! what outputs an instrument has (its definition: a kit's mics, a piano's perspectives).
//!
//! **Each to its own strip** gives every output still on Main a strip of its own — named
//! after it, placed right after the instrument's strip **in output order**, routed into
//! the instrument's strip, its pan law a balance — so the split instrument, every new
//! strip at unity, sums to the unsplit one to the bit (the lanes are added in output
//! order on both sides). **Pieces to strips** gives each family one strip. **All to Main**
//! returns every output on a strip made for outputs to Main and removes the strips nothing
//! else uses. Each is one batch, one undo step. A Pulsus kit keeps ADR 0100's *Split* and
//! *Merge back*: *Each to its own strip* and *All to Main* are those.

use super::{OutputFamily, OutputId, OutputPlan, OutputSlot, OutputView};
use crate::command::Command;
use crate::inst::tympanum::KitRef;
use crate::inst::tympanum_kit as kitdef;
use crate::model::{MixerStrip, PanLaw, Project, StripId, TrackId};
use crate::padout::{pad_made, Refusal};
use crate::vca::StripKind;
use crate::InstrumentKind;

/// Every output an instrument's definition has, in output order, each with where it plays
/// now (`strip`: `None` on Main). Tympanum: its kit's mics (a modelled kit's table, a
/// recorded kit the app's loader read). A kind or a kit not known here: its routed slots
/// alone.
pub fn definition(project: &Project, track: TrackId) -> Vec<OutputView> {
    let Some(t) = project.track(track) else {
        return Vec::new();
    };
    let Some(n) = t.instrument.native() else {
        return t.instrument.outputs();
    };
    let mut out: Vec<OutputView> = Vec::new();
    if n.kind == InstrumentKind::AcousticDrums {
        let mics: Vec<(OutputId, String, OutputFamily)> = match KitRef::of(n) {
            KitRef::Modelled(id) => kitdef::modelled_kit(&id)
                .map(|k| {
                    k.mics
                        .into_iter()
                        .map(|m| (m.output, m.name, m.family))
                        .collect()
                })
                .unwrap_or_default(),
            kit @ KitRef::Pack { .. } => kitdef::recorded(&kit.text())
                .map(|k| {
                    k.def
                        .mics
                        .iter()
                        .map(|m| (m.output, m.name.clone(), m.family))
                        .collect()
                })
                .unwrap_or_default(),
        };
        for (id, name, family) in mics {
            let strip = n.outputs.iter().find(|s| s.id == id).map(|s| s.strip);
            out.push(OutputView {
                id,
                name,
                family,
                strip,
            });
        }
    }
    // r40:chorda-src — Chorda's outputs: a recorded piano's perspectives and noises (the
    // images that are on), the model's five once it renders them apart (ADR 0125).
    if n.kind == InstrumentKind::Chorda {
        use crate::inst::chorda_source as cs;
        if cs::has_zones(n) || cs::MODEL_RENDERS_OUTPUTS {
            for o in cs::outputs(n) {
                let strip = n.outputs.iter().find(|s| s.id == o.id).map(|s| s.strip);
                out.push(OutputView {
                    id: o.id,
                    name: o.name,
                    family: o.family,
                    strip,
                });
            }
        }
    }
    // Slots the definition does not name (a pack missing, an old kit): still shown.
    for s in &n.outputs {
        if !out.iter().any(|o| o.id == s.id) {
            out.push(OutputView {
                id: s.id,
                name: s.name.clone(),
                family: s.family,
                strip: Some(s.strip),
            });
        }
    }
    out.sort_by_key(|o| o.id);
    out
}

/// A family's name on a strip *Pieces to strips* makes (the desk renames it as it likes).
pub fn family_name(f: OutputFamily) -> &'static str {
    match f {
        OutputFamily::Other => "Other",
        OutputFamily::Kick => "Kick",
        OutputFamily::Snare => "Snare",
        OutputFamily::HiHat => "Hi-hat",
        OutputFamily::Toms => "Toms",
        OutputFamily::Cymbals => "Cymbals",
        OutputFamily::Overheads => "Overheads",
        OutputFamily::Rooms => "Rooms",
        OutputFamily::Perspective => "Perspective",
        OutputFamily::Noise => "Noises",
        OutputFamily::Pad => "Pads",
    }
}

/// The one-click actions (*All to Main · Pieces to strips · Each to its own strip*) as one
/// batch, the strips made by `make_strip` (`CommandBus::new_strip`), or why not.
pub fn plan(
    project: &Project,
    track: TrackId,
    plan: OutputPlan,
    make_strip: &mut dyn FnMut(&str) -> MixerStrip,
) -> Result<Command, Refusal> {
    plan_with(project, track, plan, make_strip, &|n: &str| n.to_string())
}

/// [`plan`], each new strip named by `said` from its output's name (*Each to its own
/// strip*) or its family's English name (*Pieces to strips*): the interface's language
/// for the names that are Cantus's (r40:fix-f — a modelled kit's mics, the families).
pub fn plan_with(
    project: &Project,
    track: TrackId,
    plan: OutputPlan,
    make_strip: &mut dyn FnMut(&str) -> MixerStrip,
    said: &dyn Fn(&str) -> String,
) -> Result<Command, Refusal> {
    let t = project.track(track).ok_or(Refusal::NotAKit)?;
    if t.instrument.pulsus().is_some() {
        return match plan {
            OutputPlan::EachOwn => crate::padout::split_with(project, track, make_strip),
            OutputPlan::AllToMain => crate::padout::merge(project, track),
            OutputPlan::ByFamily => Err(Refusal::Nothing),
        };
    }
    let n = t.instrument.native().ok_or(Refusal::NotAKit)?;
    if !n.kind.has_outputs() {
        return Err(Refusal::NotAKit);
    }
    let outputs = definition(project, track);
    match plan {
        OutputPlan::AllToMain => to_main(project, track, &outputs),
        OutputPlan::EachOwn | OutputPlan::ByFamily => {
            let on_main: Vec<&OutputView> = outputs.iter().filter(|o| o.strip.is_none()).collect();
            if on_main.is_empty() {
                return Err(Refusal::Nothing);
            }
            // The groups, in output order: one per output, or one per family (in the order
            // the families first appear).
            let mut groups: Vec<(String, Vec<&OutputView>)> = Vec::new();
            for o in on_main {
                if plan == OutputPlan::EachOwn {
                    groups.push((said(&o.name), vec![o]));
                } else if let Some(g) = groups
                    .iter_mut()
                    .find(|g| g.1.first().is_some_and(|x| x.family == o.family))
                {
                    g.1.push(o);
                } else {
                    groups.push((said(family_name(o.family)), vec![o]));
                }
            }
            let kit_strip = t.strip.filter(|s| project.strip(*s).is_some());
            let mut at =
                kit_strip
                    .and_then(|s| project.strip_index(s))
                    .map(|i| {
                        let mut at = i + 1;
                        while project.strips.get(at).is_some_and(|x| {
                            x.output == kit_strip && t.instrument.feeds_strip(x.id)
                        }) {
                            at += 1;
                        }
                        at
                    })
                    .unwrap_or(project.strips.len());
            let dock = kit_strip
                .and_then(|s| project.strip(s))
                .map(|s| s.dock)
                .unwrap_or_default();
            let mut commands = Vec::new();
            // r40:fix-f — a strip made for an output that went back to Main, named after
            // it and still on the desk with nothing on it, takes it again rather than a
            // second strip of that name beside it.
            let mut spare: Vec<StripId> = crate::padout::orphans(project)
                .into_iter()
                .filter(|s| {
                    project
                        .strip(*s)
                        .is_some_and(|x| x.kind == StripKind::Pad && x.output == kit_strip)
                })
                .collect();
            for (name, members) in groups {
                let reuse = spare
                    .iter()
                    .position(|s| project.strip(*s).is_some_and(|x| x.name == name))
                    .map(|i| spare.remove(i));
                let id = match reuse {
                    Some(id) => id,
                    None => {
                        let mut strip = make_strip(&name);
                        strip.kind = StripKind::Pad;
                        strip.output = kit_strip;
                        strip.pan_law = Some(PanLaw::ZeroDb);
                        strip.dock = dock;
                        let id = strip.id;
                        commands.push(Command::InsertStrip {
                            index: at,
                            strip: Box::new(strip),
                        });
                        at += 1;
                        id
                    }
                };
                for o in members {
                    commands.push(Command::SetInstrumentOutput {
                        track,
                        id: o.id,
                        slot: Some(OutputSlot {
                            id: o.id,
                            name: o.name.clone(),
                            family: o.family,
                            strip: id,
                        }),
                    });
                }
            }
            let label = if plan == OutputPlan::EachOwn {
                "cmd.outputs_each"
            } else {
                "cmd.outputs_family"
            };
            crate::padout::with_room(
                project,
                Command::Batch {
                    label_key: label.into(),
                    commands,
                },
            )
        }
    }
}

/// *All to Main*: every output on a strip made for outputs back on Main, and each such
/// strip nothing else uses removed (no other track or output, no strip routed, sending or
/// keyed into it, no VCA following it). An output on a strip the user chose stays there.
fn to_main(project: &Project, track: TrackId, outputs: &[OutputView]) -> Result<Command, Refusal> {
    let mut commands = Vec::new();
    let mut strips: Vec<StripId> = Vec::new();
    for o in outputs {
        let Some(s) = o.strip else {
            continue;
        };
        if !project.strip(s).is_some_and(pad_made) {
            continue;
        }
        commands.push(Command::SetInstrumentOutput {
            track,
            id: o.id,
            slot: None,
        });
        if !strips.contains(&s) {
            strips.push(s);
        }
    }
    if commands.is_empty() {
        return Err(Refusal::Nothing);
    }
    let mut after = project.clone();
    let moved = Command::Batch {
        label_key: "cmd.outputs_main".into(),
        commands: commands.clone(),
    };
    if moved.apply(&mut after).is_ok() {
        for s in strips {
            let used = after
                .tracks
                .iter()
                .any(|x| x.strip == Some(s) || x.feeds_through_bus(s))
                || after.strips.iter().any(|x| x.feeds().any(|to| to == s))
                || after.strips.iter().any(|x| x.keys().any(|k| k == s))
                || after.strips.iter().any(|x| x.vcas.contains(&s))
                || after.master.effects.iter().any(|e| e.sidechain == Some(s));
            if !used {
                commands.push(Command::RemoveStrip { id: s });
            }
        }
    }
    Ok(Command::Batch {
        label_key: "cmd.outputs_main".into(),
        commands,
    })
}
