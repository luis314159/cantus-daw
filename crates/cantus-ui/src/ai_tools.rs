//! The whole tool table an agent sees (round 34, ADRs 0081–0086): the project's tools from
//! `cantus-ai` and the ones that need the application (renders and analysis, the mastering
//! assistant, stems, audio to MIDI), each list owned by one builder. The MCP server lists
//! and dispatches exactly this, in this order.

use cantus_ai::ToolDef;

// One file per builder (round 34's contract): each holds `pub static TOOLS: &[ToolDef]`.
#[path = "ai_tools_listen.rs"]
pub mod listen;
#[path = "agent_tools.rs"] // r34:tools
pub mod project_app; // r34:tools — transport, selection, the catalogue, presets, comping, export
#[path = "ai_tools_stems.rs"]
pub mod stems;
#[path = "ai_tools_transcribe.rs"]
pub mod transcribe;

/// Every tool, in a fixed order (MCP asks `tools/list` to be deterministic).
pub fn registry() -> Vec<&'static ToolDef> {
    [
        cantus_ai::tools_project::TOOLS,
        project_app::TOOLS, // r34:tools
        listen::TOOLS,
        stems::TOOLS,
        transcribe::TOOLS,
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
