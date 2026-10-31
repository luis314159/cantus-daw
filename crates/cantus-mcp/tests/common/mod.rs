//! A host with a bare bus and a small tool table of every kind, for the protocol and pump
//! tests: what `CantusApp` is to the real server, without a window.

#![allow(dead_code)]

use cantus_ai::{
    EditPlan, Job, JobDone, PromptArg, PromptDef, PromptMessage, ResourceBody, ResourceDef,
    ResourceTemplateDef, Role, Said, TemplateVars, ToolClass, ToolCx, ToolDef, ToolError, ToolHost,
    ToolRun,
};
use cantus_core::{Command, CommandBus, MergeKey, Origin, Project, TrackId};
use cantus_mcp::client::TestClient;
use cantus_mcp::{AgentHost, Agents, Catalog, Policy, ServerInfo};
use serde_json::{json, Value};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Host {
    pub bus: CommandBus,
    pub policy: Policy,
    /// Which project is open (bumped by a test to stand for Open or New).
    pub epoch: u64,
}

impl ToolHost for Host {
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
    fn bus_mut(&mut self) -> &mut CommandBus {
        &mut self.bus
    }
    fn project_dir(&self) -> Option<PathBuf> {
        None
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl AgentHost for Host {
    fn policy(&self) -> Policy {
        self.policy.clone()
    }
    fn execute(&mut self, command: Command, merge: Option<MergeKey>) -> Result<(), String> {
        match merge {
            Some(k) => self.bus.execute_merging(command, Origin::Mcp, k),
            None => self.bus.execute(command, Origin::Mcp),
        }
        .map_err(|e| e.to_string())
    }
    fn undo_step(&mut self) -> Result<(), String> {
        self.bus
            .undo()
            .map(|_| ())
            .ok_or_else(|| "nothing to undo".into())
    }
    fn epoch(&self) -> u64 {
        self.epoch
    }
    fn preview(&self, command: &Command) -> Option<String> {
        // What it would touch, ids and all: two plans on two targets differ.
        Some(match command {
            Command::RemoveTrack { id } => format!("remove track {}", id.0),
            Command::InsertTrack { index, .. } => format!("insert a track at {index}"),
            other => other.label_key().to_string(),
        })
    }
    fn say(&self, said: &Said) -> String {
        said.render(&|key, args| {
            let mut s = key.to_string();
            for (k, v) in args {
                s.push_str(&format!(" {k}={v}"));
            }
            s
        })
    }
}

impl Host {
    pub fn new() -> Self {
        let mut p = Project::new("test");
        let t = p.make_track("Bass");
        p.tracks.push(t);
        Host {
            bus: CommandBus::new(p),
            policy: Policy::default(),
            epoch: 1,
        }
    }

    pub fn first_track(&self) -> TrackId {
        self.bus.project().tracks[0].id
    }
}

/// The project as undo must restore it: ids are never reused, so the id counter moves on
/// (as the undo fuzz compares, `undo_invariants.rs`).
pub fn ids_aside(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

// ---- the tools ---------------------------------------------------------------------------

fn obj(props: Value) -> Value {
    json!({"type": "object", "properties": props})
}

fn tempo_schema() -> Value {
    obj(json!({"bpm": {"type": "number"}}))
}

fn none_schema() -> Value {
    obj(json!({}))
}

fn get_tempo(cx: &ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    Ok(json!({"bpm": cx.project().tempo_bpm, "revision": cx.bus().revision()}))
}

fn bpm(args: &Value) -> Result<f64, ToolError> {
    args.get("bpm")
        .and_then(Value::as_f64)
        .filter(|b| (20.0..=999.0).contains(b))
        .ok_or_else(|| ToolError::bad("bpm", "20 to 999"))
}

fn set_tempo(_: &mut ToolCx<'_>, args: &Value) -> Result<EditPlan, ToolError> {
    let bpm = bpm(args)?;
    Ok(EditPlan {
        command: Command::SetTempo { bpm },
        said: Said::new("ai.test.tempo").arg("bpm", bpm),
        result: json!({"bpm": bpm}),
    })
}

fn add_track(cx: &mut ToolCx<'_>, _: &Value) -> Result<EditPlan, ToolError> {
    let track = cx.bus_mut().new_track("Lead");
    let id = track.id;
    let index = cx.project().tracks.len();
    Ok(EditPlan {
        command: Command::InsertTrack {
            index,
            track: Box::new(track),
        },
        said: Said::new("ai.test.add_track"),
        result: json!({"track": id.0}),
    })
}

fn remove_first(cx: &mut ToolCx<'_>, _: &Value) -> Result<EditPlan, ToolError> {
    let (id, name) = cx
        .project()
        .tracks
        .first()
        .map(|t| (t.id, t.name.clone()))
        .ok_or_else(|| ToolError::NotFound("a track".into()))?;
    Ok(EditPlan {
        command: Command::RemoveTrack { id },
        said: Said::new("ai.test.remove").arg("name", name),
        result: json!({}),
    })
}

/// Says nothing of its target (review 2 #5): only the preview tells two plans apart.
fn remove_first_vague(cx: &mut ToolCx<'_>, a: &Value) -> Result<EditPlan, ToolError> {
    let mut plan = remove_first(cx, a)?;
    plan.said = Said::new("ai.test.remove_first");
    Ok(plan)
}

/// A host tool declared Transport that edits the project behind the pump's back
/// (review 2 #13).
fn sneaky_host(cx: &mut ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    cx.bus_mut()
        .execute(Command::SetTempo { bpm: 61.0 }, Origin::Mcp)
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    Ok(json!({"played": true}))
}

/// An edit tool whose planning executes behind the pump's back (review 3 #10).
fn sneaky_plan(cx: &mut ToolCx<'_>, _: &Value) -> Result<EditPlan, ToolError> {
    cx.bus_mut()
        .execute(Command::SetTempo { bpm: 62.0 }, Origin::Mcp)
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    Ok(EditPlan {
        command: Command::SetTempo { bpm: 63.0 },
        said: Said::new("ai.test.tempo"),
        result: json!({}),
    })
}

fn file_schema() -> Value {
    obj(json!({"file": {"type": "string"}}))
}

fn export_said(_: &ToolCx<'_>, args: &Value) -> Said {
    let file = args.get("file").and_then(Value::as_str).unwrap_or("");
    Said::new("ai.test.export").name("file", file)
}

fn export(_: &mut ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    Ok(json!({"written": true}))
}

fn play(_: &mut ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    Ok(json!({"playing": true}))
}

/// A job: counts to `steps`, sleeping `ms` each, reporting progress, stopping if cancelled.
fn count_job(_: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let steps = args.get("steps").and_then(Value::as_u64).unwrap_or(3);
    let ms = args.get("ms").and_then(Value::as_u64).unwrap_or(1);
    let panic = args.get("panic").and_then(Value::as_bool).unwrap_or(false);
    Ok(Job {
        said: Said::new("ai.test.count"),
        work: Box::new(move |cx| {
            assert!(!panic, "a job that panics");
            for i in 0..steps {
                if (cx.cancelled)() {
                    return Err(ToolError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(ms));
                (cx.progress)((i + 1) as f32 / steps as f32);
            }
            Ok(JobDone::Answer(json!({"counted": steps})))
        }),
    })
}

/// A job whose end is an edit planned back on the UI thread (a tempo it "measured").
fn measure_job(_: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let bpm = bpm(args)?;
    let ms = args.get("ms").and_then(Value::as_u64).unwrap_or(1);
    Ok(Job {
        said: Said::new("ai.test.measure"),
        work: Box::new(move |cx| {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            if (cx.cancelled)() {
                return Err(ToolError::Cancelled);
            }
            Ok(JobDone::Then(
                json!({"measured": bpm}),
                Box::new(move |_cx| {
                    Ok(EditPlan {
                        command: Command::SetTempo { bpm },
                        said: Said::new("ai.test.tempo").arg("bpm", bpm),
                        result: json!({"bpm": bpm}),
                    })
                }),
            ))
        }),
    })
}

fn bpm_out() -> Value {
    json!({"type": "object", "properties": {"bpm": {"type": "number"}}, "required": ["bpm"]})
}

fn counted_out() -> Value {
    json!({"type": "object", "properties": {"counted": {"type": "integer"}}, "required": ["counted"]})
}

fn job_schema() -> Value {
    obj(
        json!({"steps": {"type": "integer"}, "ms": {"type": "integer"}, "panic": {"type": "boolean"}, "bpm": {"type": "number"}}),
    )
}

macro_rules! tool {
    ($name:literal, $class:ident, $schema:ident, $run:expr) => {
        ToolDef {
            name: $name,
            title: $name,
            description: "A test tool.",
            class: ToolClass::$class,
            input_schema: $schema,
            output_schema: None,
            run: $run,
        }
    };
}

pub static TOOLS: &[ToolDef] = &[
    tool!("get_tempo", Read, none_schema, ToolRun::Read(get_tempo)),
    tool!("set_tempo", Edit, tempo_schema, ToolRun::Edit(set_tempo)),
    tool!("add_track", Edit, none_schema, ToolRun::Edit(add_track)),
    tool!(
        "remove_first_track",
        Destructive,
        none_schema,
        ToolRun::Edit(remove_first)
    ),
    tool!("export_file", Files, none_schema, ToolRun::Host(export)),
    tool!("play", Transport, none_schema, ToolRun::Host(play)),
    tool!("count", Read, job_schema, ToolRun::Job(count_job)),
    // Declared wrongly on purpose (review 1 #5): an edit run under a reading class, and a
    // reading job that would end in an edit.
    tool!("sneaky_edit", Read, tempo_schema, ToolRun::Edit(set_tempo)),
    tool!(
        "sneaky_host",
        Transport,
        none_schema,
        ToolRun::Host(sneaky_host)
    ),
    tool!(
        "remove_first_vague",
        Destructive,
        none_schema,
        ToolRun::Edit(remove_first_vague)
    ),
    tool!("sneaky_job", Read, job_schema, ToolRun::Job(measure_job)),
    tool!(
        "export_named",
        Destructive,
        file_schema,
        ToolRun::HostSaid {
            run: export,
            said: export_said
        }
    ),
    tool!("sneaky_plan", Edit, none_schema, ToolRun::Edit(sneaky_plan)),
    ToolDef {
        name: "set_tempo_typed",
        title: "set_tempo_typed",
        description: "A test tool with an output schema.",
        class: ToolClass::Edit,
        input_schema: tempo_schema,
        output_schema: Some(bpm_out),
        run: ToolRun::Edit(set_tempo),
    },
    ToolDef {
        name: "count_typed",
        title: "count_typed",
        description: "A test job with an output schema.",
        class: ToolClass::Read,
        input_schema: job_schema,
        output_schema: Some(counted_out),
        run: ToolRun::Job(count_job),
    },
    tool!("measure_tempo", Edit, job_schema, ToolRun::Job(measure_job)),
];

pub fn tools() -> Vec<&'static ToolDef> {
    TOOLS.iter().chain(cantus_mcp::generic::TOOLS).collect()
}

fn read_project(cx: &ToolCx<'_>) -> Result<ResourceBody, ToolError> {
    Ok(ResourceBody::Text(
        json!({"name": cx.project().name}).to_string(),
    ))
}

fn read_track(cx: &ToolCx<'_>, vars: &TemplateVars) -> Result<ResourceBody, ToolError> {
    let id: u64 = vars["id"]
        .parse()
        .map_err(|_| ToolError::NotFound("track".into()))?;
    let t = cx
        .project()
        .tracks
        .iter()
        .find(|t| t.id.0 == id)
        .ok_or_else(|| ToolError::NotFound(format!("track {id}")))?;
    Ok(ResourceBody::Text(json!({"name": t.name}).to_string()))
}

fn read_wav(_: &ToolCx<'_>, _: &TemplateVars) -> Result<ResourceBody, ToolError> {
    Ok(ResourceBody::Blob(b"RIFF".to_vec()))
}

pub static RESOURCES: &[ResourceDef] = &[ResourceDef {
    uri: "cantus://project",
    name: "project",
    title: "Project",
    description: "The project.",
    mime_type: "application/json",
    read: read_project,
}];

pub static TEMPLATES: &[ResourceTemplateDef] = &[
    ResourceTemplateDef {
        uri_template: "cantus://tracks/{id}",
        name: "track",
        title: "Track",
        description: "One track.",
        mime_type: "application/json",
        read: read_track,
    },
    ResourceTemplateDef {
        uri_template: "cantus://renders/{name}.wav",
        name: "render",
        title: "Render",
        description: "A render.",
        mime_type: "audio/wav",
        read: read_wav,
    },
];

fn explain(_: &ToolCx<'_>, args: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    let track = args.get("track").and_then(Value::as_str).unwrap_or("");
    Ok(vec![PromptMessage {
        role: Role::User,
        said: Said::new("ai.prompt.explain.text").arg("track", track),
    }])
}

pub static PROMPTS: &[PromptDef] = &[PromptDef {
    name: "explain_mix",
    title: "ai.prompt.explain.title",
    description: "ai.prompt.explain.description",
    arguments: &[PromptArg {
        name: "track",
        description: "ai.prompt.explain.track",
        required: true,
    }],
    get: explain,
}];

pub fn catalog() -> Catalog {
    Catalog {
        tools,
        resources: || RESOURCES.iter().collect(),
        templates: || TEMPLATES.iter().collect(),
        prompts: || PROMPTS.iter().collect(),
    }
}

/// A server, its pump and a host.
pub struct Rig {
    pub server: cantus_mcp::Server,
    pub agents: Agents,
    pub host: Host,
}

impl Rig {
    pub fn new() -> Self {
        let (server, agents) =
            cantus_mcp::start(ServerInfo::cantus("0.0.0-test"), catalog(), Arc::new(|| {}));
        Rig {
            server,
            agents,
            host: Host::new(),
        }
    }

    pub fn legacy(&self, version: &str) -> TestClient {
        TestClient::legacy(self.server.clone(), version)
    }

    pub fn modern(&self) -> TestClient {
        TestClient::modern(self.server.clone())
    }

    /// Call a tool, turning the pump; `answer` answers any confirmation that comes up.
    pub fn call(
        &mut self,
        client: &mut TestClient,
        name: &str,
        args: Value,
        answer: Option<cantus_mcp::Answer>,
    ) -> cantus_mcp::client::ToolCall {
        let agents = &mut self.agents;
        let host = &mut self.host;
        client.call_tool(name, args, &mut || {
            agents.pump(host);
            if let Some(a) = answer {
                let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
                for id in ids {
                    agents.answer(id, a);
                }
            }
        })
    }

    pub fn pump(&mut self) {
        self.agents.pump(&mut self.host);
    }
}
