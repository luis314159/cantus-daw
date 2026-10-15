//! Write Tonus's demo project (round 28): a sung line through three Tonus settings and a
//! track for your own voice (`views::fx::tonus::demo`).
//!
//! ```text
//! cargo run -p cantus-ui --example tonus_demo -- [out-dir]
//! ```
//!
//! Then open `tonus-demo.cantus` from that folder with File > Open.

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/tonus-demo"));
    match cantus_ui::views::fx::tonus::demo::write_demo(&dir) {
        Ok(path) => println!("{}", path.display()),
        Err(e) => {
            eprintln!("could not write the demo: {e}");
            std::process::exit(1);
        }
    }
}
