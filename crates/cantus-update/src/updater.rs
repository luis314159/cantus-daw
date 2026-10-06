//! The updater: asks GitHub on a thread, downloads and verifies on a thread, and says
//! where it is through a [`Status`] the UI reads each frame. Nothing here blocks the
//! caller, and nothing is sent anywhere but the two GitHub requests (ADR 0032 §4).

use crate::install::InstallKind;
use crate::net::{Fetch, NetError};
use crate::notes::{self, NoteLine};
use crate::release::{self, Asset, Flavor, ParseError, Release};
use crate::schedule::{self, State};
use crate::verify::{self, VerifyError};
use crate::version::Version;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The project's repository on GitHub.
pub const REPO: &str = "luis314159/cantus";
/// How many lines of release notes the app shows (the rest is on the release page).
pub const NOTE_LINES: usize = 80;
/// A download a proxy holds for scanning is tried again this often, this many times —
/// about the ten minutes such a quarantine lasts.
pub const HELD_RETRY: Duration = Duration::from_secs(120);
pub const HELD_ATTEMPTS: u32 = 6;

#[derive(Clone, Debug)]
pub struct Config {
    pub repo: String,
    pub current: Version,
    pub flavor: Flavor,
    pub kind: InstallKind,
    pub state_path: PathBuf,
    pub downloads: PathBuf,
    /// The publisher whose signature an update must carry (`verify::PUBLISHER`).
    pub publisher: Option<String>,
    /// How long to wait between tries of a held download (tests make it short).
    pub held_retry: Duration,
}

/// A newer release, as the app shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub version: String,
    pub title: String,
    pub notes: Vec<NoteLine>,
    /// The release's page on GitHub.
    pub page: String,
    /// The installer for this flavour, when this copy installs itself (an installed one).
    pub installer: Option<Asset>,
    /// Where the release's `SHA256SUMS.txt` is, when it has one.
    pub sums_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Checking,
    UpToDate {
        checked: u64,
    },
    Available(Offer),
    Downloading {
        offer: Offer,
        got: u64,
        total: Option<u64>,
    },
    /// A proxy answered with its own page instead of the file: tried again later.
    Held {
        offer: Offer,
        attempt: u32,
    },
    Ready {
        offer: Offer,
        installer: PathBuf,
    },
    /// `key` is a locale key in the `update` area; `detail` is for the log and the hint.
    Failed {
        key: &'static str,
        detail: String,
        offer: Option<Offer>,
    },
}

impl Status {
    pub fn offer(&self) -> Option<&Offer> {
        match self {
            Status::Available(o)
            | Status::Downloading { offer: o, .. }
            | Status::Held { offer: o, .. }
            | Status::Ready { offer: o, .. } => Some(o),
            Status::Failed { offer, .. } => offer.as_ref(),
            _ => None,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(
            self,
            Status::Checking | Status::Downloading { .. } | Status::Held { .. }
        )
    }
}

struct Shared {
    status: Mutex<Status>,
    state: Mutex<State>,
    cancel: AtomicBool,
}

pub struct Updater {
    config: Config,
    fetch: Arc<dyn Fetch>,
    shared: Arc<Shared>,
}

fn api_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

impl Updater {
    pub fn new(config: Config, fetch: Arc<dyn Fetch>) -> Updater {
        let state = State::load(&config.state_path);
        Updater {
            config,
            fetch,
            shared: Arc::new(Shared {
                status: Mutex::new(Status::Idle),
                state: Mutex::new(state),
                cancel: AtomicBool::new(false),
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().expect("status").clone()
    }

    fn set(&self, s: Status) {
        *self.shared.status.lock().expect("status") = s;
    }

    /// When the last check reached GitHub (Unix seconds).
    pub fn last_checked(&self) -> Option<u64> {
        self.shared.state.lock().expect("state").last_success
    }

    /// Check now (`force`, from the menu or Settings) or if a day has passed (at start).
    /// Without a check due, a release found earlier and not yet installed is offered from
    /// what was kept, without asking GitHub again.
    pub fn check(&self, force: bool) {
        self.check_at(force, schedule::now_unix());
    }

    pub fn check_at(&self, force: bool, now: u64) {
        if self.status().busy() {
            return;
        }
        let due = force || self.shared.state.lock().expect("state").due(now);
        if !due {
            let kept = self
                .shared
                .state
                .lock()
                .expect("state")
                .release_json
                .clone();
            if let Some(r) = kept.and_then(|j| Release::parse(j.as_bytes()).ok()) {
                self.decide(&r, false, now);
            }
            return;
        }
        self.set(Status::Checking);
        let (config, fetch, shared) =
            (self.config.clone(), self.fetch.clone(), self.shared.clone());
        let me = Updater {
            config,
            fetch,
            shared,
        };
        std::thread::Builder::new()
            .name("cantus-update-check".into())
            .spawn(move || me.run_check(force, now))
            .expect("spawn the update check");
    }

    fn run_check(&self, force: bool, now: u64) {
        let etag = {
            let mut st = self.shared.state.lock().expect("state");
            st.last_attempt = Some(now);
            st.release_json.as_ref().and(st.etag.clone())
        };
        let answer = self.fetch.get(&api_url(&self.config.repo), etag.as_deref());
        let failed = |key: &'static str, detail: String| Status::Failed {
            key,
            detail,
            offer: None,
        };
        let release = match answer {
            Err(NetError::Network(e)) => Err(failed("update.error.offline", e)),
            Err(e) => Err(failed("update.error.offline", format!("{e:?}"))),
            Ok(a) => match a.status {
                200 => match Release::parse(&a.body) {
                    Ok(r) => {
                        let mut st = self.shared.state.lock().expect("state");
                        st.etag = a.etag;
                        st.release_json = Some(String::from_utf8_lossy(&a.body).into_owned());
                        st.last_success = Some(now);
                        Ok(Some(r))
                    }
                    Err(ParseError::Html) => Err(failed("update.error.held", "a web page".into())),
                    Err(e) => Err(failed("update.error.answer", format!("{e:?}"))),
                },
                304 => {
                    let mut st = self.shared.state.lock().expect("state");
                    st.last_success = Some(now);
                    let kept = st.release_json.clone().unwrap_or_default();
                    Release::parse(kept.as_bytes())
                        .map(Some)
                        .map_err(|e| failed("update.error.answer", format!("{e:?}")))
                }
                // No release published yet.
                404 => {
                    let mut st = self.shared.state.lock().expect("state");
                    st.last_success = Some(now);
                    st.etag = None;
                    st.release_json = None;
                    Ok(None)
                }
                403 | 429 => Err(failed(
                    "update.error.rate_limited",
                    a.rate_reset.map(|t| t.to_string()).unwrap_or_default(),
                )),
                s => Err(failed("update.error.answer", format!("HTTP {s}"))),
            },
        };
        let _ = self
            .shared
            .state
            .lock()
            .expect("state")
            .save(&self.config.state_path);
        match release {
            Err(status) => {
                if let Status::Failed { detail, .. } = &status {
                    log::info!("update check: {detail}");
                }
                self.set(status);
            }
            Ok(None) => self.set(Status::UpToDate { checked: now }),
            Ok(Some(r)) => self.decide(&r, force, now),
        }
    }

    /// Offer `r` if it is newer, not skipped (a forced check offers even a skipped one),
    /// and reuse an installer already downloaded and verified.
    fn decide(&self, r: &Release, force: bool, now: u64) {
        let skipped = self.shared.state.lock().expect("state").skipped.clone();
        let version = r.version_text();
        if !r.is_update_for(&self.config.current)
            || (!force && skipped.as_deref() == Some(&version))
        {
            self.set(Status::UpToDate { checked: now });
            return;
        }
        let offer = Offer {
            title: r
                .name
                .clone()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("Cantus {version}")),
            notes: notes::read(r.body.as_deref().unwrap_or(""), NOTE_LINES),
            page: r.html_url.clone(),
            installer: (self.config.kind == InstallKind::Installed)
                .then(|| r.installer(self.config.flavor).cloned())
                .flatten(),
            sums_url: r.sums().map(|a| a.browser_download_url.clone()),
            version,
        };
        // A download from an earlier session that still verifies is ready as it is.
        if let Some(a) = &offer.installer {
            let path = self.config.downloads.join(&a.name);
            if path.is_file() {
                if let Some(h) = a.sha256() {
                    if verify::check_file(&path, Some(a.size), &[h]).is_ok() {
                        self.set(Status::Ready {
                            offer,
                            installer: path,
                        });
                        return;
                    }
                }
            }
        }
        self.set(Status::Available(offer));
    }

    /// Don't offer this version again (a newer one will be).
    pub fn skip(&self) {
        let Some(offer) = self.status().offer().cloned() else {
            return;
        };
        let mut st = self.shared.state.lock().expect("state");
        st.skipped = Some(offer.version);
        let _ = st.save(&self.config.state_path);
        drop(st);
        self.shared.cancel.store(true, Ordering::Relaxed);
        self.set(Status::Idle);
    }

    /// Stop a download in progress.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// Download and verify the offered installer.
    pub fn download(&self) {
        let status = self.status();
        let offer = match status {
            Status::Available(o) => o,
            Status::Failed { offer: Some(o), .. } => o,
            _ => return,
        };
        if offer.installer.is_none() {
            return;
        }
        self.shared.cancel.store(false, Ordering::Relaxed);
        self.set(Status::Downloading {
            offer: offer.clone(),
            got: 0,
            total: None,
        });
        let me = Updater {
            config: self.config.clone(),
            fetch: self.fetch.clone(),
            shared: self.shared.clone(),
        };
        std::thread::Builder::new()
            .name("cantus-update-download".into())
            .spawn(move || me.run_download(offer))
            .expect("spawn the update download");
    }

    /// Every SHA-256 the release offers for `asset`: GitHub's digest, and the line in
    /// `SHA256SUMS.txt`. On failure: the locale key and a detail for the log.
    fn hashes(&self, offer: &Offer, asset: &Asset) -> Result<Vec<String>, (&'static str, String)> {
        let mut out: Vec<String> = asset.sha256().into_iter().collect();
        if let Some(url) = &offer.sums_url {
            match self.fetch.get(url, None) {
                Ok(a) if a.status == 200 => {
                    if release::looks_like_html(&a.body) {
                        return Err(("update.error.held", "SHA256SUMS is a web page".into()));
                    }
                    let sums = release::parse_sums(&String::from_utf8_lossy(&a.body));
                    match sums.get(&asset.name) {
                        Some(h) => out.push(h.clone()),
                        None => {
                            return Err((
                                "update.error.hash",
                                format!("{} not in SHA256SUMS", asset.name),
                            ))
                        }
                    }
                }
                Ok(a) => return Err(("update.error.answer", format!("HTTP {}", a.status))),
                Err(e) => return Err(("update.error.offline", format!("{e:?}"))),
            }
        }
        if out.is_empty() {
            return Err((VerifyError::NoHash.key(), "no hash published".into()));
        }
        Ok(out)
    }

    fn run_download(&self, offer: Offer) {
        let asset = offer.installer.clone().expect("checked");
        let hashes = match self.hashes(&offer, &asset) {
            Ok(h) => h,
            Err((key, detail)) => {
                return self.set(Status::Failed {
                    key,
                    detail,
                    offer: Some(offer),
                })
            }
        };
        let dest = self.config.downloads.join(&asset.name);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let progress = |got, total| {
                if let Ok(mut s) = self.shared.status.lock() {
                    *s = Status::Downloading {
                        offer: offer.clone(),
                        got,
                        total,
                    };
                }
            };
            let got = self.fetch.download(
                &asset.browser_download_url,
                &dest,
                &progress,
                &self.shared.cancel,
            );
            let checked = match got {
                Err(NetError::Cancelled) => return self.set(Status::Available(offer)),
                Err(e) => Err(Status::Failed {
                    key: "update.error.offline",
                    detail: format!("{e:?}"),
                    offer: Some(offer.clone()),
                }),
                Ok(()) => verify::check_file(&dest, Some(asset.size), &hashes)
                    .and_then(|()| verify::check_publisher(&dest, self.config.publisher.as_deref()))
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&dest);
                        if e.worth_retrying() && attempt < HELD_ATTEMPTS {
                            Status::Held {
                                offer: offer.clone(),
                                attempt,
                            }
                        } else {
                            Status::Failed {
                                key: e.key(),
                                detail: format!("{e:?}"),
                                offer: Some(offer.clone()),
                            }
                        }
                    }),
            };
            match checked {
                Ok(()) => {
                    return self.set(Status::Ready {
                        offer,
                        installer: dest,
                    })
                }
                Err(s @ Status::Held { .. }) => {
                    log::info!("update download held by a proxy (attempt {attempt}); waiting");
                    self.set(s);
                    let until = std::time::Instant::now() + self.config.held_retry;
                    while std::time::Instant::now() < until {
                        if self.shared.cancel.load(Ordering::Relaxed) {
                            return self.set(Status::Available(offer));
                        }
                        std::thread::sleep(Duration::from_millis(50).min(self.config.held_retry));
                    }
                }
                Err(s) => {
                    log::warn!("update download refused: {s:?}");
                    return self.set(s);
                }
            }
        }
    }
}
