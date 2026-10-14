# Round 26 — release: CI budget, signing, release notes, the download button

Builder **release**, branch `r26/release` (worktree `F:\projects\daw-r26\release`, from
the contract a3b6355). Items 2b, 3 and 4 of `docs/next-session.md`. ADR **0048**.

## Resumen (para el dueño)

- **GitHub Actions, ≈ 80 % menos por push de código y nada por un push de documentos.**
  macOS sale del CI de cada push: corre a mano o los lunes si hubo código esa semana; los
  null hashes solo cuando cambia el lado DSP/motor; `cargo audit` cuando cambian las
  dependencias y cada semana; perf a mano; `concurrency` cancela la corrida vieja; tiempos
  límite en todos los trabajos. Estimado: ≈ 590 minutos facturados por push → ≈ 110 (código),
  ≈ 190 (motor), 0 (solo documentos), más ≈ 300 a la semana si se usa el macOS semanal. La
  solución de fondo es hacer público el repositorio (los runners son gratis).
- **SignPath: hoy no se puede pedir** — repositorio privado, sin versión publicada, sin
  reputación pública. Y **el CLA reserva una licencia comercial**, algo que SignPath
  rechaza («sin licenciamiento dual comercial»): hay que decidir (ver
  `docs/signing/README.md`). Tu plan de monetización no vende licencias y sí es compatible.
  La solicitud y la página de política están redactadas, sin enviar ni publicar.
- **Notas de la versión** al día con las rondas 23–25, en inglés y español.
- **El botón de la web descarga el instalador directo**:
  `…/releases/latest/download/Cantus-windows-x64-setup.exe`; cada versión publica el
  instalador también con ese nombre fijo. Funciona cuando el repositorio sea público (hoy
  da 404, igual que el enlace anterior).
- **El versículo de la web y de las notas en español ahora es de la RV1909** (dominio
  público), como la app mientras no llegue el permiso de la RVR1960: los términos de la
  ABS piden permiso también para sitios web. Volver a la RVR1960 es cambiar dos líneas
  cuando llegue el permiso.

## Done

### 2b. Cheaper GitHub Actions (ADR 0048 §1–§6)

| File | What |
|---|---|
| `.github/workflows/ci.yml` | Linux + Windows only; `paths` with `!` patterns (documents skip, documents a test reads still trigger); `concurrency` cancel-in-progress; `timeout-minutes: 90`; the pinned toolchain instead of `dtolnay@stable`; fmt and the release build once, on Linux; `CARGO_PROFILE_RELEASE_DEBUG=0`; `rust-cache` saves only from master; `workflow_dispatch` |
| `.github/workflows/macos.yml` (new) | by hand, or Mondays 06:17 UTC when a one-minute Linux job finds code commits in the last eight days; clippy, the suite, both fuzz runs |
| `.github/workflows/null-hashes.yml` (new) | the two null-hash jobs and the Windows = Linux compare, only on changes to the crates the corpus renders, the demos and presets, lock file, toolchain — or by hand |
| `.github/workflows/audit.yml` (new) | on `Cargo.lock`/`Cargo.toml` changes, weekly and by hand; `cargo-audit` cached in `~/.cargo/bin` |
| `.github/workflows/perf.yml` (new) | by hand only (it never failed a build) |

How to run macOS by hand: *Actions ▸ macOS ▸ Run workflow* (choose the branch), or
`gh workflow run macos.yml --ref <branch>`. **Do it before tagging `v0.1.0-beta.1`.**

### 3. Signing (drafts only)

`docs/signing/README.md` (Spanish: every SignPath condition with evidence, the decisions,
the steps), `signpath-application.md` (every field of their current form, read from the
form's own definition), `code-signing-policy.md` (the page their terms require, with where
each line must go), `signpath-release.draft.yml` (the release job with two SignPath
signing requests; inert, in `docs/`, not `.github/`). Nothing in `web/` and no page added.

### 4. Release notes and the direct download (ADR 0048 §7)

- `docs/releases/v0.1.0-beta.1.md`: copy and paste, linked clips, mixer cables, pre/post
  sends, the routing matrix, moving strips and groups, the export fixes (ranges, tails,
  stems with plug-ins), background save, level-matched instruments, the two-hour run,
  twelve themes and the icon colour; known issues revised (stems now include hosted
  plug-ins; Lamina ~4 dB, not 6–8; the count-in works without the metronome; the looping
  split, automation paste, master-chain stems added); the stable installer name; both
  languages, names as the app shows them (Cantus Noche, Hacer único…).
- `cantus_update::release::STABLE_INSTALLER_NAME` = `Cantus-windows-x64-setup.exe`;
  `xtask package` copies the signed installer to it (same bytes, in `SHA256SUMS.txt`); the
  updater is untouched (it asks for the versioned name).
- `release.yml`: checks the stable file before publishing; a `-beta` tag is published as
  **latest** while no stable release exists (GitHub's `releases/latest` never names a
  pre-release, so the button would have found nothing), as a pre-release once one does.
- Website: `web/src/top.rs::DOWNLOAD`; the Windows card's button *Download the installer*
  / *Descargar el instalador* on it, and *All downloads, release notes and checksums*
  under it; the roadmap columns brought up to date (cables, linked clips, copy and paste,
  the soak and the new look were still listed as "next"/"later"); the motto in Spanish
  from the RV1909.

## Measurements

- Minutes per push (estimates; ADR 0048 has the table and its assumptions):

  | Push | Before | After |
  |---|---|---|
  | documents only | ≈ 590 | 0 |
  | code (UI, app, host…) | ≈ 590 | ≈ 110 |
  | DSP/engine, demos, presets, lock file | ≈ 590 | ≈ 190 |
  | weekly fixed | 0 | ≈ 300 (macOS, only in weeks with code changes) + 2 |

- macOS from this machine (`aarch64-apple-darwin`, `cargo clippy --all-targets`,
  `-D warnings`): **clean** for cantus-core, -dsp, -project, -rt, -launch, -engine,
  -update, -ai, -host. cantus-content and cantus-plugins (`ring`), cantus-export, -ui,
  -app (`ogg_next_sys`) stop at C build scripts (no cross C compiler) — they and every
  test need the runner.
- `actionlint` 1.7.12 (Windows zip, SHA-256 checked against the release's checksums file
  and GitHub's digest): **0 findings** on all six workflows (shellcheck not installed).
- Site: `trunk build --release` (trunk 0.21.14 from the build-tools cache): success; the
  wasm contains the new link; headless-Edge pictures of the download card, the roadmap and
  the verse in English and Spanish in `F:\projects\daw-r26\shots\release\`.

## Tests and what each holds

| Test | Holds | Seen failing |
|---|---|---|
| `xtask` `package::ci_tests::the_stable_installer_name_is_the_same_in_the_packager_the_workflow_and_the_site` | `release.yml` checks `dist/<stable>`; `web/src/top.rs` links `https://github.com/<updater REPO>/releases/latest/download/<stable>`; the name carries no version | yes (before the constant, the workflow check and the site link) |
| `package_tests::the_package_task_produces_the_expected_file_list` | the release's file list includes the stable name; it differs from the versioned one | yes |
| `package_tests::an_installer_compiles_when_nsis_is_here` | (with NSIS) the stable file is byte-identical to the versioned installer and listed in the sums with the same hash | NSIS absent here: skipped |
| `package::ci_tests::the_glob_reads_patterns_as_github_does` | the test's glob matches GitHub's `*`/`**` | — |
| `package::ci_tests::ci_runs_for_code_and_for_what_tests_read_but_not_for_documents_alone` | push = pull_request lists; code, the manual, notices, privacy, top.rs, release.yml, locale and fixture files trigger; docs/research/root `*.md`/web/tools/other workflows alone do not | yes (`THIRD-PARTY-NOTICES.md` removed from the list) |
| `package::ci_tests::every_file_compiled_in_runs_ci_when_it_changes` | every `include_str!`/`include_bytes!` target in crates/, xtask/, third_party/ triggers CI (> 50 found) | — |

Suites run: `cargo test -p xtask` 23/23; `cantus-update` 10 + 18 (run copied to a neutral
name, UAC); `cargo clippy -p xtask -p cantus-update --all-targets -D warnings` clean;
`cargo fmt --all` (it reformatted two of my test files, committed).

## Follow-ups (the integrator's four, same day)

1. **Stale documents**: the manual's *Known issues* (en, es) now match the release notes
   (hosted plug-ins' automation and master-chain stems, the looping split, automation
   paste and Ctrl+V with nothing copied, Lamina ~4 dB, plug the interface in first; the
   count-in line removed — it works without the metronome since round 22);
   `docs/perf/benchmark.md` §6 says perf runs by hand; `docs/beta-checklist.md` §3 has the
   release notes and the direct download ticked and points at `docs/signing/`.
2. **Version resources (SignPath's metadata rule)** — ADR 0048 §9. `crates/cantus-host/build.rs`
   (the scanner) and `crates/cantus-plugins/build.rs` (each plug-in library, described by its
   full name from the one effect feature switched on), with `winresource` as in
   `cantus-app/icon.rs`. `xtask/src/package_meta.rs` reads each file's `StringFileInfo`;
   `xtask package` refuses a release whose signed files (and installer) lack product
   *Cantus* or the version. Windows reads them back: `Cantus Aequalis.clap` / `.vst3` →
   *Cantus · 0.1.0-beta.1 · Cantus Aequalis (CLAP and VST3 plug-in)*, `cantus-scan.exe` →
   *Cantus · 0.1.0-beta.1 · Cantus plug-in scanner*; the app's programs already had theirs.
   The NSIS uninstaller is not checked.
3. **`webpki-roots` removed** — `cantus-content`'s ureq features are now
   `rustls-no-provider`, `_ring`, `platform-verifier` (the Mozilla list was compiled in and
   never read: the agent asks the platform verifier). Gone from every target (`cargo tree
   --target all`). A real HTTPS download of the starter pack through the Zscaler proxy
   passes on Windows; in WSL Ubuntu 24.04, with CI's flags, clippy and the tests of
   `cantus-content` and `cantus-update` pass (Linux uses rustls with the platform verifier
   for content — the system store — and OpenSSL through native-tls for the updater, as
   before). Notices regenerated (296 crates). `_ring` is a ureq feature named with an
   underscore (marked unstable by ureq); the lock file pins ureq, and a rename would fail
   the build, not the program.
4. **egui's default fonts compiled out** — ADR 0048 §10. They were never used: the chains
   kept only faces named `…emoji…`, which matched `emoji-icon-font` alone (`NotoEmoji` has a
   capital E). `default_fonts` is off for egui and eframe; `emoji-icon-font.ttf` (MIT) is
   bundled in `crates/cantus-ui/assets/fonts/` with egui's 0.9 scale; every chain is one
   Plex face, then the emoji face — the same as before. Proof:
   - `crates/cantus-ui/tests/font_coverage.rs` reads the character maps (skrifa, the parser
     egui uses — `has_glyph` cannot be used: egui's own TODO, it reports false whenever the
     replacement glyph lives in the primary face) and checks every character of every
     locale file and every non-ASCII character or `\u{…}` escape in `cantus-ui/src`
     against each family's chain; and that no egui default font is built in.
   - Pictures before and after (`F:\projects\daw-r26\shots\release\fonts-before|after\`):
     `shoot_views` 36/36, `shoot_verse` 38/38 (English, RVR1960, RV1909; 75–200 %),
     `shoot_mixer` 11/12 byte-identical; the twelfth differs between two full runs of the
     same build and is byte-identical to the before-picture when rendered alone (twice).
   - **Found** (closed in the second follow-up below): nine characters have no glyph in any chain today (and
     had none before — egui's fonts were not in the chains): `▸` in three messages
     (devices, recovery, verse-hidden), `●` in Aequalis's *Listening*, `Δ` in the
     limiter's and maximizer's delta (condensed faces only), `⁻` in Modulus's graph, `τ`
     on Organum's cabinet; `♯ ♭ ▼ ▾` appear only in parsing or attributes. They draw egui's
     replacement glyph. The test lists them (`KNOWN_GAPS`, which must shrink when one is
     filled). Filling them is a design choice: a symbol face in the chain (Noto Sans
     Symbols 2, OFL) or kit vector glyphs or plain words.
   - The notices print the bundled fonts' licence files (`LICENSE-OFL.txt`,
     `LICENSE-emoji-icon-font.txt`) through a `<!-- font-licences -->` marker in
     `xtask/notices/head.md`; the Fonts table lists only Plex and emoji-icon-font.

   - Suites after all four: `cargo test -p cantus-ui` 847 passed (81 binaries),
     `-p cantus-plugins` 37, `-p xtask` 27, `-p cantus-content` and `-p cantus-update` (run
     under a neutral name); clippy `-D warnings` clean on cantus-ui, -content, -host,
     -plugins, -update and xtask; in WSL (CI's flags) clippy of cantus-content, -update,
     -host, -ui, xtask, -plugins (with and without the editor) and the tests of
     cantus-content, -update, xtask and `font_coverage` pass; `notices --check` passes.

## The glyph gaps closed (second follow-up)

`tests/font_coverage.rs` now reads **drawn text only**: locale lines that are not `#`
comments, and in the `cantus-ui` and `cantus-plugins` sources the string literals and the
char literals that are not match patterns (a pattern such as `Some('♯') | … =>` is a
parsing table: input, never shown). Comments and attribute arguments (`reason = "…"`) are
left out. It allows **no gap** and failed on the five drawn ones first; a second test pins
what the scanner takes and leaves. After the merge with the integration branch, a range's
bounds are left out too (`'\u{300}'..='\u{36f}'`, the link picker's accent filter in
`links.rs`: compared, never drawn); the scanner's test holds `..=`, `..` and open ranges.

| Was | Where | Now |
|---|---|---|
| `▸` | devices (*Driver ▸ ASIO*), recovery (*File ▸ Open*), the hidden-verse splash (*Settings ▸ General*), en and es | `>`, as the other strings write menu paths |
| `●` | `eq.match_listening` | that key and its twin `eq.match_paused` were drawn nowhere: removed from both locales |
| `Δ` | Terminus's and Culmen's delta-listen key (the condensed faces lack Greek) | *Delta* in both languages; the limiter's key widened 44 → 64 pt so the word and its light fit |
| `z⁻¹` | Modulus's back-edge pill | the index alone (the dashed line marks a back edge); the graph's hint says what a dashed line means, en and es |
| `τ` | Organum's rotor ramp label | *Horn: 63 % in 0.32 s* / *Bocina: 63 % en 320 ms* (`organ.ramp_point`), the label box widened |
| `♯ ♭ ▼ ▾` | a note-name parser, an attribute, comments | not drawn: left out by the scanner's rule, not by a list |

Pictures before/after in `F:\projects\daw-r26\shots\release\gaps-before|after\`: a new
`examples/shoot_glyphs.rs` (Modulus with a back edge, Organum's Edit page, Terminus,
Culmen; maximised; 75 % and 200 % on the same 1600 × 1000-point window; en and es), a
`hidden` case in `shoot_verse`, `shoot_audio_setup drivers`. Seen: the replacement box gone
in each, *Delta* and the organ labels whole. The `shoot` example's instrument scenes draw
the mixer without the editor (pre-existing); `shoot_glyphs` does this check instead.

## The review's 18 findings (third follow-up)

| # | Fix |
|---|---|
| 1 | `ci.yml` excludes only the root's `*.md`; Markdown under `crates/` and `xtask/` is code (`head.md`). The guard test also finds rustfmt-wrapped `include_str!(` and run-time reads relative to the workspace or crate; it failed on `head.md` first. `macos.yml`'s pathspec uses `:(exclude,glob)*.md`. ADR §2 corrected |
| 2 | Weekly macOS compared with the commit the last successful `macos.yml` run on master tested (`gh run list`, `actions: read`), not by commit date |
| 3 | `windows-release.yml`: `cargo build --release -p cantus -p cantus-launch` on Windows when those crates, the manifests, the lock file or the toolchain change (≈ 25 billed minutes then, instead of 24 on every push); ci.yml's comment and ADR §6 name `windows_subsystem` and the `cfg!(debug_assertions)` branches |
| 4 | `release.yml`: only HTTP 404 from `releases/latest` means "no latest"; anything else fails the job |
| 5 | `third_party/asio-sys/build.rs` compiles only `common/asio.cpp`, `host/asiodrivers.cpp`, `host/pc/asiolist.cpp` (no Microsoft COM-server files, no driver side; `ASIOConvertSamples` unused). Proved: the app builds with `--features asio` and `ASIO_SYS_REQUIRE_SDK=1`; `libasio.a` holds those three objects and `helpers.o`; `cantus --audio-drivers` → `WASAPI, ASIO`, `asio sdk: linked`; asio-sys's `enumerate` example loaded ASIO4ALL and FL Studio ASIO (software drivers; no hardware driver installed) and read their formats. `third_party/README.md` and signing README row 4 say exactly this |
| 6 | Application: Project Name **Cantus** (the binaries' product name), identified by the homepage cantusdaw.com, the tagline and the description; the policy page already says Cantus |
| 7 | Signing README row 18 and the draft: the job that submits to SignPath, and whatever builds what it signs, runs without `rust-cache` |
| 8 | LAME: LGPL-2.0-or-later (LAME 3.100; the crate's metadata says LGPL-3.0) |
| 9 | Linked audio copies share gain, fades and reverse (not pitch/effects): notes and manual, en/es |
| 10 | The soak as it was: a two-hour automated session (editing, recording, saving; no leak, no stall); the live-interface run still to come |
| 11 | Manual (en, es): PRE/POST sends, cables, the routing matrix, `Shift+F10`, linked copies and *Copies are linked*, the twelve themes, *Extra large*'s chain line; shortcuts `Alt+F9`, `Shift+F10`, `Alt+U`, `Alt+L`, `Shift+Alt+L`, `Alt+N`, checked against `keymap.rs` on `r26/integration` (file version 7), read only |
| 12 | Site: nine instruments and twelve effects; the effects section says "seven of the twelve" and names the delay and reverb |
| 13 | "The window never stops" softened; a known issue for a first save or *Save As* copying imported audio |
| 14 | Automation clips paste only within their project *while Cantus stays open* (notes, both manuals) |
| 15 | "suene al nivel de los demás" |
| 16 | *«Windows protegió su PC»* (notes and Spanish manual) |
| 17 | Roadmap: "Signed downloads, so you know the file is ours" / "para saber que el archivo es nuestro" |
| 18 | "Windows 10 untested" everywhere: notes, both manuals, the site's download card, hero and requirements |

## Closing the "Left" list (fourth follow-up)

1. **The uninstaller and signing.** The PFX, store and Azure routes already sign the NSIS
   uninstaller inside makensis (`!uninstfinalize` → `xtask sign-file`; the Azure variables
   are inherited from the job). SignPath cannot, so the staged packaging is built (ADR 0048
   §8): `package --stage-only` with an uninstaller-only NSIS pass (the script compiled with
   `UNINSTALLER_ONLY`, carrying no release file; its `.onInit` writes `uninstall.exe`
   beside itself), `package --from-stage <dir>` (the installer carries the signed
   uninstaller via `PREBUILT_UNINSTALLER`), and `xtask sums <dir>`. They are wired into the
   SignPath draft and stay inert until it is configured. The uninstaller's version resource
   is checked on every route (`sign-file`, `--stage-only`, `--from-stage`); NSIS gives it
   the installer's. Tested with the real NSIS 3.11 (fetched, SHA-256 equal to
   `release.yml`'s pin, into the build-tools cache).
2. **The installer's privacy page** (ADR 0048 §11): after the folder page, what Cantus
   sends and *Check for updates once a day (recommended)*. The box is preselected from
   Cantus's own `settings.json` (`"check_daily": false` → unticked). The choice goes to
   `%LOCALAPPDATA%\Cantus\installer-choices.txt` and Cantus applies it once at its next
   start (`settings::apply_installer_choices`); silent installs and updates change nothing.
   Strings `update.installer.privacy_*` in both languages; `PRIVACY.md` mentions the page.
   The NSIS test compiles it in both languages. **Not looked at on screen**: running the
   installer would write to this machine's profile and registry. Look at it in the owner's
   clean-machine installer run (beta checklist §1).
3. **The site's hero line**: "Windows 11 · app GPL-3.0-or-later · engine MPL-2.0" /
   "app GPL-3.0-o-posterior · motor MPL-2.0" (screenshot `hero-es.png`).
4. **`web/` formatted** (`cargo fmt` in that workspace), and `site.yml` checks it
   (`cargo fmt --check`, before the build).
5. **The Spanish linker message** is rustc's own filter, which hides MSVC's English
   "Creating library" line only; a note in `docs/DEVELOPING.md` §4.

## Shots: instrument scenes and the first frame (fifth follow-up)

- `raster::shoot*` runs one empty frame before the drawn ones: until a frame has run at
  the new scale egui keeps its provisional 5000-point screen, and a window first shown
  then took its place off the real one at 75 % and 200 % (effect windows cut off at the
  right; the content builder had worked around it per scene). Sound packs, the export
  dialog, Recover your work, Settings, About and Terminus checked after it.
- `shoot`'s instrument scenes (Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum,
  Pulsus) drew the mixer and the floating windows only, but an instrument editor is a
  docked tab: they now draw the whole window, light or dark from the app's appearance;
  `CANTUS_SHOOT_LANG=es` for Spanish. All 64 pictures looked at (contact sheets in
  `shots/release/shoot-fix/`).

## The second review's seven findings (sixth follow-up)

| # | Fix |
|---|---|
| 1 | `--from-stage` turns the stage folder into an absolute path first (`package::stage_dir`, a clear error when it is not a folder); test `a_relative_stage_folder_becomes_absolute` |
| 2 | `sign-file` checks the version the job builds: `package_staged` gives makensis `CANTUS_SIGN_VERSION`, else the release's own (`package::sign_version`; test `sign_file_checks_the_version_being_built`) — strict for releases, right for the NSIS tests' 0.2.0 |
| 3 | `--from-stage` checks the Authenticode signature of every program, plug-in library and the uninstaller before it builds anything (`sign::check`, which names the file that is not signed); it refuses to run off Windows |
| 4 | asio-sys keeps a stamp (`libasio.files`: the SDK folder and the file list) beside the library and rebuilds it when the stamp differs; the host files and `helpers.cpp` are `rerun-if-changed` on every run. Proved: a stale stamp rebuilt the library (three SDK objects and the glue), and `--audio-drivers` still says `WASAPI, ASIO` |
| 5 | Modulus: a back edge's number carries a return mark (the kit's ↶ glyph, in the instrument's hue), and its hover hint says "from a later operator: heard one sample late" (`fm.edge.back_tip`, en/es); the graph's own hint is short again. `shoot_glyphs fm-hover` shows the mark and the hint line |
| 6 | Organum: the ramp's key (a ring in each rotor's colour, then *Horn: 63 % in 320 ms*) sits in the corner the curves leave empty — top right while they fall, bottom right while they rise — never on a curve. `shoot_glyphs organ` and `organ-fast` use a taller window that reaches the card; looked at in both states, 75 % and 200 %, en and es |
| 7 | `font_coverage` also reads `cantus-core`, `cantus-content`, `cantus-host` and `cantus-export` sources (their unit-test modules skipped: test data and messages) and every string of the content manifest; all covered |

## Names in any script: the system font fallback (seventh follow-up)

`crates/cantus-ui/src/font_fallback.rs`: after the Plex + emoji chain, the operating
system's own faces. Windows: Yu Gothic, Meiryo and Microsoft YaHei (CJK), Malgun Gothic
(Hangul), Nirmala UI (Indic), Leelawadee UI (Thai), Segoe UI (Greek, Cyrillic, Hebrew,
Arabic), Segoe UI Symbol and Emoji, all from `%WINDIR%\Fonts`. Linux: Noto/DejaVu where
installed. macOS: the system's.

- **Never bundled or distributed**: they are read at run time, so no licence of ours is
  involved (ADR 0048 §10; the notices' Fonts section says so).
- **Lazy, per character, off the interface thread**: nothing is read until the kit lays
  out a character no bundled face has, or the user types, pastes or composes one with an
  input method (the preedit and the commit, how CJK is typed). The hook is in the kit's
  galley cache and its layout helper, plus the frame's input events. On the interface thread
  that is a few set lookups (a repeated miss measured 36-64 ns); a loader thread finds the
  system's candidate faces once (the Linux folder walk included), then for each asked
  character tries its script's faces first, then every other. Each face is read once, by
  file and collection index, and its character map is kept: a face read for a character it
  lacked serves later characters it has. A character no face maps is remembered and never
  asked again. The fonts are reinstalled by the next frame, each face once in every chain;
  the kit's text cache notices the new faces.
- **Measured** in `tests/font_fallback_live.rs`: Yu Gothic was installed 55 ms after a
  Japanese label asked for it, and the frame that asked was not held. A Latin-only text
  asks for nothing. The bytes of an installed face are kept for the session (14 MB for Yu
  Gothic). The bundled character map is built once from the fonts' cmaps.
- **Armed by the real window only**: `boot`'s font stage (the path `cantus_ui::run`
  takes) calls `font_fallback::install_for_window`. The first version armed it in
  `CantusApp::new`, which the launcher never calls, so the shipped app never had it (found
  in review); `the_boot_font_stage_arms_the_fallback` now holds the stage. Tests, shots and
  benchmarks keep the bundled fonts; `unarmed_the_fonts_are_the_bundled_ones_only` holds it.
- **Tests**: script groups; unarmed = bundled only; the system faces cover Japanese,
  Korean, Hindi, Thai, Arabic and Hebrew where some face maps the sample (chosen by
  coverage, so macOS's Helvetica lacking Arabic does not fail it; skipped with a message
  where no face does, e.g. a bare Linux runner), each face once; the live cases (no block,
  no duplicates, IME preedit and commit, an O(1) repeated miss); and
  `tests/font_fallback_faces.rs` (two bundled files as stand-ins: a face read for a
  character it lacks serves a later one, a face listed twice is read and installed once).
- **Each review fix proved by mutation**: the fix taken out by hand, the test run, the
  fix put back (the restored tree passes). Seen failing:

  | Fix taken out | Test | Failing output |
  |---|---|---|
  | boot's font stage back to `design::install_fonts(ctx)` | `the_boot_font_stage_arms_the_fallback` | `panicked at …font_fallback_live.rs:71:5: boot's font stage must arm the fallback` |
  | "tried means never again": a face read but not installed is skipped (`Some((_, false)) => continue`) | `a_face_read_once_serves_later_characters_and_appears_once` | `panicked at …font_fallback_faces.rs:56:5: assertion failed: font_fallback::resolve_among(only_icons, &order)` |
  | both IME arms of `note_input` removed | `input_method_text_asks_for_its_faces` | `panicked at …font_fallback_live.rs:126:5: the preedit asked` |
  | only the IME commit arm removed | `input_method_text_asks_for_its_faces` | `panicked at …font_fallback_live.rs:133:5: the commit asked` |

  The boot test runs `install_for_window` and also checks the source of boot's font stage
  for that call. Driving `Boot` itself needs an `eframe::CreationContext`, and its other
  stages read the settings and start the audio engine.
- **Pictures** (`examples/shoot_scripts.rs`, 100 % and 200 %, looked at; in
  `shots/release/scripts/`): a track in Japanese, a clip in Korean, a strip in Hindi, a
  track in Thai — all real glyphs.
- **Not supported yet: shaping.** egui does not shape text. Arabic and Hebrew track names
  show reversed with unjoined letters (seen in the pictures), and Devanagari's conjuncts and
  Thai's marks are not formed. This is said as a known issue in the release notes (en/es)
  and both manuals; nothing claims support.

## The final review: documents, words, two pictures (eighth follow-up)

After merging `r26/integration` (742d221):

- **The baseline of fallback faces.** A Japanese name sat about 3 pt high at 13 pt (6 px at
  200 %). egui puts a glyph's baseline at its own face's ascent, plus half the difference
  between the primary face's row height and its own. `with_faces` now gives each system
  face a `FontTweak::y_offset_factor` computed from its ascent, descent and line gap
  against the proportional family's first face (Plex). The test is
  `font_fallback::fallback_faces_sit_on_plexs_baseline`: the drawn baseline of a fallback
  glyph against Plex's, at 11–40 pt, with the icon font standing in for a system face, and
  the system's Japanese face where there is one. It failed without the fix: icons 2–3 pt
  off at 24/40 pt, Yu Gothic −3 pt at 11/13 pt and −7 pt at 24 pt. With the fix the gap is
  ≤ 0.84 pt. Looked at in `shoot_scripts` at 200 %: the Japanese track name, the inspector
  field and the heading sit on the line.
- **The credit line on Paper.** "Installed with this credit" was painted in the LED green
  (1.7:1 on Paper's panel). It is now `text_dim`, which the contrast floor holds at 4.5:1
  on panels in every theme. Looked at in `shoot_content`, Night and Paper.
- **Words.**
  - *desactivada*, not *guardada*, for the paint tool.
  - The cache's least-recently-used order in Spanish.
  - *equipo* for "this machine": the missing-pack card, the hosted-instrument message and
    the installer's privacy page. *Teclado del ordenador*, the computer keyboard, stays.
  - «» quotes.
  - "Sound pack found elsewhere".
  - Singular messages through `i18n::tf_count`, which picks the `_one` key when n = 1:
    `smp.sample_changed`, `smp.stream_late_tip` and `smp.cache_unwritable`, in both
    languages. `tests/plurals.rs` checks n = 1 and n = 3; it did not compile before the
    helper existed.
- **Documents.**
  - Both manuals and both halves of the release notes now cover round 26 for musicians:
    the clip browser (Alt+F5) and the paint tool (B or 2 in the arrangement), *Link to…*
    listing single clips, the inspector's *Link* row and group rename, the Salamander
    (8/16 layers, memory, first load, level), disk streaming and *Settings ▸ Sample
    cache*, the missing-pack card, the typed level, names in any script, and the one text
    field. They also add the installer's *Privacy* page, the cache on uninstall, and the
    known issues. Packs are CC0 or CC BY with credit, and the Salamander is in the list.
    Every key was checked against `keymap.rs`.
  - ADR 0042: format 2 and the pedal noises. ADR 0043's tests: glyph reach + 1 pt.
  - mixer.md and `route_menu::level_field`'s doc: a click away sets the level, and the
    handle is never inert.
  - content.md: notes where it was superseded.
  - public.md: the motto is the RV1909 in the tree; only a site redeploy is left.
  - The Salamander's description in the manifest and in `make_manifest.py`: about 120 and
    185 MB, from sampler.md's 99 + 16 and 159 + 16 MiB.
  - The beta checklist: §5's seven done, §3's history cleanup done in a copy, and a
    round-26 hand-test list.

## The README's pictures (ninth follow-up)

`crates/cantus-ui/examples/shoot_readme.rs` makes every picture in the README with one
command, headless. It runs on the real demo song with the demo audio interface, so there
is no "no device" line and nothing is painted over the program. The shot has its own
empty sound-pack folder, so nothing from this machine shows.

- **The pictures:** 32 in all.
  - The hero (the whole song, the browser, the desk Extra large with its cables, the
    inspector) and Night/Paper twins at 1280 × 720.
  - The desk, the inspector and the routing matrix; linked clips with the clip browser;
    the piano roll and the channel rack.
  - Eleven effect editors and the eight instrument editors. Exemplar plays the Salamander
    when `CANTUS_SALAMANDER_ARCHIVE` names its archive, and the run says which version
    it used.
  - Appearance, the twelve themes, the mark's colours, and Sound packs with the
    Salamander's credit.
- **The look:** I looked at every picture of the final run.
- **The footer:** the Exemplar picture's footer is cropped, because a headless engine
  has no instrument telemetry and it would read "0 zones".
- **The size:** written as compressed RGB, then `oxipng -o max` when it is installed.
  docs/images went from 2.38 MB in 28 files to 2.86 MB in 32. Seven unused pictures were
  removed: the ten logos, pick-element, theme-variations, both Slate windows, and the
  old arrangement and Paper windows.
- **The README:** rewritten around them. It covers Cantus Night and the mark, cables
  and the matrix, linked clips with the clip browser and paint tool, twelve effects, the
  Salamander and streaming, and CC0/CC BY with credit. Every key was checked against
  `keymap.rs`.
- **The command:** documented in DEVELOPING.md §3.

## Files outside my area

- `crates/cantus-update/src/release.rs` — one additive constant (`STABLE_INSTALLER_NAME`).
- `xtask/src/package.rs` declares `package_ci_tests.rs` and `package_meta.rs` with
  `#[path]` (no change to `xtask/src/main.rs`).
- Follow-ups, on the integrator's word (the later ones too: `third_party/asio-sys/build.rs`,
  `third_party/README.md`, `xtask/installer/cantus.nsi`, `xtask/src/nsis.rs`,
  `xtask/src/main.rs` (one `sums` arm), `crates/cantus-ui/src/settings.rs` and one call in
  `app.rs` (`Files::read`), the `update.*` locale strings, `PRIVACY.md`,
  `docs/DEVELOPING.md`, the glyph fixes' locale lines and views (`fm_graph.rs`,
  `organ_cabinet.rs`, `fx/limiter.rs`), `examples/shoot_verse.rs` and the new
  `examples/shoot_glyphs.rs`): `crates/cantus-host/{build.rs,Cargo.toml,tests/version_resource.rs}`,
  `crates/cantus-plugins/{build.rs,Cargo.toml,tests/version_resource.rs}`,
  `crates/cantus-content/Cargo.toml` (content builder's crate: one feature line),
  `Cargo.toml` (egui/eframe features), `crates/cantus-ui/src/design.rs`
  (`font_definitions`, the emoji face), `crates/cantus-ui/Cargo.toml` (dev-dependency
  `skrifa`), `crates/cantus-ui/assets/fonts/{emoji-icon-font.ttf,LICENSE-emoji-icon-font.txt}`,
  `crates/cantus-ui/tests/font_coverage.rs`, `xtask/notices/head.md` and
  `xtask/src/notices_render.rs` (content builder's: the Fonts section and the marker),
  `THIRD-PARTY-NOTICES.md` (regenerated — **regenerate again after merging content**),
  `Cargo.lock`, `docs/manual/{en,es}/README.md`, `docs/perf/benchmark.md`,
  `docs/beta-checklist.md`.

## Left

Only the owner's decisions and hands remain:

- **The CLA's commercial-licence clause** (keep, drop, or ask SignPath) and **SignPath or
  Certum** (`docs/signing/README.md` §3).
- Making the repository public and tagging `v0.1.0-beta.1`; then SignPath's reputation
  weeks, the application, the SignPath project and secrets, and moving the draft job into
  `release.yml`.
- Looking at the installer's new privacy page in the clean-machine installer run.
- Running `macos.yml` by hand before the tag.

## Notes for the integrator

1. **Commercial dual licensing — say it plainly to the owner**: `CLA.md` §2.1 and §3 and
   ADR 0047 §3 keep a commercial licence open "as Qt and others do". SignPath's first
   condition excludes "commercial dual-licensing"; a reviewer reading the CLA may refuse,
   and offering one later could cost the certificate. `docs/monetize.md` (owner's tree,
   not copied) plans donations, paid sound packs, sponsorship, grants, services and
   optional hosted AI — no software licence sales — so dropping the clause costs the plan
   nothing. It is the owner's decision (`docs/signing/README.md` §3).
2. **No rename collisions expected**: ADR 0048 is the release's number; the new workflows
   are new files. If another branch touched `ci.yml`, keep this version's structure and
   re-add its change (the `paths` lists must stay equal — the test checks).
3. **macOS**: never validated in CI since the round-25 fixes; run `macos.yml` by hand on
   `r26/integration` once the minutes allow (or once public).
4. **Before the first push of this**: the `ci.yml` `paths` filter means the integration's
   documentation-only commits no longer produce a check — expected.
5. **The tag**: with this `release.yml`, `v0.1.0-beta.1` is published as GitHub's *latest*
   (no stable release exists), so the website's button works the moment the repository is
   public.
6. The RV1909 motto on the site and in the Spanish notes is a deliberate change (the
   RVR1960 gate); confirm with the owner when reporting.
7. `docs/beta-checklist.md` §3 is ticked (follow-up 1).
8. **Merge order with content**: content regenerates `THIRD-PARTY-NOTICES.md` and edits
   `xtask/notices*`; this branch changed `head.md`'s Fonts section, added the
   `font-licences` marker and `font_licences()` in `notices_render.rs`, and dropped
   `webpki-roots` and `epaint_default_fonts` from the tree. Keep both, then run
   `cargo run -p xtask -- notices` once and commit the result. (`EXTRA` in
   `notices_render.rs` still names `epaint_default_fonts`' licence files; harmless, it is
   only read for that crate.)
