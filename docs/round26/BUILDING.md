# Round 26 — the beta ready in everything that needs neither the owner's hardware nor their paperwork

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
The ask is `docs/next-session.md`; this file turns it into five builders and an
integrator.

Read, in order: this file (your section completely); `CLAUDE.md`; `docs/STATUS.md`'s top
("The next session: what round 25 left" and the round-25 section); `docs/beta-checklist.md`;
`docs/BETA.md`; the documents your section names.

## The builders

| Builder | Branch / worktree | Delivers (the ask's item) | Builds Rust? |
|---|---|---|---|
| **public** | `r26/public`, `F:\projects\daw-r26\public` | 2 — the public repository prepared in a *copy*: history without third-party screenshots (and the RVR1960 text), a secret scan of the whole history, a report with what would change and the commands to do it for real | no (git, Python) |
| **release** | `r26/release`, `F:\projects\daw-r26\release` | 2b — cheaper GitHub Actions; 3 — the SignPath Foundation application and the code-signing policy page as drafts, and a verified list of their conditions; 4 — the release notes for rounds 23–25 in English and Spanish, a stable installer file name in the release, the website's button downloading it directly | little (xtask, web) |
| **mixer** | `r26/mixer`, `F:\projects\daw-r26\mixer` | 5 — round 25's small things (desk, route menu, inspector, look) | yes |
| **content** | `r26/content`, `F:\projects\daw-r26\content` | 6 — CC BY content in `cantus-content` (ADR 0042): manifest, receipt, credits views, the notices' *Content* section | yes |
| **linked** | `r26/linked`, `F:\projects\daw-r26\linked` | 7 — the rest of BACKLOG §6d.2: an Inspector row for a link group and a "Link to…" picker by name | yes |

The **integrator** (the session that wrote this) merges every branch into
`r26/integration`, runs adversarial reviewers on each, fixes what they find, makes CI
green on Windows **and** Linux (item 1: `tools/wsl/check.sh r26/integration clippy|test`
in WSL Ubuntu 24.04), writes STATUS, copies the memory to `docs/claude-memory/`, builds a
test version in `F:\projects\daw-r26\builds\`, and asks the owner before anything goes to
`master`.

## Rules (as in rounds 20–25)

- Your worktree and branch only (made for you from the contract commit). The untracked
  `.cargo/config.toml` is there: `[build] jobs = 3`, `debug = 0` and
  **`incremental = false`** for dev and test (incremental linking fails on this machine).
  **Never** touch `F:\projects\cantus` (the owner's tree, with the owner's uncommitted files),
  never push, never merge, never change a remote.
- **Commit WIP early and often** on your branch (sessions get cut by API errors; you will
  be resumed with your context). Write files in pieces under ~300 lines per tool call.
- Shell: Git Bash has no cargo on `PATH` — prefix `export PATH="$HOME/.cargo/bin:$PATH" &&`.
  A heredoc delimited by `EOF` fails to parse: use `XEOF` or the Write tool. UAC refuses
  test binaries named `*setup*` / `*update*` (os error 740): run a copy under a neutral
  name from the crate folder. The Zscaler proxy may answer a download with an HTML page for
  ~10 minutes: verify what arrives (a zip starts with `PK`, an exe with `MZ`), wait, retry;
  **never bypass the proxy or TLS checks**.
- **The base is fmt-clean and CI-clean** (master `78c8ff5`): run `cargo fmt --all` before
  your last commit; `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
  Code behind `#[cfg(windows)]` must not leave Linux dead-code warnings (CI is Linux too;
  `docs/DEVELOPING.md` §4) — gate it or `#[cfg_attr(not(windows), allow(dead_code))]`.
- **Tests first**, each seen failing before the fix. Every mutation is a `Command` (ADR
  0004); a new batch shape gets an arm in `crates/cantus-core/tests/undo_invariants.rs`
  (additive — say so in your report). No golden hash moves; **no schema change** this
  round (none of the items needs one — if you believe yours does, stop and say why in your
  report instead).
- Real-time rules (ADR 0005): the audio thread never allocates, locks, blocks or does I/O.
- Kit only in views (ADR 0024): no stock egui widgets, no literal colours, no
  `FontId::new`. **Look at what you build** with the `shoot` examples, at 75 % and 200 %,
  English and Spanish, in Cantus Night and one light theme; keep the PNGs out of the
  repository (`F:\projects\daw-r26\shots\<builder>\`).
- Strings: your locale areas only (below), both languages (the i18n test requires every
  English key in Spanish). Never edit `en.txt` / `es.txt` or `i18n.rs`.
- ADR numbers: **release 0048** (the CI budget and the release's stable file names),
  **public 0049** (going public: the history and what leaves it). The others amend the ADR
  of their feature (0037, 0042, 0043/0044, 0046) with a dated section; the integrator
  renumbers if needed.
- Finish: the crates you touched with `cargo test -p <crate> --no-fail-fast` (mixer,
  content and linked: `-p cantus-core -p cantus-project -p cantus-ui` at least, plus your
  own crates), clippy as above, `cargo fmt --all`; then `docs/round26/<name>.md` (done /
  measurements / tests and what each holds / files outside your area / left / notes for
  the integrator) and a short report as your final message.
- RAM: three Rust builders share a 31 GB machine. Do not start a second `cargo` while one
  runs; prefer `-p <crate>` over `--workspace` until the end.

## File owners (one owner per file; anything else: keep to an additive hook and say so)

| Owner | Files |
|---|---|
| **public** | `docs/round26/public.md`, `docs/adr/0049-*.md`, `tools/public/**` (new), the copy at `F:\projects\daw-r26\public-copy\` (not a worktree: a separate clone) |
| **release** | `.github/workflows/**`, `docs/releases/**`, `docs/signing/**` (new), `docs/adr/0048-*.md`, `web/**`, `xtask/src/package*.rs` and whatever xtask/installer file names the release asset, `docs/round26/release.md` |
| **mixer** | `crates/cantus-ui/src/views/{mixer*.rs,route_menu.rs,inspector_sends.rs,inspector_receives.rs,hint.rs,appearance*.rs}`, `crates/cantus-ui/src/routing_actions.rs`, `crates/cantus-ui/src/kit/cable.rs`, the theme-preview code in `kit/appearance.rs` (additive), `crates/cantus-ui/tests/{mixer_*.rs,frame_budget.rs,appearance*.rs}`, `examples/shoot_mixer.rs`, locale areas `routing.*`, `cables.*`, `mixer.*`, `look.*`; `docs/round26/mixer.md` |
| **content** | `crates/cantus-content/**`, `xtask/src/notices*.rs` (and the notices' generator wherever it lives), `THIRD-PARTY-NOTICES.md` (regenerated), `crates/cantus-ui/src/views/{about.rs,samples_*.rs}` and the *Sound packs* view wherever it lives, locale areas `about.*`, `samples.*`; `docs/round26/content.md` |
| **linked** | `crates/cantus-core/src/links.rs`, `crates/cantus-core/tests/links*.rs`, `crates/cantus-ui/src/links.rs`, the clip section of the Inspector (`views/inspector*.rs` files that draw a clip — not `inspector_sends.rs` / `inspector_receives.rs`), the clip context-menu lines in `views/arrangement*.rs`, a new `views/link_picker.rs` (declare it with `#[path]` from your own file, or one line in `views/mod.rs` — say so), locale area `links.*`; `docs/round26/linked.md` |

`app.rs`, `keymap.rs`, `views/mod.rs`, `kit/mod.rs`: one additive line each if you must
(a `mod`, an action arm, a key) — list it in your report.

---

## public — the repository made ready to be public, in a copy (item 2)

**The owner's words:** in a *copy* of the repository, take `research/*/screenshots/` out
of the history and (while there is no permission yet) the RVR1960 file
`crates/cantus-ui/locales/verses/es.txt`; search the whole history for secrets; leave a
report with what would change and the commands to do it for real. **No forced push, no
change to the real repository**: the history stays as it is until the day the owner
decides to make it public.

1. Make the copy: `git clone --no-local F:/projects/cantus F:/projects/daw-r26/public-copy`
   (a fresh clone, as `git filter-repo` wants), then remove its `origin` so nothing can be
   pushed from it. Work only there and in your worktree (for the report and scripts).
2. **Inventory first** (the report's first table): every path that ever held a
   third-party image (`git log --all --name-only --diff-filter=A` for images; renames
   too — `research/*/screenshots/` may have lived elsewhere before), every place the
   RVR1960 text was ever committed (the file, its old paths, any doc or test that quotes
   verses — search blob contents for distinctive RVR1960 phrasings; RV1909 is public
   domain and stays), anything else not ours that should not be published (SDK headers,
   fonts or samples with restrictive licences, other products' manuals or logos).
3. **Secrets**: scan every commit of every ref — a maintained scanner (gitleaks or
   trufflehog, a release binary or `pip`, verified through the proxy) **and** a targeted
   search (private keys, `.pfx`/`.p12`, tokens: GitHub `ghp_`/`github_pat_`, Cloudflare,
   AWS, Azure, signing passwords, `.env` files, wrangler secrets). Also what is private
   without being a secret: author/committer e-mails in commits, local paths
   (`C:\Users\...`), the owner's personal data in docs — list it and say whether it
   matters, with options (e.g. a `.mailmap` or rewriting to the GitHub no-reply address —
   the owner's choice, not yours).
4. **Rewrite the copy** with `git filter-repo` (install it the safest way you can verify:
   `pip install --user git-filter-repo`, or the single script from its GitHub release,
   checked): remove the screenshots and the RVR1960 file from every commit, then verify —
   no such path in any commit (`git log --all -- <path>` empty), the working tree of
   `master` still builds and its tests that read the verse files still pass (the RV1909
   default; the RVR1960 gate `verse::RVR1960_PERMITTED` is false, so nothing should need
   the file — prove it: `cargo test -p cantus-ui verse` in the copy, with a
   `.cargo/config.toml` like your worktree's), the size before and after, the commit
   count, which refs exist and which would be published (master and the tags, not the
   `r*/` branches — say why).
5. **The report** `docs/round26/public.md` — a Spanish summary at the top for the owner,
   then the detail: the inventory, the scan results (every finding with its commit and
   what to do), what changes (every commit hash; tags; the worktrees and the release
   workflow's tag), the **two routes** with their commands, exactly, in order — (a) the
   rewritten history pushed to the public repository, (b) a fresh one-commit history —
   and your recommendation; what to do in the tree on that day (the screenshots leave the
   tree too and go into `.gitignore`, the README's links, anything that names them); and
   what can be checked again with one command on that day (`tools/public/check.sh`: the
   scan and the path checks, read-only, runnable on any clone).
6. ADR 0049 *Going public: what leaves the history* (Context / Decision / Consequences /
   what the checks hold) — the decision is the plan, executed only when the owner says.

You change nothing outside `docs/round26/public.md`, `docs/adr/0049-*.md` and
`tools/public/`. If you find a real secret, stop rewriting and report it first.

---

## release — CI budget, signing, release notes, the download button (items 2b, 3, 4)

### 2b. Cheaper GitHub Actions

The private repository's minutes ran out on 2026-10-13 (Windows counts ×2, macOS ×10).
In `.github/workflows/ci.yml`: **macOS only by hand (`workflow_dispatch`) or once a
week** (`schedule`); **no run when a commit changes only documents** (`paths-ignore`); a
**`concurrency`** group that cancels the old run of the same ref when a new one arrives.
Careful with `paths-ignore`: some tests read Markdown (`THIRD-PARTY-NOTICES.md`'s check, the
manual, the licences, `docs/` fixtures?) — find every test that reads a non-code file
(`include_str!`, `fs::read*` of `*.md`/`docs/`) and keep those paths triggering. Look at the
other jobs with the same eye (the perf job reports and never fails; `cargo install
cargo-audit` compiles every run; the null-hash jobs build three times): propose and apply
what keeps the same guarantees for fewer minutes, with an estimate of minutes per push
before and after in ADR 0048. macOS was left unvalidated in CI after its last fixes: say
how to run it by hand, and if this machine can check the macOS target in part (a
`cargo check --target aarch64-apple-darwin` of the pure-Rust crates, as an earlier round
tried in `F:\projects\daw-r23\target-mac`), do it and report what it shows. Validate the
YAML (`actionlint` if you can fetch it verified; otherwise a careful read and a YAML parse).

### 3. Code signing — the SignPath Foundation application and the signing policy

`docs/beta-checklist.md` §2 lists the conditions as understood on 2026-10-11; read
SignPath Foundation's current terms and application form (signpath.org) yourself and
verify **each** condition against the repository, with evidence: OSI licence (ADR 0047:
MPL-2.0 engine, GPL-3.0-or-later app); **no commercial dual licensing** — check
`CLA.md`, `TRADEMARKS.md`, ADR 0047 and BACKLOG §6f (and an untracked `docs/monetize.md`
the owner keeps in their own tree — do not copy it; if a clause lets the owner sell a
commercial licence, say so plainly: it is a condition SignPath may refuse); no
proprietary components in what is signed (the ASIO SDK's licence as we use it, fonts,
the RVR1960 gate, bundled content); builds automated and verifiable (`release.yml`);
the project's maturity/activity requirements; the team roles and MFA they ask for; the
privacy statement; the policy page. Then write, as **drafts only** (nothing published,
nothing in `web/` that the site would deploy):

- `docs/signing/signpath-application.md` — every field of their form filled in from the
  repository, in English, with the owner's decisions left as marked blanks.
- `docs/signing/code-signing-policy.md` — the page SignPath requires on the project's
  website (their template's sections: what is signed, who approves, privacy, the
  team), ready to become a page of the site when the owner says.
- `docs/signing/README.md` — Spanish summary for the owner: what was verified, what
  fails or is unclear, what the owner must decide, the steps after approval (the `release.yml`
  change to call SignPath — you may draft it as a commented block or a separate
  disabled workflow, inert until the secrets exist).

### 4. Release notes and the direct download

- `docs/releases/v0.1.0-beta.1.md` brought up to date with rounds 23–25 (STATUS sections,
  `docs/round23..25/`), **in English and Spanish** (one file with both, or `.es.md`
  beside it — follow what exists), written for musicians, not for us; the known
  limitations honest.
- The release publishes the installer under a **stable file name** too (e.g.
  `Cantus-Setup-x64.exe`, besides the versioned one if the updater or the checksums need
  that — read `release.yml`, `xtask` packaging and the updater's manifest before choosing,
  and keep the updater working); the website's download button points at
  `https://github.com/<owner>/<repo>/releases/latest/download/<stable name>` instead of
  the releases page. A test (or xtask check) that keeps the name in the workflow, the
  packager and the site equal. Build the site (`web/`, trunk) or at least `cargo check`
  it for wasm if the toolchain is there; note in your report that the link works only
  once the repository and its releases are public, and what the site shows meanwhile.
- `docs/round26/release.md`: the report.

---

## mixer — round 25's small things (item 5)

From STATUS "Small things left", `docs/round25/integration.md` "What is left",
`docs/round25/look.md`, `docs/round25/cables-view.md` and `docs/beta-checklist.md` §5.
Each one with a test seen failing first, and pictures:

1. **An armed route survives the desk being hidden** (click mode, ADR 0044 §10): when the
   desk is hidden the armed route lets go (Esc already does), **but a selected cable is
   kept** — the reason the first attempt was dropped. Find the clean signal ("the desk
   was hidden": not drawn this frame / its pane closed or switched) rather than a side
   effect.
2. **Typing a level in the route menu**: *Level…* as a field (dB, the kit's numeric entry,
   Enter commits as one undo step, Esc cancels, out-of-range clamps, Spanish decimal comma
   accepted if the kit's parsers accept it elsewhere).
3. **The inspector's *Add send* offers *New return ▸*** — the same menu as the desk's
   (`route_menu::open_add_send`), one code path.
4. **Steady cable lanes**: hovering a strip lights its routes but does not re-lane — lanes
   come from the selection (and the view option), never from the hover, so an end mark
   never moves under an approaching pointer. Test it the way `docs/round25/desk.md`
   tested lanes.
5. **The hint bar's ~17 allocations a frame** while a desk control is hovered: down to 0
   (or say exactly what is left and why), measured by the frame-budget test, which then
   holds the new number.
6. **Swatches in the Look tab's base-theme menus**: each theme's row carries its preview
   swatch, as the Appearance page's rows do (painted with that theme's tokens; no
   literal colours).
7. **The Spanish legend says SALIDA like the wide mode's SALIDA row**: reword so the two
   never read the same (check every locale string of the band and the strip for the same
   kind of collision, in both languages).
8. **The cable legend is invisible when the band is at its least** (1280×720, 20 pt): a
   compact legend that fits (or a key that opens it), never overlapping; test at 1280×720
   and 200 %, English and Spanish.

Amend ADR 0043 / 0044 / 0046 where a rule changed. Report: `docs/round26/mixer.md`.

---

## content — CC BY in `cantus-content` (item 6, ADR 0042)

ADR 0042 is the spec; its "What the tests hold" is your test list. In short:

- The manifest accepts exactly `CC0-1.0`, `CC-BY-3.0`, `CC-BY-4.0`; a CC BY pack needs a
  **credit line** (author, title, licence, link — as the licensor asks) and a licence
  file; anything else fails the manifest test (NC, ND, SA, retired licences, unknown ids).
  Replace `is_cc0` with what the rule now is; keep old receipts loading (serde defaults).
- The **receipt** carries the credit.
- ***Sound packs…*** shows the credit before and after the download, and ADR 0042 §4's
  plain words (the pack is CC BY; music made with it is yours; quoting the author's words
  about crediting when the manifest has them).
- ***About ▸ Credits*** lists the content's credits (every pack in the manifest, CC0
  ones as a courtesy line, CC BY ones with their full credit), beside the Bible
  translations' credits already there.
- `THIRD-PARTY-NOTICES.md` gets a ***Content*** section generated from the manifest by
  `cargo run -p xtask -- notices`, and the notices check fails when a CC BY pack's credit
  is missing from it.
- Tests with a fixture manifest containing a CC BY pack (the real manifest need not gain
  one this round; if you think the Salamander Grand should be added, write how in your
  report — its size, URL, hash — do not download gigabytes).
- Pictures of *Sound packs* and *About ▸ Credits* (75 %, 200 %, both languages).
- Amend ADR 0042 with "What landed (round 26)". Report: `docs/round26/content.md`.

---

## linked — the rest of §6d.2 (item 7, ADR 0037)

ADR 0037's "Left for later": *an inspector row naming the group; a "Link to…" picker by
group name.* Read ADR 0037 and `docs/round24/linked.md` first.

- **The Inspector row for a link group**: when the selected clip is linked, a row says so
  — the group's name (the shared clip name), the count (×N), and its actions (*Select
  linked copies*, *Make unique*, *Link to…*); when not linked, the row offers *Link to…*
  only. Renaming from there renames the group (one `SetClipName`, every sibling, one undo
  step — `Command::apply` already reaches the siblings).
- **"Link to…" by name**: a picker listing the project's link groups (and single clips of
  a compatible kind, if that reads well — decide and say why) by name, with the count, the
  kind (notes / audio / automation) and a filter field; incompatible targets shown
  disabled with the reason (`links::compatible`); choosing one links the selection with
  `links::link_to` in one undo step. From the clip context menu, the Inspector row and a
  key or action in the keymap's clip section if one fits. Two groups with the same name
  must stay distinguishable (a track/position hint).
- A helper in `cantus-core::links` for the groups list (name, kind, count, a stable order)
  with tests; no schema change (the name is the shared `ClipContent::name`).
- New command shapes (if any) get an undo-fuzz arm.
- Pictures (75 %, 200 %, both languages, one dark and one light theme).
- Amend ADR 0037 with a dated section. Report: `docs/round26/linked.md`.
