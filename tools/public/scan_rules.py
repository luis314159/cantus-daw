"""What tools/public/scan.py looks for (ADR 0049). Patterns only; the scanning is scan.py's.

Keep FORBIDDEN_PATHS in step with the `--path-regex` arguments of the filter-repo command in
docs/round26/public.md (route (a)): `python3 tools/public/scan.py --filter-repo-args`
prints them from here.
"""

import os
import re

# Paths that must be in no published commit. Regexes, matched case-insensitively against the
# whole path; the same strings are given to `git filter-repo --path-regex` (Python `re`).
FORBIDDEN_PATHS = [
    ("third-party screenshots", r"(?i)^research/(.*/)?screenshots/"),
    ("image or PDF under research/",
     r"(?i)^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$"),
    ("the RVR1960 file", r"(?i)^crates/cantus-ui/locales/verses/es\.txt$"),
]

# Images and PDFs anywhere: listed for review when outside the folders of our own pictures.
IMAGE = re.compile(r"(?i)\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd|xcf)$")
OWN_IMAGE_FOLDERS = [
    "crates/cantus-ui/assets/",        # logos, icons
    "crates/cantus-ui/tests/golden/",  # the kit's golden renders
    "docs/images/",                    # screenshots of Cantus for the docs
    "docs/manual/img/",                # the manual's pictures of Cantus
    "web/assets/",                     # the site: our pictures, icons, press kit
]

SECRET_PATTERNS = [
    ("private-key", r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY"),
    ("github-token", r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b"),
    ("github-pat", r"\bgithub_pat_[A-Za-z0-9_]{60,}\b"),
    ("aws-access-key", r"\b(?:AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16}\b"),
    ("aws-secret", r"(?i)aws_secret_access_key\s*[:=]\s*['\"]?[A-Za-z0-9/+]{40}"),
    ("azure-storage-key", r"AccountKey=[A-Za-z0-9+/]{80,}={0,2}"),
    ("azure-sas", r"[?&]sig=[A-Za-z0-9%]{40,}"),
    ("slack-token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
    ("google-api-key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ("stripe-key", r"\b[sr]k_live_[0-9A-Za-z]{20,}"),
    ("npm-token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    ("pypi-token", r"\bpypi-AgEIcHlwaS5vcmc[A-Za-z0-9_-]{50,}"),
    ("anthropic-key", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
    ("openai-project-key", r"\bsk-proj-[A-Za-z0-9_-]{20,}"),
    ("openai-key", r"\bsk-[A-Za-z0-9]{32,}\b"),
    ("huggingface-token", r"\bhf_[A-Za-z0-9]{30,}\b"),
    ("jwt", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
    ("cloudflare-token-assign",
     r"(?i)(?:CLOUDFLARE|CF)_(?:API_)?(?:TOKEN|KEY)\s*[:=]\s*['\"]?[A-Za-z0-9_-]{37,}"),
    ("signing-secret-assign",
     r"(?i)(?:SIGNPATH|CERT|PFX|SIGNING)_(?:API_)?(?:TOKEN|PASSWORD|KEY)\s*[:=]\s*['\"]"
     r"[^'\"\s$]{8,}"),
]

# Listed for a person to look at; never a failure. No leading \b, so that `github_token`,
# `client_secret`, `signing_key` match too.
REVIEW_PATTERNS = [
    # (a dotted lower-case value is one of our locale keys, `label_key: "cmd.ai_add_notes"`)
    ("password-assign", r"(?i)(?:password|passwd|pwd|secret|token|_key|apikey)\b\s*[:=]\s*"
     r"['\"](?![a-z0-9_]+(?:\.[a-z0-9_-]+)+['\"])[^'\"\s]{8,}['\"]"),
    ("bearer", r"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{30,}"),
    ("url-credentials", r"[a-z][a-z0-9+.-]*://[^/\s:@'\"]+:[^/\s:@'\"]{3,}@[^\s'\"]+"),
]

SECRET_FILES = re.compile(
    r"(?i)(^|/)(\.env(\..*)?|\.dev\.vars|[^/]*\.(pfx|p12|pem|key|snk|keystore|jks|ppk)"
    r"|[^/]*id_(rsa|ed25519)[^/]*|[^/]*credentials[^/]*\.json)$")

EMAIL = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
USER_PATH = re.compile(r"(?i)(?:[A-Z]:[\\/]+Users[\\/]+|/home/|/Users/)([A-Za-z0-9._-]+)")
EMAIL_IGNORE = re.compile(r"(?i)(noreply|no-reply|example\.(com|org|net)|@users\."
                          r"noreply\.github\.com|\.(png|jpg|svg)$)")
USER_IGNORE = {"runner", "user", "you", "name", "username", "public", "default", "shared",
               "me", "x", "builder", "ubuntu", "runneradmin"}

# The owner's forbid list: addresses, domains or names that must appear nowhere (blobs,
# commit and tag messages, author/committer/tagger identities). It is personal, so it lives
# OUTSIDE the repository: --forbid FILE, else $CANTUS_FORBID, else this default.
DEFAULT_FORBID = os.path.join(os.path.expanduser("~"), ".cantus-public", "forbid.txt")


def load_forbid(path):
    """One entry per line, matched case-insensitively as a substring; `#` starts a comment."""
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            entry = line.split("#", 1)[0].strip().lower()
            if entry:
                out.append(entry)
    return out
