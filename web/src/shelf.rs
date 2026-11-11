//! Which systems the latest release has a file for (ADR 0067 §4). The download card says
//! "Available" for Linux or macOS only when the latest release holds that system's file under
//! its stable name; "Coming soon" otherwise — never a link that would answer 404. Round 39:
//! the answer comes from the release's facts read at build time (`release.rs`), so the
//! visitor's browser no longer asks api.github.com (and no shared address runs into its
//! hourly limit).

use yew::prelude::*;

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
        let versioned = Shelf::from_names(["Cantus-0.2.0-linux-x86_64.AppImage"]);
        assert!(!versioned.linux);
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
