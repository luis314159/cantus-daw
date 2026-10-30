//! Small line icons, drawn for the page (24×24, 1.7 stroke, currentColor), and the logo.

use yew::prelude::*;

/// The real Cantus logo (the app icon): a C of waves with a gold playhead cross.
pub fn logo() -> Html {
    html! { <img src="logo.svg" alt="" width="64" height="64" /> }
}
fn line(d: &'static str) -> Html {
    html! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path {d}/></svg>
    }
}

pub fn timeline() -> Html {
    line("M3 6h10M3 12h18M3 18h13M16 4v4M9 10v4M19 16v4")
}
pub fn mixer() -> Html {
    line("M6 3v18M12 3v18M18 3v18M4 14h4M10 8h4M16 16h4")
}
pub fn wave() -> Html {
    line("M2 12h3l2-6 3 12 3-9 2 5 2-2h5")
}
pub fn plug() -> Html {
    line("M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0zM12 17v4")
}
pub fn mic() -> Html {
    line("M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v3")
}
pub fn export() -> Html {
    line("M12 3v12M7 8l5-5 5 5M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4")
}
pub fn grid() -> Html {
    line("M4 4h4v4H4zM10 4h4v4h-4zM16 4h4v4h-4zM4 10h4v4H4zM10 10h4v4h-4zM16 10h4v4h-4zM4 16h4v4H4zM10 16h4v4h-4zM16 16h4v4h-4z")
}
pub fn check() -> Html {
    line("M4 12.5l5 5L20 6.5")
}
pub fn windows() -> Html {
    html! { <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 5.5l8-1.1v7H3zM12 4.3l9-1.3v8.4h-9zM3 12.6h8v7l-8-1.1zM12 12.6h9V21l-9-1.3z"/></svg> }
}
pub fn arrow_down() -> Html {
    line("M12 4v12M6 11l6 6 6-6M5 20h14")
}
pub fn github() -> Html {
    html! { <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2a10 10 0 0 0-3.16 19.5c.5.1.68-.22.68-.48v-1.7c-2.78.6-3.37-1.34-3.37-1.34-.46-1.16-1.11-1.47-1.11-1.47-.9-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.9 1.52 2.34 1.08 2.9.83.09-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.94 0-1.09.39-1.98 1.03-2.68-.1-.25-.45-1.27.1-2.64 0 0 .84-.27 2.75 1.02a9.5 9.5 0 0 1 5 0c1.91-1.29 2.75-1.02 2.75-1.02.55 1.37.2 2.39.1 2.64.64.7 1.03 1.59 1.03 2.68 0 3.84-2.34 4.68-4.57 4.93.36.31.68.92.68 1.85v2.74c0 .27.18.58.69.48A10 10 0 0 0 12 2z"/></svg> }
}
pub fn sun() -> Html {
    line("M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4")
}
pub fn moon() -> Html {
    line("M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z")
}
