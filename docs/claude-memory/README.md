# Claude's working memory for Cantus

These files are Claude Code's persistent memory for this project, as it stood on the
owner's first development machine: how the owner wants the work done, the direction of the
product, and each round's state. Claude Code keeps them outside the repository (in the
user's profile); they are copied here, at the owner's request (2026-10-07), so they travel
with a clone.

`MEMORY.md` is the index; each other file holds one fact or one round's notes.
`docs/STATUS.md` remains the source of truth for the code's state — a note here that
disagrees with it is older.

## Restoring them on a new machine

Claude Code reads a project's memory from `%USERPROFILE%\.claude\projects\<slug>\memory\`,
where `<slug>` is the clone's absolute path with `:`, `\` and `/` turned into `-` — for a
clone at `C:\projects\daw` it is `c--projects-daw`. Open Claude Code once in the clone so
the folder exists, then copy every `.md` here except this README into it:

```text
xcopy /y docs\claude-memory\*.md "%USERPROFILE%\.claude\projects\c--projects-daw\memory\"
del "%USERPROFILE%\.claude\projects\c--projects-daw\memory\README.md"
```

Without them Claude still works from `CLAUDE.md`, which carries the conventions and the
owner's standing direction.

## Keeping them current

When a round ends, copy the memory folder back here and commit it with the round.
