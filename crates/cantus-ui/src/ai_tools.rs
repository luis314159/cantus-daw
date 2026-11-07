//! The whole tool table an agent sees (round 34, ADRs 0081–0086): the project's tools from
//! `cantus-ai` and the ones that need the application (renders and analysis, the mastering
//! assistant, stems, audio to MIDI), each list owned by one builder. The MCP server lists
//! and dispatches exactly this, in this order.

use cantus_ai::ToolDef;

// One file per builder (round 34's contract): each holds `pub static TOOLS: &[ToolDef]`.
#[path = "ai_tools_ara.rs"] // r36:ara
pub mod ara;
#[path = "ai_tools_editor.rs"] // r36:editor
pub mod audio_editor;
#[path = "ai_tools_drums.rs"] // r37:drums
pub mod drums; // r37:drums — split_kit_outputs, set_pad_chain, make_kit_from_folder (ADR 0100)
#[path = "ai_tools_external.rs"] // r36:external
pub mod external; // r36:external — external units and surfaces (ADR 0094, 0095)
#[path = "ai_tools_library.rs"] // r37:library
pub mod library;
#[path = "ai_tools_listen.rs"]
pub mod listen;
#[path = "ai_tools_mix.rs"] // r36:mix
pub mod mix; // r36:mix — VCAs, mixer snapshots, cues (ADR 0093)
#[path = "ai_tools_piano.rs"] // r37:piano
pub mod piano; // r37:piano — slice_sample, dump_slices (ADR 0099)
#[path = "agent_tools.rs"] // r34:tools
pub mod project_app; // r34:tools — transport, selection, the catalogue, presets, comping, export
#[path = "ai_tools_record.rs"] // r36:record
pub mod record; // r36:record — prepare_to_record, leave_recording_mode, get_recording_mode (ADR 0092)
#[path = "ai_tools_stems.rs"]
pub mod stems;
#[path = "ai_tools_transcribe.rs"]
pub mod transcribe;
#[path = "ai_tools_video.rs"] // r35:video
pub mod video; // r35:video — export_video (ADR 0088) // r37:library — find_presets, audition_preset (ADR 0101)

/// Every tool, in a fixed order (MCP asks `tools/list` to be deterministic).
pub fn registry() -> Vec<&'static ToolDef> {
    [
        cantus_ai::tools_project::TOOLS,
        project_app::TOOLS, // r34:tools
        listen::TOOLS,
        stems::TOOLS,
        transcribe::TOOLS,
        video::TOOLS,               // r35:video
        mix::TOOLS,                 // r36:mix
        record::TOOLS,              // r36:record
        external::TOOLS,            // r36:external
        audio_editor::TOOLS,        // r36:editor
        ara::TOOLS,                 // r36:ara
        library::TOOLS,             // r37:library
        drums::TOOLS,               // r37:drums
        piano::TOOLS,               // r37:piano
        cantus_mcp::generic::TOOLS, // r34:mcp — the server's own: jobs, changes, undo
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The tool called `name`.
pub fn find(name: &str) -> Option<&'static ToolDef> {
    registry().into_iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_name_is_unique_and_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for tool in registry() {
            assert!(seen.insert(tool.name), "two tools are called {}", tool.name);
            assert!(
                !tool.name.is_empty()
                    && tool.name.len() <= 64
                    && tool
                        .name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{:?} is not [a-z0-9_]",
                tool.name
            );
            assert!(!tool.description.is_empty(), "{} says nothing", tool.name);
            assert_eq!((tool.input_schema)()["type"], "object", "{}", tool.name);
        }
    }
}
