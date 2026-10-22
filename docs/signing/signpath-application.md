# SignPath Foundation — the application, filled in (DRAFT)

> **Draft, not sent.** Every field of the form at <https://signpath.org/apply> as it stood
> on 2026-10-13 (a HubSpot form; its field list was read from the form's own definition,
> `forms-eu1.hsforms.com/embed/v4/render-definition/145110231/bf62807d-…`), filled in from
> the repository. **`[OWNER: …]`** marks a decision or a fact only the owner can give.
> Send it only when the conditions in [`README.md`](README.md) §2 that say *fails today*
> are fixed — above all: the repository public, a released beta, and the CLA question.

The form opens with: *"Please provide basic information about the open source project you
are submitting. This helps us verify eligibility for the SignPath Foundation program."*

## Project

| Field (required?) | Their note | Answer |
|---|---|---|
| **Project Name** (required) | *A Google search for this name should clearly identify your project.* | **Cantus** — the product name every signed file carries (and SignPath's metadata restriction will enforce). "Cantus" alone also finds a chant database, a choir library and a vocal ensemble (`docs/beta-checklist.md` §4), so the Homepage (cantusdaw.com), the Tagline and the Description identify it: *Cantus, the free open-source DAW at cantusdaw.com*. |
| **Repository URL** (required) | *Link to the project's main source code repository (GitHub or GitLab).* | `https://github.com/luis314159/cantus-daw` — **must be public before sending** (today it is private). `[OWNER: if the repository moves to an organisation, e.g. github.com/cantusdaw/cantus, put that URL here and in the policy page]` |
| **Homepage URL** (required) | *The project's official homepage. This can be a dedicated website or the repository page.* | `https://cantusdaw.com` |
| **Download URL** (optional) | *A page where users can download your software. This page must mention that the project uses the SignPath Foundation for code signing.* | `https://cantusdaw.com/#download` — the download card must first carry the sentence and the *Code signing policy* link (`code-signing-policy.md` says where). The releases page (`https://github.com/luis314159/cantus-daw/releases`) gets the same lines in each release's notes. |
| **Privacy Policy URL** (optional; *required if the software collects user data*) | | `https://github.com/luis314159/cantus-daw/blob/master/PRIVACY.md` — Cantus collects no user data, but it does make one automatic request (the daily update check, switchable), so link the policy rather than leave the field empty. `[OWNER: or a cantusdaw.com/privacy page, if the site gets one]` |
| **Wikipedia URL** (optional) | *If your project has an English Wikipedia article.* | — (none) |
| **Tagline** (required) | *A short one-sentence summary of your project. This may be displayed on the SignPath Foundation website.* | *A free, open-source digital audio workstation for recording, composing, mixing and mastering music, written in Rust.* |
| **Category** (required) | one of: Accessibility, AI Tools, **Audio**, Developer Tools, Educational, Gaming, Graphics, Productivity, Science, Other | **Audio** |
| **Description** (required) | *A short paragraph describing your project and its purpose. Avoid listing version-specific features or dependencies.* | see below |
| **Reputation** (required) | *Provide links or information showing that your project is widely used or trusted. Examples include media coverage, blog posts, download statistics, GitHub insights, or community discussions.* | see below — **the weakest field today**: there is nothing public yet. |
| **Maintainer Type** (optional) | Independent community project · Non-profit foundation or research/educational institution · For-profit company or corporate-backed project · **Individual maintainer(s)** · Other | **Individual maintainer(s)** `[OWNER: change if a legal entity or organisation exists by then]` |
| **Build System** (required) | GitHub Actions · GitLab CI/CD | **GitHub Actions** (`.github/workflows/release.yml`, GitHub-hosted `windows-latest` and `ubuntu-latest` runners only) |

### Description

> Cantus is a free and open-source digital audio workstation (DAW) for making music from
> the first note to a mastered file: recording audio and MIDI, composing in an arrangement,
> piano roll and channel rack, mixing on a mixer desk with routing, and exporting finished
> songs. It ships its own instruments and effects, which also run as CLAP and VST3 plug-ins
> in other DAWs, and it hosts third-party CLAP and VST3 plug-ins. Its real-time audio
> engine is written in Rust, multicore and deterministic, and it is tested so that what you
> hear live is what you export. Cantus has no account, no subscription and no telemetry.
> It is developed in the open on GitHub; the application is licensed under the
> GPL-3.0-or-later and the engine crates under the MPL-2.0.

### Reputation (to write when there is some — do not send it empty)

What exists today, all private: 26 development rounds since 2026-09, ~2 200 automated
tests on Windows and Linux CI, ADRs for every structural decision. None of it is visible to
SignPath while the repository is private, and their terms say: *"For executable programs
that may be downloaded and executed based on our signature, we require a certain
verifiable reputation."* So, after the repository and the unsigned `v0.1.0-beta.1` are
public, collect for a few weeks and then fill in:

- `[OWNER: release download counts — GitHub's API: /repos/<repo>/releases, assets[].download_count]`
- `[OWNER: GitHub stars, forks, watchers, contributors; issues and hardware reports from testers]`
- `[OWNER: posts, videos, forum threads (KVR Audio, Reddit r/WeAreTheMusicMakers, r/rust, r/linuxaudio), any press]`
- `[OWNER: the demo songs and any music people have published made with Cantus]`
- The project's age and activity: commit history and releases on GitHub; the website.

## The person (the SignPath user account)

| Field (required?) | Their note | Answer |
|---|---|---|
| **First Name** (required) | *The name of the user account that will be created in SignPath.* | `[OWNER: e.g. Luis Ángel]` |
| **Last Name** (required) | (same) | `[OWNER: e.g. Almazán …, as on an ID]` |
| **Email** (required) | *The email address for the SignPath account and application notifications.* | `[OWNER: the address to use — it receives every approval request]` |
| **Company Name** (optional) | *Your organization or employer, if applicable.* | `[OWNER: leave empty, or the project's legal entity if one exists]` |
| **Primary Discovery Channel** (required) | Organic search · AI / LLM tools · Developer platforms · Community platforms · Social media · Events · Referral · Direct contact · Other | `[OWNER: choose — this project found SignPath through its own research notes (docs/round20/signing.md), written with an AI assistant]` |
| **Please specify the exact source** (optional) | *For example: Google search, ChatGPT, GitHub repository, …* | `[OWNER]` |

## The two boxes at the end

- **Required:** *"I have read and agree to the SignPath Foundation Code of Conduct and I
  understand that certificates are issued in SignPath Foundation's name and may be revoked
  if terms are violated."* — tick it only after reading <https://signpath.org/terms>
  (a copy as read on 2026-10-13 is summarised condition by condition in `README.md` §2).
  It means: the publisher users see is **SignPath Foundation**, not the owner's name; and
  a later commercial licence, a proprietary component or a missing approval can cost the
  certificate.
- Optional: *"I agree to receive other communications from SignPath."* — `[OWNER]`

## What to have ready before pressing Submit

1. The repository public (the history cleaned first — `docs/round26/public.md`, ADR 0049).
2. `v0.1.0-beta.1` tagged and published (unsigned) — *"Released: the project must already
   be released in the form that should be signed."*
3. The CLA decision taken (`README.md` §3, decision 1) — today's CLA reserves a
   commercial licence.
4. `code-signing-policy.md` turned into a page of cantusdaw.com (or the repository), the
   *Code signing policy* link on the home page and the download card, and the sentence
   on the download page.
5. MFA on the owner's GitHub account (and later on SignPath).
6. A few weeks of public reputation to write in the *Reputation* field.
