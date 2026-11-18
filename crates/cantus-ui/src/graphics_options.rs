//! **Safe-graphics start** (GitHub issue #1, ADR 0146 — provisional number): the ways to
//! open the window that a user, or we on a user's machine, can try one at a time when the
//! window misbehaves — flickers, stays black, jumps. Each reaches `eframe::NativeOptions`
//! (`crate::native_options`) or the boot (`crate::boot::main_window_commands`), and each
//! has a test that it does. `docs/troubleshooting-graphics.md` says how to use them.
//!
//! | Flag | `CANTUS_GRAPHICS` word | What |
//! |---|---|---|
//! | `--safe-graphics` | `safe` | all of: no splash, scale 100 %, vsync on, no multisampling, no transparency |
//! | `--vsync on\|off` | `vsync=on\|off` | the swap interval (on by default) |
//! | `--msaa N` | `msaa=N` | multisampling (0 by default) |
//! | `--no-splash` | `no-splash` | open the main window at once, decorated, instead of the splash that becomes it |
//! | `--scale F` | `scale=F` | our interface scale for this session (the screen not followed; never stored) |
//!
//! The variable takes the words comma-separated (`CANTUS_GRAPHICS=no-splash,vsync=off`);
//! a flag on the command line wins over the variable.

/// The variable that holds the graphics options, for a start without a command line.
pub const VAR: &str = "CANTUS_GRAPHICS";

/// The window's graphics options for this start.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graphics {
    /// `--safe-graphics`: every safer choice below at once.
    pub safe: bool,
    /// The swap interval; `None` = eframe's default (on).
    pub vsync: Option<bool>,
    /// Multisampling samples; `None` = eframe's default (0).
    pub msaa: Option<u16>,
    /// Open the main window at once instead of the splash that becomes it.
    pub no_splash: bool,
    /// Our interface scale for this session.
    pub scale: Option<f32>,
}

fn on_off(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Some(true),
        "off" | "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn scale_of(v: &str) -> Option<f32> {
    let v = v.trim().trim_end_matches('%');
    let f: f32 = v.parse().ok()?;
    // "125" and "1.25" both mean 125 %.
    let f = if f > 4.0 { f / 100.0 } else { f };
    (f.is_finite() && f > 0.0).then_some(f)
}

impl Graphics {
    /// One command-line argument (`next` gives the one after, for flags with a value).
    /// Returns whether it was one of these; an unknown value is ignored with a warning.
    pub fn take_arg(&mut self, arg: &str, next: &mut dyn Iterator<Item = String>) -> bool {
        match arg {
            "--safe-graphics" => self.safe = true,
            "--no-splash" => self.no_splash = true,
            "--vsync" => match next.next().as_deref().and_then(on_off) {
                Some(v) => self.vsync = Some(v),
                None => log::warn!("--vsync takes on or off"),
            },
            "--msaa" => match next.next().and_then(|v| v.trim().parse().ok()) {
                Some(n) => self.msaa = Some(n),
                None => log::warn!("--msaa takes a number of samples (0, 2, 4, 8)"),
            },
            "--scale" => match next.next().as_deref().and_then(scale_of) {
                Some(f) => self.scale = Some(f),
                None => log::warn!("--scale takes a scale (1.25 or 125)"),
            },
            _ => return false,
        }
        true
    }

    /// The options a `CANTUS_GRAPHICS` value names (`safe`, `no-splash`, `vsync=off`,
    /// `msaa=0`, `scale=1.25`, comma-separated); unknown words are skipped.
    pub fn from_words(value: &str) -> Graphics {
        let mut g = Graphics::default();
        for word in value
            .split([',', ';', ' '])
            .filter(|w| !w.trim().is_empty())
        {
            let (k, v) = match word.split_once('=') {
                Some((k, v)) => (k.trim(), Some(v)),
                None => (word.trim(), None),
            };
            match (k.to_ascii_lowercase().as_str(), v) {
                ("safe" | "safe-graphics", _) => g.safe = true,
                ("no-splash" | "nosplash", _) => g.no_splash = true,
                ("vsync", Some(v)) => g.vsync = on_off(v),
                ("msaa", Some(v)) => g.msaa = v.trim().parse().ok(),
                ("scale", Some(v)) => g.scale = scale_of(v),
                _ => log::warn!("{VAR}: unknown word {word:?}"),
            }
        }
        g
    }

    /// The variable's options, if it is set.
    pub fn from_env() -> Graphics {
        std::env::var(VAR)
            .map(|v| Graphics::from_words(&v))
            .unwrap_or_default()
    }

    /// `self` (the command line's) over `base` (the variable's): what a flag set wins.
    pub fn over(self, base: Graphics) -> Graphics {
        Graphics {
            safe: self.safe || base.safe,
            vsync: self.vsync.or(base.vsync),
            msaa: self.msaa.or(base.msaa),
            no_splash: self.no_splash || base.no_splash,
            scale: self.scale.or(base.scale),
        }
    }

    /// The swap interval the window asks for.
    pub fn vsync(&self) -> bool {
        self.vsync.unwrap_or(true)
    }

    /// Multisampling samples.
    pub fn msaa(&self) -> u16 {
        if self.safe && self.msaa.is_none() {
            0
        } else {
            self.msaa.unwrap_or(0)
        }
    }

    /// Whether the window opens as the splash (ADR 0024 §12).
    pub fn splash(&self) -> bool {
        !(self.safe || self.no_splash)
    }

    /// The scale fixed for the session: `--scale`, else 100 % when safe.
    pub fn scale(&self) -> Option<f32> {
        self.scale.or(self.safe.then_some(1.0))
    }

    /// Whether a window with alpha may be asked for (the dancers' see-through companion,
    /// ADR 0091 §2): never in safe graphics.
    pub fn alpha_allowed(&self) -> bool {
        !self.safe
    }

    /// One line for the log: what this start asked for.
    pub fn summary(&self) -> String {
        format!(
            "graphics: renderer=glow vsync={} msaa={} splash={} scale={} safe={}",
            if self.vsync() { "on" } else { "off" },
            self.msaa(),
            self.splash(),
            self.scale()
                .map_or_else(|| "follow".to_string(), |s| format!("{s}")),
            self.safe,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Graphics {
        let mut g = Graphics::default();
        let mut it = list.iter().map(|s| s.to_string());
        while let Some(a) = it.next() {
            assert!(g.take_arg(&a, &mut it), "{a} is a graphics flag");
        }
        g
    }

    #[test]
    fn the_flags_parse() {
        assert!(Graphics::default().vsync());
        assert_eq!(Graphics::default().msaa(), 0);
        assert!(Graphics::default().splash());
        assert_eq!(Graphics::default().scale(), None);
        let g = args(&[
            "--vsync",
            "off",
            "--msaa",
            "4",
            "--no-splash",
            "--scale",
            "125",
        ]);
        assert!(!g.vsync());
        assert_eq!(g.msaa(), 4);
        assert!(!g.splash());
        assert_eq!(g.scale(), Some(1.25));
        let s = args(&["--safe-graphics"]);
        assert!(s.vsync() && s.msaa() == 0 && !s.splash() && !s.alpha_allowed());
        assert_eq!(s.scale(), Some(1.0));
        let mut none = Graphics::default();
        assert!(!none.take_arg("--mixer", &mut std::iter::empty()));
        assert_eq!(args(&["--scale", "1.5"]).scale(), Some(1.5));
        assert_eq!(args(&["--scale", "150%"]).scale(), Some(1.5));
    }

    #[test]
    fn the_variable_parses_and_a_flag_wins() {
        let env = Graphics::from_words("no-splash, vsync=off,scale=1.1, bogus");
        assert!(!env.splash() && !env.vsync());
        assert_eq!(env.scale(), Some(1.1));
        assert!(Graphics::from_words("safe").safe);
        let both = args(&["--vsync", "on"]).over(env);
        assert!(both.vsync(), "the flag wins");
        assert!(!both.splash(), "the variable's no-splash stays");
        assert!(both.summary().contains("vsync=on"), "{}", both.summary());
    }
}
