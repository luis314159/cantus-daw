//! What arrives is checked before anything runs it (ADR 0030 §5):
//!
//! 1. not empty and not a web page (a proxy holding the file for scanning answers with its
//!    own page and `200 OK`);
//! 2. the size GitHub reported, when it reported one;
//! 3. the SHA-256 GitHub computed at upload (the asset's `digest`) and the one in the
//!    release's `SHA256SUMS.txt` — every one present must match;
//! 4. when this build was signed by a publisher, the installer's Authenticode signature
//!    must be valid (Windows' own check, chain and timestamp) and by that same publisher.

use crate::release::looks_like_html;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// The publisher whose certificate signs Cantus's releases, set when the release workflow
/// builds a signed release (`CANTUS_PUBLISHER`, the certificate's common name). A build
/// made without it (every development build, and unsigned releases) checks hashes only.
pub const PUBLISHER: Option<&str> = option_env!("CANTUS_PUBLISHER");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    Io(String),
    Empty,
    /// The file is a web page: most likely a proxy holding the download. Try later.
    Html,
    Size {
        expected: u64,
        got: u64,
    },
    Hash {
        expected: String,
        got: String,
    },
    /// No hash to check against: the release published none.
    NoHash,
    Unsigned,
    /// Windows did not accept the signature (its status: `HashMismatch`, `NotTrusted`, …).
    Signature(String),
    Publisher {
        expected: String,
        got: String,
    },
}

impl VerifyError {
    /// The locale key that says it to the user.
    pub fn key(&self) -> &'static str {
        match self {
            VerifyError::Io(_) => "update.error.io",
            VerifyError::Empty | VerifyError::Html => "update.error.held",
            VerifyError::Size { .. } | VerifyError::Hash { .. } => "update.error.hash",
            VerifyError::NoHash => "update.error.no_hash",
            VerifyError::Unsigned | VerifyError::Signature(_) | VerifyError::Publisher { .. } => {
                "update.error.signature"
            }
        }
    }

    /// Whether waiting and downloading again may help (a proxy's quarantine).
    pub fn worth_retrying(&self) -> bool {
        matches!(
            self,
            VerifyError::Empty | VerifyError::Html | VerifyError::Io(_)
        )
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of a file, read in pieces.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Steps 1–3: the file at `path` is what the release says it is. `hashes` are every
/// SHA-256 the release offers for it (at least one is required).
pub fn check_file(path: &Path, size: Option<u64>, hashes: &[String]) -> Result<(), VerifyError> {
    let meta = std::fs::metadata(path).map_err(|e| VerifyError::Io(e.to_string()))?;
    if meta.len() == 0 {
        return Err(VerifyError::Empty);
    }
    let mut head = vec![0u8; 1024.min(meta.len() as usize)];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .map_err(|e| VerifyError::Io(e.to_string()))?;
    if looks_like_html(&head) {
        return Err(VerifyError::Html);
    }
    if let Some(expected) = size.filter(|s| *s > 0) {
        if meta.len() != expected {
            return Err(VerifyError::Size {
                expected,
                got: meta.len(),
            });
        }
    }
    if hashes.is_empty() {
        return Err(VerifyError::NoHash);
    }
    let got = sha256_file(path).map_err(|e| VerifyError::Io(e.to_string()))?;
    for expected in hashes {
        if !expected.eq_ignore_ascii_case(&got) {
            return Err(VerifyError::Hash {
                expected: expected.clone(),
                got,
            });
        }
    }
    Ok(())
}

/// What Windows says of a file's Authenticode signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// `Valid`, `NotSigned`, `HashMismatch`, `NotTrusted`, `UnknownError`, …
    /// (`System.Management.Automation.SignatureStatus`).
    pub status: String,
    /// The signer certificate's subject, `CN=…, O=…`.
    pub subject: String,
    pub thumbprint: String,
    pub message: String,
}

impl Signature {
    /// The subject's common name.
    pub fn common_name(&self) -> Option<&str> {
        common_name(&self.subject)
    }
}

/// `CN=Luis Almazan, O=…` → `Luis Almazan` (a quoted CN keeps its commas).
pub fn common_name(subject: &str) -> Option<&str> {
    let start = subject.find("CN=")? + 3;
    let rest = &subject[start..];
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split('"').next();
    }
    Some(rest.split(',').next().unwrap_or(rest).trim())
}

/// Step 4's rule, apart from asking Windows: valid, and by `publisher`.
pub fn check_signature(sig: &Signature, publisher: &str) -> Result<(), VerifyError> {
    match sig.status.as_str() {
        "Valid" => {}
        "NotSigned" => return Err(VerifyError::Unsigned),
        other => return Err(VerifyError::Signature(other.to_owned())),
    }
    let got = sig.common_name().unwrap_or_default();
    if got != publisher {
        return Err(VerifyError::Publisher {
            expected: publisher.to_owned(),
            got: got.to_owned(),
        });
    }
    Ok(())
}

/// Ask Windows (`Get-AuthenticodeSignature`, which is `WinVerifyTrust`) about `path`.
/// The path travels in an environment variable, never inside the command text.
#[cfg(windows)]
pub fn authenticode(path: &Path) -> Result<Signature, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "$ErrorActionPreference = 'Stop'; \
        [Console]::OutputEncoding = [Text.Encoding]::UTF8; \
        $s = Get-AuthenticodeSignature -LiteralPath $env:CANTUS_VERIFY_PATH; \
        [pscustomobject]@{ status = [string]$s.Status; \
        subject = [string]$s.SignerCertificate.Subject; \
        thumbprint = [string]$s.SignerCertificate.Thumbprint; \
        message = [string]$s.StatusMessage } | ConvertTo-Json -Compress";
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CANTUS_VERIFY_PATH", path)
        // A PowerShell 7 parent leaves its module path behind, and Windows PowerShell then
        // fails to load its own Security module: let it build its default path.
        .env_remove("PSModulePath")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    // UTF-8 asked for above; lossy all the same, so a localised message never fails it.
    let text = String::from_utf8_lossy(&out.stdout);
    let json = text.trim().trim_start_matches('\u{feff}');
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("powershell: {e}"))?;
    let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    if field("status").is_empty() {
        return Err("powershell gave no signature status".into());
    }
    Ok(Signature {
        status: field("status"),
        subject: field("subject"),
        thumbprint: field("thumbprint"),
        message: field("message"),
    })
}

#[cfg(not(windows))]
pub fn authenticode(_path: &Path) -> Result<Signature, String> {
    Err("Authenticode is Windows-only".into())
}

/// Step 4, when this build has a publisher.
pub fn check_publisher(path: &Path, publisher: Option<&str>) -> Result<(), VerifyError> {
    let Some(publisher) = publisher else {
        return Ok(());
    };
    let sig = authenticode(path).map_err(VerifyError::Signature)?;
    check_signature(&sig, publisher)
}
