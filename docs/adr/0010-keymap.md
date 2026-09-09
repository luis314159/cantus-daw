# ADR 0010 — Every shortcut in one editable file

**Status:** Accepted · **Date:** 2026-09-08

## Context

Shortcuts were spelled out inline in `CantusApp::global_shortcuts`: a long chain of
`if cmd && z`, with the piano-roll tool letters hard-coded a second time in
`views::piano_roll::Tool`. Three problems followed.

- **Muscle memory is not portable.** Musicians arrive from FL Studio, Live, Bitwig or
  Logic with a decade of habits. A DAW that cannot be re-keyed is a DAW they will not use
  for the second hour.
- **Duplication drifts.** The toolbar tooltip said "Draw (P / 1)" because someone typed
  that string, not because `P` was bound to anything. Two sources of truth, one of which
  was decorative.
- **Nothing could enumerate the shortcuts.** No settings screen, no conflict check, no
  printable list, and no way for a future script or MCP agent to ask what a key does.

## Decision

An **`Action`** is a thing the application can do. A **`Binding`** is a key plus exactly
the modifiers that must be held with it. A **`Keymap`** joins them and lives in
`keymap.toml`, next to `theme.json` and `layouts.json`.

```toml
preset = "cantus"

[bindings]
# Transport
play-pause = ["Space"]
record = ["R"]
# Edit
undo = ["Ctrl+Z"]
redo = ["Ctrl+Y", "Ctrl+Shift+Z"]
tool-draw = ["P", "Num1"]
```

- **TOML, not JSON** (the exception to ADR 0006's choice): this is a file people edit by
  hand, and it wants comments. The project format stays JSON because it is written by the
  application and diffed by tools.
- **`Action` ids are permanent.** Renaming one silently unbinds it in everyone's file.
  Adding one is free — it simply has no binding until a preset or the user gives it one.
- **`CantusApp::run_action` is the single dispatch.** A shortcut, a menu item, and any
  future script or MCP call all go through the same `match`, so a binding can never reach
  something a menu cannot (the keyboard equivalent of ADR 0004's command bus).
- **Modifiers match exactly.** `Ctrl+Z` must not fire when `Ctrl+Shift+Z` is pressed,
  because that is Redo. Every comparison is on the whole modifier triple, never "contains".
- **Presets are built conflict-free.** Each preset is a shared spine plus its own bindings,
  and a later binding takes the shortcut from whoever held it. A test asserts that no
  shipped preset conflicts with itself, and that none leaves the shortcuts a DAW cannot be
  used without unbound.
- **A damaged file never stops the application.** An unknown action id or an unreadable
  shortcut is logged and skipped; a file that is not TOML at all falls back to the default
  map. Losing a shortcut must not cost the user their session.

## The presets for other DAWs are approximations

`Preset::FlStudio`, `Preset::Live` and `Preset::Bitwig` follow each program's conventions
**for the actions Cantus actually has**. They are a starting point to edit, not a claim of
parity, and the dialog says so. Only `Preset::Cantus` is authoritative.

Where a convention collides with something Cantus needs, the DAW's convention wins for the
action it is famous for and ours moves aside — Live binds `Ctrl+E` to split, so under the
Live preset WAV export is on `Ctrl+Shift+E`.

## Two things that are not conflicts

- **The typing piano owns the bare letter keys** while it is on, so bindings with no
  modifier are suppressed. That is a mode, not a conflict, and it is why every tool action
  also carries a number binding. Modified letters (`Ctrl+S`) are never affected.
- **Mouse and wheel modifiers** — Alt to ignore the grid, Shift to lock pitch, Ctrl+wheel
  to zoom — are not bindings and are not in the file. They are listed at the bottom of the
  shortcuts dialog so the information is not lost.

## Consequences

- `views` must not read keys. A view that wants a shortcut adds an `Action` and lets the
  keymap bind it; a view that wants to *display* one asks `Keymap::display_for`, which is
  why the piano-roll tooltips now show whatever is bound rather than a typed-in letter.
- Adding an action means: the enum, `id()`, `category()`, a `match` arm in `run_action`,
  an `action.<id>` string in **both** locales, and a binding in the presets that should
  have one.
- `Binding` stores `command`, which is Ctrl on Windows and Linux and Cmd on macOS. The
  file always says `Ctrl` so a keymap is portable between machines; only the display
  changes.
- A settings dialog (the next step) hosts the editor rather than defining it: the editor
  is already a self-contained window driven by `Keymap` alone.
