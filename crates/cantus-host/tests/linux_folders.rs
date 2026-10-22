//! Where Cantus looks for plug-ins on Linux (ADR 0065): the CLAP entry header's folders
//! and `CLAP_PATH`, the VST3 SDK's folders and the `VST3_PATH` other hosts and Flatpak's
//! Linux audio extensions use.
#![cfg(target_os = "linux")]

use cantus_host::backend::Backend;
use cantus_host::clap::ClapBackend;
use std::path::PathBuf;

#[test]
fn linux_plug_in_folders() {
    std::env::set_var("HOME", "/home/ana");
    std::env::set_var("CLAP_PATH", "/opt/clap:/srv/more-clap");
    std::env::set_var("VST3_PATH", "/opt/vst3:relative/ignored");
    let clap = ClapBackend.search_paths();
    for p in [
        "/home/ana/.clap",
        "/usr/lib/clap",
        "/usr/local/lib/clap",
        "/opt/clap",
        "/srv/more-clap",
    ] {
        assert!(clap.contains(&PathBuf::from(p)), "{p} in {clap:?}");
    }
    #[cfg(feature = "vst3")]
    {
        let vst3 = cantus_host::vst3::Vst3Backend::new().search_paths();
        for p in [
            "/home/ana/.vst3",
            "/usr/lib/vst3",
            "/usr/local/lib/vst3",
            "/opt/vst3",
        ] {
            assert!(vst3.contains(&PathBuf::from(p)), "{p} in {vst3:?}");
        }
        assert!(
            !vst3.contains(&PathBuf::from("relative/ignored")),
            "only absolute folders: {vst3:?}"
        );
    }
}
