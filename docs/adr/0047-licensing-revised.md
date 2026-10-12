# ADR 0047 — Licensing revised: the engine under MPL-2.0, a contributor agreement with a free-forever promise, the Cantus marks

**Status:** Accepted · **Date:** 2026-10-11 · **Round:** 25 · **Amends:** ADR 0001

## Context

ADR 0001 split the licence by crate: the "SDK" (`cantus-core`, `cantus-dsp`,
`cantus-engine`, `cantus-project`, `cantus-rt`) under **MIT OR Apache-2.0**, the
application under **GPL-3.0-or-later**, and **no** contributor agreement (CLA).

The owner asked (2026-10-11) that whoever modifies Cantus be obliged to share the
modifications, and that the project keep the ability to grow a team around it. Two facts
shaped the answer:

- No open-source licence can require sending changes *to the author*: that fails the
  Open Source Definition and the Free Software Definition (the "desert island" and
  "dissident" tests), and would push Cantus out of Linux distributions and away from
  contributors. What open licences can require is that a modified version **that is
  distributed** comes with its source, under the same licence — copyleft.
- Under MIT/Apache, anyone may take the engine, improve it and ship it closed without
  giving anything back. The application was already copyleft (GPL); the engine was not.

The repository is private and the owner is its only author, so every crate can still be
relicensed: nobody has received a copy under MIT/Apache.

## Decision

1. **The SDK crates move to MPL-2.0** (`cantus-core`, `cantus-dsp`, `cantus-engine`,
   `cantus-project`, `cantus-rt`). The Mozilla Public License is copyleft **per file**:
   whoever distributes a modified version of a Cantus file must publish that file's
   source under MPL-2.0, while the engine can still be used, unchanged, inside other
   programs (plug-ins, research, other DAWs) — the reason ADR 0001 made it permissive.
   The files carry no "Incompatible With Secondary Licenses" notice (Exhibit B), so the
   MPL code combines with the GPL application, and the program as distributed stays one
   combined work under GPL-3.0-or-later. The notice of MPL's Exhibit A lives in a
   `LICENSE` file in each of those crates (MPL §Exhibit A allows a file in the directory
   instead of a header in every file); the licence text is `LICENSE-MPL` at the root.
2. **The application stays GPL-3.0-or-later** (`cantus-app`, `cantus-ui`, `cantus-ai`,
   `cantus-host`, `cantus-content`, `cantus-export`, `cantus-launch`, `cantus-update`,
   `cantus-plugins`, `cantus-bench`, `xtask`, `web`). AGPL was considered and left for
   when Cantus offers something over a network; nothing does today.
3. **A contributor licence agreement from the first outside contribution** (`CLA.md`),
   reversing ADR 0001's "no CLA". It is a licence grant, not a transfer: contributors
   keep their copyright and give the project's steward (Luis Ángel Almazán, until a
   Cantus organisation exists, to which it may pass) a licence broad enough to relicense
   — which keeps open a commercial licence beside the free one (BACKLOG §6f), as Qt and
   others do. In exchange the steward **promises that every accepted contribution will
   always remain available under the licence it was contributed under** (GPL-3.0-or-later
   or MPL-2.0): Cantus can never be closed with the community's work. Agreement is given
   per pull request (a checkbox in the template, then a bot when the team grows).
4. **The name and the logos are the owner's marks** (`TRADEMARKS.md`): forks are welcome
   but must use another name and logo; unmodified builds, truthful references ("works with
   Cantus", "based on Cantus") and community use are free. The trademark search in class 9
   (ADR 0001's TODO, BETA §6) is still to be ordered before the public beta.
5. **`CONTRIBUTING.md`** says how to contribute, under which licence each crate's code
   goes, the CLA, and the house rules (CLAUDE.md's, for humans).

## Consequences

- The website says "app GPL-3.0-or-later · engine MPL-2.0" where it named the licence;
  its promise "Open source" still holds. The About box says the same in both languages.
- `LICENSE-MIT` goes; `LICENSE-APACHE` stays (third-party Apache notices are generated
  from it, `xtask notices`).
- GPL code still cannot be copied into the SDK crates (an MPL file cannot absorb GPL
  code); references under GPL are read, not copied (`docs/instruments/BUILDING.md`).
- The CLA is a draft written in plain terms; it should be read by a lawyer before the
  first outside contribution is merged, and before any commercial licence is offered.

## What holds it

- Each SDK crate's `Cargo.toml` declares `license = "MPL-2.0"` and carries the Exhibit A
  `LICENSE` file; the application crates declare `GPL-3.0-or-later`.
- `xtask notices --check` still passes (the workspace's own crates are not third-party).
