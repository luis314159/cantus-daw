# Round 24 — integration: three builders, the soak's finding, and the merge

Branch **`r24/integration`** (worktree `daw-r23/beta-hardening`), on master a9871e2 (round
23 plus the website session's two commits). Builders' reports: `beta-rest.md`,
`linked.md`, `perf2.md` in this folder.

## How the round ran

Three builders in worktrees of their own (`daw-r24/{beta-rest,linked,perf2}`, base
cead440), one workflow: each built, then two adversarial reviewers read its diff (one for
correctness and invariants, one for the real app and the honesty of the claims), then the
builder fixed what they found — 29 findings in all (beta-rest 9, linked 13, perf2 8),
every one confirmed and fixed with a test seen failing first, none rejected. Nothing was
committed on the branches; each diff was applied here.

| Builder | What it built | Decisions |
|---|---|---|
| beta-rest | the instruments' gain staging (Lamina +5 dB, Exemplar's placeholder +8 dB, as a hidden output trim; old projects keep their sound, bit for bit), Save off the interface thread (a save only copies the project in the frame; the file is written on a thread; edits made meanwhile stay unsaved; New/Open/quit wait), the recovery dialog in `shoot_recovery` (two layout faults found and fixed by looking) | ADR 0039, ADR 0040, schema 21 |
| linked | linked clips (BACKLOG §6d.2): copies of a pattern are the same clip until *Make unique*; audio clips share their settings; a link mark, a ×N count, the siblings outlined; *Link selection to this clip*, *Select linked copies*; paste linked or unique by a setting; `cantus-clip/2` | ADR 0037, schema 22 (21 on its branch) |
| perf2 | performance wave 2: the heaviest instruments (Calor, Unda, Exemplar) render their voices on the pool's idle threads, summed in voice order (parallel = serial to the bit); the demos measured over the whole song at 256 frames: every song's worst window ≤ 32 % of the budget (BETA blocker 2 asks < 70 %); startup ×0.42, house ×0.47, ballad ×0.39 at the median | ADR 0038, no schema change |

## The soak's finding (and a second bug it uncovered)

The 2-hour soak (`CANTUS_SOAK_MINUTES=120`) was run on round 23's master while the three
builders compiled. At 65 minutes a frame took **18.5 s** with no edit in it. The cause:
the crash-recovery journal (round 22) was written by the window's thread every time the
"unsaved changes" flag flipped, ending with `fsync` — and three builds writing gigabytes
kept the disk (and the page file: 57 of 75 GB committed) busy enough to hold that `fsync`
for seconds. A user recording or copying large files would see the same stall.

**Fix** (`recovery.rs`): the journal is written by a thread of its own, newest state wins;
the session's first write stays synchronous; `flush()` and the end of a session wait for
it. Test `journal_writer_tests::a_slow_disk_does_not_hold_up_the_caller` (an 800 ms disk
costs the caller nothing; the last state reaches the file; nothing writes it after the
end).

Moving the write changed the timing enough to expose a **race at the start of a
recording**: the engine reports "playing" with each block, so the first poll after
Record could read a block from before the Play and close the pass at once, deleting its
empty take (with a 2048-frame buffer, a recording that would not start). beta-rest found
the same bug independently in its round; its fix (a pass closes itself only after the
transport was seen running) and its test (`rec_flows`) are the ones merged.

## A send's place in its strip's list (found by the mixer-routing study)

Research for BACKLOG §6d.3 (`research/mixer-routing/notes.md`, the design for the next
round in `docs/round25/mixer-routing-design.md`) noticed that undoing the removal of a send
put it back at the *end* of the strip's list: `RemoveStripSend`'s inverse was
`SetStripSend`, which appends. The sound did not change (each send goes to its own
strip), but the inverse was not exact (ADR 0004) and the desk showed the sends reordered.
A new command, `InsertStripSend { id, index, send }`, is the exact inverse; the undo fuzz
has an arm for it and `cantus-core/tests/strip_sends.rs` holds the order (it failed
before: `[R0, R2, R1]`).

## The merge

- perf2 and beta-rest applied cleanly; linked conflicted only where expected: the schema
  (beta-rest took 21, so linked became **22**: `migrate_21_to_22`, `fixtures/v22.cantus`
  regenerated with both changes, its tests and ADR 0037 renumbered), the locale areas'
  registration (`save`, then `links`), and the fixture writer.
- ADR numbers: 0037 linked clips, 0038 voices on several threads, 0039 instrument output
  calibration, 0040 save off the interface thread (0034 is the website's, 0035 and 0036
  round 23's). Keymap file version 4 (linked's new keys reach existing installs).
- One clippy lint in the website session's new example (`render_fx_ab.rs`) fixed so the
  workspace is clean; that example is otherwise left as it came.
- Results: **2 000 tests green** (201 test binaries), 0 failed, 32 ignored (with `InsertStripSend`'s test); the engine's fuzz
  at `CANTUS_WORKERS=3`: 396 green; clippy clean.

## The soak on this tree

120 minutes on a quiet machine: passed — no frame over 250 ms, handles 152 → 152,
threads 10 → 7, private bytes +70 MB with 199 s of audio recorded (allowed 96 MB plus the
audio, which stays in memory by design).

## Left

- BETA blocker 2's second half: a 10-minute live run with a real device and 0 overruns
  (perf2 measured offline and paced, not on a device); a quiet-machine `xtask perf --full
  --save` baseline for this Ryzen class; a loom model of `cantus_rt::fork`.
- Linked clips: no Inspector row for the group, no "Link to…" picker by name; a hosted
  instrument's overlapping copies are told apart by a 31-bit tag (collision ~1 in 2³⁰).
- Saves: collecting audio into the project folder still copies on the interface thread
  before a first save; Lamina's remaining loudness gap (4 LU under the median, capped by
  the headroom rule) is a question for the owner's ear.
- By hand, with the real build: record with a 2048-frame ASIO buffer (the race fix), save
  a large project while playing, link/unlink a Pulsus pattern and listen that nothing else
  changes.
