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
- [ ] L5 The round's new start-up clean-ups leave a live process's work alone (lock), and tests reading a process-wide value (the theme) take turns.
- [ ] L6 The round's new downloads retry on every error and are checked.
- [ ] L7 The last failed run's failures were all fixed in one commit before this rehearsal.
- [ ] L8 One session watches the release run, at most once every five minutes.
- [ ] L9 The rehearsal's two signature steps are green (the program's publisher is the signing's; the installer passes the program's own check), the previous release's in-app update on Windows was done and recorded, and this release's is in docs/beta-checklist.md for right after it is out.
- [ ] L10 Every effect's plug-in zips and the zip with all are in the rehearsal's artifacts for each system, in its SHA256SUMS.txt; one, unpacked, loads in a host; the Mac's plug-in step fits its job's time.
