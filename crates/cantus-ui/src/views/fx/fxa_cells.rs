//! What wave A's editors (Loquax, Fractus, Caelum) share: a knob with a short label of its
//! own (the card says whose it is), a segmented switch under its caption, and a card of
//! them sized to its words — every label whole, never elided (the owner's rule).

use super::controls::{self, Size};
use super::host::FxHost;
use crate::kit::text::Src;
use crate::kit::Style;
use egui::Rect;

/// One control of a card.
#[derive(Copy, Clone, Debug)]
pub enum Item {
    /// A knob on `key`, labelled by the locale key `label`.
    Knob(&'static str, &'static str),
    /// A stepped knob on `key` labelled `label` whose value reads as its choice's words:
    /// `names[i]` for the value `i` (a note value, a scale, a window shape).
    Choice(&'static str, &'static str, &'static [&'static str]),
    /// A knob whose value is written by its own function (a unit too long for a cell).
    Pill(&'static str, &'static str, fn(f32, &mut String)),
    /// A segmented switch on `key` under the caption `label`, one locale key per choice.
    Switch(&'static str, &'static str, &'static [&'static str]),
}

/// A switch's column at 100 %, at least.
const COLUMN: f32 = 112.0;

/// The width a word takes as a label, 100 %.
fn word(ui: &egui::Ui, key: &str) -> f32 {
    super::arca::words::cell_width(ui, key)
}

/// An item's width at 100 %.
pub fn width(ui: &egui::Ui, item: &Item) -> f32 {
    match *item {
        Item::Knob(_, label) | Item::Pill(_, label, _) => {
            word(ui, label).max(controls::cells_width(&[Size::M]))
        }
        Item::Choice(_, label, names) => {
            // The value box shows a choice's words: as wide as the widest.
            let widest = names.iter().map(|c| word(ui, c)).fold(0.0, f32::max);
            word(ui, label)
                .max(widest + 8.0)
                .max(controls::cells_width(&[Size::M]))
        }
        Item::Switch(_, label, choices) => {
            let widest = choices.iter().map(|c| word(ui, c)).fold(0.0, f32::max);
            word(ui, label)
                .max(widest * choices.len() as f32)
                .max(COLUMN)
        }
    }
}

/// A card's width at 100 %: its items and the card's own margins.
pub fn card_width(ui: &egui::Ui, items: &[Item]) -> f32 {
    let w: f32 = items.iter().map(|i| width(ui, i)).sum();
    w + 4.0 * items.len().saturating_sub(1) as f32 + 16.0
}

/// A knob on `key` with the label `label`, as wide as its word.
pub fn knob(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &str,
    pill: Option<controls::Pill<'_>>,
    width: Option<f32>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let w = width.unwrap_or_else(|| word(ui, label).max(controls::cells_width(&[Size::M])));
    let mut cell = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .label(Src::Key(label))
        .width(w);
    // A stepped value with no words of its own is a whole number ("4", not "4.00").
    let whole = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{v:.0}");
    };
    let pill: Option<controls::Pill<'_>> = pill.or(if info.stepped { Some(&whole) } else { None });
    if let Some(f) = pill {
        cell = cell.format(f);
    }
    let out = cell.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// A segmented switch on `key` under its caption.
pub fn switch(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, item: &Item) {
    let Item::Switch(key, label, choices) = *item else {
        return;
    };
    let w = width(ui, item);
    controls::column(ui, s, w, |ui| {
        controls::caption(ui, s, Src::Key(label), w);
        let labels: Vec<Src<'_>> = choices.iter().map(|c| Src::Key(c)).collect();
        controls::segmented_param(ui, s, host, key, &labels, w);
    });
}

/// A card titled `title` holding `items`, left to right.
pub fn card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    title: &'static str,
    items: &[Item],
) {
    controls::card(ui, s, rect, Src::Key(title), None, |ui, _| {
        for it in items {
            match *it {
                Item::Knob(key, label) => knob(host, ui, s, key, label, None, None),
                Item::Choice(key, label, names) => {
                    let pill = move |v: f32, b: &mut String| {
                        let i = (v.round().max(0.0) as usize).min(names.len() - 1);
                        b.push_str(super::frame::tr(names[i]));
                    };
                    knob(host, ui, s, key, label, Some(&pill), Some(width(ui, it)));
                }
                Item::Pill(key, label, f) => knob(host, ui, s, key, label, Some(&f), None),
                Item::Switch(..) => switch(host, ui, s, it),
            }
        }
    });
}

/// A row of cards across `row` (a rect the cards share by their widths).
pub fn row(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    g: &mut crate::kit::shell::Grid,
    height: f32,
    cards: &[(&'static str, &[Item])],
) {
    let w: Vec<f32> = cards
        .iter()
        .map(|(_, items)| card_width(ui, items))
        .collect();
    let sum: f32 = w.iter().sum::<f32>().max(1.0);
    let shares: Vec<f32> = w.iter().map(|x| x / sum).collect();
    let rects: Vec<Rect> = g.row(height, &shares).collect();
    for ((title, items), rect) in cards.iter().zip(rects) {
        card(host, ui, s, rect, title, items);
    }
}

/// A MIDI note by its name: "C3" (60 is C4), sharps written with "#".
pub fn note_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let n = v.round() as i32;
    let _ = write!(
        b,
        "{}{}",
        NAMES[n.rem_euclid(12) as usize],
        n.div_euclid(12) - 1
    );
}

/// Semitones, signed: "+12 st".
pub fn semis_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:+.0} st");
}

/// A card: its title and its items.
pub type Card<'a> = (&'static str, &'a [Item]);

/// The editor's rows of cards as they fit `avail` points across at scale `k`: each row's
/// cards in order, a card that does not fit beside the ones before starting a new row — so
/// no card is ever squeezed narrower than its words (the Spanish ones are longer).
pub fn wrap<'a>(ui: &egui::Ui, avail: f32, k: f32, rows: &[&[Card<'a>]]) -> Vec<Vec<Card<'a>>> {
    let gap = 6.0 * k;
    let mut out: Vec<Vec<Card<'a>>> = Vec::new();
    for row in rows {
        let mut cur: Vec<Card<'a>> = Vec::new();
        let mut used = 0.0;
        for card in row.iter() {
            let w = card_width(ui, card.1) * k;
            if !cur.is_empty() && used + gap + w > avail {
                out.push(std::mem::take(&mut cur));
                used = 0.0;
            }
            used += if cur.is_empty() { w } else { gap + w };
            cur.push(*card);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// The cards in their order laid in as few rows as fit `avail` points across at scale `k`,
/// the breaks chosen so the rows are as evenly filled as they can be — no card left alone
/// on a row of its own while another row is crowded (the owner's v1 verdict: Followers
/// orphaned in Spanish). Never squeezes a card under its words: a card wider than the
/// window has a row of its own.
pub fn pack<'a>(ui: &egui::Ui, avail: f32, k: f32, cards: &[Card<'a>]) -> Vec<Vec<Card<'a>>> {
    let w: Vec<f32> = cards.iter().map(|c| card_width(ui, c.1) * k).collect();
    breaks(&w, 6.0 * k, avail)
        .into_iter()
        .map(|(i, j)| cards[i..j].to_vec())
        .collect()
}

/// The rows (`i..j` of `w`) as [`pack`] lays them: the fewest that fit `avail` with `gap`
/// between cards, the breaks where each row's unfilled share, squared and summed, is least
/// (an even spread costs least, a lone card on a row most).
pub fn breaks(w: &[f32], gap: f32, avail: f32) -> Vec<(usize, usize)> {
    let n = w.len();
    if n == 0 {
        return Vec::new();
    }
    // The width of cards i..j on one row.
    let span = |i: usize, j: usize| w[i..j].iter().sum::<f32>() + gap * (j - i - 1) as f32;
    let fits = |i: usize, j: usize| j == i + 1 || span(i, j) <= avail;
    // best[i][r]: the least cost for cards i.. in r rows, and where its first row ends.
    let mut best: Vec<Vec<Option<(f32, usize)>>> = vec![vec![None; n + 1]; n + 1];
    best[n][0] = Some((0.0, n));
    for i in (0..n).rev() {
        for r in 1..=n {
            let mut pick: Option<(f32, usize)> = None;
            for (j, after) in best.iter().enumerate().skip(i + 1) {
                if !fits(i, j) {
                    break;
                }
                if let Some((rest, _)) = after[r - 1] {
                    let slack = (1.0 - span(i, j) / avail).max(0.0);
                    let cost = slack * slack + rest;
                    if pick.is_none_or(|(c, _)| cost < c) {
                        pick = Some((cost, j));
                    }
                }
            }
            best[i][r] = pick;
        }
    }
    let rows = (1..=n).find(|&r| best[0][r].is_some()).unwrap_or(n);
    let mut out = Vec::with_capacity(rows);
    let (mut i, mut r) = (0, rows);
    while i < n && r > 0 {
        let Some((_, j)) = best[i][r] else { break };
        out.push((i, j));
        i = j;
        r -= 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::breaks;

    #[test]
    fn the_fewest_rows_evenly_filled_and_no_card_left_alone() {
        // Greedy wrapping would give [300, 300, 300] [100]: a card alone on its row.
        let rows = breaks(&[300.0, 300.0, 300.0, 100.0], 0.0, 1000.0);
        assert_eq!(rows, vec![(0, 4)]);
        let rows = breaks(&[400.0, 400.0, 400.0, 100.0], 0.0, 1000.0);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|&(i, j)| j - i >= 2), "{rows:?}");
        // A card wider than the window has a row of its own, never squeezed.
        assert_eq!(breaks(&[1200.0, 100.0], 0.0, 1000.0), vec![(0, 1), (1, 2)]);
        assert!(breaks(&[], 0.0, 1000.0).is_empty());
    }
}
