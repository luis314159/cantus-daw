//! Which systems the latest release has a file for (ADR 0067 §4). The download card says
//! "Available" for Linux or macOS only when the latest release holds that system's file under
//! its stable name; "Coming soon" otherwise — never a link that would answer 404. Round 39:
//! the answer comes from the release's facts read at build time (`release.rs`), so the
//! visitor's browser no longer asks api.github.com (and no shared address runs into its
//! hourly limit).

use yew::prelude::*;

use crate::lang::Lang;
use crate::release;
use crate::top::{STABLE_APPIMAGE, STABLE_DMG};

/// The systems beside Windows whose file the latest release holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shelf {
    pub linux: bool,
    pub macos: bool,
}

impl Shelf {
    /// From the names of the latest release's files.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Shelf {
        let mut s = Shelf::default();
        for n in names {
            s.linux |= n == STABLE_APPIMAGE;
            s.macos |= n == STABLE_DMG;
        }
        s
    }

    /// The latest release's, as of the build.
    pub fn latest() -> Shelf {
        Shelf::from_names(release::REL_FILES.iter().map(|f| f.name))
    }

    /// macOS in a few words, the same wherever the site names its systems (round 41: the hero
    /// said "macOS preview" while the download card said "Coming soon"). A preview while the
    /// latest release holds the disk image; while it does not — beta.15 was published for
    /// Windows and Linux only, macOS held for the next beta — "macOS: next beta".
    pub fn mac(self, l: Lang) -> &'static str {
        if self.macos {
            l.t("macOS preview", "Vista previa para macOS")
        } else {
            l.t("macOS: next beta", "macOS: próxima beta")
        }
    }

    /// What a system's card says where its download would be, while the latest release has no
    /// file for it: macOS is held for the next beta; any other system is still to come.
    pub fn wait(os_is_mac: bool, l: Lang) -> &'static str {
        if os_is_mac {
            l.t("Next beta", "Próxima beta")
        } else {
            l.t("Coming soon", "Muy pronto")
        }
    }
}

/// The shelf (a hook still, so the components that ask keep their shape).
#[hook]
pub fn use_shelf() -> Shelf {
    Shelf::latest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_is_available_only_when_its_stable_file_is_there() {
        let none = Shelf::from_names(["Cantus-windows-x64-setup.exe", "SHA256SUMS.txt"]);
        assert_eq!(none, Shelf::default());
        // The versioned file alone is not what the button links.
        let versioned = Shelf::from_names(["Cantus-0.2.0-x86_64.AppImage"]);
        assert!(!versioned.linux);
        // Nor is the old stable name alone (beta.15 and before): the button links the new one.
        let old = Shelf::from_names(["Cantus-linux-x86_64.AppImage"]);
        assert!(!old.linux);
        let both = Shelf::from_names([STABLE_APPIMAGE, STABLE_DMG]);
        assert_eq!(
            both,
            Shelf {
                linux: true,
                macos: true
            }
        );
        assert_eq!(
            Shelf::from_names([STABLE_DMG]),
            Shelf {
                linux: false,
                macos: true
            }
        );
    }
}
