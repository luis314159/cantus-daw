//! The tool table (round 34, ADRs 0081–0082): one list of typed tools drives the MCP server
//! and, later, the assistant panel. A tool is data plus a function; it never executes a
//! command itself — it *plans* one, and the host decides (the user's permissions, the
//! confirmation card) and executes it on the UI thread with [`cantus_core::Origin::Mcp`], so
//! one tool call is one undo step through the same bus as a mouse click (ADR 0004).
//!
//! Each builder of round 34 owns one list: `crate::tools_project::TOOLS` here, the others in
//! `cantus-ui`'s `ai_tools*.rs`; `cantus_ui::ai_tools::registry()` puts them together.

use cantus_core::{Command, CommandBus, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::path::PathBuf;
use thiserror::Error;

/// What a tool may do: the permission class the user grants (`research/ai/mcp-server.md` §4).
/// The server decides by class, in Cantus; a client's annotations are only hints.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolClass {
    /// Reads the project, or renders and analyses into Cantus's own cache. Never asks.
    Read,
    /// Changes the project — one undo step. Asks unless the user allowed edits.
    Edit,
    /// Removes or replaces the user's material, or undoes. Asks unless allowed for the session.
    Destructive,
    /// Reads or writes a file outside the project's own folder (import, export). Denied
    /// unless the user enabled it.
    Files,
    /// Play, stop, locate: not undoable (and the tool's description says so).
    Transport,
}

/// One tool. `name` is a promise kept forever: `[a-z0-9_]`, units in argument names
/// (`_db`, `_beats`, `_hz`), positions in bars:beats *and* beats, never ticks.
pub struct ToolDef {
    pub name: &'static str,
    /// Short English title (MCP `title`).
    pub title: &'static str,
    /// What the model reads: what it does, its limits, its units.
    pub description: &'static str,
    pub class: ToolClass,
    /// JSON Schema 2020-12 of the arguments.
    pub input_schema: fn() -> Value,
    /// JSON Schema of `structuredContent`, when the tool answers with structure.
    pub output_schema: Option<fn() -> Value>,
    pub run: ToolRun,
}

/// How a tool runs. Every function is called on the UI thread, which owns the bus.
pub enum ToolRun {
    /// Answered at once from the project as it stands.
    Read(fn(&ToolCx<'_>, &Value) -> Result<Value, ToolError>),
    /// Planned at once: the command (never executed by the tool) and what to answer after.
    Edit(fn(&mut ToolCx<'_>, &Value) -> Result<EditPlan, ToolError>),
    /// Prepared at once (a project clone, a snapshot, the services it needs), then run on a
    /// worker thread; long ones answer with a job handle the agent can wait on.
    Job(fn(&mut ToolCx<'_>, &Value) -> Result<Job, ToolError>),
    /// Done by the host itself at once (transport, selection): not an undo step. When its
    /// class asks, the card reads the tool's title.
    Host(fn(&mut ToolCx<'_>, &Value) -> Result<Value, ToolError>),
    /// [`ToolRun::Host`] with its own sentence for the confirmation card, worded from the
    /// arguments ("Export the WAV file “Song.wav”") before it runs.
    HostSaid {
        run: fn(&mut ToolCx<'_>, &Value) -> Result<Value, ToolError>,
        said: fn(&ToolCx<'_>, &Value) -> Said,
    },
}

/// What a tool sees on the UI thread: the host, which owns the bus (mcp's first delivery,
/// ADR 0081: the bus is reached *through* the host, so a tool that downcasts the host to
/// the application sees the very bus it plans against — never a second borrow of it).
pub struct ToolCx<'a> {
    pub host: &'a mut dyn ToolHost,
}

impl ToolCx<'_> {
    /// The bus: the project, its history, its ids.
    pub fn bus(&self) -> &CommandBus {
        self.host.bus()
    }

    /// The bus, to allocate ids while planning (never to execute: the host does that).
    pub fn bus_mut(&mut self) -> &mut CommandBus {
        self.host.bus_mut()
    }

    /// The project as it stands.
    pub fn project(&self) -> &Project {
        self.host.bus().project()
    }
}

/// The application's side of a tool call. `cantus-ui` implements it for `CantusApp`; a tool
/// that needs more than this (the engine, the ML helper) is written in `cantus-ui` and
/// downcasts [`ToolHost::as_any_mut`] to the concrete host to take what it needs.
pub trait ToolHost {
    /// The bus the project lives in.
    fn bus(&self) -> &CommandBus;
    fn bus_mut(&mut self) -> &mut CommandBus;
    /// The open project's folder, if it was saved — the one place file tools may write
    /// without the user's leave.
    fn project_dir(&self) -> Option<PathBuf>;
    /// Every folder file tools may read or write in: the project's own and the ones the
    /// user added in Settings ▸ AI (a `Files` tool is refused unless the user enabled
    /// files at all). Check a path with [`may_touch`].
    fn file_roots(&self) -> Vec<PathBuf> {
        self.project_dir().into_iter().collect()
    }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// The path a file tool must open for `path`, when it lies inside one of the host's
/// [`ToolHost::file_roots`]: resolved on disk — links and `..` followed, a link whose
/// target is missing refused (review 2 #9: a write through it would land outside), a file
/// not yet written judged by its folder. Open the path this returns, never the one given,
/// so nothing can swap a link in between.
pub fn resolve_touch(host: &dyn ToolHost, path: &std::path::Path) -> Option<PathBuf> {
    let resolved = match std::fs::symlink_metadata(path) {
        // It exists (a file, a folder, or a link): where it really is. A dangling link
        // fails here and is refused.
        Ok(_) => std::fs::canonicalize(path).ok()?,
        // Not there yet: its folder, resolved, and a plain name in it.
        Err(_) => {
            let parent = std::fs::canonicalize(path.parent()?).ok()?;
            let name = path.file_name()?;
            if name == ".." || name == "." {
                return None;
            }
            parent.join(name)
        }
    };
    host.file_roots()
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .any(|root| resolved.starts_with(&root))
        .then_some(resolved)
}

/// Whether `path` lies inside one of the host's [`ToolHost::file_roots`] ([`resolve_touch`]).
pub fn may_touch(host: &dyn ToolHost, path: &std::path::Path) -> bool {
    resolve_touch(host, path).is_some()
}

/// How a file tool opens a file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Touch {
    Read,
    /// A new file; refused if anything (a file, a link) is there already.
    CreateNew,
    /// A file written whole, made if missing.
    Replace,
}

/// Open `path` for a file tool, inside the host's roots only — the way to touch a file
/// (review 3 #11, review 4 #6): resolved as [`resolve_touch`] does, opened without
/// following a link at its last step and **without truncating**, then judged by where the
/// opened handle really is (`/proc/self/fd`, `F_GETPATH`, `GetFinalPathNameByHandleW`) —
/// so a folder swapped for a link between the check and the open is caught before a byte
/// is lost; only then is a [`Touch::Replace`] file emptied. Returns the file and where it
/// is.
pub fn open_touch(
    host: &dyn ToolHost,
    path: &std::path::Path,
    how: Touch,
) -> std::io::Result<(std::fs::File, PathBuf)> {
    use std::io::{Error, ErrorKind};
    let outside = || Error::new(ErrorKind::PermissionDenied, "outside the folders allowed");
    let resolved = resolve_touch(host, path).ok_or_else(outside)?;
    open_resolved(host, &resolved, how)
}

/// [`open_touch`] past its resolving: `resolved` may have changed since (a folder on its
/// way swapped for a link) — what the handle says decides.
fn open_resolved(
    host: &dyn ToolHost,
    resolved: &std::path::Path,
    how: Touch,
) -> std::io::Result<(std::fs::File, PathBuf)> {
    use cantus_rt::private::{open_no_follow, real_path, Open};
    use std::io::{Error, ErrorKind};
    let outside = || Error::new(ErrorKind::PermissionDenied, "outside the folders allowed");
    let open = match how {
        Touch::Read => Open::Read,
        Touch::CreateNew => Open::CreateNew,
        Touch::Replace => Open::Write,
    };
    // Whether this call makes the file: if it is refused, what it made goes again
    // (review 5 #11: a refused CreateNew or Replace left an empty file outside the roots).
    let made = how != Touch::Read && std::fs::symlink_metadata(resolved).is_err();
    let file = open_no_follow(resolved, open)?;
    let roots: Vec<PathBuf> = host
        .file_roots()
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .collect();
    let at = match real_path(&file) {
        Ok(real) => real,
        // A system that cannot say: the folder resolved again instead.
        Err(e) if e.kind() == ErrorKind::Unsupported => {
            let parent = resolved.parent().ok_or_else(outside)?;
            let again = std::fs::canonicalize(parent).map_err(|_| outside())?;
            if again != parent {
                return Err(outside());
            }
            resolved.to_path_buf()
        }
        Err(e) => return Err(e),
    };
    if !roots.iter().any(|root| at.starts_with(root)) {
        if made {
            drop(file);
            // Checked again where the handle said it is: still the empty file this call
            // made, and this user's — then removed.
            let ours = std::fs::symlink_metadata(&at).is_ok_and(|m| m.is_file() && m.len() == 0)
                && cantus_rt::private::owned_by_me(&at).unwrap_or(false);
            if ours {
                let _ = std::fs::remove_file(&at);
            }
        }
        return Err(outside());
    }
    if how == Touch::Replace {
        file.set_len(0)?;
    }
    Ok((file, at))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Roots(CommandBus, Vec<PathBuf>);

    impl ToolHost for Roots {
        fn bus(&self) -> &CommandBus {
            &self.0
        }
        fn bus_mut(&mut self) -> &mut CommandBus {
            &mut self.0
        }
        fn project_dir(&self) -> Option<PathBuf> {
            self.1.first().cloned()
        }
        fn file_roots(&self) -> Vec<PathBuf> {
            self.1.clone()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn a_path_is_touched_only_inside_the_roots() {
        let base = std::env::temp_dir().join(format!("cantus-ai-roots-{}", std::process::id()));
        let (proj, other) = (base.join("song"), base.join("elsewhere"));
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("secret.txt"), "x").unwrap();
        let host = Roots(CommandBus::default(), vec![proj.clone()]);
        assert!(
            may_touch(&host, &proj.join("mix.wav")),
            "a file to write in the project"
        );
        assert!(!may_touch(&host, &other.join("secret.txt")));
        assert!(!may_touch(
            &host,
            &proj.join("..").join("elsewhere").join("secret.txt")
        ));
        assert!(!may_touch(&host, &proj.join("..")));
        assert!(!may_touch(
            &host,
            &proj.join("no").join("such").join("x.wav")
        ));
        let none = Roots(CommandBus::default(), vec![]);
        assert!(
            !may_touch(&none, &proj.join("mix.wav")),
            "an unsaved project: nowhere"
        );
        let r = resolve_touch(&host, &proj.join("mix.wav")).unwrap();
        assert!(
            r.starts_with(std::fs::canonicalize(&proj).unwrap()),
            "{r:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A link, made where the system lets this user make one (Windows needs Developer Mode
    /// or the privilege; the test says so and checks the rest).
    fn link(target: &std::path::Path, at: &std::path::Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, at).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, at).is_ok()
        }
    }

    /// Review 2 #9, review 3 #11: a link inside the project pointing out of it — dangling
    /// or not — is never a way out, by `may_touch` or `open_touch`; a file opened inside
    /// is where `open_touch` says. On every system.
    #[test]
    fn a_link_out_of_the_roots_is_refused_dangling_or_not() {
        let base = std::env::temp_dir().join(format!("cantus-ai-links-{}", std::process::id()));
        let (proj, other) = (base.join("song"), base.join("elsewhere"));
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let host = Roots(CommandBus::default(), vec![proj.clone()]);
        // Inside: made new, then read back, where it was said to be.
        let (mut f, at) = open_touch(&host, &proj.join("mix.txt"), Touch::CreateNew).unwrap();
        std::io::Write::write_all(&mut f, b"ok").unwrap();
        drop(f);
        assert!(at.starts_with(std::fs::canonicalize(&proj).unwrap()));
        assert!(
            open_touch(&host, &proj.join("mix.txt"), Touch::CreateNew).is_err(),
            "a new file never replaces one"
        );
        let (mut f, _) = open_touch(&host, &proj.join("mix.txt"), Touch::Read).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).unwrap();
        assert_eq!(s, "ok");
        assert!(open_touch(&host, &other.join("x.txt"), Touch::Replace).is_err());
        std::fs::write(other.join("there.txt"), "x").unwrap();
        let made = link(&other.join("not-yet.txt"), &proj.join("dangling"))
            && link(&other.join("there.txt"), &proj.join("link"));
        if made {
            assert!(!may_touch(&host, &proj.join("dangling")), "a dangling link");
            assert!(!may_touch(&host, &proj.join("link")), "a link out");
            for how in [Touch::Read, Touch::Replace, Touch::CreateNew] {
                assert!(
                    open_touch(&host, &proj.join("dangling"), how).is_err(),
                    "{how:?}"
                );
                assert!(
                    open_touch(&host, &proj.join("link"), how).is_err(),
                    "{how:?}"
                );
            }
            assert!(
                !other.join("not-yet.txt").exists(),
                "nothing written outside"
            );
            assert_eq!(
                std::fs::read_to_string(other.join("there.txt")).unwrap(),
                "x"
            );
        } else {
            eprintln!("links cannot be made by this user here: the link cases are not run");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Review 4 #6: a folder swapped for a link out *after* the path was resolved (the
    /// race, made to happen) is caught by where the opened handle is — before a byte of
    /// the file outside is lost: nothing is truncated until the handle is judged.
    #[test]
    fn a_folder_swapped_after_the_check_loses_nothing() {
        let base = std::env::temp_dir().join(format!("cantus-ai-swap-{}", std::process::id()));
        let (proj, other) = (base.join("song"), base.join("elsewhere"));
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("keep.txt"), "precious").unwrap();
        let host = Roots(CommandBus::default(), vec![proj.clone()]);
        // What resolving said: inside the project, in its folder `sub`…
        let said = std::fs::canonicalize(&proj)
            .unwrap()
            .join("sub")
            .join("keep.txt");
        // …which is now a link to the folder outside.
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&other, proj.join("sub")).is_ok();
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir(&other, proj.join("sub")).is_ok();
        if linked {
            for how in [Touch::Replace, Touch::Read] {
                assert!(open_resolved(&host, &said, how).is_err(), "{how:?}");
            }
            // Review 5 #11: a new file refused there is not left behind, made or not.
            for (how, name) in [(Touch::CreateNew, "new.txt"), (Touch::Replace, "new2.txt")] {
                let target = std::fs::canonicalize(&proj).unwrap().join("sub").join(name);
                assert!(open_resolved(&host, &target, how).is_err(), "{how:?}");
                assert!(!other.join(name).exists(), "{how:?} left {name} outside");
            }
            assert_eq!(
                std::fs::read_to_string(other.join("keep.txt")).unwrap(),
                "precious",
                "not emptied"
            );
        } else {
            eprintln!("links cannot be made by this user here: the swap is not run");
        }
        // Inside, Replace empties the file it opened, and only then.
        std::fs::write(proj.join("mine.txt"), "old!").unwrap();
        let (mut f, at) = open_touch(&host, &proj.join("mine.txt"), Touch::Replace).unwrap();
        std::io::Write::write_all(&mut f, b"new").unwrap();
        drop(f);
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// A planned edit: one command (a `Command::Batch` when it needs several) → one undo step.
pub struct EditPlan {
    pub command: Command,
    /// What the confirmation card and the history say, localised by the host.
    pub said: Said,
    /// Returned to the agent once the command ran (the ids it made, what changed).
    pub result: Value,
}

/// A sentence to localise: a key in the `ai` area and its named arguments — each plain
/// text, a phrase localised in its turn, a range of bars, or a name the user gave, kept
/// verbatim and quoted (`cantus-ai` has no locale: the host words it all, [`Said::render`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Said {
    pub key: &'static str,
    pub args: Vec<(&'static str, SaidArg)>,
}

/// The quotation characters a language may put around a name (ASCII, guillemets, curly,
/// low-9, single and double, CJK corner brackets).
const QUOTE_MARKS: [char; 15] = [
    '"', '\'', '\u{AB}', '\u{BB}', '\u{201C}', '\u{201D}', '\u{201E}', '\u{2018}', '\u{2019}',
    '\u{201A}', '\u{2039}', '\u{203A}', '\u{300C}', '\u{300D}', '\u{300E}',
];

/// Whether `c` is invisible or reorders text (review 5 #5, review 6 #6): a control
/// character (Cc), a format character (Cf), or a default-ignorable code point (Unicode's
/// `Default_Ignorable_Code_Point`, which takes in the Hangul fillers and the variation
/// selectors a name could be made of and still look blank) — except the zero-width
/// joiner and non-joiner (U+200C, U+200D), which Persian, the Indic scripts and emoji
/// need to be spelt right.
pub fn is_invisible(c: char) -> bool {
    if matches!(c, '\u{200C}' | '\u{200D}') {
        return false;
    }
    c.is_control()
        || matches!(
            c,
            // Cf outside Default_Ignorable_Code_Point.
            '\u{600}'..='\u{605}'
                | '\u{6DD}'
                | '\u{70F}'
                | '\u{890}'..='\u{891}'
                | '\u{8E2}'
                | '\u{110BD}'
                | '\u{110CD}'
                | '\u{13430}'..='\u{1345F}'
                | '\u{FFF9}'..='\u{FFFB}'
                // Default_Ignorable_Code_Point (Unicode 15.1, DerivedCoreProperties).
                | '\u{AD}'
                | '\u{34F}'
                | '\u{61C}'
                | '\u{115F}'..='\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF0}'..='\u{FFF8}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E0FFF}'
        )
}

/// Whether `text` shows nothing: every character in it blank — whitespace, a joiner
/// (U+200C, U+200D, kept by [`is_invisible`] for the scripts that need them, but nothing
/// to see alone) or invisible (the integrated review: a name of joiners drew empty quotes).
pub fn is_blank(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_whitespace() || matches!(c, '\u{200C}' | '\u{200D}') || is_invisible(c))
}

/// `text` without its invisible and reordering characters ([`is_invisible`]).
pub fn strip_invisible(text: &str) -> String {
    text.chars().filter(|c| !is_invisible(*c)).collect()
}

/// A name someone else chose (a track, a file, a client's own name), in the language's
/// quotes as data, never words: no invisible or reordering characters, none of the
/// language's quote marks to close the quotation early (they become plain ones), no braces
/// a host's localiser could read as a placeholder (review 4 #10, review 5 #5). `word` is
/// the host's localiser; the frame is its `ai.quoted`.
pub fn quote_name(word: &Wording<'_>, name: &str) -> String {
    // The language's quote marks: the quotation characters its frame puts around a name —
    // only those (a period or a space of the frame is no reason to touch the name's own).
    let frame = word("ai.quoted", &[("name", String::new())]);
    let marks: Vec<char> = frame.chars().filter(|c| QUOTE_MARKS.contains(c)).collect();
    let plain = if marks.contains(&'"') { '\'' } else { '"' };
    let clean: String = name
        .chars()
        .filter(|c| !is_invisible(*c))
        .map(|c| match c {
            '{' => '\u{FF5B}',
            '}' => '\u{FF5D}',
            c if marks.contains(&c) => plain,
            c => c,
        })
        .collect();
    // Nothing left to show (a name of Hangul fillers, of blanks): said so, not a blank
    // between quotes (review 6 #6).
    if is_blank(&clean) {
        return word("ai.unnamed", &[]);
    }
    word("ai.quoted", &[("name", clean)])
}

/// A host's localiser: a key and its named arguments, worded — [`fill`] the frame, so an
/// argument's text is never read for placeholders.
pub type Wording<'a> = dyn Fn(&str, &[(&str, String)]) -> String + 'a;

/// Fill `frame`'s `{name}` placeholders from `args` in one pass (review 4 #10): what an
/// argument brings in — a name the user gave that says `{db}` — is never filled in its
/// turn. A placeholder with no argument stays as it is.
pub fn fill(frame: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(frame.len());
    let mut rest = frame;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(n, _)| *n == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// One argument of a [`Said`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaidArg {
    /// Kept as it is (a number, a unit).
    Text(String),
    /// A phrase of its own, localised in its turn ("the level of “Piano”").
    Said(Box<Said>),
    /// Bars, numbered from 1: one (`from == to`, key `ai.bar`, "bar {n}") or a range (key
    /// `ai.bars`, "bars {from}–{to}").
    Bars { from: u32, to: u32 },
    /// A name the user gave (a track, a clip, a file): verbatim, in the language's quotes
    /// (key `ai.quoted`, "“{name}”") — data, never words of Cantus's.
    Name(String),
}

impl Said {
    pub fn new(key: &'static str) -> Self {
        Said {
            key,
            args: Vec::new(),
        }
    }

    /// A plain argument, kept as it is.
    pub fn arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, SaidArg::Text(value.to_string())));
        self
    }

    /// A phrase as an argument, localised in its turn.
    pub fn nested(mut self, name: &'static str, said: Said) -> Self {
        self.args.push((name, SaidArg::Said(Box::new(said))));
        self
    }

    /// Bars `from`–`to` (from 1) as an argument.
    pub fn bars(mut self, name: &'static str, from: u32, to: u32) -> Self {
        self.args.push((name, SaidArg::Bars { from, to }));
        self
    }

    /// A name the user gave, quoted, as an argument.
    pub fn name(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, SaidArg::Name(value.into())));
        self
    }

    /// The sentence in words: `word(key, args)` is the host's localiser (a locale's `tf`);
    /// every nested phrase, bar range and quoted name is worded through it first.
    pub fn render(&self, word: &Wording<'_>) -> String {
        let args: Vec<(&str, String)> = self
            .args
            .iter()
            .map(|(n, a)| {
                let v = match a {
                    SaidArg::Text(t) => t.clone(),
                    SaidArg::Said(s) => s.render(word),
                    SaidArg::Bars { from, to } if from == to => {
                        word("ai.bar", &[("n", from.to_string())])
                    }
                    SaidArg::Bars { from, to } => word(
                        "ai.bars",
                        &[("from", from.to_string()), ("to", to.to_string())],
                    ),
                    SaidArg::Name(name) => quote_name(word, name),
                };
                (*n, v)
            })
            .collect();
        word(self.key, &args)
    }
}

#[cfg(test)]
mod said_tests {
    use super::*;

    /// A locale of three keys, the rest as their key with the arguments after.
    fn es(key: &str, args: &[(&str, String)]) -> String {
        let frame = match key {
            "ai.bars" => "compases {from}–{to}",
            "ai.bar" => "compás {n}",
            "ai.quoted" => "«{name}»",
            "set_level" => "Poner {what} a {db} dB en {where}",
            "level_of" => "el nivel de {track}",
            other => other,
        };
        fill(frame, args)
    }

    /// A localiser that fills by replacing in turn, as a careless host might.
    fn careless(key: &str, args: &[(&str, String)]) -> String {
        let frame = match key {
            "ai.quoted" => "“{name}”",
            "set_level" => "Set {what} to {db} dB",
            "level_of" => "the level of {track}",
            other => other,
        };
        args.iter().fold(frame.to_string(), |s, (n, v)| {
            s.replace(&format!("{{{n}}}"), v)
        })
    }

    /// A host whose quoting frame holds a period or a space leaves the name's own alone
    /// (the first version took every non-letter of the frame for a quote mark).
    #[test]
    fn invisible_characters_go_and_joiners_stay() {
        let q = |name: &str| quote_name(&es, name);
        // Joiners spell Persian, the Indic scripts and emoji: kept.
        assert_eq!(
            q("\u{645}\u{6CC}\u{200C}\u{62E}"),
            "«\u{645}\u{6CC}\u{200C}\u{62E}»"
        );
        assert_eq!(
            q("\u{1F469}\u{200D}\u{1F4BB}"),
            "«\u{1F469}\u{200D}\u{1F4BB}»"
        );
        // Cf, default-ignorables and blanks: gone.
        for c in [
            '\u{890}',
            '\u{8E2}',
            '\u{110BD}',
            '\u{13430}',
            '\u{1BCA0}',
            '\u{1D173}',
            '\u{34F}',
            '\u{115F}',
            '\u{17B4}',
            '\u{180B}',
            '\u{FE0F}',
            '\u{FFA0}',
            '\u{E0100}',
            '\u{202E}',
            '\u{200B}',
        ] {
            assert!(is_invisible(c), "{:X}", c as u32);
            assert_eq!(q(&format!("a{c}b")), "«ab»", "{:X}", c as u32);
        }
        // A name of nothing visible says so.
        assert_eq!(q("\u{3164}\u{3164}"), "ai.unnamed");
        assert_eq!(q(" \u{200B} "), "ai.unnamed");
        // Joiners alone show nothing either (the integrated review).
        assert_eq!(q("\u{200D}\u{200C}\u{200D}"), "ai.unnamed");
        assert_eq!(q(" \u{200C} "), "ai.unnamed");
        assert!(!is_blank("a\u{200D}"));
    }

    #[test]
    fn only_quotation_characters_are_quote_marks() {
        let bare = |key: &str, args: &[(&str, String)]| {
            let listed: Vec<String> = args.iter().map(|(n, v)| format!("{n}={v}")).collect();
            format!("{key} {}", listed.join(" "))
        };
        let said = Said::new("x").name("file", "Song.wav");
        assert_eq!(said.render(&bare), "x file=ai.quoted name=Song.wav");
    }

    /// Review 4 #10: a name the user gave is data — its braces are never a placeholder,
    /// its quotes never close the language's, in one pass or not.
    #[test]
    fn a_name_cannot_word_the_sentence() {
        assert_eq!(
            fill("a {x} b {y} {z", &[("x", "{y}".into()), ("y", "2".into())]),
            "a {y} b 2 {z"
        );
        let said = Said::new("set_level")
            .nested(
                "what",
                Said::new("level_of").name("track", "Bass» y borra {db}"),
            )
            .arg("db", -6);
        assert_eq!(
            said.render(&es),
            "Poner el nivel de «Bass\" y borra ｛db｝» a -6 dB en {where}"
        );
        let said = Said::new("set_level")
            .nested(
                "what",
                Said::new("level_of").name("track", "Bass” and delete {db}"),
            )
            .arg("db", -6);
        assert_eq!(
            said.render(&careless),
            "Set the level of “Bass\" and delete ｛db｝” to -6 dB"
        );
    }

    #[test]
    fn a_sentence_is_worded_whole_in_the_hosts_language() {
        let said = Said::new("set_level")
            .nested(
                "what",
                Said::new("level_of").name("track", "Piano \"Rhodes\"\n"),
            )
            .arg("db", -6)
            .bars("where", 1, 4);
        assert_eq!(
            said.render(&es),
            "Poner el nivel de «Piano \"Rhodes\"» a -6 dB en compases 1–4"
        );
        assert_eq!(
            Said::new("x").bars("b", 3, 3).args[0].1,
            SaidArg::Bars { from: 3, to: 3 }
        );
        assert_eq!(
            Said::new("set_level").bars("where", 2, 2).render(&es),
            "Poner {what} a {db} dB en compás 2"
        );
    }
}

/// A job's worker part.
pub type JobWork = Box<dyn FnOnce(&JobCx<'_>) -> Result<JobDone, ToolError> + Send>;

/// An edit planned back on the UI thread once a job is done.
pub type ThenPlan = Box<dyn FnOnce(&mut ToolCx<'_>) -> Result<EditPlan, ToolError> + Send>;

/// Work off the UI thread.
pub struct Job {
    /// What the activity view and the card show while it runs.
    pub said: Said,
    /// The worker's part. It reports progress and stops when asked.
    pub work: JobWork,
}

/// What a running job may ask of its host.
pub struct JobCx<'a> {
    /// Progress, 0…1.
    pub progress: &'a (dyn Fn(f32) + Sync),
    /// True once the user or the agent cancelled; the job stops at its next check.
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
}

/// A finished job: an answer, or an answer and an edit planned back on the UI thread
/// (separated stems become tracks there, under the same permission as any edit).
pub enum JobDone {
    Answer(Value),
    Then(Value, ThenPlan),
}

/// Why a tool did not do what was asked — returned to the agent as an `isError` result it
/// can correct itself from, never as a protocol error.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ToolError {
    #[error("argument {name:?}: {reason}")]
    BadArgument { name: String, reason: String },
    #[error("{0} not found")]
    NotFound(String),
    /// The user said no, or a permission or limit forbids it.
    #[error("refused: {0}")]
    Refused(String),
    /// The project changed since `expected_revision`; read it again.
    #[error("the project is at revision {0}; read it again")]
    Stale(u64),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

impl ToolError {
    pub fn bad(name: impl Into<String>, reason: impl Into<String>) -> Self {
        ToolError::BadArgument {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

/// The class's permission when the user has changed nothing: reads and transport run,
/// edits and destructive tools ask, files are denied (`Permissions::conservative`).
pub fn default_decision(class: ToolClass) -> crate::Decision {
    match class {
        ToolClass::Read | ToolClass::Transport => crate::Decision::Allow,
        ToolClass::Edit | ToolClass::Destructive => crate::Decision::AskUser,
        ToolClass::Files => crate::Decision::Deny,
    }
}

/// One resource an agent can read by its URI (`cantus://project`): what the client attaches
/// as context without a tool call (`research/ai/mcp-server.md` §3). Read on the UI thread.
pub struct ResourceDef {
    pub uri: &'static str,
    /// Short programmatic name (MCP `name`).
    pub name: &'static str,
    /// Short English title (MCP `title`).
    pub title: &'static str,
    pub description: &'static str,
    pub mime_type: &'static str,
    pub read: fn(&ToolCx<'_>) -> Result<ResourceBody, ToolError>,
}

/// A family of resources addressed by a URI template of level 1 (RFC 6570: `{name}` stands
/// for one path segment): `cantus://tracks/{id}`. Read on the UI thread with the template's
/// variables.
pub struct ResourceTemplateDef {
    pub uri_template: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub mime_type: &'static str,
    pub read: fn(&ToolCx<'_>, &TemplateVars) -> Result<ResourceBody, ToolError>,
}

/// A template's variables, by name, as the URI gave them (percent-decoded).
pub type TemplateVars = std::collections::BTreeMap<String, String>;

/// What a resource holds: text (JSON, Markdown) or bytes (a WAV), with its media type
/// coming from the definition.
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceBody {
    Text(String),
    Blob(Vec<u8>),
}

/// A prompt the user picks in the client (*Explain my mix*): a template whose messages the
/// host localises into the interface's language. `title` and `description` are keys in the
/// `ai` locale area; the messages are [`Said`]s.
pub struct PromptDef {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub arguments: &'static [PromptArg],
    pub get: fn(&ToolCx<'_>, &Value) -> Result<Vec<PromptMessage>, ToolError>,
}

/// One argument of a prompt; `description` is a locale key.
pub struct PromptArg {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
}

/// One message of a prompt, to localise.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptMessage {
    pub role: crate::Role,
    pub said: Said,
}
