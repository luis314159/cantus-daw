# Round 41 — Fides's key, wired through (`r41/key-wiring`)

*Branch `r41/key-wiring` from `r41/integration` 1fb1c570f. What ADR 0154's builder left open:
the agents, the routing views, and the nylon preset's level.*

## The agents and the MCP server

- `set_track` takes **`sidechain_strip_id`** (integer or null — the name `set_effect` uses for an
  insert's key): the strip Fides's *Input* exciter hears. Refused as `SetTrackSidechain` refuses,
  in words an agent can act on: *this track's instrument takes no input (only fides hears a
  sidechain)*, *the master cannot be an instrument's input*, *a VCA has no audio*, *strip N not
  found*, *a loop: the track plays into that strip, or into one it reaches*. When the same call
  also moves the track (`strip_id`), the command checks the pair.
- `get_track` (the track summary) carries `sidechain_strip_id` when one is set; Fides's
  catalogue line names the field; `get_unit`/`list_units`' instrument entries say `sidechain`.
- The MCP golden changed only by those words (set_track's field, Fides's line in the two
  instrument lists).

## The routing views

The key is a route of `cantus_core::routing` as an insert's is: `RouteKind::TrackKey(track)`,
`EdgeKind::TrackKey` (from the key strip to the strip the track plays into, after the inserts'
keys), `Feeders::track_keys`, `Route::track_key`; `route_check` agrees with the command (its own
strip *to itself*, a loop naming the strip, *not keyable* for an instrument that takes none,
*already there*); `plan::move_route` moves its source, `plan::remove_route` clears it. From that:

- **The desk** draws it as a key cable — dashed, the dynamics hue, the key glyph — from the key
  strip's output jack to Fides's strip, under the Keys option; hovered "key · Fides"; selected,
  dragged by its source, deleted like any route. The strip's route count and the lit strip's
  "> key" tag count it.
- **The matrix** marks the cell (key strip row, Fides's strip column) as a key; its loop test
  knows the edge (Fides's row into the key strip is hatched); the hint says *keys Fides's input*
  (*llave en la entrada de Fides*); its menu and Delete reach it.
- **The route menu** (*Key · Keys → Fides's input on Fides*, *Open the keyed instrument*), the
  strip's menu of routes, the inspector's **Receives** (*Fides, input ← key* / *Fides, entrada
  ← clave*; a click opens the instrument) and its **Sidechain** section (*Key into* the input,
  *From* a list of the strips the command would take).

Looked at (shoot examples, both languages, Night and Paper): `shoot_cables39 -- <dir> flow fides`
(a new case: Fides keyed from the Kick, its strip selected — `CANTUS_SHOOT_LANG=es` added) and
`shoot_matrix` with `CANTUS_SHOOT_KINDS=matrix-fides,insp-fides`. The first Spanish inspector
cut *entrada de Fides ← cl…*: the row now reads *Fides, entrada ← clave* (as an insert's
*EQ, ranura 2 ← clave*) and its value well may take all but 48 pt of the row.

## The nylon guitar

`examples/fides_listen` (the role's phrase through the real engine): Nylon guitar −20.0 LUFS,
+1.0 LU over Pluck's −21.0. Its gain 9.8 → **8.8 dB**: −21.0 LUFS, 0.0 LU, phrase peak −8.9 dBFS
(was −7.9); `preset_render` (debug) passes, its row −21.0 LUFS, 0.0 LU, CPU 0.029. The other 24 presets measured as before — the harp is +0.4 LU (not 0.3), left as
it was.

## Tests

- `cantus-ai` `tools_music` 11 (new: `an_agent_keys_fides_s_input_from_a_strip_and_is_refused_a_loop`).
- `cantus-core` `routing` 25 (new: `a_keyed_instrument_is_an_edge_a_feeder_and_a_route_the_commands_agree_with`),
  `undo_invariants` 17, `fides_sidechain` 5.
- `cantus-ui` `fides_input_e2e` 3 (new: `an_agent_keys_fides_s_input_through_the_server`,
  `fides_s_key_is_a_cable_on_the_desk_and_a_row_of_the_inspector` — both through
  `CantusApp::assemble`; the three take turns over `CANTUS_DATA_DIR`), `ai_mcp_boot` 1 (the
  golden), `agent_tools` 14, `mixer_cables_view` 10, `cables39_e2e` 6, `mixer_routing` 33,
  `route_entry_points` 8, `routing_matrix` 13, `inspector` 7, the lib's `views::routing` (the
  matrix's random graphs now with keyed Fides tracks) and `i18n`.

Verified only in emulation: everything here is the model and the views; no device is involved.
