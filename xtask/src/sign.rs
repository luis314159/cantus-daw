//! Code signing (ADR 0030 §3): `signtool` from the Windows SDK, configured only through
//! the environment, so no secret is ever in the repository or on a command line that the
//! log prints.
//!
//! ```text
//! CANTUS_SIGN=pfx    CANTUS_SIGN_PFX=<file.pfx>  CANTUS_SIGN_PFX_PASSWORD=<password>
//! CANTUS_SIGN=store  CANTUS_SIGN_THUMBPRINT=<sha1>          (a certificate in the user's
//!                                                           store: a token, an EV card)
//! CANTUS_SIGN=azure  CANTUS_SIGN_AZURE_DLIB=<Azure.CodeSigning.Dlib.dll>
//!                    CANTUS_SIGN_AZURE_METADATA=<metadata.json>   (Azure Trusted Signing;
//!                                                           the login is Azure's own env)
//! CANTUS_SIGN_TIMESTAMP=<RFC 3161 url> | none     (default: DigiCert's; Azure: its own)
//! CANTUS_SIGNTOOL=<path to signtool.exe>          (default: the newest Windows SDK's)
//! ```
//!
//! `cargo run -p xtask -- sign-test-cert` makes a self-signed certificate into
//! `target/signing-test/` (never committed) and `package --sign-test` signs with it: the
//! whole pipeline, proved without buying anything. Windows does not trust that
//! certificate, so its signatures verify as present but not trusted.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_TIMESTAMP: &str = "http://timestamp.digicert.com";
pub const AZURE_TIMESTAMP: &str = "http://timestamp.acs.microsoft.com";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signer {
    Pfx { file: PathBuf, password: String },
    Store { thumbprint: String },
    Azure { dlib: PathBuf, metadata: PathBuf },
}

/// Read the signer from the environment; `Ok(None)` when nothing asks for signing.
pub fn from_env() -> Result<Option<Signer>, String> {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let need = |k: &str| var(k).ok_or_else(|| format!("{k} is not set"));
    match var("CANTUS_SIGN").as_deref() {
        None => Ok(None),
        Some("pfx") => Ok(Some(Signer::Pfx {
            file: need("CANTUS_SIGN_PFX")?.into(),
            password: need("CANTUS_SIGN_PFX_PASSWORD")?,
        })),
        Some("store") => Ok(Some(Signer::Store {
            thumbprint: need("CANTUS_SIGN_THUMBPRINT")?,
        })),
        Some("azure") => Ok(Some(Signer::Azure {
            dlib: need("CANTUS_SIGN_AZURE_DLIB")?.into(),
            metadata: need("CANTUS_SIGN_AZURE_METADATA")?.into(),
        })),
        Some(other) => Err(format!("CANTUS_SIGN={other}: expected pfx, store or azure")),
    }
}

/// The test certificate made by `sign-test-cert`.
pub fn test_signer(target: &Path) -> Result<Signer, String> {
    let dir = target.join("signing-test");
    let file = dir.join("cantus-test.pfx");
    let password = std::fs::read_to_string(dir.join("password.txt")).map_err(|_| {
        "no test certificate: run `cargo run -p xtask -- sign-test-cert`".to_string()
    })?;
    Ok(Signer::Pfx {
        file,
        password: password.trim().to_owned(),
    })
}

/// `signtool sign`'s arguments for `file`; the password is the only secret, and
/// [`shown`] masks it.
pub fn sign_args(signer: &Signer, timestamp: Option<&str>, file: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["sign", "/fd", "SHA256"].map(OsString::from).to_vec();
    match signer {
        Signer::Pfx {
            file: pfx,
            password,
        } => {
            a.push("/f".into());
            a.push(pfx.into());
            a.push("/p".into());
            a.push(password.into());
        }
        Signer::Store { thumbprint } => {
            a.push("/sha1".into());
            a.push(thumbprint.into());
        }
        Signer::Azure { dlib, metadata } => {
            a.push("/dlib".into());
            a.push(dlib.into());
            a.push("/dmdf".into());
            a.push(metadata.into());
        }
    }
    let ts = match signer {
        Signer::Azure { .. } => Some(timestamp.unwrap_or(AZURE_TIMESTAMP)),
        _ => timestamp,
    };
    if let Some(ts) = ts {
        a.push("/tr".into());
        a.push(ts.into());
        a.push("/td".into());
        a.push("SHA256".into());
    }
    a.push("/d".into());
    a.push("Cantus".into());
    a.push(file.into());
    a
}

/// The arguments as the log prints them: the password replaced.
pub fn shown(args: &[OsString]) -> String {
    let mut out = Vec::new();
    let mut hide = false;
    for a in args {
        if hide {
            out.push("********".to_string());
            hide = false;
            continue;
        }
        hide = a == "/p";
        out.push(a.to_string_lossy().into_owned());
    }
    out.join(" ")
}

/// The timestamp server: `CANTUS_SIGN_TIMESTAMP`, `none` for none.
pub fn timestamp_from_env() -> Option<String> {
    match std::env::var("CANTUS_SIGN_TIMESTAMP") {
        Ok(v) if v.eq_ignore_ascii_case("none") => None,
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => Some(DEFAULT_TIMESTAMP.to_owned()),
    }
}

/// `signtool.exe`: `CANTUS_SIGNTOOL`, else the newest Windows 10/11 SDK's x64 one.
pub fn signtool() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CANTUS_SIGNTOOL") {
        return Some(PathBuf::from(p));
    }
    let root = PathBuf::from(
        std::env::var_os("ProgramFiles(x86)").unwrap_or_else(|| r"C:\Program Files (x86)".into()),
    )
    .join(r"Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join(r"x64\signtool.exe").is_file())
        .collect();
    versions.sort_by_key(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.split('.')
                    .filter_map(|x| x.parse::<u32>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    versions.pop().map(|p| p.join(r"x64\signtool.exe"))
}

/// Sign `files` in place. Exits the task on failure: an unsigned file in a signed
/// release is a release to redo, not a warning.
pub fn sign(signer: &Signer, files: &[PathBuf]) {
    let Some(tool) = signtool() else {
        eprintln!("signtool.exe not found: install the Windows SDK or set CANTUS_SIGNTOOL");
        std::process::exit(1);
    };
    let ts = timestamp_from_env();
    for f in files {
        let args = sign_args(signer, ts.as_deref(), f);
        eprintln!("> signtool {}", shown(&args));
        let out = Command::new(&tool)
            .args(&args)
            .output()
            .expect("run signtool");
        if !out.status.success() {
            eprintln!(
                "signing {} failed:\n{}{}",
                f.display(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            std::process::exit(1);
        }
    }
}

/// What Windows says of a file's signature: (status, signer subject).
pub fn status(file: &Path) -> (String, String) {
    let script = "$s = Get-AuthenticodeSignature -LiteralPath $env:CANTUS_VERIFY_PATH; \
        '{0}|{1}' -f $s.Status, $s.SignerCertificate.Subject";
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CANTUS_VERIFY_PATH", file)
        .env_remove("PSModulePath")
        .output()
        .expect("run powershell");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let (s, subj) = text.split_once('|').unwrap_or((&text, ""));
    (s.to_owned(), subj.to_owned())
}

/// Check every file carries a signature that matches its bytes. With the test
/// certificate Windows answers `UnknownError` (signed, intact, root not trusted); a real
/// certificate answers `Valid`. `NotSigned` or `HashMismatch` stop the task.
pub fn check(files: &[PathBuf]) {
    for f in files {
        let (st, subject) = status(f);
        eprintln!("  {}: {st} ({subject})", f.display());
        if st == "NotSigned" || st == "HashMismatch" || st.is_empty() {
            eprintln!("{} is not signed as it should be", f.display());
            std::process::exit(1);
        }
    }
}

/// A pseudo-random password for the throw-away test certificate.
fn test_password() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    (0..4)
        .map(|i| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Make the self-signed test certificate: created in the user's store, exported with its
/// key to `target/signing-test/cantus-test.pfx`, and removed from the store again.
pub fn make_test_cert(target: &Path) {
    let dir = target.join("signing-test");
    std::fs::create_dir_all(&dir).unwrap();
    let pfx = dir.join("cantus-test.pfx");
    let password = test_password();
    let script = "$ErrorActionPreference = 'Stop'; \
        $c = New-SelfSignedCertificate -Type CodeSigningCert \
          -Subject 'CN=Cantus Test Signing (not trusted)' \
          -CertStoreLocation Cert:\\CurrentUser\\My -KeyAlgorithm RSA -KeyLength 3072 \
          -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(1); \
        $pw = ConvertTo-SecureString -String $env:CANTUS_TEST_PFX_PASSWORD -Force -AsPlainText; \
        Export-PfxCertificate -Cert $c -FilePath $env:CANTUS_TEST_PFX -Password $pw | Out-Null; \
        Remove-Item -Path ('Cert:\\CurrentUser\\My\\' + $c.Thumbprint) -DeleteKey; \
        $c.Thumbprint";
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CANTUS_TEST_PFX", &pfx)
        .env("CANTUS_TEST_PFX_PASSWORD", &password)
        .env_remove("PSModulePath")
        .output()
        .expect("run powershell");
    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr));
        std::process::exit(1);
    }
    std::fs::write(dir.join("password.txt"), &password).unwrap();
    eprintln!(
        "test certificate {} in {}\n(thumbprint {}; not trusted by Windows — for proving the pipeline only)\n\
         sign with it: cargo run -p xtask -- package --sign-test",
        "CN=Cantus Test Signing (not trusted)",
        pfx.display(),
        String::from_utf8_lossy(&out.stdout).trim()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_never_reaches_the_log() {
        let s = Signer::Pfx {
            file: "c.pfx".into(),
            password: "hunter2".into(),
        };
        let args = sign_args(&s, Some(DEFAULT_TIMESTAMP), Path::new("cantus.exe"));
        let line = shown(&args);
        assert!(!line.contains("hunter2"), "{line}");
        assert!(line.contains("/p ********"));
        assert!(line.starts_with("sign /fd SHA256 /f c.pfx"));
        assert!(line.ends_with("/tr http://timestamp.digicert.com /td SHA256 /d Cantus cantus.exe"));
    }

    #[test]
    fn azure_signs_through_its_dlib_with_its_own_timestamp() {
        let s = Signer::Azure {
            dlib: "dlib.dll".into(),
            metadata: "m.json".into(),
        };
        let line = shown(&sign_args(&s, None, Path::new("x.exe")));
        assert!(line.contains("/dlib dlib.dll /dmdf m.json /tr http://timestamp.acs.microsoft.com"));
        let store = Signer::Store {
            thumbprint: "AB12".into(),
        };
        let line = shown(&sign_args(&store, None, Path::new("x.exe")));
        assert_eq!(line, "sign /fd SHA256 /sha1 AB12 /d Cantus x.exe");
    }
}
