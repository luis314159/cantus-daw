//! `Field` — the one text field of the app (round 26, ADR 0024 §15): a well of ours with
//! egui's single-line editing inside it, and one set of rules everywhere a word or a
//! number is typed — a rename, a search, a name in a dialog, a typed value.
//!
//! - **The look**: the kit's well, its text inset inside it (the inset is the frame's own
//!   inner margin: `TextEdit::margin` is ignored under a custom frame), the hint in the
//!   placeholder colour. The lit well *is* the focus indication — no second ring
//!   (`design::paint_focus_ring` leaves text edits alone).
//! - **Opening** (an editor appearing, or a click into a field): it takes the keyboard
//!   with its words selected, so what is typed replaces them.
//! - **Enter** commits; **Esc** cancels — the words go back to what they were when the
//!   field opened — and closes; **a click elsewhere** commits and closes. Either way the
//!   field lets the keyboard go.
//! - **While it holds the keyboard nothing else reads it**: [`active`] is true from the
//!   pass it takes the keyboard to the pass after it lets go (the Esc that closes it
//!   included), and the app's shortcuts and the typing piano wait on it.
//!
//! - **An untouched session changes nothing**: [`Out::edited`] says whether the words at
//!   the commit differ (spaces round them aside) from those it opened with; a caller sets
//!   nothing when they do not — no rounding of a prefilled value, no empty undo step.
//! - **Every field has an id of its own**, given by its caller and salted by what it edits
//!   (a row's channel, a tag's key): two fields shown with one id in a pass would share one
//!   edit, so typing into one would write both — refused in debug builds.
//!
//! An editor that exists only while editing (a rename) shows with [`Field::open`] on its
//! first pass and closes on [`Event::Commit`] or [`Event::Cancel`]; a field that stays (a
//! search) simply shows every frame and reads its text.

use super::text::Role;
use super::{well, with_brush, State, Style};
use egui::text::{CCursor, CCursorRange};
use egui::{Align, Context, Id, Rect, Response, Ui};

/// How a typing session ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Enter, or a click elsewhere: keep the words.
    Commit,
    /// Esc: the words are back to what they were when the field opened.
    Cancel,
}

/// What a field did this frame.
#[derive(Debug)]
pub struct Out {
    pub resp: Response,
    /// The session ended this frame.
    pub event: Option<Event>,
    /// The words changed this frame (typing, or a cancel putting them back).
    pub changed: bool,
    /// It holds the keyboard.
    pub focused: bool,
    /// On a commit: the words differ from those the session opened with (spaces round
    /// them aside). An untouched commit sets nothing.
    pub edited: bool,
    /// On a commit: it was Enter (not a click elsewhere).
    pub enter: bool,
}

/// A text field to show (see the module docs).
#[derive(Copy, Clone, Debug)]
pub struct Field<'a> {
    id: Id,
    hint: Option<&'a str>,
    role: Role,
    align: Align,
    pad: (f32, f32),
    open: bool,
    /// Open on the first pass it is shown after a pass it was not (an editor appearing).
    auto: bool,
    /// The scale of the window it sits in (a device editor's own; 1 elsewhere).
    k: f32,
}

/// What a session remembers: the words it opened with.
#[derive(Clone, Debug, Default)]
struct Session {
    original: String,
}

fn session_id(id: Id) -> Id {
    id.with("kit-field-session")
}

fn active_id() -> Id {
    Id::new("kit-field-active")
}

/// The ids of the fields shown this pass (with the pass): a second field with one of them
/// is a caller's mistake that would join two fields into one edit.
#[derive(Clone, Default)]
struct Shown(u64, Vec<Id>);

fn note_shown(ctx: &Context, id: Id) -> bool {
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Shown>(Id::new("kit-field-shown-ids"));
        if s.0 != now {
            s.0 = now;
            s.1.clear();
        }
        let clash = s.1.contains(&id);
        if !clash {
            s.1.push(id);
        }
        clash
    })
}

/// The id of the text edit inside the field `id` (tests and tools focus it).
pub fn edit_id(id: Id) -> Id {
    id.with("kit-field-edit")
}

/// Whether a field holds the keyboard — this pass or the last (so the key that closed it,
/// Esc or Enter, is not read again by the app's shortcuts).
pub fn active(ctx: &Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<u64>(active_id()))
        .is_some_and(|p| p + 1 >= now)
}

fn mark_active(ctx: &Context) {
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(active_id(), now));
}

impl<'a> Field<'a> {
    pub fn new(id: Id) -> Self {
        Field {
            id,
            hint: None,
            role: Role::Body,
            align: Align::Min,
            pad: (0.0, 0.0),
            open: false,
            auto: false,
            k: 1.0,
        }
    }

    /// The scale of the window it sits in (a device editor's `Style::k`): its words and
    /// inset grow with it.
    pub fn scale(mut self, k: f32) -> Self {
        self.k = k;
        self
    }

    /// The words shown while it is empty.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// The type role of its words (`Body` by default; `Value` for numbers).
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Centre the words (a typed value).
    pub fn centered(mut self) -> Self {
        self.align = Align::Center;
        self
    }

    /// Room beside the words, in points at 100 %, inside the standard inset (a search
    /// field's magnifier at the left, its clear key at the right).
    pub fn pad(mut self, left: f32, right: f32) -> Self {
        self.pad = (left, right);
        self
    }

    /// Take the keyboard now, the words selected (an editor's first pass).
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Take the keyboard, the words selected, whenever it appears: the first pass it is
    /// shown after one it was not (a rename, a name dialog, a typed value).
    pub fn opens_when_shown(mut self) -> Self {
        self.auto = true;
        self
    }

    /// Show the field filling `rect`, editing `text`.
    pub fn show(mut self, ui: &mut Ui, rect: Rect, text: &mut String) -> Out {
        let s = Style::current().scaled(self.k);
        let ctx = ui.ctx().clone();
        let now = ctx.cumulative_pass_nr();
        let clash = note_shown(&ctx, self.id);
        debug_assert!(
            !clash,
            "two kit fields share the id {:?} in one pass: give each its own, salted by \
             what it edits",
            self.id
        );
        let shown_id = self.id.with("kit-field-shown");
        let appears = ctx
            .data(|d| d.get_temp::<u64>(shown_id))
            .is_none_or(|p| p + 1 < now);
        ctx.data_mut(|d| d.insert_temp(shown_id, now));
        if self.auto && appears {
            self.open = true;
            // A new appearance: whatever an earlier one left is gone.
            ctx.data_mut(|d| d.remove_temp::<Session>(session_id(self.id)));
        }
        let eid = edit_id(self.id);
        let sid = session_id(self.id);
        let had = ctx.memory(|m| m.has_focus(eid));
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let hovered = ui.rect_contains_pointer(rect);
        with_brush(ui.painter(), |b| {
            let st = State {
                hovered,
                ..State::REST
            }
            .lit(had || self.open);
            well::paint(b, rect, &s, st, None);
        });
        let k = s.k;
        let font = self.role.font(s.tk());
        let row = ctx.fonts_mut(|f| f.row_height(&font));
        // The inset is the frame's own margin (under a custom frame `TextEdit::margin` is
        // ignored), and the margins centre the line: the edit is the whole well.
        let m = |v: f32| v.round().clamp(0.0, 127.0) as i8;
        let v = ((rect.height() - row) * 0.5).max(0.0);
        let margin = egui::Margin {
            left: m((6.0 + self.pad.0) * k),
            right: m((6.0 + self.pad.1) * k),
            top: m(v.floor()),
            bottom: m(v.ceil()),
        };
        let before = text.clone();
        let mut edit = egui::TextEdit::singleline(text)
            .id(eid)
            .frame(egui::Frame::NONE.inner_margin(margin))
            .font(font)
            .text_color(s.t.text)
            .horizontal_align(self.align)
            .vertical_align(Align::Center)
            .desired_width(rect.width());
        if let Some(h) = self.hint {
            edit = edit.hint_text(egui::RichText::new(h).color(s.t.placeholder));
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(self.id));
        let mut out = edit.show(&mut child);
        let resp = out.response.response.clone();
        let opening = self.open && !had && !ctx.data(|d| d.get_temp::<Session>(sid).is_some());
        let gained = resp.gained_focus() || opening;
        if opening {
            resp.request_focus();
        }
        if gained {
            // A new session: remember the words, select them all.
            let n = text.chars().count();
            out.state
                .cursor
                .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
            out.state.store(&ctx, eid);
            ctx.data_mut(|d| {
                d.insert_temp(
                    sid,
                    Session {
                        original: before.clone(),
                    },
                )
            });
        }
        let mut event = None;
        let mut edited = false;
        if resp.lost_focus() || (had && !resp.has_focus() && !opening) {
            let original = ctx
                .data_mut(|d| d.remove_temp::<Session>(sid))
                .map(|s| s.original);
            if escape {
                if let Some(o) = original {
                    *text = o;
                }
                event = Some(Event::Cancel);
            } else {
                edited = original.is_none_or(|o| o.trim() != text.trim());
                event = Some(Event::Commit);
            }
        }
        let focused = resp.has_focus() || (opening && event.is_none());
        if focused || had || event.is_some() {
            mark_active(&ctx);
        }
        Out {
            changed: *text != before,
            resp,
            event,
            focused,
            edited,
            enter: enter && event == Some(Event::Commit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Event as E, Key, Modifiers, Pos2};

    struct Rig {
        ctx: Context,
        frame: u64,
        text: String,
        out: Vec<Option<Event>>,
        open: bool,
        /// Where the last frame painted words.
        words: Vec<Rect>,
    }

    const FIELD: Rect = Rect {
        min: Pos2 { x: 20.0, y: 20.0 },
        max: Pos2 { x: 220.0, y: 44.0 },
    };

    impl Rig {
        fn new(text: &str) -> Rig {
            let ctx = Context::default();
            crate::design::install_fonts(&ctx);
            crate::design::install_style(&ctx);
            Rig {
                ctx,
                frame: 0,
                text: text.into(),
                out: Vec::new(),
                open: false,
                words: Vec::new(),
            }
        }

        fn frame(&mut self, events: Vec<E>) -> Out {
            self.frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                time: Some(self.frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            let mut got = None;
            let text = &mut self.text;
            let open = self.open;
            let mut o = self.ctx.run_ui(input, |ui| {
                // Something else to click: a button-sized area away from the field.
                ui.interact(
                    Rect::from_min_size(pos2(20.0, 200.0), vec2(80.0, 30.0)),
                    Id::new("elsewhere"),
                    egui::Sense::click(),
                );
                let f = Field::new(Id::new("f")).hint("Name");
                let f = if open { f.opens_when_shown() } else { f };
                got = Some(f.show(ui, FIELD, text));
            });
            o.textures_delta.clear();
            self.words.clear();
            for c in &o.shapes {
                if let egui::Shape::Text(t) = &c.shape {
                    self.words.push(t.galley.rect.translate(t.pos.to_vec2()));
                }
            }
            let out = got.unwrap();
            self.out.push(out.event);
            out
        }

        fn key(&mut self, key: Key) -> Out {
            self.frame(vec![E::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }])
        }

        fn click(&mut self, p: Pos2) {
            self.frame(vec![E::PointerMoved(p)]);
            for pressed in [true, false] {
                self.frame(vec![E::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }]);
            }
            self.frame(Vec::new());
        }

        fn opened(text: &str) -> Rig {
            let mut r = Rig::new(text);
            r.open = true;
            r.frame(Vec::new());
            r.frame(Vec::new());
            r
        }
    }

    #[test]
    fn opening_selects_the_words_so_typing_replaces_them() {
        let mut r = Rig::opened("Keys");
        assert!(r.ctx.memory(|m| m.has_focus(edit_id(Id::new("f")))));
        assert!(active(&r.ctx), "a field holds the keyboard");
        r.frame(vec![E::Text("Bass".into())]);
        assert_eq!(r.text, "Bass");
    }

    #[test]
    fn enter_commits_and_lets_the_keyboard_go() {
        let mut r = Rig::opened("Keys");
        r.frame(vec![E::Text("Bass".into())]);
        r.key(Key::Enter);
        assert!(r.out.contains(&Some(Event::Commit)));
        assert_eq!(r.text, "Bass");
        r.frame(Vec::new());
        r.frame(Vec::new());
        assert!(
            !r.ctx.memory(|m| m.focused().is_some()),
            "the keyboard let go"
        );
        assert!(!active(&r.ctx));
    }

    #[test]
    fn esc_cancels_puts_the_words_back_and_closes() {
        let mut r = Rig::opened("Keys");
        r.frame(vec![E::Text("Bass".into())]);
        assert_eq!(r.text, "Bass");
        let out = r.key(Key::Escape);
        assert_eq!(out.event, Some(Event::Cancel));
        assert_eq!(r.text, "Keys", "the words as they were");
        assert!(
            active(&r.ctx),
            "the closing Esc is the field's, not the app's"
        );
        r.frame(Vec::new());
        r.frame(Vec::new());
        assert!(!r.ctx.memory(|m| m.focused().is_some()));
    }

    #[test]
    fn a_click_elsewhere_commits_closes_and_lets_the_keyboard_go() {
        let mut r = Rig::opened("Keys");
        r.frame(vec![E::Text("Bass".into())]);
        r.click(pos2(40.0, 210.0));
        assert!(r.out.contains(&Some(Event::Commit)), "{:?}", r.out);
        assert_eq!(r.text, "Bass");
        assert!(!r.ctx.memory(|m| m.has_focus(edit_id(Id::new("f")))));
        r.frame(Vec::new());
        assert!(!active(&r.ctx));
    }

    #[test]
    fn a_click_into_a_standing_field_opens_a_session() {
        let mut r = Rig::new("drums");
        r.frame(Vec::new());
        r.click(pos2(100.0, 32.0));
        r.frame(vec![E::Text("bass".into())]);
        assert_eq!(r.text, "bass", "the words were selected on the click");
        r.key(Key::Escape);
        assert_eq!(
            r.text, "drums",
            "Esc puts back what was there before the click"
        );
    }

    /// Final review: an untouched commit says so (a caller sets nothing), an edited one
    /// says so; and which commit was Enter.
    #[test]
    fn a_commit_says_whether_the_words_were_edited_and_how_it_came() {
        let mut r = Rig::opened("1.2 kHz");
        let out = r.key(Key::Enter);
        assert_eq!(out.event, Some(Event::Commit));
        assert!(!out.edited && out.enter, "untouched, by Enter");
        let mut r = Rig::opened("1.2 kHz");
        r.frame(vec![E::Text("800 Hz".into())]);
        let out = r.key(Key::Enter);
        assert!(out.edited && out.enter, "edited, by Enter");
        // Spaces round the same words: untouched.
        let mut r = Rig::opened("Keys");
        r.frame(vec![E::Text("Keys ".into())]);
        let out = r.key(Key::Enter);
        assert!(!out.edited, "the same words");
    }

    /// Final review, fields 3: two fields with one id in one pass would be one edit (typing
    /// into one writes both) — refused.
    #[test]
    #[should_panic(expected = "two kit fields share the id")]
    fn two_fields_with_one_id_are_refused() {
        let ctx = Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let (mut a, mut b) = (String::from("a"), String::from("b"));
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            Field::new(Id::new("same")).show(ui, FIELD, &mut a);
            Field::new(Id::new("same")).show(ui, FIELD.translate(vec2(0.0, 40.0)), &mut b);
        });
    }

    /// Review finding 7: the inset is the frame's own margin, so the words never touch
    /// the well's edge — typed or a hint.
    #[test]
    fn the_words_sit_inside_the_well() {
        for words in ["Keys", ""] {
            let mut r = Rig::opened(words);
            r.frame(Vec::new());
            let resp = r.ctx.read_response(edit_id(Id::new("f"))).expect("drawn");
            assert!(
                (resp.rect.left() - FIELD.left()).abs() < 0.01
                    && (resp.rect.right() - FIELD.right()).abs() < 0.01
                    && (resp.rect.height() - FIELD.height()).abs() <= 1.0,
                "the edit is the well: {:?}",
                resp.rect
            );
            let first = r
                .words
                .iter()
                .filter(|w| FIELD.contains(w.center()))
                .map(|w| w.left())
                .fold(f32::INFINITY, f32::min);
            assert!(
                first >= FIELD.left() + 5.5 && first < FIELD.left() + 9.0,
                "{words:?}: the words start at {first}, the well at {}",
                FIELD.left()
            );
        }
    }
}
