# build — sharing one machine between several builders

`slot.sh` limits how many cargo builds run at once on the machine, whichever worktree starts
them (round 40 ran up to a dozen builders on one 31 GB PC: four release builds at once ran it
out of memory, and the failures looked like compiler bugs). Wrap every cargo command in it, and
every WSL check in its `--wsl` slot (one at a time: the WSL image is one disk):

```bash
bash tools/build/slot.sh cargo test -p cantus-dsp --lib chorda
bash tools/build/slot.sh --wsl wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/<clone>/tools/wsl/check.sh <branch> test'
bash tools/build/slot.sh --status
```

| variable | | default |
|---|---|---|
| `CANTUS_SLOTS` | build slots | 3 |
| `SLOT_PRIORITY=1` | one more slot for this builder (round 40: a build-and-measure cycle that kept losing the race; keep its builds at `-j 2`) | — |
| `CANTUS_SLOTS_DIR` | where the slots live — **the same folder for every builder on the machine** | `/tmp/cantus-build-slots` |

A slot is a directory (`mkdir` is atomic) holding its owner's pid, start time, folder and
command. A slot whose owner is gone, or older than three hours, is taken back. Waiting is
silent and can be long: run long commands in the background. Scripts that build call it
themselves (`tools/chorda-voicing/build_lab.sh`, `tools/tympanum-fit/cycle*.sh`,
`refit_hybrid.sh`); set `SLOT=` (empty) for those when already inside a slot.

Pair it with the per-worktree `.cargo/config.toml` of `CLAUDE.md` (`[build] jobs = 4`, no debug
info) and check `df -h` before a workspace build.
