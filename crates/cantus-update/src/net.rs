//! The network, behind a trait so the updater is tested without one.
//!
//! The real client (`Https`) speaks through the operating system's TLS (SChannel on
//! Windows) and its trust store, so a TLS-inspecting corporate proxy whose root the
//! company installed is trusted exactly as the browser trusts it — and nothing else is:
//! verification is never turned off. The proxy is, in order: the one typed in Settings,
//! the `HTTPS_PROXY`/`ALL_PROXY`/`HTTP_PROXY` variables (with `NO_PROXY`), Windows'
//! fixed proxy (Internet Options). A proxy script (PAC) is not evaluated; a client that
//! tunnels transparently (Zscaler's) needs none.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The API's answers are small; anything larger is not a release.
pub const MAX_JSON: u64 = 4 << 20;
/// No Cantus installer is this large; a download that grows past it is stopped.
pub const MAX_DOWNLOAD: u64 = 1 << 30;

/// GitHub asks for a User-Agent. This one names the program and nothing else — no
/// version, no system, no identifier (no telemetry, ADR 0030 §4).
pub const USER_AGENT: &str = "cantus-update";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub etag: Option<String>,
    /// `X-RateLimit-Reset` on a refusal: when GitHub will answer again (Unix seconds).
    pub rate_reset: Option<u64>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetError {
    /// Could not reach the server (offline, DNS, TLS, proxy refused).
    Network(String),
    Status(u16),
    TooLarge,
    Cancelled,
    Io(String),
}

pub trait Fetch: Send + Sync {
    /// `GET url`, with `If-None-Match` when an `etag` is given.
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Answer, NetError>;

    /// Download `url` into `dest` (written beside it and renamed when complete). `progress`
    /// is told the bytes so far and the total when known.
    fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: &dyn Fn(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<(), NetError>;
}

/// Windows' fixed proxy from `ProxyServer`: `host:port`, or per scheme
/// `http=host:port;https=host:port` (the HTTPS one is ours).
pub fn parse_windows_proxy(server: &str) -> Option<String> {
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    if !server.contains('=') {
        return Some(server.to_owned());
    }
    let mut http = None;
    for part in server.split(';') {
        let Some((scheme, addr)) = part.split_once('=') else {
            continue;
        };
        match scheme.trim().to_ascii_lowercase().as_str() {
            "https" => return Some(addr.trim().to_owned()),
            "http" => http = Some(addr.trim().to_owned()),
            _ => {}
        }
    }
    http
}

/// Read Internet Options' fixed proxy, when it is on.
#[cfg(windows)]
pub fn windows_proxy() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let query = |name: &str| -> Option<String> {
        let out = std::process::Command::new("reg")
            .args(["query", key, "/v", name])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let line = text.lines().find(|l| l.trim_start().starts_with(name))?;
        // "    ProxyEnable    REG_DWORD    0x1"
        line.split_whitespace().nth(2).map(str::to_owned)
    };
    let enabled = query("ProxyEnable")?;
    if enabled != "0x1" {
        return None;
    }
    parse_windows_proxy(&query("ProxyServer")?)
}

#[cfg(not(windows))]
pub fn windows_proxy() -> Option<String> {
    None
}

/// The real client.
pub struct Https {
    agent: ureq::Agent,
}

impl Https {
    /// `proxy`: the one typed in Settings, if any.
    pub fn new(proxy: Option<&str>) -> Https {
        use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
        let proxy = proxy
            .filter(|p| !p.trim().is_empty())
            .and_then(|p| ureq::Proxy::new(p.trim()).ok())
            .or_else(ureq::Proxy::try_from_env)
            .or_else(|| windows_proxy().and_then(|p| ureq::Proxy::new(&p).ok()));
        let tls = TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .proxy(proxy)
            .https_only(true)
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .build()
            .into();
        Https { agent }
    }
}

fn net(e: ureq::Error) -> NetError {
    match e {
        ureq::Error::StatusCode(s) => NetError::Status(s),
        ureq::Error::BodyExceedsLimit(_) => NetError::TooLarge,
        other => NetError::Network(other.to_string()),
    }
}

impl Fetch for Https {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Answer, NetError> {
        let mut req = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tag) = etag {
            req = req.header("If-None-Match", tag);
        }
        let mut resp = req.call().map_err(net)?;
        let header = |name: &str| {
            resp.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let status = resp.status().as_u16();
        let etag = header("etag");
        let rate_reset = header("x-ratelimit-reset").and_then(|v| v.parse().ok());
        let body = resp
            .body_mut()
            .with_config()
            .limit(MAX_JSON)
            .read_to_vec()
            .map_err(net)?;
        Ok(Answer {
            status,
            etag,
            rate_reset,
            body,
        })
    }

    fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: &dyn Fn(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<(), NetError> {
        let mut resp = self
            .agent
            .get(url)
            .header("Accept", "application/octet-stream")
            .call()
            .map_err(net)?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(NetError::Status(status));
        }
        let total = resp.body().content_length();
        if total.is_some_and(|t| t > MAX_DOWNLOAD) {
            return Err(NetError::TooLarge);
        }
        let part = dest.with_extension("part");
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| NetError::Io(e.to_string()))?;
        }
        let mut file = std::fs::File::create(&part).map_err(|e| NetError::Io(e.to_string()))?;
        let mut reader = resp
            .body_mut()
            .with_config()
            .limit(MAX_DOWNLOAD)
            .reader();
        let mut buf = vec![0u8; 1 << 16];
        let mut got = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                drop(file);
                let _ = std::fs::remove_file(&part);
                return Err(NetError::Cancelled);
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| NetError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| NetError::Io(e.to_string()))?;
            got += n as u64;
            progress(got, total);
        }
        file.sync_all().map_err(|e| NetError::Io(e.to_string()))?;
        drop(file);
        let _ = std::fs::remove_file(dest);
        std::fs::rename(&part, dest).map_err(|e| NetError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_fixed_proxy_settings_read_either_way() {
        assert_eq!(parse_windows_proxy("proxy.corp:8080").as_deref(), Some("proxy.corp:8080"));
        assert_eq!(
            parse_windows_proxy("http=a:80;https=b:443;ftp=c:21").as_deref(),
            Some("b:443")
        );
        assert_eq!(parse_windows_proxy("http=a:80").as_deref(), Some("a:80"));
        assert_eq!(parse_windows_proxy("ftp=c:21"), None);
        assert_eq!(parse_windows_proxy("  "), None);
    }
}
