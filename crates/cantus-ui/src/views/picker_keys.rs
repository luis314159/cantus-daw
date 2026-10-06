//! The picker's keyboard (ADR 0029 §4) and what its gestures end in.
//!
//! Typing goes into the search whatever has focus; ← → ↑ ↓ move between tiles by where
//! they are on screen (so ↓ from the end of one group lands in the next group's nearest
//! column), Page Up / Page Down by a screenful, Home / End to the ends; Enter chooses;
//! Ctrl+D stars; Esc clears the search, then closes. Once the search field has been
//! clicked into, ← → edit its text until another key moves the selection.

use super::picker::PickerView;
use crate::app::CantusApp;
use egui::{Key, Modifiers, Pos2, Rect};

/// What a frame's gestures ask of the application.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Action {
    #[default]
    None,
    Close,
    /// Choose the device with this key.
    Choose(String),
}

/// Where a key moves the selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// The tile `dir` leads to from tile `from`, given every tile's rect in display order
/// (content coordinates) and the height of a screenful.
pub fn step(tiles: &[Rect], from: usize, dir: Dir, page: f32) -> usize {
    let n = tiles.len();
    if n == 0 {
        return 0;
    }
    let from = from.min(n - 1);
    let here = tiles[from].center();
    // The tile in the nearest row on one side, nearest in x within it.
    let row = |below: bool, target_y: Option<f32>| -> usize {
        let side = |r: &Rect| {
            if below {
                r.center().y > here.y + 1.0
            } else {
                r.center().y < here.y - 1.0
            }
        };
        let ys: Vec<f32> = tiles
            .iter()
            .filter(|r| side(r))
            .map(|r| r.center().y)
            .collect();
        if ys.is_empty() {
            return from;
        }
        let y = match target_y {
            // The row closest to a screenful away (or the last row on that side).
            Some(t) => ys
                .iter()
                .copied()
                .min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs()))
                .unwrap_or(here.y),
            None if below => ys.iter().copied().fold(f32::MAX, f32::min),
            None => ys.iter().copied().fold(f32::MIN, f32::max),
        };
        nearest(tiles, Pos2::new(here.x, y)).unwrap_or(from)
    };
    match dir {
        Dir::Left => from.saturating_sub(1),
        Dir::Right => (from + 1).min(n - 1),
        Dir::Down => row(true, None),
        Dir::Up => row(false, None),
        Dir::PageDown => row(true, Some(here.y + page)),
        Dir::PageUp => row(false, Some(here.y - page)),
        Dir::Home => 0,
        Dir::End => n - 1,
    }
}

/// The tile in the row at `p.y` whose centre is nearest `p.x`.
fn nearest(tiles: &[Rect], p: Pos2) -> Option<usize> {
    tiles
        .iter()
        .enumerate()
        .filter(|(_, r)| (r.center().y - p.y).abs() < 1.0)
        .min_by(|(_, a), (_, b)| {
            (a.center().x - p.x)
                .abs()
                .total_cmp(&(b.center().x - p.x).abs())
        })
        .map(|(i, _)| i)
}

/// Read this frame's keys. `search_focused`: the search field holds focus (it reads
/// typing itself then); otherwise typed text is appended here.
pub fn read(v: &mut PickerView, ui: &egui::Ui, search_focused: bool) -> Action {
    let mut action = Action::None;
    let mut moves: Vec<Dir> = Vec::new();
    let (enter, esc, star) = ui.input_mut(|i| {
        let mut take = |k: Key| i.consume_key(Modifiers::NONE, k);
        for (k, d) in [
            (Key::ArrowUp, Dir::Up),
            (Key::ArrowDown, Dir::Down),
            (Key::PageUp, Dir::PageUp),
            (Key::PageDown, Dir::PageDown),
        ] {
            if take(k) {
                moves.push(d);
            }
        }
        if !v.editing {
            for (k, d) in [
                (Key::ArrowLeft, Dir::Left),
                (Key::ArrowRight, Dir::Right),
                (Key::Home, Dir::Home),
                (Key::End, Dir::End),
            ] {
                if take(k) {
                    moves.push(d);
                }
            }
        }
        let enter = take(Key::Enter);
        let esc = take(Key::Escape);
        let star = i.consume_key(Modifiers::COMMAND, Key::D);
        (enter, esc, star)
    });
    if !search_focused {
        // Type to search: the field need not have focus.
        let (typed, back) = ui.input_mut(|i| {
            let mut s = String::new();
            let mut back = 0;
            i.events.retain(|e| match e {
                egui::Event::Text(t) => {
                    s.push_str(t);
                    false
                }
                egui::Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    back += 1;
                    false
                }
                _ => true,
            });
            (s, back)
        });
        if !typed.is_empty() || back > 0 {
            let mut text = v.query.text.clone();
            for _ in 0..back {
                text.pop();
            }
            text.push_str(&typed);
            v.set_text(&text);
        }
    }
    if !moves.is_empty() {
        v.editing = false;
        let rects: Vec<Rect> = v.geo.tiles.iter().map(|t| t.1).collect();
        let keys: Vec<String> = v.geo.tiles.iter().map(|t| t.0.clone()).collect();
        let mut at = v
            .selected
            .as_deref()
            .and_then(|k| keys.iter().position(|x| x == k));
        for d in moves {
            at = Some(match at {
                Some(i) => step(&rects, i, d, v.geo.page),
                None => 0,
            });
        }
        if let Some(k) = at.and_then(|i| keys.get(i)) {
            v.selected = Some(k.clone());
            v.reveal = true;
        }
    }
    if star {
        if let Some(k) = v.selected.clone() {
            v.toggle_favourite(&k);
        }
    }
    if enter {
        if let Some(k) = v.selected.clone() {
            action = Action::Choose(k);
        }
    }
    if esc {
        if v.query.text.is_empty() {
            action = Action::Close;
        } else {
            v.set_text("");
            v.editing = false;
        }
    }
    action
}

/// Do what the frame asked.
pub fn act(app: &mut CantusApp, action: Action) {
    match action {
        Action::None => {}
        Action::Close => super::picker::close(app),
        Action::Choose(key) => super::picker::choose(app, &key),
    }
}
