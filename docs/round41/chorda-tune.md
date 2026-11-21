# Round 41 — Chorda, every string of every key in tune (`DSP_VERSION` 11, for the owner's ear)

Branch `r41/chorda-tune` (from master 5b38a884). ADR 0098's fifth amendment.

The owner, 2026-11-20, on beta.15: *"Chorda: some notes sound like some of their strings are
slightly out of tune, and I can tell quite clearly. E4, C#3, D3, C3, B2, E5. Alone, they sound
like one of the strings inside the key is out of tune. I haven't checked the other keys."* He
decided: no comparison against a recorded piano; measure each string of each note and adjust
its tuning; review all 88 keys.

__STATUS__

## Why round 41's first measurement passed what the owner heard

`docs/round41/chorda-strings.md` measured every loop alone and found everything "within
tolerance". It was right about what it measured and wrong about what to measure against:

- each loop was held to its **designed** partials, and the design scattered each unison's
  strings on purpose — a fan of about ±1–2 cents (`detune_scale`, `uni_skew`), and a
  stiffness scatter (`uni_b`, twice as wide in the bass) that moves the upper partials of
  each string apart. That scatter is enormous: on C#3 the three strings' third partials stand
  at −14.7, +2.5 and +10.0 cents from the curve (−14.7 against *designed* −14.71: "in
  tolerance"), so they beat 5.7 Hz. A real unison's strings are one wire; their inharmonicity
  differs by a percent or so, which moves a third partial by hundredths of a cent;
- the beat between the strings sounding together was never measured — only each string
  alone against its own design.

__BODY__
