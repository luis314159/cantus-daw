//! Getting bytes: `https://` through ureq with the operating system's certificate
//! verifier, `file://` from disk (offline installers and tests), and the one routine that
//! downloads a file to a `.part`, resumes it, refuses a proxy's HTML page and checks the
//! SHA-256 before anything else may use it.

use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Why a download did not produce a verified file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FetchError {
    /// Connection refused, reset, timed out, DNS: try again.
    #[error("network: {0}")]
    Network(String),
    /// An HTML page came back where a file was expected: a filtering proxy holding the
    /// file for inspection (Zscaler does, for up to ten minutes), a captive portal, a
    /// login page. Never written into the library; tried again later.
    #[error("the network answered with a web page instead of the file")]
    Intercepted,
    #[error("the server answered {0}")]
    Http(u16),
    /// The stream ended before the manifest's size: resumable.
    #[error("the download stopped early ({got} of {expected} bytes)")]
    Incomplete { got: u64, expected: u64 },
    /// The bytes are not the ones the manifest names. The file is deleted.
    #[error("the file is not the one expected (SHA-256 {got})")]
    Corrupt { got: String },
    #[error("cancelled")]
    Cancelled,
    #[error("disk: {0}")]
    Io(String),
}

impl FetchError {
    /// How long to wait before trying again, or `None` when trying again cannot help.
    pub fn retry_after(&self, attempt: u32) -> Option<Duration> {
        let backoff = |base: u64, cap: u64| Duration::from_secs((base << attempt.min(6)).min(cap));
        match self {
            FetchError::Network(_) | FetchError::Incomplete { .. } => Some(backoff(2, 60)),
            // A quarantine lasts minutes: wait long enough not to hammer it.
            FetchError::Intercepted => Some(backoff(15, 120)),
            FetchError::Http(code) if *code == 429 || *code >= 500 => Some(backoff(5, 120)),
            // A corrupt file may have been a truncated or replaced response once.
            FetchError::Corrupt { .. } if attempt == 0 => Some(Duration::from_secs(1)),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FetchError {
    fn from(e: std::io::Error) -> Self {
        FetchError::Io(e.to_string())
    }
}

/// An open response.
pub struct Opened {
    pub reader: Box<dyn Read + Send>,
    /// The bytes start at the requested offset (a 206, or a seek in a local file);
    /// otherwise they start at zero and any partial file must be thrown away.
    pub resumed: bool,
    /// The server said the body is HTML.
    pub html: bool,
}

/// Where bytes come from. The real one is [`Net`]; tests may supply their own.
pub trait Transport: Send + Sync {
    fn open(&self, url: &str, from: u64) -> Result<Opened, FetchError>;
}

/// `https://` (and loopback `http://`) through ureq; `file://` from disk.
pub struct Net {
    agent: ureq::Agent,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    pub fn new() -> Net {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            // A stall in the middle of a body is a network error, retried and resumed.
            .timeout_recv_body(None)
            .user_agent(concat!("Cantus/", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Net { agent }
    }
}

/// A `file://` URL as a path (`file:///C:/x/y.zip`, `file:///home/x.zip`).
pub fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = percent_decode(rest);
    // `file:///C:/...` on Windows: drop the slash before the drive letter.
    let b = rest.as_bytes();
    if b.len() > 2 && b[0] == b'/' && b[2] == b':' {
        return Some(PathBuf::from(&rest[1..]));
    }
    Some(PathBuf::from(rest))
}

/// A path as a `file://` URL (for tests and offline installers).
pub fn file_url(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.replace(' ', "%20");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Transport for Net {
    fn open(&self, url: &str, from: u64) -> Result<Opened, FetchError> {
        if let Some(path) = file_url_path(url) {
            let mut f = File::open(&path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => FetchError::Http(404),
                _ => FetchError::Io(e.to_string()),
            })?;
            f.seek(SeekFrom::Start(from))?;
            return Ok(Opened {
                reader: Box::new(f),
                resumed: true,
                html: false,
            });
        }
        let mut req = self.agent.get(url);
        if from > 0 {
            req = req.header("Range", format!("bytes={from}-"));
        }
        let resp = req.call().map_err(|e| FetchError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if status == 416 {
            // The range starts at or past the end: the partial file is suspect.
            return Err(FetchError::Incomplete {
                got: from,
                expected: from,
            });
        }
        if !(200..300).contains(&status) {
            return Err(FetchError::Http(status));
        }
        let html = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.to_ascii_lowercase().starts_with("text/html"));
        Ok(Opened {
            resumed: status == 206,
            html,
            reader: Box::new(resp.into_body().into_reader()),
        })
    }
}

/// Bytes that start like a web page.
pub fn looks_like_html(head: &[u8]) -> bool {
    let t: Vec<u8> = head
        .iter()
        .copied()
        .skip_while(|c| c.is_ascii_whitespace() || *c == 0xEF || *c == 0xBB || *c == 0xBF)
        .take(16)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    t.starts_with(b"<!doctype") || t.starts_with(b"<html") || t.starts_with(b"<head")
}

/// SHA-256 of a file, as lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
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

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Download `url` into `part` (resuming what is there), check its size and SHA-256, and
/// rename it to `dest`. `progress(bytes_so_far)` is called as bytes arrive. A cancelled
/// or interrupted download keeps its `.part` for the next try; a wrong one is deleted.
#[allow(clippy::too_many_arguments)]
pub fn download(
    t: &dyn Transport,
    url: &str,
    size: u64,
    sha256: &str,
    part: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), FetchError> {
    if dest.exists() && sha256_file(dest)? == sha256 {
        progress(size);
        return Ok(());
    }
    if let Some(dir) = part.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > size {
        have = 0;
    }
    if have < size {
        let opened = t.open(url, have)?;
        if opened.html {
            return Err(FetchError::Intercepted);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(part)?;
        if !opened.resumed {
            have = 0;
        }
        file.set_len(have)?;
        file.seek(SeekFrom::Start(have))?;
        progress(have);
        let mut reader = opened.reader;
        let mut buf = vec![0u8; 1 << 16];
        let mut first = have == 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(FetchError::Cancelled);
            }
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    file.flush()?;
                    return Err(FetchError::Network(e.to_string()));
                }
            };
            if first {
                first = false;
                if looks_like_html(&buf[..n]) {
                    drop(file);
                    let _ = std::fs::remove_file(part);
                    return Err(FetchError::Intercepted);
                }
            }
            if have + n as u64 > size {
                drop(file);
                let _ = std::fs::remove_file(part);
                return Err(FetchError::Corrupt {
                    got: "longer than the manifest says".into(),
                });
            }
            file.write_all(&buf[..n])?;
            have += n as u64;
            progress(have);
        }
        file.flush()?;
        if have < size && url.starts_with("file://") {
            // A local file does not grow by trying again: it is the wrong file.
            drop(file);
            let _ = std::fs::remove_file(part);
            return Err(FetchError::Corrupt {
                got: "shorter than the manifest says".into(),
            });
        }
        if have < size {
            return Err(FetchError::Incomplete {
                got: have,
                expected: size,
            });
        }
    }
    let got = sha256_file(part)?;
    if got != sha256 {
        let _ = std::fs::remove_file(part);
        return Err(FetchError::Corrupt { got });
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(part, dest)?;
    Ok(())
}
