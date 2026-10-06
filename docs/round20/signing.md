# Code signing — what the owner must buy or set up

Round 20, packaging (ADR 0030 §3). The pipeline is built and proved with a self-signed test
certificate; what is missing is a certificate Windows trusts. Nothing secret is, or ever
should be, in the repository. Prices are what was public by mid-2026 — **check them before
paying**.

## Why sign at all

An unsigned `Cantus-…-setup.exe` downloaded from GitHub gets Windows SmartScreen's blue
"Windows protected your PC" screen, where the user must click *More info → Run anyway*.
Many will not. A signature names the publisher, lets SmartScreen build reputation for it,
and lets the app's updater require that an update comes from the same publisher (ADR
0030 §5). Since 2023 every publicly trusted code-signing key must live in a hardware
module (a USB token, a cloud HSM): there is no "download a .pfx" certificate any more.
Since 2024 an EV certificate no longer skips SmartScreen's reputation building either, so
EV is not worth its premium for Cantus.

## The options

| Option | Cost (approx.) | Publisher shown | Works in GitHub Actions | Notes |
|---|---|---|---|---|
| **SignPath Foundation** (free for open source) | free | "SignPath Foundation" (the project's name in the description) | yes — their GitHub integration signs the CI build's artifacts | Requires an OSI licence (GPL-3 qualifies), builds from public CI, a short application and review. The certificate is theirs, not Cantus's. |
| **Azure Trusted Signing** (Microsoft) | Basic ≈ US$10/month (5 000 signatures) | the validated name (yours or your company's) | yes — `CANTUS_SIGN=azure` is already wired (`release.yml`) | Identity validation by Microsoft. **Eligibility:** organisations in the USA, Canada, the EU and the UK; individual developers were limited to the USA and Canada. Confirm whether a developer in Mexico can enrol. Cheapest and most CI-friendly if eligible. |
| **Certum Open Source Code Signing** | ≈ €30–70/year (cloud "SimplySign" or a card + reader) | your name + "Open Source Developer" | awkward: SimplySign is a virtual card on a logged-in Windows machine; signing on a local machine then uploading works | The cheapest certificate in your own name. Use `CANTUS_SIGN=store` with its thumbprint. |
| OV certificate from Sectigo / SSL.com / DigiCert | ≈ US$200–500/year, plus a token or a cloud-signing plan (SSL.com eSigner ≈ US$20+/month) | your name or company | yes with a cloud-signing plan | Standard commercial route. |
| EV certificate | ≈ US$300–700/year | company only | with cloud signing | No longer bypasses SmartScreen reputation; not recommended. |

**Recommendation.** Apply to the **SignPath Foundation** programme first (free, CI-native,
made for projects like this). If you want the publisher to read *your* name, use **Azure
Trusted Signing** if you are eligible, otherwise a **Certum Open Source** certificate.

## Choose the publisher name once

The app pins the publisher's common name at build time (`CANTUS_PUBLISHER`). A signed
Cantus only installs updates signed by the same name. If the name ever changes, users of
the older builds must download the next version by hand once. Set `CANTUS_PUBLISHER` to
the certificate's CN exactly (e.g. `Luis Angel Almazan Lopez`).

## Setting it up

### Azure Trusted Signing

1. In Azure, create a *Trusted Signing account*, complete identity validation, create a
   *certificate profile* (Public Trust).
2. Create an app registration (service principal) with the role *Trusted Signing
   Certificate Profile Signer* on that account.
3. In the GitHub repository: variables `AZURE_SIGNING_ENDPOINT` (e.g.
   `https://eus.codesigning.azure.net`), `AZURE_SIGNING_ACCOUNT`,
   `AZURE_SIGNING_PROFILE`, `CANTUS_PUBLISHER`; secrets `AZURE_CLIENT_ID`,
   `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`. `release.yml` does the rest.

### A PFX (only for a certificate that allows exporting, e.g. a test or an internal CA)

Secrets `CANTUS_SIGN_PFX_BASE64` (`[Convert]::ToBase64String([IO.File]::ReadAllBytes("c.pfx"))`)
and `CANTUS_SIGN_PFX_PASSWORD`; variable `CANTUS_PUBLISHER`.

### A token or a cloud card on your own machine

```powershell
$env:CANTUS_SIGN = "store"
$env:CANTUS_SIGN_THUMBPRINT = "<the certificate's SHA-1 thumbprint>"
$env:CANTUS_PUBLISHER = "<its CN>"
cargo run -p xtask -- package --all --sign
```

then upload `target/package/dist/*` to the release by hand (or tag, and let CI build an
unsigned set you replace).

## The proof done in round 20 (no purchase)

```text
cargo run -p xtask -- sign-test-cert          # CN=Cantus Test Signing (not trusted), in target/signing-test/
cargo run -p xtask -- package --sign-test     # signs cantus.exe, every plug-in, the uninstaller, the installer
```

Done here on 2026-10-06: `sign-test-cert` made the certificate (thumbprint `0D5ECA86…873B`,
removed from the store right after the export — checked); `xtask sign-file` signed an
installer built by the package test, with an RFC 3161 timestamp from DigiCert fetched
through the corporate proxy (`TimeStamperCertificate: DigiCert SHA256 RSA4096 Timestamp
Responder 2026 1`). Every signed file then reads, to Windows, as *signed, intact, root not trusted*
(`Get-AuthenticodeSignature` → `UnknownError`, the chain ending in an untrusted root),
which is exactly what a self-signed certificate should give; flipping one byte gives
`HashMismatch`. The app's updater would refuse such an installer if it required that
publisher — a trusted chain is what the purchase buys. The certificate is removed from
the user's store right after being exported, and `target/` is never committed.
