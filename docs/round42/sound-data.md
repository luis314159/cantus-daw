# Round 42 — `sound-data`: the sound references, durable

Branch `r42/sound-data` (from `r42/integration` 94cab9580). The owner (2026-11-25,
translated): *"Are all the sound development and sound evaluation tools in the repo and
documented well for future sessions?"* The tools were; the data they read was not. It lived in
the round's own folder (`F:\projects\daw-r42\downloads`, 8.2 GB; the study's refdb in
`F:\projects\daw-r42\study\refdb`; the lab's work in `F:\projects\daw-r42\soundlab-work`), and
old round folders get deleted: round 40's had already taken Tympanum's DRSKit and Virtuosity
installs with it (their junctions in `downloads\tympanum\content` pointed nowhere).

Now every set has a catalogue entry, a checksummed file list and one command that fetches it;
the data has a home outside every round (`F:\projects\cantus-refs`, `CANTUS_REFS`); no sound
tool names a round's folder.

## 1. What is where now

| | where | how it is made again |
|---|---|---|
| the catalogue | `tools/sound-refs/catalog.toml` (35 sets), `files/<set>.tsv` (27 821 files: size, SHA-256, origin), `sources/` (the sets' `SOURCE.md` notes) | — |
| the fetcher | `tools/sound-refs/fetch.py` (`list`, `fetch`, `verify`, `manual`, `record`; stdlib, `python -I`) | — |
| the paths | `tools/sound-refs/cantus_paths.py`, `cantus_paths.sh` | — |
| the references | `F:\projects\cantus-refs\` (`CANTUS_REFS`), 12.3 GB with the packs | `fetch.py fetch [--packs --cargo]` |
| the study's refdb | `F:\projects\cantus-refs\derived\refdb\` (`CANTUS_STUDY_REFDB`), rebuilt here; the round's own copy kept as `derived\refdb-r42\` | `bash tools/sound-refs/rebuild_refdb.sh [--pass5]` |
| the venv | `F:\projects\cantus-refs\venv\` (`CANTUS_PY`), from `tools/sound-refs/requirements.txt` | `tools/sound-refs/setup.md` §1 |
| scratch, renders, kits | `F:\projects\cantus-work\` (`CANTUS_WORK`): `soundlab\` (the lab's runs, was `daw-r42\soundlab-work`), `listen\` (copied: `strings-rec`, `arcus\before`, the Arcus and Tympanum kits, their WAV twins, **their keys**, the two zips), `fetch-cache\` | the tools |
| the ABX page `upper-listen.sh` copies | `tools/listen/abx.html` (it was only on `r42/contract`; the script fell back to `../contract/`) | — |

## 2. The audit

Size is what the catalogue lists (the packs as installed). *Auto* is whether `fetch.py fetch`
brings it without a hand. Use: **ship** (CC0), **ship-credit** (CC BY), **measure** (analysis
only, never shipped), **reading** (documents).

| set | licence | use | size | files | auto | source |
|---|---|---|---|---|---|---|
| `arcus` | custom (Iowa MIS) + CC-BY-4.0 (TinySOL) + CC0-1.0 (VSCO, Karoryfer, Smolken) | measure (Iowa); ship-credit (TinySOL); ship (CC0) | 739.6 MB | 742 | yes | 10.5281/zenodo.3685367 (TinySOL 6.0) |
| `arcus-upper` | CC-BY-4.0 (CNSM) + custom (Iowa MIS) + CC0-1.0 (VSCO)  | ship-credit (CNSM); measure (Iowa); ship (VSCO) | 396.0 MB | 255 | yes | 10.5281/zenodo.18696786 (CNSM Dataset 1.0.0) |
| `strings-rec` | CC0-1.0 | ship | 2.9 GB | 2998 | yes | https://github.com/sfzinstruments/karoryfer-bigcat.cello |
| `study-aalto-anechoic-orchestra` | custom | measure | 125.8 MB | 42 | yes | https://research.cs.aalto.fi/acoustics/virtual-acoustics/research/acoustic-measurement-and-analysis/85-anechoic-recordings.html |
| `study-abx-refs` | per file (index.json): CC-BY-4.0 / CC0-1.0 / custom | ship-credit (TinySOL, HSD) / ship (CC0) / measure (Philharmonia, PHENICX) | 38.0 MB | 33 | derived (a tool remakes it) |  |
| `study-cnsm-violin` | CC-BY-4.0 | ship-credit | 218.7 MB | 122 | yes | 10.5281/zenodo.18696786 |
| `study-freesound-cc0-previews` | CC0-1.0 | measure (the previews are lossy; the originals may ship) | 14.6 MB | 30 | yes (via curl: Python's TLS check refuses the CDN's chain) | https://freesound.org/people/cms4f/packs/9834/ |
| `study-good-sounds-cello` | CC-BY-4.0 | ship-credit | 794.3 MB | 959 | yes | 10.5281/zenodo.820937; 10.34810/DATA2314 (CORA, metadata) |
| `study-good-sounds-violin` | CC-BY-4.0 | ship-credit | 571.9 MB | 800 | yes | 10.5281/zenodo.820937 |
| `study-hsd-string-quartet-k465` | CC-BY-4.0 | ship-credit | 208.9 MB | 42 | yes | 10.5281/zenodo.3601097 |
| `study-iowa-violin-viola` | custom | measure | 535.4 MB | 139 | yes | https://theremin.music.uiowa.edu/MISviolin.html |
| `study-papers` | custom | reading | 62.0 MB | 65 | **no: by hand** |  |
| `study-phenicx-anechoic` | custom | measure | 232.8 MB | 83 | yes | 10.5281/zenodo.840025 |
| `study-philharmonia` | custom | measure | 38.8 MB | 1742 | yes | https://philharmonia.co.uk/audio-sound-samples/ |
| `study-philharmonia-violin-viola` | custom | measure | 47.8 MB | 2476 | yes | https://philharmonia.co.uk/audio-sound-samples/ |
| `study-string-tension-references` | custom | reading | 455.4 kB | 4 | yes | https://www.aitchisoncellos.com/string-tensions-charts/ |
| `study-tinysol-violin-viola` | CC-BY-4.0 | ship-credit | 399.0 MB | 595 | yes | 10.5281/zenodo.3685367 |
| `study-vsco-solo-violin` | CC0-1.0 | ship | 8.1 kB | 2 | yes | https://github.com/sgossner/VSCO-2-CE |
| `study-pointers` | see each SOURCE.md | reading | 5.3 kB | 5 | yes |  |
| `study-zenodo-lampis-bridge-force-attack` | CC-BY-4.0 | ship-credit | 557.2 MB | 3949 | yes | 10.5281/zenodo.13374477 |
| `study-zenodo-lampis-bridge-force-static-dynamic` | CC-BY-4.0 | ship-credit | 26.9 MB | 177 | yes | 10.5281/zenodo.10619541 |
| `study-zenodo-lampis-friction-force` | CC-BY-4.0 | ship-credit | 108.2 MB | 355 | yes | 10.5281/zenodo.15879128 |
| `study-zenodo-mores-torsion-electrical` | CC-BY-4.0 | ship-credit | 86.5 MB | 56 | yes | 10.5281/zenodo.1257872 |
| `study-zenodo-mores-torsion-letter` | CC-BY-4.0 | reading | 197.5 kB | 2 | yes | 10.5281/zenodo.2546399 |
| `study-zenodo-mores-torsion-optical` | CC-BY-4.0 | ship-credit | 986.2 kB | 12 | yes | 10.5281/zenodo.1302895 |
| `study-zenodo-pamies-vila-bow-kinematics` | CC-BY-4.0 | ship-credit | 37.8 MB | 9 | yes | 10.5281/zenodo.10696680 |
| `study-zenodo-pamies-vila-contemporary-bowing` | CC-BY-4.0 | ship-credit | 356.3 MB | 8 | yes | 10.5281/zenodo.17207713 |
| `tuning` | none stated | measure | 3.1 MB | 5403 | yes | https://huygens-fokker.org/docs/scales.zip |
| `soundlab-example` | CC0-1.0 | ship | 44.7 MB | 120 | derived (a tool remakes it) |  |
| `tympanum` | CC-BY-4.0 (DRSKit) + CC0-1.0 (Virtuosity, hybrids) | ship-credit | 1.5 GB | 5287 | yes (pack code; cargo or the app's copy) | https://drumgizmo.org/wiki/doku.php?id=kits:drskit |
| `tympanum-extra` | CC-BY-4.0 (Crocell, Muldjord) + CC0-1.0 (Big Rusty) | ship-credit | — | 0 | yes (pack code; cargo or the app's copy) | https://drumgizmo.org/wiki/doku.php?id=kits:crocellkit |
| `piano` | CC-BY-3.0 | ship-credit | 746.9 MB | 644 | yes (pack code; cargo or the app's copy) | https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html |
| `piano-bitklavier` | CC-BY-4.0 | ship-credit | 1.6 GB | 665 | yes (pack code; cargo or the app's copy) | https://bitklavier.com/the-bitklavier-grand/ |
| `ebu-loudness` | custom | measure | — | 0 | **no: by hand** | https://tech.ebu.ch/publications/ebu_loudness_test_set |
| `mdb-drums` | CC-BY-NC-SA-4.0 | measure | — | 0 | **no: by hand** | https://github.com/CarlSouthall/MDBDrums |

The tools that read each set are in its `tools` field. In short: `arcus`, `arcus-upper`,
`strings-rec` and the `study-*` sets feed `tools/arcus` (match, study, passes 4–6, the upper
strings), the lab's `arcus-cello` subject and its tests; `tuning` feeds
`tools/test/scala-archive.sh`; `soundlab-example` the lab's worked example and `test_kit.py`;
`tympanum` `tympanum_abx` and `tools/tympanum-fit`; `piano` / `piano-bitklavier`
`chorda_compare`, `chorda_abx`, `chorda_pianos` and `tools/chorda-voicing`; `ebu-loudness`
the lab's `test_loudness.py`; `mdb-drums` the ignored `onset_drums_mdb` and `sectio_mdb` tests.

How the files arrive (`origin` in the TSVs): 10 769 by HTTP range requests on 14 remote zips
(only the members a set took — the 14 GB Good-sounds zip, the 3 GB Lampis attacks — the same
way round 42 took them, now one command), 1 193 from TinySOL's `.tar.gz` (downloaded once,
resumable, then read), 1 156 plain downloads (Iowa MIS, Zenodo files, GitHub raw at pinned
commits and release assets, CORA, Freesound's previews, the Wayback copy of the Scala
archive), 7 523 members of zips already in the set (the strings' four release zips, the Scala
archive), 276 copies of another catalogued file (Arcus's notes, `arcus-upper/refs`), 29 repo
texts, 189 derived, 25 saved pages, 65 by hand, 6 596 pack files.

## 3. What moved, what was verified

- **Copied** (robocopy, not moved): `daw-r42\downloads` → `cantus-refs`, without the two
  builders' scratch folders (`arcus-look`: screenshots and logs; `tuning-face`: logs and
  shots) and `*.log`. Left out of the catalogue and deleted **from the copy only**: four
  14-byte `arcus\cello_D2_*.wav` (each holds `404: Not Found`, a failed download of round 42)
  and `arcus-upper\{adm,tail,rec_csv}.bin` (pieces of a zip read by hand).
- **Verified against the originals:** every file was hashed in `daw-r42\downloads` before the
  copy; the catalogue was recorded from the copy. 21 233 of 21 233 catalogued files that came
  from there have the same size and SHA-256. `fetch.py verify`: every set OK.
- **The packs:** `virtuosity-drums`, `salamander-grand-piano`, `bitklavier-grand-stage-lid-ortf`
  copied from the app's own `%LOCALAPPDATA%\Cantus\Content` at the manifest's version
  (`fetch.py fetch … --packs`), the hybrid packs from the round's folder.
- **The refdb:** the round's copy (`refdb-r42`) equals every distribution committed in
  `research/instruments-next/bowed-refdb/` (`compare_refdb.py`: SAME ×9). **Rebuilt** from
  `cantus-refs` by `rebuild_refdb.sh` (31 minutes, 3 workers, while other checks ran): the
  same 3 733 jobs in the same order (the catalogue equal to the round's but for the root), 5 160
  per-note records, 485 phrases, no extraction error; against the committed files `bowing`,
  `cnsm_validation`, `friction`, `summary`, `validation` equal to the byte's value, `bass`,
  `cello`, `viola`, `violin` equal but for 21 numbers in their last bit (the
  `dynamics_slopes_per_step` sums: `extract.py` writes the notes in the order its workers
  finish, so a sum over them ends in another last bit; `compare_refdb.py` accepts a relative
  1e-12 and says how many). `vibrato.json` and `strokes.json` (`--pass5`) were not rebuilt.
- **A real fetch** into an empty folder (`--refs F:\projects\cantus-work\fetchtest\refs2`),
  then `verify` with the hash cache deleted (every file hashed again): Mores's optical traces
  (Zenodo files), Lampis's static/dynamic bridge force (176 members by range requests of a
  1.3 GB zip), the VSCO solo violin's note (a repo text and a copy that fetched its source from
  GitHub raw), the Scala archive (the Wayback zip, then its 5 401 members), Good-sounds' CORA
  metadata plus 33 members of the 14 GB Zenodo zip, and the Freesound previews: all OK. A
  `.part` cut at 50 kB resumed by a range request and checked. The full 12 GB was not fetched
  again.
- **Freesound's CDN:** Python's TLS check refuses its chain (*certificate has expired*,
  2026-11-25; the server's own certificate is valid to 2027-02-05; curl, which verifies with
  Windows' store, accepts it). fetch.py never retries or bypasses a TLS failure: it asks curl,
  a verifying client. Noted in `setup.md`.

## 4. No round paths in the tools

Every sound tool reads `CANTUS_REFS` / `CANTUS_WORK` / `CANTUS_STUDY_REFDB` / `CANTUS_PY` through
one helper per language; the older names still override; no number changed. Touched:
`tools/arcus/pass4/*.py` and `measure.sh`, `pass5/vibrato.py`, `upper_study.py`, the study's
scripts (`catalog`, `extract`, `phrases`, `aggregate`, `plots`, `cnsm`, `friction`, `bowing`,
`make_abx`, `validate_synth`: the references from `CANTUS_REFS`; their output into
`CANTUS_STUDY_REFDB` when set, else the committed folder as before), the Arcus shell scripts
(`match`, `abx`, `listen`, `demo`, `presets`, `upper-*`), the lab (`subject.py`'s defaults,
`kit.py` expands a voice file's `$NAME`, the `arcus-cello` subject's phrase now
`$SOUNDLAB_LISTEN/…`, the tests read `CANTUS_REFS` / `CANTUS_STUDY_REFDB` and a recorded path
under any old root), `tools/chorda-voicing` (`CHORDA_LAB` default `$CANTUS_WORK/chorda-lab`,
`references.sh` default `$CANTUS_REFS/piano`), `tools/tympanum-fit` (`TYMP_CONTENT` default
`$CANTUS_REFS/tympanum/content`, `TYMP_WORK` `$CANTUS_WORK/tympanum-work`),
`tools/test/scala-archive.sh` (default `$CANTUS_REFS/tuning`), `tools/content/strings` (its slot
script from the tree, not the main clone), and the examples `tympanum_abx`, `chorda_abx`,
`chorda_pianos` (their default folders from `CANTUS_REFS`; they named `daw-r42` and the deleted
`daw-r37`). Left as they were (not sound references): `tools/video`, `tools/wsl`,
`tools/public`, `tools/obs`, `tools/shots`, `tools/arcus-figures` (another builder's), the UI
tests' screenshot paths, `cantus-ml`'s model folder.

The checks, each with `CANTUS_REFS=F:\projects\cantus-refs`:

| check | result |
|---|---|
| `python -I tools/soundlab/tests/run.py --light` | 53 tests, OK (5 skipped), 41 s |
| `python -I tools/soundlab/tests/run.py` (`CANTUS_STUDY_REFDB` = the round's refdb) | 53 tests, OK (2 skipped: the EBU files, a kit key), 162 s; TinySOL's 34 pitches median −1.3 cents |
| `pass5/vibrato.py note` on VSCO's solo violin A4 f | rate 5.21 Hz, extent 20.7 cents, 59 cycles |
| `study_compare.py tools/arcus/study <refdb> <one TinySOL cello note as a render>` | the bands read, every feature printed |
| `cantus_paths.rebase` on the round's 3 733 catalogue paths moved under a deleted round | 3 733 found under `CANTUS_REFS` |
| Chorda: `labpaths.LAB`, `env.sh`'s `V`, `references.sh`'s folder | `F:\projects\cantus-work\chorda-lab`; `…\cantus-refs\piano` holds the Salamander |
| Tympanum: `cycle.sh`'s `W` and `CONTENT` | `…\cantus-work\tympanum-work`; `…\cantus-refs\tympanum\content` holds Virtuosity and the hybrids |
| `cargo check` of `tympanum_abx`, `chorda_abx`, `chorda_pianos`, `arcus_match` (through `slot.sh`, no debug info) | clean, no warnings (4 min) |

## 5. What still needs a hand

- **DRSKit (Tympanum)** is not on this machine any more (round 40's folder took it; the app
  does not hold it): `python -I tools/sound-refs/fetch.py fetch tympanum --packs --cargo`
  installs it (2.8 GB download through Cantus's pack code), then `fetch.py record tympanum`.
  Until then the Tympanum ABX cannot be rebuilt. Likewise `bitklavier-grand-piano-bar`
  (`piano-bitklavier`) and the optional `tympanum-extra` kits.
- **The EBU loudness test set:** tech.ebu.ch answers scripts with a browser challenge; never
  worked around. `fetch.py manual ebu-loudness` says what to download in a browser and where;
  then `fetch.py record ebu-loudness`.
- **The study's papers** (62 MB): saved by hand from the publishers' pages; their URLs were not
  recorded. The checksums are: a copy found again by title verifies.
- **MDB Drums:** gone with round 37's folder, never catalogued; `fetch.py manual mdb-drums`.
- **Not data, not copied:** `daw-r42\listen` beyond what §1 names (3.7 GB of earlier renders
  and drafts: `arcus`, `arcus-p5`, `arcus-p6`, `arcus-demo*`, `arcus-upper`, `abx-ciego`,
  `abx-compartir`, `arcus-presets`, `fides-presets`, `arcus-draft`), the builders' scratch
  (`downloads\arcus-look`, `downloads\tuning-face`, `daw-r42\tmp`). Look before deleting the
  round's folder if any of it is wanted.
