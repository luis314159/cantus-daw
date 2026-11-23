# Scale Workshop's MTS exporter test — the values the MTS tests compare with

Scale Workshop (xenharmonic-devs, MIT licence): <https://github.com/xenharmonic-devs/scale-workshop>,
`src/exporters/__tests__/mts.spec.ts` and `test-data.ts` (fetched 2026-11-22; the files sit in the
round's research folder, `research/scale-workshop/`). No code of it is copied: the test
`crates/cantus-core/tests/tuning_mts.rs` (`scale_workshop_s_bulk_dump_hashes_are_ours`) holds only
the facts its test states —

- the scale it exports: A4 = 440 Hz on MIDI key 69, the degrees `100.` (cents), `C5_5` (528 Hz,
  i.e. 6/5 above A4), `4\5` (2^(4/5)), `5/3`, `1,3591409142295225r` (e/2), `3486784401/3276800000`
  ((81/80)^5) and `2/1`; the title "Test Scale"; the preset (program) 1; the device ID `00`;
- the SHA-256 (base64) of its 384 bytes of frequency words for keys 0–127:
  `NLwkn1HRQKdNAyYdxl6RQwfz0JvFyShWaB0DHHtPVZo=`;
- the SHA-256 (base64) of the whole bulk dump `F0 7E 00 08 01 01 <name> <words> <checksum> F7`
  with its checksum convention (the XOR of every byte below `0x80`, seeded with `0xFF`, `& 0x7F`):
  `Ghut5DZnxeRXQE0TbOJl6Sh66xI2x1QouHhVhIdQXrQ=`;
- its MTS value of a frequency (`ftomts`): `69 + 12·log2(f/440)` rounded to a millionth, `0` for
  f ≤ 0 and the top word `7F 7F 7E` above 13 289.656616 Hz.

The frequency words themselves are written by Cantus's encoder (`tuning_mts::encode_word`): the
hashes match only if every one of the 128 words is Scale Workshop's, byte for byte.
