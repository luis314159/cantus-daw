# Promotion plan — adapted to what Cantus has (round 41, draft for the owner)

The outside reviewer proposed a phased plan: prepare, Show HN and the Rust communities,
musicians' forums, Spanish-speaking communities, weekly posts, and metrics with UTM links.
This is that plan fitted to what Cantus really has on 2026-11-20. **Every step is the owner's
to take** — no agent posts anywhere, opens accounts or writes in someone else's community.

Ground rules for every post:

- **Say who you are**: "I made this" in the first line. Every community below expects the
  author to disclose it, and several remove posts that hide it.
- **Only what is true and checkable**: the claims on the site and the README, each with its
  source (the scorecard with its date and machine, the tests). Never "the best DAW" as a
  claim — the scorecard says where Cantus wins and where it loses, and that honesty is the
  pitch.
- **Never ask for votes or stars**, never post the same text in many places on one day, never
  use second accounts. Hacker News and Reddit both penalise it.
- **Answer every comment** in the first hours; turn each bug into an issue and link it back
  when fixed. Issue #1 (a flicker, reported 14 November, fixed in beta.15) is the model.

## Phase 0 — prepare (before any post)

What is ready: the site in English and Spanish (instruments and effects heard in the browser,
the scorecard, the download page), the README's new first screen, the manual in both
languages, the press kit (`web/assets/press/`), seven demo songs, the issue forms (bug,
hardware, feature), the releases with SHA-256 sums.

What to close first, in this order:

1. **Deploy this round's site and publish the README** (the integrator, on the owner's word).
2. **The beta.15 release notes on GitHub still list a macOS disk image** that was not
   published (the row in *Install* and the "macOS is a preview" paragraph, in both
   languages). Edit the release body to say macOS is held for beta.16.
3. **A green CI run on the public master**, or drop the CI badge from the README: the last
   runs on master read *cancelled* and *failure* (the beta.15 release saga), and a red badge
   is the first thing a Rust developer looks at.
4. **The demo video** (being planned in `docs/round41/video/`): a 60–90 s video is what
   forums and Show HN readers click first. Post without it only if it is far off.
5. **Decide the community channel** (GitHub Discussions, a Discord, or issues only). See
   `docs/round41/showcase.md`. Issues alone are fine for the first wave.
6. Prefer **beta.16 with macOS** for the musicians' forums (phase 2): a large share of
   producers are on Macs, and "macOS next beta" invites "come back later".

## Phase 1 — Show HN and the Rust communities (week 1)

Developers are the audience most likely to read the engine claims, build it, and file
precise issues.

| Where | How | Note |
|---|---|---|
| **Hacker News — Show HN** | Title: *Show HN: Cantus – a free, open-source DAW in Rust*. Link the repository (Show HN prefers something people can try). First comment by the owner: who he is, why (from the approved "Why" text), what is measured and what is lost, what is not done (macOS, signing, capacity bench). | Post on a weekday morning, US Eastern time. Stay to answer for the first three hours. |
| **r/rust** | A text post: the engine's rules (no allocation on the audio thread, counted by a test; offline equals real time to the bit; `forbid(unsafe_code)` in the engine, OS code in one crate), with links. | Read the subreddit's self-promotion rule first. |
| **This Week in Rust** | Submit the repository or a write-up to its "Project/Tooling Updates" through a pull request to its repository. | Free, weekly; needs a post or release page to link. |
| **users.rust-lang.org** | An announcement in its *announcements* category. | |
| **Rust audio** | The Rust Audio community (its Discord / GitHub organisation) and the `nih-plug` / CLAP communities: the CLAP/VST3 builds of Cantus's effects are of interest there. | Check where they gather now before posting. |

A short technical write-up to link from all of these would multiply phase 1: *How Cantus
keeps the audio thread allocation-free and offline renders bit-identical* (ADR 0005, the null
tests). The repository already holds the material; it needs the owner's voice.

## Phase 2 — musicians' forums (weeks 2–4, ideally with beta.16)

Musicians care about sound, workflow and stability, not Rust. Lead with the demo songs, the
instruments heard on the site, and "free, no account".

| Where | How |
|---|---|
| **KVR Audio** | A product entry for Cantus (developer account) and an announcement in its forum's software section. KVR's database is where producers look up free instruments and effects; list the CLAP/VST3 effects too. |
| **Bedroom Producers Blog** | A news tip (they cover free software): the instruments, the effects as free CLAP/VST3 plug-ins, the demo songs. |
| **r/WeAreTheMusicMakers, r/edmproduction, r/FL_Studio** | Only where the rules allow (some keep self-promotion to weekly threads). In r/FL_Studio, frame it carefully: a free program inspired by FL's workflow, not a replacement pitch. |
| **Linux audio** | LinuxMusicians forum, r/linuxaudio, the `linuxaudio.org` lists: the AppImage with PipeWire, JACK and ALSA, the whole suite run on Linux. Flathub (planned, BACKLOG) would help here. |
| **Gearspace** | Its *Music Computers* / product forums; slower, more critical — post once the hardware reports have a few interfaces. |

Ask for one concrete thing in each post: **a hardware report** (the form exists) or **a song
made with it** (`MADE_WITH` in `web/src/proof.rs` lists songs only with the author's
permission).

## Phase 3 — Spanish-speaking communities (weeks 3–6)

The program, the manual, the site and the issue forms are already in Spanish — an advantage
few DAWs have. Post in Spanish, written by the owner.

| Where | How |
|---|---|
| **Hispasonic** | The largest Spanish-language music-production forum: an announcement in its software section. |
| **Reddit in Spanish** | Subreddits on music production and on programming in Spanish, where self-promotion is allowed (check each one's rules). |
| **Rust en español** | The Spanish-speaking Rust communities (forums, Telegram groups) with the engine write-up translated. |
| **Mexico** | The owner's own circles first: music schools, producers he knows, university audio and computing programmes — a talk or a workshop is worth more than a post. |
| **YouTube in Spanish** | Producers who review free plug-ins: offer the press kit and the effects' CLAP/VST3 builds; never pay for a review without saying so. |

## Phase 4 — a steady rhythm (from week 2, weekly)

One post a week, not more, each with one picture or a short clip and one link:

- an instrument or effect of the week (its page on the site, its presets heard);
- a demo song taken apart (which instruments, which effects, the master chain);
- what a release fixed, with the issue it came from;
- a measurement, with its date and machine — including when Cantus loses.

Where: the owner's own accounts (X/Twitter, Mastodon — the Rust and Linux audio communities
are active there, Bluesky, Instagram/TikTok for short clips), and the GitHub release notes,
which the site's Atom feed (`/releases.xml`) already carries.

## Phase 5 — measuring it, without tracking visitors

The site has **no analytics by design** (ADR 0034, `PRIVACY.md`: no cookies, no external
requests). UTM parameters would be read by nothing, so this plan does not use them. What can
be measured honestly:

| Signal | Where | How often |
|---|---|---|
| Visitors to the repository and where they came from | GitHub ▸ Insights ▸ Traffic (views, unique visitors, **referring sites**, popular pages; the last 14 days only — write them down weekly) | weekly |
| Downloads per file and per release | `https://api.github.com/repos/luis314159/cantus-daw/releases` (`download_count` of each asset) | after each post, weekly |
| Stars, forks, watchers | the repository page; the site shows stars from 50 and downloads from 500 (`proof.rs`) | weekly |
| Issues opened, hardware reports, songs offered | the issue tracker, by label | weekly |
| Requests to cantusdaw.com | Cloudflare's dashboard for the Worker (server-side request counts, no cookies) | weekly |
| Replies and questions in each thread | the threads themselves | after each post |

Keep one table (date, post, where, referrer views in the next 7 days, downloads in the next
7 days, issues) — a spreadsheet is enough. If the owner ever wants per-link counts, the
privacy-respecting way is distinct short paths on the site that redirect server-side and are
counted by the Worker; that changes ADR 0034's "no analytics" and `PRIVACY.md`, so it is his
decision, not this plan's.

## What not to do

- No invented testimonials, numbers or quotes — on the site, in posts or in replies.
- No comparison claims beyond the scorecard's rows; no capacity claims until the live
  DAWbench run exists.
- No screenshots of other products in posts (the research screenshots stay private).
- No RVR1960 text anywhere public (it is kept privately until its publisher permits it).
