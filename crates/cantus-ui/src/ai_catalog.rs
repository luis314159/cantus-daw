//! What the application's server offers (ADR 0081): the tool table
//! (`crate::ai_tools::registry()`), and the resources, resource templates and prompts of
//! each builder's list, joined in a fixed order. A builder with resources or prompts of its
//! own adds its list here by a marked line.

use cantus_ai::{PromptDef, ResourceDef, ResourceTemplateDef};
use cantus_mcp::Catalog;

/// The catalogue the application serves.
pub fn app() -> Catalog {
    Catalog {
        tools: crate::ai_tools::registry,
        resources,
        templates,
        prompts,
    }
}

/// Every resource, in a fixed order.
pub fn resources() -> Vec<&'static ResourceDef> {
    [
        cantus_ai::tools_project::RESOURCES,
        crate::ai_tools::project_app::RESOURCES, // r34:tools
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Every resource template, in a fixed order.
pub fn templates() -> Vec<&'static ResourceTemplateDef> {
    [
        cantus_ai::tools_project::RESOURCE_TEMPLATES,
        crate::ai_tools::project_app::RESOURCE_TEMPLATES, // r34:tools
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Every prompt, in a fixed order.
pub fn prompts() -> Vec<&'static PromptDef> {
    [
        cantus_ai::tools_project::PROMPTS,
        crate::ai_tools::listen::PROMPTS, // r34:listen — Master for streaming
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_prompt_names_are_unique_and_prompts_are_worded_in_both_languages() {
        let mut seen = std::collections::BTreeSet::new();
        for r in resources() {
            assert!(seen.insert(r.uri.to_string()), "{} twice", r.uri);
            assert!(r.uri.starts_with("cantus://"), "{}", r.uri);
        }
        for t in templates() {
            assert!(
                seen.insert(t.uri_template.to_string()),
                "{} twice",
                t.uri_template
            );
            assert!(
                t.uri_template.starts_with("cantus://"),
                "{}",
                t.uri_template
            );
        }
        let mut names = std::collections::BTreeSet::new();
        for p in prompts() {
            assert!(names.insert(p.name), "{} twice", p.name);
            for key in [p.title, p.description]
                .into_iter()
                .chain(p.arguments.iter().map(|a| a.description))
            {
                for locale in [crate::i18n::Locale::En, crate::i18n::Locale::Es] {
                    assert!(
                        crate::i18n::has(locale, key),
                        "{key} has no words in {locale:?}"
                    );
                }
            }
        }
    }

    /// Every schema the application lists has an object root, as MCP 2025-06-18 and
    /// 2025-11-25 require of `inputSchema` and `outputSchema` (review 3 #1).
    #[test]
    fn every_listed_schema_has_an_object_root() {
        let list = cantus_mcp::catalog::tools_list(&app());
        for t in list["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            if let Some(out) = t.get("outputSchema") {
                assert_eq!(out["type"], "object", "{}", t["name"]);
            }
        }
    }

    /// Every input schema of every builder's tools closed to the bottom: wherever it names
    /// its keys, `additionalProperties: false` (the integrated review: only the
    /// application's own tools were checked).
    #[test]
    fn every_registered_input_schema_is_closed() {
        fn open(v: &serde_json::Value, at: &str, out: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    if m.contains_key("properties")
                        && m.get("additionalProperties") != Some(&serde_json::json!(false))
                    {
                        out.push(at.to_string());
                    }
                    for (k, x) in m {
                        open(x, &format!("{at}.{k}"), out);
                    }
                }
                serde_json::Value::Array(a) => a.iter().for_each(|x| open(x, at, out)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        for t in crate::ai_tools::registry() {
            let mut here = Vec::new();
            open(&(t.input_schema)(), "", &mut here);
            found.extend(here.into_iter().map(|at| format!("{}{at}", t.name)));
        }
        assert!(found.is_empty(), "open schemas: {found:#?}");
    }

    /// A job that ends in an edit (`JobDone::Then`) belongs to a tool whose class may
    /// change the project — every builder's, read from the source of each file that makes
    /// one (the pump refuses it at run time too).
    #[test]
    fn every_job_that_ends_in_an_edit_has_a_mutating_class() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut folders: Vec<std::path::PathBuf> = ["cantus-ai/src", "cantus-ui/src"]
            .iter()
            .map(|d| root.join(d))
            .collect();
        let mut sources = Vec::new();
        while let Some(dir) = folders.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                    continue;
                }
                let file = path.file_name().unwrap().to_string_lossy().to_string();
                if !file.ends_with(".rs") || file.ends_with("tests.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                let code = text.split("#[cfg(test)]").next().unwrap_or("").to_string();
                sources.push((file, code));
            }
        }
        // Every function — its file, its name, its text up to the next one.
        let mut fns: Vec<(String, String, String)> = Vec::new();
        for (file, code) in &sources {
            for piece in code.split("fn ").skip(1) {
                let name: String = piece
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    fns.push((file.clone(), name, piece.to_string()));
                }
            }
        }
        // The functions that make a `JobDone::Then`, and those of the same file that call
        // one of them, in turn, until nothing more joins (a name is a file's own: two
        // files may each have a `prepare`).
        let mut edits: Vec<(String, String)> = fns
            .iter()
            .filter(|(_, _, body)| body.contains("JobDone::Then("))
            .map(|(f, n, _)| (f.clone(), n.clone()))
            .collect();
        loop {
            let more: Vec<(String, String)> = fns
                .iter()
                .filter(|(f, n, body)| {
                    !edits.iter().any(|(ef, en)| ef == f && en == n)
                        && edits
                            .iter()
                            .any(|(ef, en)| ef == f && en != n && body.contains(&format!("{en}(")))
                })
                .map(|(f, n, _)| (f.clone(), n.clone()))
                .collect();
            if more.is_empty() {
                break;
            }
            edits.extend(more);
        }
        // Of them, the job functions: `fn name(…) -> Result<Job, …>`.
        let job_fns: Vec<String> = fns
            .iter()
            .filter(|(f, n, body)| {
                edits.iter().any(|(ef, en)| ef == f && en == n)
                    && body[..body.find('{').unwrap_or(body.len()).min(400)]
                        .contains("-> Result<Job,")
            })
            .map(|(_, n, _)| n.clone())
            .collect();
        // The tools that run them: `run: ToolRun::Job(path::name)` after their `name: "…"`.
        let registry = crate::ai_tools::registry();
        let mut checked = Vec::new();
        for (file, code) in &sources {
            let mut rest = code.as_str();
            while let Some(at) = rest.find("ToolRun::Job(") {
                let before = &rest[..at];
                let after = &rest[at + "ToolRun::Job(".len()..];
                let path = &after[..after.find(')').unwrap_or(0)];
                let f = path.rsplit("::").next().unwrap_or("").trim();
                if job_fns.iter().any(|j| j == f) {
                    if let Some(name) = before
                        .rfind("name: \"")
                        .map(|n| &before[n + 7..])
                        .and_then(|n| n.split('"').next())
                    {
                        if let Some(tool) = registry.iter().find(|t| t.name == name) {
                            assert!(
                                cantus_mcp::catalog::mutates(tool.class),
                                "{name} ({file}) can end in an edit but is {:?}",
                                tool.class
                            );
                            checked.push(name.to_string());
                        }
                    }
                }
                rest = after;
            }
        }
        for expected in ["separate_stems", "audio_to_midi", "set_mix_targets"] {
            assert!(
                checked.iter().any(|c| c == expected),
                "{expected} not found among {checked:?} (jobs that edit: {job_fns:?})"
            );
        }
    }

    /// An edit's plan is executed only for a class that may change the project (review 1
    /// #5): a tool that plans an edit must say it does.
    #[test]
    fn every_tool_that_plans_an_edit_has_a_mutating_class() {
        for tool in crate::ai_tools::registry() {
            if matches!(tool.run, cantus_ai::ToolRun::Edit(_)) {
                assert!(
                    cantus_mcp::catalog::mutates(tool.class),
                    "{} plans an edit but is {:?}",
                    tool.name,
                    tool.class
                );
            }
        }
    }
}
