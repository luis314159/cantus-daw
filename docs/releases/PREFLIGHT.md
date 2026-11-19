# Preflight for Cantus {version}

<!--
Copy to docs/releases/v<version>.preflight.md (docs/RELEASING.md, step 3). Fill the two
fields, then tick a lesson ("- [x]") only after checking it for this release.
release.yml's preflight job refuses the tag while a box is empty, a lesson of
docs/RELEASING.md is missing here, or the rehearsal is not a green hand-run of release.yml
on a commit that differs from the tag's only in docs/releases/.
-->

Rehearsal run: {the run id, the number in its address .../actions/runs/<id>}
Rehearsal commit: {the cantus-daw commit it ran on, in full}

- [ ] L1 Rehearse before the tag: the run above is green on all three systems.
- [ ] L2 The whole suite runs (--no-fail-fast) and each test step took under 2/3 of its timeout-minutes.
- [ ] L3 The round's new timing tests compare with a reference (or best of several over a wide margin) and take turns.
- [ ] L4 The round's new read_dir walks sort, or create their destinations first.
- [ ] L5 The round's new start-up clean-ups leave a live process's work alone (lock).
- [ ] L6 The round's new downloads retry on every error and are checked.
- [ ] L7 The last failed run's failures were all fixed in one commit before this rehearsal.
- [ ] L8 One session watches the release run, at most once every five minutes.
