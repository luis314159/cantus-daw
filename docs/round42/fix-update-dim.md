# Round 42 — fix-update-dim: the update notice's veil, and the updater that refused every update

Branch `r42/fix-update-dim` (from `r42/integration` e63ceddd2). Three bugs the owner met in
the released 0.1.0-beta.16 on Windows, in one screenshot: the app went dark around the
update notice and stayed dark; *Download* ended in red with "The installer is not signed by
Cantus's publisher, so it was deleted"; and a "Neon Tide · 124 BPM" tag drew over the
Updates window.

## 1. The veil (commit 622db9bdb)

**Cause.** `views/update.rs` `notice()` painted the notice's face with
`kit::dialog::sheet`, which also lays the dialogs' full-window scrim. So the whole app was
dark whenever the notice showed, and dark again each time the window closed (×, *Later*,
`Esc`, *Release page*) and gave the notice back. On top of that, the Updates window was an
`egui::Window` of the middle order. The scrim is a painter layer of that same order, which
egui draws after the order's areas, so the window sat under its own veil, dimmed with the
app (see the before shot of the shoot tool).

**Fix.** The notice paints `kit::dialog::face` (no veil). The window is
`Order::Foreground`, like every other kit dialog, and lays no veil on its first, invisible
sizing pass. *Skip this version* closes the window whatever answers it.

**Test.** `crates/cantus-ui/tests/update_dim_e2e.rs` runs through `CantusApp::assemble`, the
frame loop, and the real `Updater` over a table network serving cantus-update's recorded
release. It checks:
- the notice alone lays no veil;
- once open, exactly one veil, painted under the window's title;
- after closing by ×, *Later*, `Esc`, *Skip*, *Download* then `Esc`, and *Release page*
  (with the opener faked) then *Later*, no veil, and the Help menu takes a click.

It failed on the old code (the notice's veil). It failed again with only the window's order
reverted (the window under its veil).

## 2. The updater refused every unsigned update (Windows, beta.16 and beta.17)

**How it verifies** (`crates/cantus-update/src/verify.rs`):
1. not a web page;
2. the size GitHub reported;
3. the SHA-256 in GitHub's asset `digest` and in `SHA256SUMS.txt`;
4. for the Windows installer only, and only when the build has a publisher, Authenticode
   through `Get-AuthenticodeSignature`: the status must be `Valid` and the signer's CN must
   equal the publisher.

No thumbprint is pinned, and there is no `.sig` file. The publisher is
`option_env!("CANTUS_PUBLISHER")`, read at compile time.

**What was published.** The beta.17 and beta.16 `…-windows-x64-setup.exe` were downloaded
from the releases (MZ, full size). Their SHA-256 match the release digests (beta.17
`1cbe1f4d…`, beta.16 `6249fb2f…`). `Get-AuthenticodeSignature` says `NotSigned` for both:
the repository has no signing secret, and the release notes say so. The updater picked the
right asset.

**Cause.** `.github/workflows/release.yml`'s package job, since round 20 (7e664957), had
`env: CANTUS_PUBLISHER: ${{ vars.CANTUS_PUBLISHER }}`. No such variable exists. The
repository's only Actions variable is `CANTUS_RELEASE_MACOS`, read with a GET. So the
variable expanded to an empty string; beta.16's package-job log shows `CANTUS_PUBLISHER: `
on every step. `option_env!` reads an empty variable as `Some("")`. Beta.16 and beta.17 were
therefore built to demand a signature from a publisher named "", and refused their unsigned
updates as `Unsigned`. The owner's error proves the build had `Some(_)`: with `None`, the
check returns before asking Windows. Linux and macOS were unaffected, since their files are
held to SHA-256 alone. The in-app path on Windows has likely never worked end to end.

**Fix.**
- `verify::publisher()`: an unset, empty or blank `CANTUS_PUBLISHER` is no publisher (a
  `const fn`, so `PUBLISHER` keeps its type). `check_publisher` and `verdict` treat a blank
  name as none too.
- `release.yml`: no job-level `CANTUS_PUBLISHER`. Each signing step exports it after it has
  set signing up. "The publisher the program checks for is the signing's" stops a signed
  build with no name, or a name with no signing. "The installer passes the program's own
  update check" runs Authenticode on the built installer when a publisher is set.
- Each failure has its own words, in English and Spanish, and each points to the release
  page (the window keeps its *Release page* button): `update.error.unsigned`, `.signature`
  (Windows rejected it), `.publisher` (signed by someone else), `.signature_check` (Windows
  could not be asked).
- `docs/RELEASING.md` L9, `docs/releases/PREFLIGHT.md` L9, and `docs/beta-checklist.md`
  "la actualización dentro de la app, de punta a punta": the in-app update from beta.N to
  beta.N+1 on the owner's PC, with the expected Status texts, after every release and
  before announcing it.

**Tests** (`crates/cantus-update/tests/check.rs`, `xtask/src/package_ci_tests.rs`):
- `an_empty_or_unset_publisher_is_no_publisher`: `publisher(None | "" | "  ")` is None, and
  this build's own `PUBLISHER` is never blank.
- `beta_17_s_unsigned_installer_passes_on_hashes_alone_without_a_publisher`: Windows's
  recorded answer for the real beta.17 installer (`fixtures/authenticode-beta17-setup.json`,
  only the folder in the message made anonymous) passes with no publisher and is refused as
  unsigned with one. Windows PowerShell's Microsoft-signed `powershell.exe`
  (`fixtures/authenticode-windows-powershell.json`) passes for its signer and fails for
  another.
- `each_signature_failure_says_what_failed`: four keys, both locales, each naming the
  release page.
- `a_build_with_a_blank_publisher_installs_an_unsigned_update`: the whole download with
  `publisher: Some("")` ends `Ready`. On the old code it ended in `update.error.signature`.
- `the_publisher_comes_only_from_a_signing_step_and_is_checked`: the workflow's shape.

**What only the owner can do.** Nothing needs re-signing or re-uploading. The copies already
out (beta.16 and beta.17 on Windows) cannot be repaired: they demand a signer named "",
which no certificate is. They must download beta.18 once by hand. The paragraph below goes
into beta.18's notes. After beta.18 is out, do the in-app update of the beta-checklist.

### Draft for beta.18's release notes (not published)

English, under "Fixed" or as its own short section near the top:

> **Windows: updating from 0.1.0-beta.16 or beta.17 — download this one by hand, once.**
> Those two versions were built by mistake to accept only updates signed by a publisher
> that does not exist, so *Download* in their Updates window ends with "The installer is
> not signed by Cantus's publisher, so it was deleted". Nothing is wrong with the file.
> Install 0.1.0-beta.18 from this page (or click *Release page* in that window); your
> settings, presets and projects are kept. From beta.18 on, Cantus updates itself again.

Spanish:

> **Windows: si tienes 0.1.0-beta.16 o beta.17, descarga esta versión a mano, una vez.**
> Esas dos versiones se compilaron por error para aceptar solo actualizaciones firmadas por
> un editor que no existe, así que *Descargar* en su ventana de Actualizaciones termina con
> «El instalador no está firmado por el editor de Cantus, así que se borró». El archivo está
> bien. Instala 0.1.0-beta.18 desde esta página (o pulsa *Página de la versión* en esa
> ventana); tus ajustes, presets y proyectos se conservan. Desde beta.18, Cantus vuelve a
> actualizarse solo.

### Draft for the site (download page, one line, not published)

- EN: "On Windows with 0.1.0-beta.16 or beta.17? Download this version once by hand —
  their built-in updater cannot install it. From beta.18 on, updates install themselves."
- ES: "¿En Windows con 0.1.0-beta.16 o beta.17? Descarga esta versión a mano una vez:
  su actualizador no puede instalarla. Desde beta.18, las actualizaciones se instalan
  solas."

## 3. Something drawn over the Updates window ("Neon Tide · 124 BPM")

**Cause.** Nothing in the main window's own paint covers the dialog. The new e2e test
checks every word painted after the window's last one, with the pointer at six places, on
the project the program opens on (Neon Tide). The tag is the **stage companion** ("Cantus
Stage", ADR 0091), the dancer's window of its own. Its defaults are a see-through backdrop,
the song chip on ("Neon Tide" over "124 BPM", `views/stage_vis_paint.rs` `song()`), and
`on_top: true` (`stage_companion.rs`: a topmost layered window on Windows, `AlwaysOnTop`
elsewhere). Only its glass chips show, so it reads as a tag floating over every window,
Cantus's own dialogs included.

**Fix.**
- `kit::dialog::mark_modal` and `modal_up`: a dialog that wants the window to itself says so
  each pass. The scrim says it, and so do Classic's dialogs, which have no scrim.
- `stage::companion::keeps_on_top(settings, ctx)`: *On top*, except while a dialog is up.
  It drives the native window's topmost state and the egui viewport's level. On Windows, a
  window leaving the top band is also put just behind the main window
  (`cantus_rt::window::Layered::set_behind`), since otherwise it would still sit over it.
  It returns to the front when the dialog closes. An OBS capture does not depend on the
  window's place.

**Test.** `nothing_draws_over_the_updates_window` in `update_dim_e2e.rs` checks:
- the companion keeps on top at rest and with the notice up;
- it steps back while the Updates window is open, wherever the pointer rests;
- nothing is painted over the window;
- it is in front again after `Esc`.

**Verified only in emulation.** The native window's z-order on a real desktop needs the owner's
eye: with the companion open and *On top*, open *Help ▸ Check for updates…*. The song chip
must go behind Cantus while the window is open, and come back in front after it closes
(added to the beta-checklist entry).
