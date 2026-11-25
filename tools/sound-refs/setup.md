# Sound references: setting up a machine (round 42, sound-data)

Every sound tool — the lab (`tools/soundlab`), the bowed-strings study and Arcus's passes
(`tools/arcus`), the listening kits (`tools/listening`, `crates/cantus-ui/examples/*_abx`),
Chorda's voicing lab (`tools/chorda-voicing`), Tympanum's fit (`tools/tympanum-fit`) — reads
its recordings from one folder outside every repository and every round's folder. Deleting a
round deletes no data; a new machine rebuilds it with the commands below.

| variable | what | default |
|---|---|---|
| `CANTUS_REFS` | the reference sets, at the sub-paths `catalog.toml` names | `F:\projects\cantus-refs` (Windows), `~/cantus-refs` |
| `CANTUS_WORK` | scratch, renders, the lab's runs, listening kits, download cache | `cantus-work` beside `CANTUS_REFS` |
| `CANTUS_STUDY_REFDB` | the bowed-strings study's measured database (per-note records, catalogue, plots) | `$CANTUS_REFS/derived/refdb` |
| `CANTUS_PY` | the Python with `requirements.txt` | `$CANTUS_REFS/venv`, else `python` |

`cantus_paths.py` (Python) and `cantus_paths.sh` (sourced by the shell scripts) give these
defaults; the older names (`SOUNDLAB_REFS`, `SOUNDLAB_WORK`, `SOUNDLAB_LISTEN`,
`CANTUS_STUDY_DOWNLOADS`, `ARCUS_REFS`, `ARCUS_UPPER_REFS`, `TYMP_CONTENT`, `TYMP_WORK`,
`CHORDA_LAB`) still override them where a script read them before.

## 1. The Python environment (once)

```bash
python -m venv /f/projects/cantus-refs/venv          # Python 3.13; any 3.11+ works
/f/projects/cantus-refs/venv/Scripts/python -m pip install -r tools/sound-refs/requirements.txt
export CANTUS_REFS=/f/projects/cantus-refs            # or leave the default
export CANTUS_PY=$CANTUS_REFS/venv/Scripts/python.exe # bin/python on Linux and macOS
```

`requirements.txt` pins numpy, scipy, soundfile and matplotlib at the versions the round-42
study and lab ran with; it covers every sound tool (the lab's own file is a subset).
`fetch.py` itself needs only the standard library.

## 2. The references

```bash
python -I tools/sound-refs/fetch.py list                 # every set: files, size, licence, use
python -I tools/sound-refs/fetch.py fetch                # every set a script can download (~8 GB)
python -I tools/sound-refs/fetch.py fetch arcus study-*  # or chosen ones (ids, globs)
python -I tools/sound-refs/fetch.py fetch tympanum tympanum-extra piano piano-bitklavier --packs --cargo
python -I tools/sound-refs/fetch.py manual               # what only a hand can fetch
python -I tools/sound-refs/fetch.py verify               # sizes and SHA-256 of what is there
```

- A remote zip is never downloaded whole: its central directory and the members a set needs
  are read by HTTP range requests (of the 14 GB Good-sounds zip, only the ~1.4 GB the two
  sets take).
  TinySOL's `.tar.gz` cannot be range-read: it is downloaded once (1 GB, resumable) into
  `$CANTUS_WORK/fetch-cache` and removed after (`--keep-cache` keeps it).
- Interrupted? Run it again: a `.part` resumes, files already right are not touched (their
  hashes are cached by size and time in `$CANTUS_REFS/.sound-refs-stamps.tsv`).
- Behind the corporate proxy a file may come back as an HTML page for ~10 minutes: fetch.py
  checks every file's first bytes and waits (`CANTUS_FETCH_WAIT`, 60 s; `CANTUS_FETCH_TRIES`,
  20). It never bypasses the proxy or a TLS check. When Python's own TLS check refuses a chain
  (seen with Freesound's CDN on 2026-11-25), curl is tried — it verifies with the system's
  certificate store.
- The packs (`--packs`): Cantus's own sound packs as the app installs them. A pack the app
  already holds at the manifest's version is copied from its folder
  (`%LOCALAPPDATA%\Cantus\Content`); the others are installed by Cantus's pack code
  through cargo (`--cargo`, or the command `fetch` prints).
- By hand: `ebu-loudness` (tech.ebu.ch answers scripts with a browser challenge),
  `study-papers` (saved from the publishers' pages), `mdb-drums` (not pinned upstream). Place
  the files, then `fetch.py record <set>` writes their checksums.

## 3. What the tools compute from them

```bash
bash tools/sound-refs/rebuild_refdb.sh            # the study's refdb into $CANTUS_STUDY_REFDB (~30 min)
bash tools/sound-refs/rebuild_refdb.sh --pass5    # also vibrato.json and strokes.json (~25 min more)
python -I tools/arcus/study/make_abx.py           # study/abx-refs, the blind-listening references
python -I tools/soundlab/subjects/example-tibia/synth_bar.py refs $CANTUS_REFS/soundlab-example
bash tools/content/strings/map.sh $CANTUS_REFS/strings-rec   # strings-rec's Cantus/ SFZ and strings.json
```

`rebuild_refdb.sh` ends with `compare_refdb.py`: every distribution committed in
`research/instruments-next/bowed-refdb/` against the rebuilt one.

## 4. The checks

```bash
$CANTUS_PY -I tools/soundlab/tests/run.py --light   # needs nothing downloaded
$CANTUS_PY -I tools/soundlab/tests/run.py           # with CANTUS_REFS and CANTUS_STUDY_REFDB set
```

The full run reads TinySOL under `$CANTUS_REFS/arcus/tinysol` and the study's records under
`$CANTUS_STUDY_REFDB/notes`; the EBU files when `SOUNDLAB_EBU=$CANTUS_REFS/ebu-loudness`.

## Maintaining the catalogue

A new set: download it under `$CANTUS_REFS/<dir>/` with its `SOURCE.md` (URL, date, the
licence verbatim, what was taken), add a `[[set]]` to `catalog.toml` with its origins, run
`fetch.py record <id>` (it reads remote zips' central directories to name each file's member,
and copies `repo` texts into `sources/`), check no file is `UNRESOLVED`, then fetch it into an
empty folder (`--refs <tmp>`) and `verify` it there before committing `files/<id>.tsv`.
