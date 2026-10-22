# Code signing policy (DRAFT)

> **Draft for the page SignPath Foundation requires on the project's website** (their
> terms, *Conditions for the website / repository*, read 2026-10-13). Not published: when
> the owner says so — and only after SignPath accepts Cantus — it becomes a page of
> cantusdaw.com (`/code-signing`, in English and Spanish like the rest of the site), and
> the words **"Code signing policy"** become a link on the **home page** (the footer) and
> on the **download card** and each release's notes. Until a certificate exists, nothing
> here is true yet: Cantus's releases are unsigned. `[OWNER: …]` marks what to confirm.

---

## Code signing policy

**Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).**

Windows shows **SignPath Foundation** as the publisher of Cantus's programs and installer:
the certificate is issued to the foundation, which lends it to open-source projects that
meet [its conditions](https://signpath.org/terms). A valid signature means the file is an
unmodified, automated build of the source code in
[github.com/luis314159/cantus-daw](https://github.com/luis314159/cantus-daw), made by GitHub
Actions and approved by the Cantus team for release.

### What is signed

Every Windows release (a version tag such as `v0.1.0`) is built by the
[release workflow](https://github.com/luis314159/cantus-daw/blob/master/.github/workflows/release.yml)
on GitHub-hosted runners, from the tagged commit, and these files are signed:

| File | What it is |
|---|---|
| `cantus.exe` | the launcher that starts the program built for your processor |
| `cantus-x86-64-v3.exe`, `cantus-x86-64-v2.exe` (and `cantus-x86-64-v4.exe` when a release has it) | Cantus itself, one build per processor level |
| `cantus-scan.exe` | the plug-in scanner Cantus starts to read third-party plug-ins safely |
| `cantus-sandbox.exe` | the plug-in sandbox Cantus starts to run third-party plug-ins out of its own process, so one that crashes cannot take the session down |
| `Cantus <Name>.clap`, `Cantus <Name>.vst3` | Cantus's own effects as plug-ins for other DAWs |
| `uninstall.exe` | the uninstaller (in the install folder) |
| `Cantus-<version>-windows-x64-setup.exe` (also published as `Cantus-windows-x64-setup.exe`) | the installer |

The portable zip and the plug-ins zip hold the same signed files. `SHA256SUMS.txt` lists the
SHA-256 of every file of the release.

Only software built from this repository's source is signed with this certificate. Cantus
installs no third-party program of its own accord. The installer's optional, unticked
*FlexASIO driver* component downloads FlexASIO from its author's GitHub releases at install
time; that file is not ours and is not signed by Cantus's certificate. Third-party plug-ins
you add to Cantus are yours and are never signed by us.

### Who does what

| Role | Who | What it means |
|---|---|---|
| Committers and reviewers | [Luis Ángel Almazán](https://github.com/luis314159) `[OWNER: the GitHub team page, once the repository is in an organisation]` | may change the source code; every change proposed by anyone else (a pull request) is reviewed by a committer before it is merged |
| Approvers | [Luis Ángel Almazán](https://github.com/luis314159) | approves each signing request in SignPath, release by release, after checking that it comes from the expected tag and workflow run |

Every team member uses multi-factor authentication on GitHub and on SignPath. Outside
contributions are merged under the [Cantus Contributor Licence Agreement](https://github.com/luis314159/cantus-daw/blob/master/CLA.md)
`[OWNER: keep this line only if the CLA is compatible with SignPath's "no commercial dual-licensing" — see docs/signing/README.md §3]`.

### Privacy

Cantus collects nothing about you: no telemetry, no analytics, no crash reports, no
account. It connects to the internet only:

- for the **update check** — once a day, an anonymous request to GitHub's API about new
  releases (on by default; *Settings ▸ Updates ▸ Check daily* turns it off);
- to download a **sound pack** or an **update**, only when you click to;
- during installation, only if you tick the **FlexASIO** component.

Every case, with exactly what is sent, is in the
[privacy statement](https://github.com/luis314159/cantus-daw/blob/master/PRIVACY.md). Those
requests go to GitHub (github.com, api.github.com, raw.githubusercontent.com), whose own
[privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement)
covers what GitHub keeps.

### Reporting a problem

If you believe a file signed for Cantus is not what this policy says, write to
`[OWNER: a contact address or the GitHub security advisory form]`; you may also report it
to SignPath at support@signpath.io.

---

## Política de firma de código

**Firma de código gratuita proporcionada por [SignPath.io](https://about.signpath.io/),
certificado de [SignPath Foundation](https://signpath.org/).**

*(La versión en español de la página: las mismas secciones — Qué se firma, Quién hace qué,
Privacidad, Reportar un problema — traducidas cuando el dueño apruebe el texto inglés. La
frase de arriba se queda en inglés en la versión inglesa, palabra por palabra, como piden
las condiciones de SignPath.)*

---

## Where each line must appear (the terms' checklist)

| SignPath asks | Where, on cantusdaw.com and GitHub |
|---|---|
| The term "Code signing policy" on the home page | the footer of every page: a link *Code signing policy* / *Política de firma de código* to this page |
| … and on the download/release pages | the download card (beside *All downloads, release notes and checksums*), and a last line in each release's notes: *Code signing policy: https://cantusdaw.com/code-signing* |
| "Free code signing provided by SignPath.io, certificate by SignPath Foundation" | the first line of this page, verbatim |
| Team roles and their members | *Who does what* above |
| A privacy policy link (or their fixed sentence) | *Privacy* above — their fixed sentence (*"This program will not transfer any information to other networked systems unless specifically requested…"*) would **not** be true of Cantus, whose update check runs by itself, so the policy is linked instead |
| The download page mentions SignPath Foundation (the application form's note) | the same download-card line |
