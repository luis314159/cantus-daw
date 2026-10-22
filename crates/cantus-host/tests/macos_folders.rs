//! Where plug-ins are looked for on macOS, and what counts as one (ADR 0066): the system's
//! and the user's `Library/Audio/Plug-Ins/{CLAP,VST3}`, `.clap` and `.vst3` bundles taken
//! whole (never walked into), their binaries found inside.

#![cfg(target_os = "macos")]

use cantus_host::scan::Format;
use std::path::PathBuf;

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
}

#[test]
fn the_standard_folders_are_the_libraries() {
    let clap = cantus_host::backend_for(Format::Clap)
        .unwrap()
        .search_paths();
    assert!(clap.contains(&PathBuf::from("/Library/Audio/Plug-Ins/CLAP")));
    assert!(clap.contains(&home().join("Library/Audio/Plug-Ins/CLAP")));
    #[cfg(feature = "vst3")]
    {
        let vst3 = cantus_host::backend_for(Format::Vst3)
            .unwrap()
            .search_paths();
        assert!(vst3.contains(&PathBuf::from("/Library/Audio/Plug-Ins/VST3")));
        assert!(vst3.contains(&home().join("Library/Audio/Plug-Ins/VST3")));
    }
}

#[test]
fn bundles_are_candidates_and_their_insides_are_not() {
    let dir = tempfile::tempdir().unwrap();
    let clap = dir.path().join("Some.clap");
    let macos = clap.join("Contents").join("MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::write(macos.join("Some"), b"").unwrap();
    let backend = cantus_host::backend_for(Format::Clap).unwrap();
    let found = cantus_host::scan::candidates(backend.as_ref(), &[dir.path().to_path_buf()]);
    assert_eq!(found, vec![clap.clone()]);
    assert_eq!(cantus_host::bundle::binary(&clap), Some(macos.join("Some")));
}
