//! The room card over an instrument's picture stays whole (round 40 phase 3, the
//! integrator; the coordinator found Chorda's Floor, Walls and Rug fallen out of their card
//! and over the keyboard): Chorda's Play view and Tympanum's Kit view draw the one shared
//! card, and in each — English and Spanish, 100 % and 150 %, a 1920 × 1080 screen and a
//! 1280 × 720 one — the card stands inside the picture, its three fields inside the card,
//! none over another. (Both cards are popovers since the round's polish: opened from
//! their chips for the sweep.)
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use cantus_core::InstrumentKind;
use cantus_ui::i18n::Locale;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::inst::room_card::probe;
use cantus_ui::views::inst::View;
use egui::{vec2, Rect};
use sweep::{Look, Rig};

fn open(r: &mut Rig, kind: InstrumentKind) {
    r.reset();
    r.app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(kind),
    );
    let track = r.app.project().tracks.last().map(|t| t.id).unwrap();
    cantus_ui::views::inst::demo_setup(&mut r.app, track, kind);
    r.app.open_inst_editor(track);
    r.app.inst.view = View::Play;
    if kind == InstrumentKind::AcousticDrums {
        // Round 40 polish: Tympanum's card opens from its room chip.
        let mut lk = cantus_ui::views::inst::tympanum::look(&r.ctx, track);
        lk.room_open = true;
        cantus_ui::views::inst::tympanum::keep(&r.ctx, track, lk);
    }
    if kind == InstrumentKind::Chorda {
        // Chorda's too (the same polish).
        cantus_ui::views::inst::chorda::view::set_room_open(&mut r.app, track, true);
    }
    r.idle(4);
}

fn inside(outer: Rect, inner: Rect) -> bool {
    outer.expand(0.5).contains_rect(inner)
}

#[test]
fn the_room_card_and_its_fields_stay_inside_the_picture() {
    let mut bad = Vec::new();
    let mut seen = 0;
    for locale in [Locale::En, Locale::Es] {
        for (scale, small) in [(1.0, false), (1.0, true), (1.5, false)] {
            let mut r = Rig::new(Look {
                locale,
                dark: true,
                scale,
            });
            if small {
                r.size = vec2(1280.0, 720.0);
            }
            for kind in [InstrumentKind::Chorda, InstrumentKind::AcousticDrums] {
                open(&mut r, kind);
                probe::start();
                r.frame(None);
                let cards = probe::take();
                let at = format!("{kind:?} {locale:?} {scale}x {}", r.size);
                if cards.is_empty() {
                    bad.push(format!("{at}: no room card drawn"));
                }
                for c in cards {
                    seen += 1;
                    if !inside(c.picture, c.card) {
                        bad.push(format!(
                            "{at}: card {:?} out of picture {:?}",
                            c.card, c.picture
                        ));
                    }
                    for (i, f) in c.fields.iter().enumerate() {
                        if !inside(c.card, *f) {
                            bad.push(format!("{at}: field {i} {f:?} out of card {:?}", c.card));
                        }
                        for g in &c.fields[i + 1..] {
                            if f.intersects(g.shrink(0.5)) {
                                bad.push(format!("{at}: fields {f:?} and {g:?} overlap"));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(seen >= 12, "{seen} cards seen");
}
