//! macOS's global menu bar, made from plain data (round 30, ADR 0066). The interface
//! (`cantus_ui::macos::menu`) decides what the menus hold — titles, items, their key
//! equivalents, in the user's language and keymap — and hands them here as [`Menu`]s;
//! this turns them into `NSMenu`s and reports which item was chosen by its tag
//! ([`take`]). Also the general pasteboard's text, for a paste a menu hands to a text
//! field. AppKit runs on the main thread (the UI thread), where all of this is called.
//!
//! Off macOS the types exist (the interface builds its model everywhere, and tests it)
//! and the functions do nothing.

/// An AppKit key equivalent: its characters and the modifiers held with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub chars: String,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// An item of Cantus's: choosing it queues `tag` for [`take`].
    Item {
        label: String,
        tag: usize,
        key: Option<Key>,
    },
    /// An item AppKit answers through the responder chain (`hide:`, `performZoom:`…).
    System {
        label: String,
        selector: &'static str,
        key: Option<Key>,
    },
    Separator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub title: String,
    pub entries: Vec<Entry>,
    /// AppKit lists the open windows in it (the Window menu).
    pub windows: bool,
    /// AppKit adds its search field to it (the Help menu).
    pub help: bool,
}

/// Make `menus` the application's menu bar; `wake` is called when an item is chosen (the
/// interface asks for a frame). Replaces any bar made before.
pub fn install(menus: &[Menu], wake: Box<dyn Fn()>) {
    #[cfg(target_os = "macos")]
    imp::install(menus, wake);
    #[cfg(not(target_os = "macos"))]
    let _ = (menus, wake);
}

/// The tags of the items chosen since the last call.
pub fn take() -> Vec<usize> {
    #[cfg(target_os = "macos")]
    {
        imp::take()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// The general pasteboard's text, if it holds any (`None` off macOS).
pub fn pasteboard_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        imp::pasteboard_text()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod imp {
    use super::{Entry, Key, Menu};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem, NSPasteboard};
    use objc2_foundation::{NSObject, NSString};
    use std::cell::RefCell;

    thread_local! {
        /// Tags chosen since the last [`take`].
        static PICKS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
        /// What to call when an item is chosen.
        static WAKE: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
        /// The bar and the object its items send to, kept alive while installed.
        static LIVE: RefCell<Option<(Retained<Target>, Retained<NSMenu>)>> =
            const { RefCell::new(None) };
    }

    define_class!(
        // SAFETY: a plain NSObject subclass with no instance variables and no Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "CantusMenuTarget"]
        struct Target;

        impl Target {
            /// Every item's action: its tag says which.
            #[unsafe(method(pick:))]
            fn pick(&self, sender: &NSMenuItem) {
                if let Ok(tag) = usize::try_from(sender.tag()) {
                    PICKS.with(|p| p.borrow_mut().push(tag));
                    WAKE.with(|w| {
                        if let Some(wake) = w.borrow().as_ref() {
                            wake();
                        }
                    });
                }
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker) -> Retained<Target> {
            let this = Target::alloc(mtm).set_ivars(());
            // SAFETY: NSObject's designated initialiser.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn mask(k: &Key) -> NSEventModifierFlags {
        let mut m = NSEventModifierFlags::empty();
        for (on, flag) in [
            (k.command, NSEventModifierFlags::Command),
            (k.shift, NSEventModifierFlags::Shift),
            (k.alt, NSEventModifierFlags::Option),
            (k.control, NSEventModifierFlags::Control),
        ] {
            if on {
                m |= flag;
            }
        }
        m
    }

    fn item(
        mtm: MainThreadMarker,
        label: &str,
        action: Option<Sel>,
        key: Option<&Key>,
    ) -> Retained<NSMenuItem> {
        let chars = key.map_or("", |k| k.chars.as_str());
        // SAFETY: a new item with a valid title, selector and key, on the main thread.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str(label),
                action,
                &NSString::from_str(chars),
            )
        };
        if let Some(k) = key {
            item.setKeyEquivalentModifierMask(mask(k));
        }
        item
    }

    pub fn install(menus: &[Menu], wake: Box<dyn Fn()>) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        WAKE.with(|w| *w.borrow_mut() = Some(wake));
        let target = Target::new(mtm);
        let bar = NSMenu::initWithTitle(mtm.alloc::<NSMenu>(), &NSString::from_str(""));
        let app = NSApplication::sharedApplication(mtm);
        for m in menus {
            let menu = NSMenu::initWithTitle(mtm.alloc::<NSMenu>(), &NSString::from_str(&m.title));
            for e in &m.entries {
                match e {
                    Entry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                    Entry::Item { label, tag, key } => {
                        let it = item(mtm, label, Some(sel!(pick:)), key.as_ref());
                        it.setTag(*tag as isize);
                        let to: &AnyObject = &target;
                        // SAFETY: the target is kept alive in `LIVE` as long as the bar.
                        unsafe { it.setTarget(Some(to)) };
                        menu.addItem(&it);
                    }
                    Entry::System {
                        label,
                        selector,
                        key,
                    } => {
                        let Ok(name) = std::ffi::CString::new(*selector) else {
                            continue;
                        };
                        // No target: AppKit sends it along the responder chain (the key
                        // window, the application), which answers these standard actions.
                        let it = item(mtm, label, Some(Sel::register(&name)), key.as_ref());
                        menu.addItem(&it);
                    }
                }
            }
            let top = NSMenuItem::new(mtm);
            top.setSubmenu(Some(&menu));
            bar.addItem(&top);
            if m.windows {
                app.setWindowsMenu(Some(&menu));
            }
            if m.help {
                app.setHelpMenu(Some(&menu));
            }
        }
        app.setMainMenu(Some(&bar));
        LIVE.with(|l| *l.borrow_mut() = Some((target, bar)));
    }

    pub fn take() -> Vec<usize> {
        PICKS.with(|p| std::mem::take(&mut *p.borrow_mut()))
    }

    pub fn pasteboard_text() -> Option<String> {
        let board = NSPasteboard::generalPasteboard();
        // SAFETY: the pasteboard type is a constant AppKit exports.
        let kind = unsafe { objc2_app_kit::NSPasteboardTypeString };
        board.stringForType(kind).map(|s| s.to_string())
    }
}
