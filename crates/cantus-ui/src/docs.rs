//! The documents Cantus opens for the reader (round 22): the quick-start manual, the
//! third-party notices, the privacy statement, the licence — the copies installed beside
//! `cantus.exe` when they are there, else their pages on GitHub — and the project's links
//! (issue forms, source, the author's site).

use crate::i18n::Locale;
use std::path::{Path, PathBuf};

pub const REPO: &str = "https://github.com/luis314159/cantus";
pub const SITE: &str = "https://lalmazan.com";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Doc {
    /// The quick-start manual in the interface's language.
    Manual,
    Notices,
    Privacy,
    /// The GPL, which the program is under.
    Licence,
    /// Release notes of this version.
    ReleaseNotes,
    /// GitHub's issue form for a bug (with the system and audio-device fields).
    ReportBug,
    /// GitHub's issue form for an idea.
    RequestFeature,
    Source,
    Site,
}

/// Where `doc` is: a file beside the program (`exe_dir`), else a web address. `locale` picks
/// the manual's language.
pub fn target(doc: Doc, exe_dir: Option<&Path>, locale: Locale) -> String {
    let lang = match locale {
        Locale::En => "en",
        Locale::Es => "es",
    };
    let local = |name: &str| -> Option<PathBuf> {
        let p = exe_dir?.join(name);
        p.is_file().then_some(p)
    };
    let file = match doc {
        Doc::Manual => local(&format!("manual/{lang}/index.html")),
        Doc::Notices => local("THIRD-PARTY-NOTICES.txt"),
        Doc::Privacy => local("PRIVACY.txt"),
        Doc::Licence => local("LICENSE.txt"),
        _ => None,
    };
    if let Some(p) = file {
        return p.to_string_lossy().into_owned();
    }
    let version = crate::app::APP_VERSION;
    match doc {
        Doc::Manual => format!("{REPO}/blob/master/docs/manual/{lang}/README.md"),
        Doc::Notices => format!("{REPO}/blob/master/THIRD-PARTY-NOTICES.md"),
        Doc::Privacy => format!("{REPO}/blob/master/PRIVACY.md"),
        Doc::Licence => format!("{REPO}/blob/master/LICENSE-GPL"),
        Doc::ReleaseNotes => format!("{REPO}/releases/tag/v{version}"),
        Doc::ReportBug => format!("{REPO}/issues/new?template=bug_report.yml"),
        Doc::RequestFeature => format!("{REPO}/issues/new?template=feature_request.yml"),
        Doc::Source => REPO.to_owned(),
        Doc::Site => SITE.to_owned(),
    }
}

/// The folder `cantus.exe` runs from.
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Open `doc` in the browser (or, for a text file, the system's viewer).
pub fn open(doc: Doc) {
    let t = target(doc, exe_dir().as_deref(), crate::i18n::locale());
    crate::audio_setup::drivers::open_url(&t);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_copies_are_preferred_and_the_web_is_the_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        assert_eq!(
            target(Doc::Privacy, Some(d), Locale::En),
            format!("{REPO}/blob/master/PRIVACY.md")
        );
        assert_eq!(
            target(Doc::Manual, None, Locale::Es),
            format!("{REPO}/blob/master/docs/manual/es/README.md")
        );
        std::fs::write(d.join("PRIVACY.txt"), "x").unwrap();
        std::fs::create_dir_all(d.join("manual/es")).unwrap();
        std::fs::write(d.join("manual/es/index.html"), "x").unwrap();
        assert_eq!(
            target(Doc::Privacy, Some(d), Locale::En),
            d.join("PRIVACY.txt").to_string_lossy()
        );
        assert_eq!(
            target(Doc::Manual, Some(d), Locale::Es),
            d.join("manual/es/index.html").to_string_lossy()
        );
        // English is not installed here: the web page.
        assert!(target(Doc::Manual, Some(d), Locale::En).starts_with("https://"));
        assert!(target(Doc::ReportBug, Some(d), Locale::En).ends_with("bug_report.yml"));
        assert!(target(Doc::ReleaseNotes, None, Locale::En)
            .ends_with(&format!("/releases/tag/v{}", crate::app::APP_VERSION)));
    }
}
