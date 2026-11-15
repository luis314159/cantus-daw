//! DrumGizmo kits for Tympanum (ADR 0124 §3). A user's DrumGizmo kit is built at install
//! into an SFZ multi-mic kit (`cantus_content::user_pack::dg`, r40:fix-f), which
//! [`super::read_sfz`] reads; only a receipt an earlier build of round 40 wrote still names
//! the `.xml` itself, and it is asked to be installed again.

use cantus_core::inst::tympanum::KitRef;
use cantus_core::inst::tympanum_kit::RecordedKit;
use std::path::Path;

/// A DrumGizmo kit (`drumkit.xml`).
pub fn read_drumgizmo(path: &Path, kit: KitRef, name: &str) -> Result<RecordedKit, String> {
    let _ = (kit, name);
    Err(format!(
        "{}: a DrumGizmo kit installed by an earlier build; install it again from the file (Your own kits)",
        path.display()
    ))
}
