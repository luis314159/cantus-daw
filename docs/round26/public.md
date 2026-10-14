# Round 26 — public: the repository made ready to be public, in a copy

*Builder **public**, branch `r26/public`, 2026-10-13. ADR 0049. Tools: `tools/public/`.*

## Resumen para el dueño (español)

**Nada cambia hoy.** Todo esto es un plan para el día en que tú decidas hacer público el
repositorio; hasta entonces el repositorio real, su historial y GitHub se quedan como
están. El plan está hecho para **no perder nada**: el repositorio privado se conserva
entero (renombrado y archivado), las capturas se quedan en tu disco, y los archivos que no
están en git (`.claude/`, `docs/monetize.md`) se copian a mano.

**Qué hice.** Cloné el repositorio en una copia aparte (`F:\projects\daw-r26\public-copy`,
sin `origin`, nada se puede subir desde ahí), hice el inventario de todo lo que no es
nuestro en **todo** el historial, busqué secretos en cada commit de cada rama con un
escáner mantenido (gitleaks 8.30.1, verificado por su suma SHA-256) y con uno propio, y
reescribí copias desechables con `git filter-repo` (2.47.0, verificado contra PyPI). **No
toqué el repositorio real ni GitHub**: ni push, ni remotos, ni nada forzado sobre él.

**Lo que encontré.**

- **Secretos: ninguno.** Ni claves privadas, ni tokens (GitHub, Cloudflare, AWS, Azure,
  Anthropic, OpenAI, Hugging Face…), ni `.pfx`/`.p12`, ni `.env`, ni contraseñas de firma,
  en ningún commit, mensaje, etiqueta ni archivo binario.
- **Capturas de otros productos:** 479 imágenes en `research/*/screenshots/`, todas
  añadidas en un solo commit (`4c5841b`, 2026-10-07); nunca vivieron en otra carpeta. 77 de
  ellas (`research/ui/screenshots/cantus-audit/`) son de Cantus mismo; salen igual, como
  pediste. Se quedan en tu disco, fuera de git.
- **RVR1960:** el archivo `crates/cantus-ui/locales/verses/es.txt` (desde `b0dce1f`,
  2026-10-05) **y además citas**: el lema, Colosenses 3:23, en la web (`web/src/body.rs`) y
  en las notas de la versión, y cinco palabras del Salmo 33:3 en `docs/round20/verse.md`
  (ya cambiadas en el árbol). Según los términos que leímos el 2026-10-11 una web también
  necesita permiso, y **la web publicada ya muestra el lema en RVR1960**. Propuse citar el
  lema en la RV1909: **ya está hecho en el árbol** (ronda 26, `web/src/body.rs` y las notas
  de la versión dicen la RV1909). Falta el historial (`--replace-text`) y volver a
  publicar la web.
- **Sin `es.txt`, Cantus no compilaba** (`include_str!` lee el archivo aunque esa rama no
  se use). El integrador ya aplicó el arreglo en esta ronda: el programa hace lo mismo, y
  los tests leen el archivo sólo si existe.
- **Datos personales (no secretos, pero se publicarían):** la dirección de correo del
  trabajo aparece en una parte de los commits (**detalles guardados en privado**, fuera del
  repositorio, en `F:\projects\daw-r26\private\public-identity.md`, con las opciones). Cada
  commit publica también su fecha y hora. **Esto lo decides tú.** El chequeo final falla
  si cualquier dirección que tú pongas en tu lista privada (`forbid.txt`, fuera del
  repositorio) aparece en lo que se va a publicar.
- El resto (fuentes IBM Plex, Syne e Instrument Sans con OFL; cpal y asio-sys Apache-2.0;
  los XSD de DAWproject con licencia MIT; los MP3 de demostración hechos con Cantus; las
  imágenes de `docs/`, `web/` y los goldens) es nuestro o tiene licencia que permite
  publicarlo. El SDK de ASIO nunca estuvo en el repositorio.

**Tamaño:** el historial publicado pasaría de **155 MB a 30 MB** (la rama `master` sola,
sin historial, pesa 25 MB: el historial completo cuesta sólo ~5 MB más).

**Recomendación (para cuando decidas): ruta (a)** — subir el historial reescrito a un
**repositorio nuevo** en GitHub; el privado se renombra `cantus-private` y se archiva con
todo su historial (nunca se le hace push forzado ni se publica, porque GitHub guarda los
commits viejos accesibles por hash). Se conserva el historial (por qué se decidió cada
cosa). La probé de punta a punta en copias desechables, sobre la rama de integración de
esta ronda: `tools/public/check.sh` dice `READY`. La **ruta (b)** — un solo commit nuevo,
sin historial — también la probé; es la opción si prefieres no publicar fechas y horas.

**Lo que tienes que decidir** (antes del día, sin prisa):

1. **El correo de los commits viejos:** dejarlo, o reescribirlo (sugerencia: la dirección
   «no-reply» de GitHub). Las opciones y los archivos (`identities.mailmap`, `forbid.txt`)
   están en la carpeta privada.
2. ~~**El lema en RVR1960** en la web y las notas~~ — **hecho en el árbol** (RV1909, ronda
   26). Solo queda volver a publicar la web para que la página en línea lo muestre.
3. **Repositorio nuevo** (recomendado) o el mismo con push forzado (no recomendado).
4. **Ruta (a) o (b).**
5. Si `docs/claude-memory/` se publica o se saca del árbol ese día.

**El día que decidas**, los comandos exactos están en la sección *Route (a)*; antes de subir
nada, `tools/public/check.sh --forbid <tu lista> <copia> master --tags` debe decir `READY`
(con gitleaks: el chequeo no corre sin él).

**Nota para el integrador:** esta rama tiene commits intermedios cuyos documentos
nombraban la dirección del trabajo; se corrigieron, y la ronda 26 se aplasta (*squash*) en
un solo commit sobre `master`, así que esos blobs intermedios nunca llegan a `master`.

---

## 1. The copy

```sh
git clone --no-local F:/projects/cantus F:/projects/daw-r26/public-copy   # a fresh clone
cd F:/projects/daw-r26/public-copy
for b in $(git for-each-ref --format='%(refname:strip=3)' refs/remotes/origin | grep -v '^HEAD$'); do
  [ "$b" = master ] || git branch -q --track "$b" "origin/$b"     # every branch, local
done
git remote remove origin                                          # nothing can be pushed
```

At clone time: **349 commits** over **16 branches** (`master` and the `r25/*`, `r26/*`
builders' branches; `master` alone has 294), **no tags**, 7 831 objects, pack 155.4 MiB.
GitHub's private `origin` holds `master` only (last pushed `ffffe50`). The owner's tree was
only read (`git clone` reads committed objects; their uncommitted files never left it).

Because `origin` had to go before the rewrite, `git filter-repo`'s fresh-clone heuristic
("expected one remote, origin") refused to run, and its `--force` was needed — in this
disposable copy only; nothing was forced anywhere else. On the day it is not needed:
filter-repo runs on a fresh clone with its `origin` (the local private repository) and
removes that remote itself (§6).

## 2. Inventory — what is not ours, in the whole history

Found with `git log --all --name-status -M` (additions and renames — **there is no rename
in the whole history**, so nothing lived under another path), every distinct blob read by
`tools/public/scan.py`, and a reading of each kind of file.

| Path (every commit that ever held it) | What | Ours? | Action |
|---|---|---|---|
| `research/fabfilter/screenshots/` (85) | FabFilter Pro-Q/C/L/MB/DS/G/R screenshots | **No** | leaves the history |
| `research/instruments/screenshots/` (140) | Arturia Pigments, Kontakt, Serum 2, Pianoteq 9, Omnisphere, Diva, Massive, XO, SubLab… | **No** | leaves |
| `research/ui/screenshots/{fl-studio,ableton-live,bitwig-studio,logic-studio-one,plugin-instrument-uis}/` (42+36+44+36+19) | other DAWs' and plug-ins' UIs | **No** | leaves |
| `research/ui/screenshots/cantus-audit/` (77) | Cantus itself (an old UI audit) | yes | leaves with the folder (the owner's rule; nothing outside `research/` links to it) |
| ↳ all 479 added by `4c5841b` (2026-10-07), never modified or moved | | | ~125 MB of the 155 MB pack |
| `crates/cantus-ui/locales/verses/es.txt` (241 commits, one blob, from `b0dce1f` 2026-10-05) | RVR1960, 281 passages, © Sociedades Bíblicas Unidas | **No** | leaves the history (until the permission) |
| `web/src/body.rs` (3 blobs, 99 commits) | the motto, Col 3:23, **quoted in the RVR1960** (and "(RVR1960)" in the caption) — also on the live site's wasm today | the quote, no | RV1909 by `--replace-text` (§5); the live site with the next deploy |
| `docs/releases/v0.1.0-beta.1.md` (3 blobs, 128 commits) | the same verse, first half, "(RVR1960)" | the quote, no | RV1909 by `--replace-text` |
| `docs/round20/verse.md` (older blobs) | five words of Ps 33:3 in the RVR1960 (found by the 4-word review net after the review) | the quote, no | rephrased in the tree (round 26); RV1909 by `--replace-text` in the history |
| `crates/cantus-ui/locales/verses/{en,es-1909}.txt` | World English Bible, Reina-Valera 1909 | public domain | stay |
| `crates/cantus-ui/assets/fonts/IBMPlex*.ttf` | IBM Plex | OFL-1.1, `LICENSE-OFL.txt` beside, in the notices | stay |
| `web/assets/fonts/{syne,instrumentsans}.ttf` | Syne, Instrument Sans | OFL-1.1, `OFL-*.txt` beside | stay |
| `third_party/cpal/`, `third_party/asio-sys/` | patched crates (incl. `asio_stub_bindings.rs` as shipped on crates.io) | Apache-2.0, `LICENSE` in each | stay. **The Steinberg ASIO SDK was never committed** (build.rs downloads it) |
| `crates/cantus-project/tests/fixtures/dawproject/` | Bitwig's DAWproject XSDs, `Validate.java` | MIT, `LICENSE` beside | stay |
| `crates/cantus-update/tests/fixtures/*-recorded.json` | recorded public GitHub API answers | public data | stay |
| `web/assets/demos/*.mp3`, `web/assets/fx/audio/*.mp3` (20) | demo songs and before/after clips, rendered by Cantus | yes | stay |
| `docs/images/` (28), `docs/manual/img/` (9), `web/assets/**` (24 images), `crates/cantus-ui/assets/logos/` (10 SVG), `crates/cantus-ui/tests/golden/kit/` (138) | Cantus's own pictures, logos, goldens (checked: e.g. `fx-hosted.png` shows our test plug-in, not a third party's) | yes | stay |
| `docs/icons/cantus—Iconos.html` | the owner's icon proposals (a bundled page; embeds React 18.3.1, MIT) | yes | stay |
| `research/**/*.md` (text) | our notes on other products, citing their pages; short quotations | yes | stay; **173 links** to the screenshots become plain text that day (`unlink_screenshots.py`) |

Not found anywhere in the history: PDFs or manuals of other products, other products'
logos, SDK headers, sample libraries, `.wav` files (ignored since the first commit).

## 3. Secrets — every commit of every ref

| Scanner | Scope | Result |
|---|---|---|
| **gitleaks 8.30.1** (`gitleaks_8.30.1_windows_x64.zip`, SHA-256 `d29144de…c4e` = the release's `checksums.txt` = GitHub's asset digest) | `git --log-opts="--all"`: 313 commits with a diff; `--all -m` (merges against each parent): 346 commits, 57.6 MB; `dir` on the working tree | **no leaks** (all three) |
| **`tools/public/scan.py`** (ours), first run | `git ls-tree -r` of all 349 commits → 4 652 distinct blobs (text only); commit messages | **0** high-confidence secrets, **0** secret-shaped files |
| **`scan.py`**, hardened after review (2026-10-13, a fresh clone, 408 commits by then) | 4 891 distinct blobs: 4 192 text, **699 binary** (their printable ASCII runs ≥ 8 characters; UTF-16 with a BOM decoded); commit messages; annotated tags (none exist) and their tagger identities | **0** secrets, **0** secret-shaped files, **0** low-confidence assignments to review |

The targeted patterns (`tools/public/scan_rules.py`): PEM/OpenSSH/PGP private-key headers;
GitHub `ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`; AWS `AKIA…`/`ASIA…` and
`aws_secret_access_key`; Azure `AccountKey=` and SAS `sig=`; Slack `xox?-`; Google `AIza`;
Stripe `sk_live_`; npm `npm_`; PyPI `pypi-AgEI…`; Anthropic `sk-ant-`; OpenAI `sk-proj-`
and `sk-` + 32; Hugging Face `hf_`; JWTs (`eyJ….eyJ….…`); Cloudflare
`CLOUDFLARE_API_TOKEN=`/`CF_*` with a value; `SIGNPATH_*`/`CERT_*`/`PFX_*`/`SIGNING_*`
passwords/tokens with a literal value; file names `*.pfx`, `*.p12`, `*.pem`, `*.key`,
`*.snk`, `*.keystore`, `*.jks`, `*.ppk`, `id_rsa*`, `id_ed25519*`, `.env*`, `.dev.vars`
(wrangler's secrets file), `*credentials*.json`; and, for review, `password|passwd|pwd|
secret|token|_key|apikey = "…"` (any prefix: `github_token`, `signing_key`; our dotted
locale keys excepted), `Bearer …` and credentials in URLs. A synthetic repository with one
hidden finding of each kind (a token inside a binary, a forbidden address in a UTF-16 file
and in an annotated tag's message, the motto in `\u{…}` escapes, mixed-case and nested
screenshot folders, an image elsewhere under `research/`, a JWT) was caught entirely. The
workflows use `${{ secrets.* }}` only; `web/wrangler.jsonc` holds no secret.

**Nothing to rotate, nothing to remove for secrecy.**

## 4. Private without being secret — the owner's choice

The detail of this section is **kept privately**, outside every repository, in
`F:\projects\daw-r26\private\public-identity.md` (with `identities.mailmap` and
`forbid.txt` beside it): a committed document must not name what it advises to remove.

| What | Matters? | Options |
|---|---|---|
| **The work address (details kept privately)** — author and committer of part of the commits | **Yes** (the owner decides) | (i) keep; (ii) **rewrite to the GitHub no-reply address** `98241129+luis314159@users.noreply.github.com` (suggested); (iii) rewrite to the personal address. With filter-repo `--mailmap <private>/identities.mailmap` (tested, §5). Route (b) publishes no old identity at all. Whatever is rewritten goes into the private `forbid.txt`, and `check.sh` then **fails** if it appears anywhere in what is published (paths, blobs, commit and tag messages, author/committer/tagger identities). |
| The personal address | Low (it is the GitHub account's) | same options; set `git config user.email` before the first public commit |
| Commit dates and times | Low–medium (details kept privately) | only route (b) hides them |
| `Co-Authored-By: Claude …` trailers (257 messages) | No (transparent) | keep |
| E-mails in files: `THIRD-PARTY-NOTICES.md` (crate authors, from their licences), `third_party/asio-sys/Cargo.toml`, `licensing@americanbible.org`, `info@steinberg.de`, `link-devs@ableton.com`, `hosting-request@osuosl.org` | No: public addresses, as their sources publish them | keep |
| Local user paths: none real (`C:/Users/Luis/Music/…` is an invented path in `examples/shoot_recovery.rs`) | No | keep |
| Notes about this machine's setup in `CLAUDE.md`, `docs/DEVELOPING.md`, `docs/claude-memory/` (details kept privately) | Low | keep, reword that day, or keep `docs/claude-memory/` private |
| The owner's name, `lalmazan.com`, `luis314159` | No: public on purpose | keep |
| `docs/monetize.md` | No: never committed (untracked in the owner's tree) | keep it untracked |

No phone numbers, postal addresses, identity numbers or financial data in any blob.

## 5. The rewrite (in the copy) and its proof

The first run (below) used the brief's globs; after the review, the plan uses the
case-insensitive `--path-regex` set of §7 (which also takes nested screenshot folders and
any image under `research/`) and was rehearsed again on `r26/integration` (§7).

```sh
cd F:/projects/daw-r26/public-copy
python -I git_filter_repo.py --force --invert-paths \
  --path-glob 'research/*/screenshots/*' \
  --path crates/cantus-ui/locales/verses/es.txt
```

(`git_filter_repo.py` from `git_filter_repo-2.47.0-py3-none-any.whl`, SHA-256
`2cd04929…ad83` = PyPI's; git 2.50.0. `--force` only because `origin` was already removed,
§1.) 2.2 s.

| | Before | After (paths only) | After the full plan (paths + motto + mailmap, a second clone) | Route (b) (one commit) |
|---|---|---|---|---|
| Commits, all refs | 349 | 349 (none emptied) | 356 (the builders had committed meanwhile) | 1 |
| Commits on `master` | 294 | 294 | 294 | 1 |
| Rewritten | — | **241** (186 on `master`), from `b0dce1f`, 2026-10-05; 108 keep their hash | 356 (the mailmap touches every commit) | — |
| Pack | **155.4 MiB** (7 831 objects) | **29.7 MiB** (7 343) | **29.4 MiB** | **25.0 MiB** |
| `.git` on disk | 156 MB | 31 MB | 31 MB | 26 MB |
| `git log --all -- 'research/*/screenshots/*' crates/cantus-ui/locales/verses/es.txt` | 2 commits (`b0dce1f`, `4c5841b`, which added them; 241 commits hold them) | **empty** | **empty** | empty |
| `tools/public/scan.py` | 480 forbidden paths, 7 RVR1960 blobs | 0 paths, **6 RVR1960 blobs** (the motto) | **CLEAN** | CLEAN |
| `tools/public/check.sh <clone> master --tags` | NOT READY | NOT READY (motto; tree) | **READY** (after the day-of commit, §8) | **READY** |

Refs in the rewritten copy: `master`, `r25/{cables-view,desk,integration,interaction,look,
matrix,night,polish,site}`, `r26/{content,integration,linked,mixer,public,release}`; no
tags. **Published would be `master` and the release tags only**: the `r*/` branches are
builders' work in progress whose result reached `master` through each round's
integration (round 25 was applied rather than merged, so `r25/integration` alone carries
54 commits that are not ancestors of `master`); publishing them adds half-finished,
superseded states and nothing a reader needs. The day's clone takes `master` alone (`--single-branch`), so they never enter it.

**The build without `es.txt`.** In the rewritten copy, `cargo test -p cantus-ui --test
verse --no-run` fails:

```
error: couldn't read `crates\cantus-ui\src\../locales/verses/es.txt`: The system cannot find the file specified. (os error 2)
   --> crates\cantus-ui\src\verse.rs:149:37
149 |             Translation::Rvr1960 => include_str!("../locales/verses/es.txt"),
```

`include_str!` reads its file at compile time even in a match arm the constant
`RVR1960_PERMITTED = false` makes dead, and `tests/verse.rs` includes it twice more. The
fix, `tools/public/rvr1960-out-of-tree.patch` (applies cleanly to `master` `78c8ff5`):

- `verse.rs`: the RVR1960 arm returns `""` (what it already returned while not permitted)
  with a comment saying how to bring the file back, and a compile-time
  `assert!(!RVR1960_PERMITTED, …)` so permitting it without restoring the file and its
  `include_str!` does not build. Behaviour unchanged.
- `tests/verse.rs`: `rvr1960()` reads the file at run time and returns `Option`; only a
  file that does not exist (`NotFound`), while the RVR1960 is not permitted, counts as
  absent — then the RVR1960-only checks (its list, the allowance, the motto's wording) are
  skipped; any other read error, or a missing file once permitted, fails the test; when
  present (the owner's private tree) they run exactly as before.

With it, in the copy (no `es.txt`): `cargo test -p cantus-ui --test verse --test about` —
**16 + 3 passed**; `cargo test -p cantus-ui verse` (every target, filtered) passed;
`rustfmt --check` clean; `cargo clippy -p cantus-ui --all-targets -- -D warnings` clean
(a first draft tripped `assertions_on_constants`; the test now panics in a guarded match
arm instead); and the **whole `cantus-ui` suite** without the file,
`cargo test -p cantus-ui --no-fail-fast`: **845 passed, 0 failed** (29 test binaries,
4 min 34 s). With the file put back (the private tree's
case) the same 16 + 3 pass, and with a damaged copy of it (one passage removed)
`every_translation_holds_the_same_passages_in_the_same_order` fails as before — the guard
still works where the file exists. A first version of the patch failed to compile
(`verse::parse` takes `&'static str`); the test now leaks the file's `String`, as tests may.

## 6. What changes on the day

- **Commits.** Every commit from `b0dce1f` (2026-10-05, the verse files) on gets a new
  hash: 186 of `master`'s 294 (241 of 349 over all refs) at the time of this report, and
  every later one. With the mailmap (identities), **every** commit changes; the 108 older ones keep theirs only if the
  identities are kept. The exact old → new list is the run's
  `.git/filter-repo/commit-map`; the copy's is in the appendix, only as an example — the
  real one will differ (later commits, the mailmap).
- **Commit messages** that quote a rewritten hash are translated by filter-repo.
  **Documents** are not: 58 hash-like tokens in Markdown resolve to commits, 52 of them in
  the rewritten range (STATUS, round reports, the memory). They keep pointing at the
  private history. Keep the commit map with the private archive, or publish it as
  `docs/history-map.txt` so a reader can translate them.
- **Tags.** None exist today. `v0.1.0-beta.1` is created **after** the rewrite, on the
  public `master`; `release.yml` runs on a pushed tag `v[0-9]+.[0-9]+.[0-9]+*` and refuses
  a tag that does not match `Cargo.toml`'s version, so the first push of the tag to the
  public repository is the first release.
- **Branches and worktrees.** The public repository gets `master` only. The private
  repository keeps everything (its history unchanged) and is archived. The worktrees in
  `F:\projects\daw-r23`, `daw-r25`, `daw-r26` belong to the private repository: finish
  and merge what is open first, remove them (`git worktree remove`), and start new rounds
  from the public clone. A branch that must survive moves over as patches
  (`git format-patch master..<branch>` in the private repository, `git am` in the public
  one).
- **GitHub.** The private `luis314159/cantus` is renamed (`cantus-private`) and archived; a
  new, empty, public `luis314159/cantus` receives the push. The website workflow
  (`site.yml`) needs its Cloudflare secrets again in the new repository (secrets do not
  travel), and so do the signing secrets when they exist. Issue templates, the funding
  file and the workflows travel with the tree.
- **The owner's working copy** moves to the public clone (§7, last step); the untracked
  files of `F:\projects\cantus` (`.claude/`, `docs/monetize.md`, the screenshots, any
  `.cargo/config.toml`) are copied over by hand.

## 7. The two routes — commands, exactly, in order

**Not now.** The owner has said they do not want to go public yet; these are the commands
for the day they decide. Both assume round 26 is on `master` in `F:\projects\cantus` (so
`tools/public/` exists there), the tree is clean, and the decisions of the summary are
taken. Git Bash; `python` is Python 3.8+. **Nothing below writes to `F:\projects\cantus`**
until the last step of route (a), which only renames the folder — the private repository
and its whole history are kept as the archive; nothing is deleted.

`PRIVATE` is the owner's private folder, **outside every repository** (today
`F:\projects\daw-r26\private`; copy it somewhere safe before the round's folders are
cleaned): `forbid.txt` (what must appear nowhere), `identities.mailmap` (how old
identities are rewritten), `public-identity.md` (why).

### Route (a) — the rewritten history, pushed to a new public repository (recommended)

```sh
# 0. Tools, pinned to what round 26 checked: git filter-repo by hash; gitleaks 8.30.1 from
#    its GitHub release, its SHA-256 compared with the release's checksums.txt
printf 'git-filter-repo==2.47.0 --hash=sha256:2cd04929b9024e83e65db571cbe36aec65ead0cb5f9ec5abe42158654af5ad83\n' > /tmp/gfr.txt
python -m pip install --user --require-hashes -r /tmp/gfr.txt
export GITLEAKS=/path/to/gitleaks.exe                     # check.sh refuses to run without it
PRIVATE=/f/projects/daw-r26/private                       # wherever the owner keeps it

# 1. A fresh clone of master alone, in an empty folder outside the repository
W=/f/projects/going-public && mkdir -p $W && cd $W
git clone --no-local --single-branch --branch master F:/projects/cantus cantus-public

# 2. The RVR1960 replacement rules, built from the private es.txt, kept outside the repo
python F:/projects/cantus/tools/public/motto_replacements.py \
  F:/projects/cantus/crates/cantus-ui/locales/verses $W/motto.replace

# 3. The rewrite (filter-repo removes the clone's origin, the local private repo, by itself).
#    The --path-regex values are scan_rules.py's: `python tools/public/scan.py --filter-repo-args`
#    prints them. Drop the --mailmap line to keep the identities as they are.
cd $W/cantus-public
python -m git_filter_repo --invert-paths \
  --path-regex '(?i)^research/(.*/)?screenshots/' \
  --path-regex '(?i)^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$' \
  --path-regex '(?i)^crates/cantus-ui/locales/verses/es\.txt$' \
  --replace-text $W/motto.replace \
  --mailmap $PRIVATE/identities.mailmap
cp .git/filter-repo/commit-map $W/commit-map.txt          # keep it with the private archive
git remote -v                                             # must print nothing

# 4. The day-of commit (section 8)
python tools/public/unlink_screenshots.py .
printf "\n# Kept privately, never published (ADR 0049)\nresearch/**/screenshots/\ncrates/cantus-ui/locales/verses/es.txt\n" >> .gitignore
#    by hand: crates/cantus-ui/locales/verses/README.md (es.txt is kept privately until the
#    permission); optionally the machine-setup notes (the private note says which)
git config user.name  "Luis Ángel Almazán"
git config user.email 98241129+luis314159@users.noreply.github.com
git add -A && git commit -m "Going public (ADR 0049): the screenshots ignored and unlinked; the RVR1960 file kept privately"

# 5. Proof: build, tests, the check (gitleaks and the forbid list are both required)
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace --no-fail-fast
bash tools/public/check.sh --forbid $PRIVATE/forbid.txt . master --tags   # must end: READY

# 6. GitHub (web): rename luis314159/cantus to cantus-private (Settings > General), then
#    "Archive this repository" (archived, not deleted: everything stays). Create a new
#    PUBLIC repository luis314159/cantus, EMPTY (no README, licence or .gitignore). Add its
#    secrets again (site, signing).

# 7. The first push: nothing forced, the repository is new
git remote add public https://github.com/luis314159/cantus.git
git push public master
git tag -a v0.1.0-beta.1 -m "Cantus 0.1.0-beta.1"         # when the release is ready
bash tools/public/check.sh --forbid $PRIVATE/forbid.txt . master --tags   # again, with the tag
git push public v0.1.0-beta.1                             # starts release.yml

# 8. Move the work over (close editors first; the private repository's worktrees removed
#    beforehand, section 6, or they break). Nothing is deleted: the old folder is the archive.
mv /f/projects/cantus /f/projects/cantus-private
mv $W/cantus-public /f/projects/cantus
cp -r /f/projects/cantus-private/.claude /f/projects/cantus/
cp /f/projects/cantus-private/docs/monetize.md /f/projects/cantus/docs/
for d in /f/projects/cantus-private/research/*/screenshots; do
  cp -r "$d" "/f/projects/cantus/${d#/f/projects/cantus-private/}"   # ignored by .gitignore
done
cp /f/projects/cantus-private/crates/cantus-ui/locales/verses/es.txt \
   /f/projects/cantus/crates/cantus-ui/locales/verses/              # ignored by .gitignore
printf '.claude/\ndocs/monetize.md\n.cargo/\n' >> /f/projects/cantus/.git/info/exclude
git -C /f/projects/cantus status --short                  # must print nothing
git -C /f/projects/cantus remote rename public origin
```

### Route (b) — a fresh one-commit history

```sh
PRIVATE=/f/projects/daw-r26/private; export GITLEAKS=/path/to/gitleaks.exe
W=/f/projects/going-public && mkdir -p $W && cd $W
git clone --no-local --single-branch --branch master F:/projects/cantus cantus-fresh
cd cantus-fresh
git remote remove origin
git ls-files | grep -i -E '^research/(.*/)?screenshots/|^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$|^crates/cantus-ui/locales/verses/es\.txt$' \
  > $W/leaving.txt
git rm -q --cached --pathspec-from-file=$W/leaving.txt
python tools/public/unlink_screenshots.py .
printf "\n# Kept privately, never published (ADR 0049)\nresearch/**/screenshots/\ncrates/cantus-ui/locales/verses/es.txt\n" >> .gitignore
#    (the motto in web/src/body.rs and docs/releases/v0.1.0-beta.1.md already reads the
#    RV1909 in the tree, round 26: nothing to change there)
#    by hand: crates/cantus-ui/locales/verses/README.md
git config user.name  "Luis Ángel Almazán"
git config user.email 98241129+luis314159@users.noreply.github.com
git checkout --orphan public
git add -A
git commit -m "Cantus 0.1.0-beta.1: the first public commit (the history is kept privately, ADR 0049)"
git branch -D master && git branch -m master
git reflog expire --expire=now --all && git gc --prune=now
export PATH="$HOME/.cargo/bin:$PATH" && cargo test --workspace --no-fail-fast
bash tools/public/check.sh --forbid $PRIVATE/forbid.txt . master   # must end: READY
#    then steps 6-8 of route (a), with cantus-fresh in place of cantus-public (the
#    untracked screenshots and es.txt are still on disk in cantus-fresh, now ignored)
```

**Rehearsed after the review, on what will really be `master`.** In a throwaway clone
(`public-tmp\reh-src`, its `origin` removed before anything else ran), `r26/integration`
was merged with this branch (one conflict, `crates/cantus-ui/tests/verse.rs`: this branch's
side) and squashed onto `master` `78c8ff5` as one commit — what the integrator will do.
Then:

- **Route (a)**, steps 1–5 literally (filter-repo as the verified script, no `--force`;
  step 0 checked earlier as `pip download --require-hashes`; the workspace test not run):
  filter-repo removed the clone's `origin` itself; 296 commits on `master`, pack
  **29.4 MiB**, `.git` 31 MB; `scan.py --filter-repo-args` printed exactly the three
  regexes of step 3; `check.sh --forbid <private>/forbid.txt . master --tags` →
  scan **CLEAN** (0 forbidden paths, 0 RVR1960 blobs, 0 secrets, 0 forbid-list hits in
  4 062 blobs, 223 of them binary), gitleaks **no leaks**, tree **ok** → **`READY`**.
- **Route (b)** literally (the motto edits applied with the same rules as route (a)):
  480 paths untracked (still on disk: the screenshots and `es.txt` are kept locally, now
  ignored), one commit, pack **25.0 MiB**, `check.sh` → **`READY`**.
- The only review note left in both: one 4-word match in a Spanish UI string of the
  sampler (an ordinary phrase, not a quotation).

### The recommendation: route (a)

- The history is the project's record: 294 commits whose messages carry the reasons
  (ADRs, STATUS and round reports point at them), `git blame` for every line, and the
  record of authorship. It costs ~4.5 MB over route (b) (29.4 vs 25.0 MiB).
- Everything that must not be published leaves it, verified: no forbidden path, no
  RVR1960 text, no secret, nothing from the owner's forbid list.
- A **new** repository, so nothing is ever force-pushed and the public repository never
  held the old objects (a force-push to the existing one leaves old commits reachable by
  hash and in caches). The private repository is archived whole: nothing is lost.
- Route (b) is right if the owner prefers not to publish the commit dates and times, or
  these weeks' history at all.

## 8. The tree on that day

What the day-of commit holds (route (a) step 4; route (b) does the same before its one
commit) — rehearsed, with `check.sh` then saying `READY`:

1. **The screenshots leave the tree** (the rewrite already took them out of every commit)
   and `research/**/screenshots/` goes into `.gitignore`, so the owner keeps them on disk
   and they can never be committed again; `crates/cantus-ui/locales/verses/es.txt` too.
2. **The links to them** (173 in 12 `research/**/*.md` files) become plain text:
   `tools/public/unlink_screenshots.py` turns an inline link into its label (or the file
   name for an image with no alt text) and removes reference-style definitions, turning
   their uses into plain labels. Other mentions of the folder in prose (`docs/BETA.md`,
   `docs/beta-checklist.md`, `docs/next-session.md`, round reports, the memory) are
   history and may stay; BETA.md's blocker 6 and the checklist item get ticked.
3. **The RVR1960 file stays out**: the build no longer needs it (applied in round 26), and
   `crates/cantus-ui/locales/verses/README.md` says the file is kept privately until the
   permission (its table row, and "`es.txt` stays in the repository" in the 2026-10-11
   update). `LICENSE` and the notices already say so (round 26).
4. **The RVR1960 quotations** read the RV1909 in `web/src/body.rs`, the release notes and
   (older commits of) `docs/round20/verse.md` — by the rewrite's `--replace-text` in route
   (a); in the tree they already read the RV1909 (round 26), so route (b) needs no edit and
   `--replace-text` finds nothing to replace in the newest commits. The live site keeps
   the RVR1960 motto until its next deploy.
5. Optional, the owner's call (the private note): the machine-setup notes; whether
   `docs/claude-memory/` stays in the public tree.

When the RVR1960 permission arrives (before or after going public), one commit:
1. put `crates/cantus-ui/locales/verses/es.txt` back (and take it out of `.gitignore`);
2. restore `include_str!("../locales/verses/es.txt")` in `Translation::source`;
3. delete the compile-time `assert!(!RVR1960_PERMITTED, …)` beside the constant;
4. set `RVR1960_PERMITTED = true`;
5. remove the RVR1960 entry from `FORBIDDEN_PATHS` in `tools/public/scan_rules.py` and the
   es.txt probe and pattern from the tree check in `tools/public/check.sh`;
6. delete `tools/public/rvr1960.fingerprints` and `rvr1960-4.fingerprints` (`scan.py`
   skips the RVR1960 check without them, with a warning);
7. drop the es.txt `--path-regex` and the `--replace-text` rules from the plan if it has
   not run yet; restore the RVR1960 notice in `xtask/notices/head.md` and `LICENSE`.

## 9. `tools/public/` — what can be checked again with one command

| File | What it does |
|---|---|
| `check.sh [--forbid FILE \| --no-forbid] [--no-gitleaks] [REPO] [REF…]` | The day's gate, **read-only**, runnable on any clone: (1) `scan.py` over every commit of the refs (all refs by default; `master --tags` for what is published); (2) **gitleaks**, over the same commits, merges included — **mandatory**: without it on `PATH` or in `$GITLEAKS` the check refuses to run unless `--no-gitleaks` is passed, and then it can only say "CLEAN, BUT NOT A PUBLISHING CHECK"; (3) the tree: `research/zz/screenshots/x.png`, a nested `…/deeper/screenshots/…` and `es.txt` probed against `.gitignore`, nothing tracked under those rules, no inline or reference-style link to the screenshots. Refs: names and only `--all`, `--tags`, `--branches`. Ends `READY` (exit 0) or `NOT READY` (exit 1); exit 2 when it cannot run. |
| `scan.py` (+ `scan_rules.py`, `scan_git.py`, `scan_report.py`) | The history scanner (Python 3.8+, standard library only): `git ls-tree -r` of every commit, one `git cat-file --batch` for every distinct blob (text; UTF-16 with a BOM decoded; other binaries by their printable ASCII runs), commit messages, annotated tags, raw author/committer/tagger identities. **Fails** on forbidden paths (case-insensitive regexes: `research/**/screenshots/**`, any image or PDF under `research/`, the RVR1960 file), RVR1960 6-word fingerprints, secret patterns and secret-shaped files, and any entry of the **forbid list** (a file outside the repository: `--forbid FILE`, else `$CANTUS_FORBID`, else `~/.cantus-public/forbid.txt`; missing → exit 2 unless `--no-forbid`; inside the repository → refused). Lists for review: RVR1960 4-word runs, low-confidence assignments, images outside our picture folders, identities, e-mails, local user paths. A refused ref or **no commit to scan → exit 2** (never a false CLEAN). `--filter-repo-args` prints the `--path-regex` arguments; `--json OUT`. ~60 s on this history. |
| `rvr1960.fingerprints`, `rvr1960-4.fingerprints` | 1 666 (6-word, failing) and 1 424 (4-word, review) salted SHA-256 hashes (20 hex digits) of the RVR1960 file's word runs that do not occur in the RV1909 or the WEB. **No Bible text.** |
| `make_fingerprints.py` | Rebuilds both from a tree that holds `es.txt` (the owner's private one). |
| `textnorm.py` | The shared normalisation: HTML entities, `\u{…}`/`\uXXXX` escapes, words hyphenated across lines and escaped `\n`/`\t` undone first; then accents, case and punctuation folded. |
| `motto_replacements.py` | Writes the `--replace-text` rules (Col 3:23 whole and in part, its caption, Ps 33:3's five words) from the private `es.txt` and the RV1909, **outside** the repository. |
| `unlink_screenshots.py` | The day-of link cleanup in `research/**/*.md` (inline and reference-style, case-insensitive). |
| `rvr1960-out-of-tree.patch` | `verse.rs` and `tests/verse.rs` without the compile-time need for `es.txt` — **applied in round 26** (the integrator's commit, plus the NotFound fix on this branch); kept for the record. |

Today, on a fresh clone of the owner's history (408 commits), `scan.py` with the forbid
list finds 480 forbidden paths, 7 RVR1960 blobs and the forbid entries in the old
identities and in this branch's own intermediate report blobs — `NOT READY`, as it must be
until the day.

## 10. Tests and what each holds

- **The out-of-tree change** (`verse.rs`, `tests/verse.rs`), in the rewritten copy (no
  `es.txt`): `cargo test -p cantus-ui --test verse --test about` 16 + 3 passed; the whole
  `cantus-ui` suite 845 passed, 0 failed; clippy `-D warnings` clean; seen failing first
  (the `include_str!` compile error; then the first draft's lifetime error and its clippy
  lint). With the file present 16 + 3 pass; with a damaged file the passage-list test
  fails. **After review #13**: only `NotFound` while not permitted counts as absent — with
  a directory in place of `es.txt` (another read error) three tests now fail, where before
  they would have skipped silently; absent and present still pass.
- **xtask**: `cargo run -p xtask -- notices` regenerated `THIRD-PARTY-NOTICES.md` (only the
  RVR1960 paragraph moved); `cargo test -p xtask` 19 passed.
- **`check.sh` / `scan.py`**: on the original history (`NOT READY`), the paths-only rewrite
  (`NOT READY`), the full plan with the day-of commit on `r26/integration` (`READY`, §7),
  route (b) (`READY`), and a synthetic repository with one hidden finding of each kind
  (every one caught, §3). Refusals checked: `--refs master --json-x` and a missing forbid
  list both exit 2.
- **gitleaks 8.30.1**: `--all`, `--all -m` and `dir` on the original history, all
  `no leaks`; inside every `check.sh` run since.

## 11. Files outside `tools/public/` and my documents

All by the integrator's leave for the review fixes: `crates/cantus-ui/tests/verse.rs` (the
NotFound rule, on top of the integration's applied patch `7e3b129`, cherry-picked here as
`185805b`), `docs/round20/verse.md` (one line: the RVR1960 words
replaced by a description), `LICENSE` (the RVR1960 paragraph), `xtask/notices/head.md`
(the RVR1960 bullet, minimal) and `THIRD-PARTY-NOTICES.md` (regenerated; r26/content
regenerated it too — regenerate after merging).

## 12. Left

- The day itself — **not now** (the owner's word): route (a) §7.
- The owner's choices: identities, the RVR1960 quotations, the memory folder, (a) or (b).
- The live site still shows the RVR1960 motto (until a deploy with the RV1909 wording).
- `docs/history-map.txt` (the commit map) if old hashes in documents should be
  translatable.

## 13. Notes for the integrator

1. **Squash round 26 into one commit on `master`.** Intermediate commits of this branch hold
   report and ADR blobs that named the work address (since corrected): squashed, they never
   reach `master`, and `check.sh --forbid …` on `master` stays clean of them.
2. **Merge this branch again**: it adds the NotFound rule to `tests/verse.rs` — **one
   conflict there, in `rvr1960()`: take this branch's side** (`git checkout --theirs
   crates/cantus-ui/tests/verse.rs`; rehearsed) — the licence/notices fix (regenerate the notices
   after merging r26/content too: `cargo run -p xtask -- notices`), the round-20 line, and
   the hardened tools. Run `cargo test -p cantus-ui --test verse --test about` and
   `cargo test -p xtask`.
3. **The live website** quotes the RVR1960 (`web/src/body.rs`; the wasm served today has
   it): the release builder's `web/` — the RV1909 wording on the next deploy. Same for
   `docs/releases/v0.1.0-beta.1.md`.
4. The private folder `F:\projects\daw-r26\private` (`forbid.txt`, `identities.mailmap`,
   `public-identity.md`) is **not in any repository**; tell the owner where it is, and keep
   it out of the cleanup.
5. Cleanup when the round closes: `F:\projects\daw-r26\public-copy` (4.6 GB `target/`; a
   stash and the patch in its working tree) and `F:\projects\daw-r26\public-tmp` (downloads,
   rehearsal clones, scan outputs).
6. `check.sh` is `eol=lf` (root `.gitattributes`); `tools/public/.gitattributes` keeps the
   patch, the Python and the fingerprints LF too.

## Appendix — the copy's commit map (`master`, paths-only rewrite; an example only)

The real run (§7) gives other hashes (later commits, the motto rule, the mailmap). 186 of `master`'s 294 commits changed (241 of 349 over all refs); the 108 before `b0dce1f` kept theirs.

<details><summary>186 rewritten commits on master, oldest first</summary>

| Private (old) | Rewritten copy (new) | Date | Subject |
|---|---|---|---|
| `b0dce1fe63` | `3d39c42f57` | 2026-10-05 | verse: a verse at every start (WIP): WEB, RVR1960 and RV1909 files fr… |
| `d770626081` | `3f3c0904cd` | 2026-10-05 | verse: tests pass; prose wrap leaves a pixel; shoot_verse example; ro… |
| `dffb6d2e1b` | `241d75b71c` | 2026-10-06 | verse: the splash band sized to its passage, lines balanced, card edg… |
| `1bc267908c` | `59812df0a8` | 2026-10-06 | verse: clippy and fmt |
| `f26f208c51` | `26b8f72718` | 2026-10-06 | Merge r20/verse (the verse card draws under the picker) |
| `41baa535c2` | `771f4a21d6` | 2026-10-06 | Merge r20/automation (keymap v3 also fills in round 19's keys) |
| `92a0556812` | `f84d0e759d` | 2026-10-06 | Merge r20/device-ux |
| `35ed2aef80` | `e438baf3ae` | 2026-10-06 | Swapping an effect points its automation at the new one, in the same … |
| `8785692ae5` | `ff251fd331` | 2026-10-06 | Merge r20/audio-setup (status bar: the device tray left, the audio ch… |
| `14c694c47c` | `12940744ff` | 2026-10-06 | Merge r20/hosted-inst (schema 20 after automation's 19; one instrumen… |
| `00378a81cf` | `7bbd7fa017` | 2026-10-06 | ADR 0029 §9: swapping, from device-ux; automation follows a swapped e… |
| `9ef0464f8d` | `1d852bff31` | 2026-10-06 | Merge r20/packaging (its ADR renumbered 0032; samples kept 0030) |
| `6b74c39756` | `e31bebd48a` | 2026-10-06 | ADR 0032: the numbering note |
| `74c26e6fb8` | `2ac8297664` | 2026-10-06 | device_ux test: a hosted instrument fits a track now (ADR 0028) |
| `0e8df266b7` | `82841a28c0` | 2026-10-06 | Merge r20/bench (the status bar shows the chip and the dropout's culp… |
| `13719a5ee2` | `1c3f05a6b7` | 2026-10-06 | STATUS: round 20 |
| `be216de34b` | `967d912555` | 2026-10-06 | perf_status test drops the frame's textures like the other headless t… |
| `37db4bcf4b` | `655ada082c` | 2026-10-06 | Vendor cpal 0.18.2 and asio-sys 0.4.0 unchanged (to patch ASIO) |
| `49f02f34d5` | `728e738236` | 2026-10-06 | Round 21 (performance, wave 1): the brief |
| `dd68910409` | `ceecf4432b` | 2026-10-06 | sched: WIP scheduler (both queues, pool, fuzz) and tests |
| `58fe965e0e` | `c93d42fef1` | 2026-10-06 | cantus-rt: thread priority, topology, padding, memory, fpenv, denorma… |
| `49ec3cbdce` | `da7fe0f8cc` | 2026-10-06 | sched: tests pass; park test robust to a loaded machine; buggify with… |
| `3c8113b9aa` | `987cd52c4d` | 2026-10-06 | cantus-rt: tests, probe, re-assert after hosted plug-ins, jitter and … |
| `dfd93b42de` | `d7efd2f215` | 2026-10-06 | at-rest: FxUnit/InstrumentUnit/EffectUnit::at_rest and skip, cantus_d… |
| `f0537b8a41` | `83141dd958` | 2026-10-06 | WIP r21/asio: patched cpal/asio-sys (buffers once before ASIOStart, o… |
| `8be60f574b` | `b56f8b597e` | 2026-10-06 | at-rest: Latitudo, Fervor, Porta (MIDI mode), Terminus; filter primit… |
| `654a67a730` | `813184d9bb` | 2026-10-06 | WIP math: route every transcendental through cantus_dsp::math (std ba… |
| `4f6429cedc` | `4918020988` | 2026-10-06 | WIP graph: plan, gather, outboxes, nodes, limits 256, FTZ shim (not y… |
| `d7f11f20ad` | `0d69503da0` | 2026-10-06 | sched: fan-out stall test; bench separates stalled blocks; report dra… |
| `decc5d03d3` | `fbff8aae89` | 2026-10-06 | at-rest: Multiplex (and its linear split), Culmen, Sibilus; Terminus'… |
| `bc4f94f3e1` | `034d941fac` | 2026-10-06 | WIP graph: compiles; graph_order, limits, ftz pass |
| `31c042c990` | `9bdced58b9` | 2026-10-06 | sched bench: rounds with medians, fixed work size, MMCSS promotion th… |
| `7ed8119d32` | `5014baf2f6` | 2026-10-06 | at-rest: Comprimo, Aequalis (all three modes), Lamina; +0.0 bit for b… |
| `ca3bfc41c7` | `66998ae30c` | 2026-10-06 | Comprimo's loudness meters flush both state variables at once, so sil… |
| `1437010ad3` | `a05b8380c7` | 2026-10-06 | at-rest: the instruments — Calor, Unda, Modulus, Exemplar, Gravis (+ … |
| `3f75fbd71c` | `1a351e0665` | 2026-10-06 | graph: promotion on the first block aligned with cantus-rt's API; eng… |
| `067549ab3d` | `87230bc1b7` | 2026-10-06 | cantus-rt: no-alloc test, Linux affinity fix, mac timebase; jitter an… |
| `556b925ceb` | `a8fe461249` | 2026-10-06 | Terminus's loudness sections flush both state variables at once, so s… |
| `3502c0083c` | `efe7f9d588` | 2026-10-06 | at-rest: Porta skips only a MIDI gate's start; blockers named per see… |
| `de5540ed63` | `b76271b8b8` | 2026-10-06 | Multiplex's detectors flush their filters' two states together, so th… |
| `7934823bbb` | `b275071e96` | 2026-10-06 | WIP r21/asio: ignore ASIO4ALL's start-up reset, keep a named driver l… |
| `94dbb0e67b` | `e8567fdf5c` | 2026-10-06 | sched: both ready queues, the real-time pool and the PCT schedule fuz… |
| `e077ab7f90` | `9d71d9547b` | 2026-10-06 | at-rest: a skipped linear split asks for its filters at the block sta… |
| `bd3ec8b0c7` | `919c20869b` | 2026-10-06 | at-rest: Gravis's single voice rests with a released slot; temporary … |
| `3ef5129f12` | `4b3901064c` | 2026-10-06 | at-rest: remove the temporary diagnostics test |
| `a09ddaf13f` | `d1952d2721` | 2026-10-06 | at-rest: rustfmt; docs/round21/at-rest.md and requests.md (draft) |
| `972b849f99` | `9ab22a13f2` | 2026-10-06 | WIP r21/asio: SDK download via PowerShell then curl, rustfmt, round-2… |
| `7f9971b882` | `467dadcd6b` | 2026-10-06 | cantus-rt: MMCSS spin experiment; workers promoted by how they wait (… |
| `0eb17a73e7` | `1bb447a143` | 2026-10-06 | graph: the strip's gather charged to the strip in the profiler; drop … |
| `af74c7b120` | `30c0847944` | 2026-10-06 | DSP_VERSION 2: every transcendental from the pinned pure-Rust libm (A… |
| `fdb7611cf5` | `91a2bc8577` | 2026-10-06 | cantus-rt: safe worker promotion compiles and passes; rt.md §5 (MMCSS… |
| `863764ff15` | `2cd7c66985` | 2026-10-06 | Merge r21/rt |
| `dcc878444d` | `1ece30ac8a` | 2026-10-06 | Merge r21/sched |
| `64f418bf22` | `c8f55fea57` | 2026-10-06 | The road to the first public beta (0.1.0-beta.1) |
| `7cff1908af` | `d33c5a5674` | 2026-10-06 | at-rest: clippy (type alias for the split runs, is_multiple_of) |
| `a8380b6b5d` | `00807c357e` | 2026-10-06 | graph: the processor's node buffers are touched once where it is buil… |
| `26d2a3d628` | `bc3b00f327` | 2026-10-06 | math: round 21 report (accuracy, speed, the bump's size, what is left… |
| `ec641c4bd2` | `45333ac145` | 2026-10-06 | docs/round21/graph.md: the graph builder's report |
| `135513201b` | `1d7797adc1` | 2026-10-06 | Merge r21/at-rest |
| `4cc7f3ed56` | `82cd915343` | 2026-10-06 | Merge r21/graph |
| `7fd3b63153` | `2a0e56a3be` | 2026-10-06 | The engine calls cantus-rt directly (promotion, denormal guard) and f… |
| `05db70cdb1` | `0153d2d649` | 2026-10-06 | math.md: the UI suite on libm |
| `42d199513b` | `f2b4f3cfb0` | 2026-10-06 | at-rest: docs |
| `113e2bb6c8` | `eb1141693e` | 2026-10-06 | r21/asio: round-21 doc with live results; package test counts one edi… |
| `2937af3d1f` | `14e2996554` | 2026-10-06 | Merge r21/math (DSP_VERSION 2); graph's last std transcendentals go t… |
| `10edba9138` | `9ef59906f7` | 2026-10-06 | r21/asio: clippy (example) |
| `59cd0c8dbb` | `46e149b5d2` | 2026-10-06 | third_party/cpal: allow frames_to_duration unused (path dependency wa… |
| `fe6aa48241` | `588a79367b` | 2026-10-06 | third_party/README: note the cpal lint allowance |
| `ca1fe281cd` | `19b8f14909` | 2026-10-06 | Latitudo and the heavy test signals through cantus_dsp::math too; gra… |
| `0cd54f1c1e` | `11ca45c092` | 2026-10-06 | Merge r21/asio (one build with every driver; cpal/asio-sys patched fo… |
| `3058b78b72` | `544345c9a5` | 2026-10-06 | backend: the promotion line graph wired replaces asio's TODO |
| `80041f2730` | `459a9757f3` | 2026-10-06 | fmt |
| `41c9324779` | `ab3cc98bcc` | 2026-10-06 | at-rest: docs/round21/at-rest.md — the units, the findings, the measu… |
| `660dcd9446` | `4caf591c98` | 2026-10-06 | Merge r21/at-rest (its final docs) |
| `33f9c733ad` | `bf6cdc44ce` | 2026-10-06 | STATUS: round 21; graph_order's reference model allows an index loop |
| `c8bc9f21b3` | `32194a52d1` | 2026-10-06 | Round 22: the brief (the multicore live pool, beta hardening) |
| `c3ae9e7fc1` | `4d9fbbcad7` | 2026-10-06 | build: cantus-launch, the start-up stub that picks the x86-64-v3 or v… |
| `b90fbb04e2` | `b849dc4378` | 2026-10-06 | beta-docs WIP: version 0.1.0-beta.1, a beta's update check asks for t… |
| `66ca9a014c` | `dedc839cd4` | 2026-10-06 | soak WIP: session journal, recovery offer, refusals named, autosave f… |
| `9cfbb5c6b7` | `788ed69463` | 2026-10-06 | beta-docs WIP: notices generator and THIRD-PARTY-NOTICES.md, manual E… |
| `a1f9c21f8e` | `72c3d7aee5` | 2026-10-06 | WIP export: cantus-export crate (settings, tags, pipeline, WAV/FLAC/M… |
| `14147367bf` | `3cdee59e2d` | 2026-10-06 | build: xtask package ships the launcher, the v2 and v3 programs and t… |
| `28ddacb5be` | `d651e6d3e9` | 2026-10-06 | WIP export: the export dialog in the kit, settings, locales, ADR 0033… |
| `4cd620481c` | `64ca1207fa` | 2026-10-06 | WIP export: Cargo.lock |
| `0f627cddcd` | `0ee51bd897` | 2026-10-06 | WIP pool: engine graph on cells, sched topology owned by the pool, bo… |
| `47953ae270` | `6ae119fc29` | 2026-10-06 | soak WIP: crash tests, recovery flows, soak harness, the fast-undo ef… |
| `570fbc4041` | `53f31c829b` | 2026-10-07 | soak harness: never wait for the process sampler; log slow frames wit… |
| `debf6142b1` | `7b1c629ed3` | 2026-10-07 | export: the crate's tests pass (FLAC/WAV bit-exact, MP3 gapless, Ogg,… |
| `037979f0da` | `b3d58d35b5` | 2026-10-07 | build: launcher's probe type; corpus loop; docs draft; fmt |
| `a2e47f0b4d` | `68e687ca0c` | 2026-10-07 | soak harness: count every pass's recorded frames; clippy |
| `c4f0f9fe88` | `956c49cd53` | 2026-10-07 | beta-docs WIP: manual pictures, About polish, About tests, round doc … |
| `a6819891f9` | `96c1522228` | 2026-10-07 | WIP pool: tests (pool bit-identity on random sessions, fuzz, wakes, p… |
| `51ac1be70f` | `9317cf3379` | 2026-10-07 | export: the dialog's tests pass (persisted choices, off-thread export… |
| `c4f2be1ee5` | `55f5d96b50` | 2026-10-07 | The autosave is written on a thread (the soak saw a 0.7-1.8 s frame e… |
| `96f737a6b5` | `44d5e8bbc6` | 2026-10-07 | beta-docs: the round's report; About's motto test |
| `33e5c023ac` | `07da6758c4` | 2026-10-07 | export: clippy clean, controls fill their rows, shorter stem labels, … |
| `896ad67dad` | `6b187680e0` | 2026-10-07 | WIP pool: A/B example, paced bench subcommand |
| `1cd0fbc801` | `8bfb953a83` | 2026-10-07 | Manual frictions: New/Open ask about unsaved changes, a new audio tra… |
| `65a81c881b` | `c44e9e5a75` | 2026-10-07 | build: null corpus — demos as eight bars (--whole for the songs), aud… |
| `30f8b9c31a` | `f4b79533bf` | 2026-10-07 | export: the round's report |
| `625d23c9ed` | `3f3dca3a84` | 2026-10-07 | Merge r22/beta-docs |
| `594d3a2dd5` | `90de32d863` | 2026-10-07 | Merge r22/export |
| `656c442721` | `ebf8e89666` | 2026-10-07 | LGPL allowed for LAME (MP3 export); notices regenerated; the manual a… |
| `f451589e38` | `35a3397d71` | 2026-10-07 | Round 22 soak: report, curves; rec_flows follows a new audio track th… |
| `a3a0510d82` | `8919efaa2b` | 2026-10-07 | Notices: the BSD-3-Clause standard text (the Vorbis crates declare it… |
| `b93e6f4b6d` | `a4cb88bfb0` | 2026-10-07 | WIP pool: render_with_progress (export's request), count-in clicks wh… |
| `8005e6817e` | `e6a3ef7ece` | 2026-10-07 | Merge r22/soak (crash recovery, autosave off the UI thread, clean ref… |
| `04d308ae3c` | `4253ce82e8` | 2026-10-07 | docs/DEVELOPING.md: a new machine from a fresh clone (tools, ASIO SDK… |
| `7143e4d2ba` | `f76e23703c` | 2026-10-07 | WIP pool: the callback's wait on a parallel pass is nobody's time (bl… |
| `3065931c26` | `762be8d379` | 2026-10-07 | pool: clippy and fmt |
| `5a7ab5376f` | `a094ba9c8a` | 2026-10-07 | pool: the critical path's head is seeded to the callback; report draft |
| `e3add4a79d` | `cd740369e0` | 2026-10-07 | DSP_VERSION 3: our own f32 powf, db_to_gain, counter-based dither (R1… |
| `34066f9a31` | `f7fbd58845` | 2026-10-07 | Round 22 math2: report (docs/round22/math2.md) |
| `b30985a1c5` | `3629e255a6` | 2026-10-07 | build: xtask perf --sizes and High priority for paired runs, zero-rat… |
| `64693b5d44` | `4c49daac67` | 2026-10-07 | build: the null corpus's own test; ADR 0031 §5.3 records what round 2… |
| `f6a59648fe` | `5a77347827` | 2026-10-07 | build: the null-hash list as evidence; doc references after the clean… |
| `e213cd9397` | `ec499ff155` | 2026-10-07 | pool: spin share configurable, 0.1 by default (paced runs); paced sho… |
| `e7148678b5` | `1c7b8e048e` | 2026-10-07 | build: doc, the clean-up's size |
| `29c5085e89` | `313ebab6b4` | 2026-10-07 | build: doc, files outside the area |
| `6f58f38504` | `cca206e67e` | 2026-10-07 | pool: report draft; wake test follows the spin share |
| `aef668d2e1` | `0317be9a19` | 2026-10-07 | Merge r22/build (x86-64-v3 and v2 programs, the launcher, null-hashes… |
| `75262b6851` | `20e986083c` | 2026-10-07 | Cargo.lock: cantus-launch takes the workspace's 0.1.0-beta.1 |
| `4c5841b64a` | `544796a181` | 2026-10-07 | Research screenshots travel with the clone (the owner, 2026-10-07; th… |
| `97a8f89f5a` | `09a154cf2d` | 2026-10-07 | docs/claude-memory: Claude Code's memory for the project travels with… |
| `61d5078cd9` | `aef3dfde49` | 2026-10-07 | pool report: final paced run |
| `4813ac1e7c` | `c49304f11a` | 2026-10-07 | docs/round22/pool.md: the report |
| `b8d0371eec` | `6a596d16ee` | 2026-10-07 | Merge r22/pool (the multicore live engine: the graph on cells, the sc… |
| `25f9aed325` | `6c71b7fa39` | 2026-10-07 | Merge r22/math2 (fast deterministic powf, counter-based dither, Aequa… |
| `5bc1d0ceee` | `df164a2a45` | 2026-10-07 | STATUS: round 22; the manual and release notes follow the soak's fixe… |
| `baba1a84f4` | `11b27c0b78` | 2026-10-07 | Integration: export seeds its dither with a u64 (math2's Ditherer); t… |
| `40641bbf26` | `ea5c69e4cc` | 2026-10-07 | Backlog §3b: more effects — distortion pedals, fuzz, chorus, flanger,… |
| `1d40ad6fc6` | `4508ce1781` | 2026-10-07 | Backlog §3c: a Melodyne-class pitch and time editor for recordings (t… |
| `33bf936d07` | `928f93bfd9` | 2026-10-07 | Backlog §3d: a simple real-time auto-tuner (scale or chosen notes or … |
| `5f28e2482d` | `70f7b24b82` | 2026-10-07 | Backlog §0: the final goal — undeniably the best DAW in the world, fr… |
| `7dc5c9747b` | `0c9fd355e9` | 2026-10-07 | CLAUDE.md: the final goal (backlog §0) |
| `2c34fa6363` | `708924dbd5` | 2026-10-07 | Backlog §6a: the website and backend on cantusdaw.com / cantusdaw.stu… |
| `745fb702e7` | `ca230f5ac7` | 2026-10-07 | STATUS header: round 22, 1 824 tests; Claude's memory refreshed |
| `7b66dde5e7` | `f8860d583a` | 2026-10-07 | STATUS: the fresh-clone check; where the owner's build is |
| `7a841ec795` | `2fb638b763` | 2026-10-07 | STATUS: the fresh clone's result; the build's path |
| `6b3493d1c5` | `89bd82b7e4` | 2026-10-07 | STATUS: the owner's test build (the multicore engine on by default, e… |
| `2ad043bf36` | `365c4ded47` | 2026-10-07 | backlog update |
| `cead4402e8` | `a85c748e03` | 2026-10-09 | Round 23: the beta blockers — export dialog, text after a theme chang… |
| `59ab765739` | `b348898ff4` | 2026-10-09 | The website: a Yew single-page app for cantusdaw.com (ADR 0034) |
| `a9871e2883` | `511a1907b5` | 2026-10-09 | Backlog 4c: a recording mode for high performance; site: cache and se… |
| `5e93f09afe` | `fc0366792b` | 2026-10-10 | Site: a line of faith in the footer; backlog: Linux first, then macOS |
| `68af8410b0` | `e2d74a39ab` | 2026-10-10 | Site: a workflow that publishes it to Cloudflare Pages on every push |
| `a1580ff461` | `9931663b14` | 2026-10-10 | Site workflow: list the Pages projects before deploying; pin Ubuntu 2… |
| `656251d007` | `39b3795fa2` | 2026-10-10 | Site workflow: deploy to the Cloudflare Worker that serves the site |
| `6d5907c8e4` | `b7169efc29` | 2026-10-10 | Site: a favicon without a tile, made for tab sizes |
| `a89edf4ed8` | `61cffc7369` | 2026-10-10 | Round 24: the beta's rest, linked clips, performance wave 2, the soak… |
| `14ddaf5c3a` | `ee71300f23` | 2026-10-11 | Round 25: mixer cabling, the theme family, the taskbar mark |
| `fe66f79185` | `d5f313348e` | 2026-10-11 | Round 25 polish: exact 0 dB pan law, automatic icon colour, cable leg… |
| `c2e500d244` | `617aa75bd1` | 2026-10-11 | Site screenshots WIP: Night in the shoot examples, shoot_site, fx pic… |
| `f906bcb925` | `3cbe3821ec` | 2026-10-11 | Merge branch 'r25/polish' into r25/site |
| `956ed177ec` | `f79bd24b85` | 2026-10-11 | Beta checklist: the CANTUSDAW question, the marks found, sources |
| `7586b2d5e2` | `96993bcd81` | 2026-10-11 | RVR1960: the permission request (English and Spanish) and the list of… |
| `63fa2d9933` | `0ce857556f` | 2026-10-11 | The Cantus mark: ™ on the site, README and About; About's licence tex… |
| `c1c296113f` | `efc03bd2b4` | 2026-10-11 | Cable view options: scope, kinds, detail, quick views (WIP) |
| `15d7da9e1b` | `6b4557f963` | 2026-10-11 | Site: every app screenshot regenerated in Cantus Night (cables and le… |
| `debf594778` | `3672fb86ff` | 2026-10-11 | Site hero: playing, ASIO device connected, the desk ends cleanly |
| `bdeb9e75c7` | `17e5e1b31c` | 2026-10-11 | RVR1960 waits for its publisher's written permission: not in the buil… |
| `2d8fb284e6` | `0c617cfbd8` | 2026-10-11 | Merge branch 'r25/site' into r25/polish |
| `ee30aaf1f0` | `18cf0f86d8` | 2026-10-11 | Cable options: the desk menu holds them, the key gives way on a narro… |
| `c9b8acdcf0` | `f057f4e88f` | 2026-10-12 | Cable view options: round report |
| `3d3aa97649` | `a949ccc810` | 2026-10-12 | Site: the trademark sign small and light beside the name, not the dis… |
| `22f69c85df` | `767df5feeb` | 2026-10-12 | Merge branch 'r25/cables-view' into r25/polish |
| `4bf5bbeab1` | `5ab689ac3e` | 2026-10-12 | STATUS: the cable view options, the RVR1960 gate, the site's screensh… |
| `9f5a1f6c7f` | `be3ceb327c` | 2026-10-13 | Cantus Night refined for long sessions; gradient tokens (sheen, glows… |
| `da7ddb2f16` | `d1aa6a7ae9` | 2026-10-13 | Night: gradient tokens registered for inspect mode; budget test; roun… |
| `cc5bfead10` | `5af8598cdd` | 2026-10-13 | Night: glows at 8 rings x 32 segments (same look, +1.1k triangles, no… |
| `68741778e2` | `bf70d648f7` | 2026-10-13 | Night: wider glows, a quieter cool one (no dark crescent between them… |
| `24654bd8d8` | `374e6f2715` | 2026-10-13 | Merge branch 'r25/night' into r25/polish |
| `20f07c8772` | `647207f00b` | 2026-10-13 | Site: the hero in the refined Cantus Night |
| `8202241297` | `0cfae14e29` | 2026-10-13 | STATUS: Cantus Night refined (2 164 tests) |
| `26284c0389` | `629cdf375e` | 2026-10-13 | Linux: CI's -D warnings — Windows-only code no longer warns elsewhere… |
| `47a28bd3c9` | `83f9fb3b6f` | 2026-10-13 | Linux: the VST3 editor's test helpers are Windows-only so far; check.… |
| `ef00744234` | `91d6f819a2` | 2026-10-13 | Linux: the CLAP editor queue, the test plug-ins' windows, the ASIO ex… |
| `23c39f69ea` | `2589c5ed6d` | 2026-10-13 | cargo fmt --all: CI's format check (24 files left unformatted by earl… |
| `e340ead43d` | `fee139bae2` | 2026-10-13 | Linux: the release's level programs are .exe wherever it is packaged;… |
| `d1700fe31f` | `aaa2858823` | 2026-10-13 | tools/wsl/check.sh: cargo test has no --keep-going |
| `4da32acde6` | `bcab8c2e8a` | 2026-10-13 | CI: no debug info in test builds (the Ubuntu runner ran out of room l… |
| `18a99b67c7` | `b2e11a8a48` | 2026-10-13 | tools/wsl/check.sh: also the plug-ins without their editor, as xtask … |
| `3ad6ca69e1` | `df7039a28b` | 2026-10-13 | macOS (Apple silicon): the launcher's x86-64 test helpers are quiet e… |
| `aa137bf788` | `649a8c35bc` | 2026-10-13 | Null hashes, Windows = Linux: the bench's test signal used std's sin … |
| `ffffe50c34` | `1a86d818c4` | 2026-10-13 | macOS (Apple silicon): the launcher's level test is x86-64's; elsewhe… |
| `78c8ff58f2` | `10fe3403de` | 2026-10-13 | Next session: keep the history until public; make GitHub Actions chea… |

</details>
