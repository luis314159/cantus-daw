//! Which system the visitor is on, from the browser's own user-agent string (read locally,
//! never sent anywhere). It only decides what the download card says.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    /// A phone or tablet: Cantus is a desktop program.
    Mobile,
}

impl Os {
    /// Windows in the pre-renderer: the static page offers the platform we ship, and the app
    /// speaks to the visitor's own system once it runs.
    pub fn detect() -> Self {
        if !cfg!(target_arch = "wasm32") {
            return Os::Windows;
        }
        let ua = web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .unwrap_or_default()
            .to_lowercase();
        if ua.contains("android") || ua.contains("iphone") || ua.contains("ipad") {
            Os::Mobile
        } else if ua.contains("windows") {
            Os::Windows
        } else if ua.contains("mac os") || ua.contains("macintosh") {
            Os::MacOs
        } else if ua.contains("linux") || ua.contains("x11") {
            Os::Linux
        } else {
            Os::Windows // unknown: show the platform we ship
        }
    }
}
