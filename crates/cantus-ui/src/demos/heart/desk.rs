//! The desk: inserts with the settings a mix engineer would choose, the drum and music
//! buses, the reverb and delay returns, the sends, the master chain and the levels (each
//! part's trim set by measuring it soloed in the first chorus: the `levels` report).

use super::{Band, Desk};
use crate::demos::song::{colors, Song};
use crate::i18n::t;
use cantus_core::{EffectKind as K, StripId};

/// An EQ's band 0 as a high-pass at `hz` (`slope` dB/octave).
fn high_pass(hz: f32, slope: f32) -> [(&'static str, f32); 4] {
    [
        ("b0.used", 1.0),
        ("b0.shape", 3.0),
        ("b0.freq", hz),
        ("b0.slope", slope),
    ]
}

fn hp(s: &mut Song, strip: StripId, hz: f32) {
    s.insert(strip, K::Eq, None, &high_pass(hz, 18.0));
}

pub fn desk(s: &mut Song, b: &Band) -> Desk {
    let drums = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[b.kick, b.snare, b.hats, b.perc, b.fills],
    );
    let music = s.bus(
        &t("demos.strip.music_bus"),
        colors::PAD,
        &[b.keys, b.pad, b.arp, b.guitar],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    let d = Desk {
        drums,
        music,
        verb,
        delay,
    };
    drums_desk(s, b, &d);
    music_desk(s, b, &d);
    returns(s, &d);
    sends(s, b, &d);
    master(s);
    levels(s, b, &d);
    d
}

fn drums_desk(s: &mut Song, b: &Band, d: &Desk) {
    // Impetus shapes each drum at its source: the kick's click and the snare's crack.
    s.insert(b.kick.strip, K::Transient, Some("preset.impetus.kick"), &[]);
    s.insert(b.kick.strip, K::Eq, Some("eq.preset.kick"), &[]);
    s.insert(
        b.snare.strip,
        K::Transient,
        Some("preset.impetus.snap"),
        &[("sustain", 0.25), ("length", 260.0)],
    );
    hp(s, b.snare.strip, 110.0);
    hp(s, b.hats.strip, 380.0);
    s.insert(
        b.hats.strip,
        K::DeEsser,
        None,
        &[
            ("thr", -30.0),
            ("range", 4.0),
            ("lo", 7_000.0),
            ("hi", 14_000.0),
        ],
    );
    hp(s, b.perc.strip, 300.0);
    hp(s, b.fills.strip, 60.0);
    // The bus: Discrimen lifts the attacks and tucks the body, Comprimo glues, Tonsor
    // shaves the peaks the master would otherwise limit (oversampled, gain matched).
    s.insert(
        d.drums,
        K::TransientSplit,
        Some("preset.discrimen.snappy"),
        &[("b.gain", -2.0), ("t.gain", 3.0)],
    );
    s.insert(
        d.drums,
        K::Compressor,
        Some("comp.preset.glue"),
        &[("thr", -18.0)],
    );
    s.insert(
        d.drums,
        K::Clipper,
        Some("preset.tonsor.drum_bus"),
        &[("drive", 4.0)],
    );
}

fn music_desk(s: &mut Song, b: &Band, d: &Desk) {
    // The bass: mono below its fundamental's octave, and a follower from the kick ducks
    // its fader (the modulators, `moves.rs`).
    s.insert(
        b.bass.strip,
        K::Eq,
        Some("eq.preset.mono_bass"),
        &[("mode", 0.0), ("b0.freq", 28.0)],
    );
    // The piano: a gentle low cut, and Vibrans's tape wow, at rest (depth 0) until the
    // intro's and the outro's automation bring it in.
    hp(s, b.piano.strip, 45.0);
    s.insert(
        b.piano.strip,
        K::Vibrato,
        Some("preset.vibrans.tape_wow"),
        &[("depth", 0.0)],
    );
    // Keys: Juno chorus, Tremor's wide auto-pan, Fluctus pumping in the choruses.
    s.insert(b.keys.strip, K::Chorus, Some("preset.concentus.juno_two"), &[]);
    s.insert(
        b.keys.strip,
        K::Tremolo,
        Some("preset.tremor.wide_pan"),
        &[("depth", 0.65)],
    );
    s.insert(
        b.keys.strip,
        K::VolumeShaper,
        Some("preset.fluctus.pump_quarters"),
        &[("depth", 0.0)],
    );
    hp(s, b.keys.strip, 160.0);
    // The pad: a string ensemble out of Concentus, and Fluctus's pump.
    s.insert(
        b.pad.strip,
        K::Chorus,
        Some("preset.concentus.string_ensemble"),
        &[],
    );
    s.insert(
        b.pad.strip,
        K::VolumeShaper,
        Some("preset.fluctus.pump_quarters"),
        &[("depth", 0.0)],
    );
    hp(s, b.pad.strip, 170.0);
    hp(s, b.arp.strip, 260.0);
    // The guitar through a Phase 90 (script), slow enough to breathe.
    s.insert(
        b.guitar.strip,
        K::Phaser,
        Some("preset.phasis.phase90_script"),
        &[("rate", 0.35), ("mix", 0.5)],
    );
    hp(s, b.guitar.strip, 140.0);
    // The topline: Tonus hard-tunes it to F minor, with a created vibrato on the long
    // notes and the throat a little shorter; then EQ, compression and a light double.
    s.insert(
        b.vox.strip,
        K::AutoTune,
        Some("preset.tonus.hard_vibrato"),
        &[
            ("key", 5.0),
            ("scale", 2.0),
            ("vdepth", 22.0),
            ("throat", 1.5),
        ],
    );
    s.insert(b.vox.strip, K::Eq, Some("eq.preset.vocal"), &[]);
    s.insert(b.vox.strip, K::Compressor, Some("comp.preset.vocal"), &[]);
    s.insert(
        b.vox.strip,
        K::Chorus,
        Some("preset.concentus.vocal_double"),
        &[("mix", 0.25)],
    );
    // The last chorus's doubling lead: a synced through-zero sweep.
    s.insert(
        b.lead.strip,
        K::Flanger,
        Some("preset.flexus.synced_sweep"),
        &[("mix", 0.4), ("fb", 0.45)],
    );
    hp(s, b.lead.strip, 300.0);
    hp(s, b.choir.strip, 220.0);
    // The bells through Anulus: a ring modulator brought in by automation (the breakdown's
    // sound-design moment).
    s.insert(
        b.bells.strip,
        K::RingMod,
        Some("preset.anulus.stereo_shimmer"),
        &[("mix", 0.0)],
    );
    hp(s, b.bells.strip, 400.0);
    // The riser through Translatio's rising barber pole.
    s.insert(
        b.rise.strip,
        K::FreqShift,
        Some("preset.translatio.barber_up"),
        &[("mix", 0.0)],
    );
    hp(s, b.rise.strip, 220.0);
    // The music bus: a high-pass the builds sweep up (automation), a light glue.
    s.insert(d.music, K::Eq, None, &high_pass(20.0, 12.0));
    s.insert(
        d.music,
        K::Compressor,
        Some("comp.preset.glue"),
        &[("thr", -14.0)],
    );
}

fn returns(s: &mut Song, d: &Desk) {
    s.insert(
        d.verb,
        K::Space,
        Some("space.preset.concert_hall"),
        &[("mix", 1.0), ("decay", 2.4), ("predelay", 30.0)],
    );
    s.insert(d.verb, K::Eq, None, &high_pass(320.0, 18.0));
    // A dotted eighth at 116 BPM: 388 ms.
    s.insert(
        d.delay,
        K::Delay,
        None,
        &[
            ("time_ms", 388.0),
            ("feedback", 0.34),
            ("tone_hz", 3_800.0),
            ("mix", 1.0),
        ],
    );
    s.insert(d.delay, K::Eq, None, &high_pass(350.0, 18.0));
}

fn sends(s: &mut Song, b: &Band, d: &Desk) {
    for (p, level) in [
        (b.snare, -15.0),
        (b.piano, -13.0),
        (b.keys, -15.0),
        (b.pad, -13.0),
        (b.arp, -12.0),
        (b.guitar, -15.0),
        (b.vox, -11.0),
        (b.lead, -12.0),
        (b.choir, -7.0),
        (b.bells, -7.0),
        (b.rise, -9.0),
    ] {
        s.send(p.strip, d.verb, level);
    }
    for (p, level) in [
        (b.arp, -11.0),
        (b.guitar, -16.0),
        (b.vox, -14.0),
        (b.lead, -14.0),
        (b.bells, -10.0),
    ] {
        s.send(p.strip, d.delay, level);
    }
}

fn master(s: &mut Song) {
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 24.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 2.0),
            ("b1.freq", 11_000.0),
            ("b1.gain", 1.0),
        ],
    );
    s.master(K::Multiband, Some("mb.preset.glue"), &[]);
    s.master(K::Stereo, Some("preset.st.master_air"), &[]);
    // Tonsor rounds the loudest peaks off (oversampled, true-peak aware) so Culmen's
    // limiter works less; Culmen brings the song to its loudness under −1 dBTP.
    s.master(
        K::Clipper,
        Some("preset.tonsor.soft_master"),
        &[("drive", 1.5)],
    );
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 8.0)]);
    // Speculum at the end of the chain: a pass-through, there to look at (loudness against
    // the song's target).
    s.master(
        K::Analyzer,
        Some("preset.speculum.master"),
        &[("target", -11.0)],
    );
}

fn levels(s: &mut Song, b: &Band, d: &Desk) {
    for (p, trim, fader, pan) in [
        (b.kick, 6.0, 0.0, 0.0),
        (b.snare, 12.0, 0.0, 0.0),
        (b.hats, 12.0, 0.0, 0.15),
        (b.perc, 12.0, 0.0, -0.2),
        (b.fills, 10.0, 0.0, 0.0),
        (b.bass, 0.0, -2.0, 0.0),
        (b.piano, 6.0, 0.0, 0.0),
        (b.keys, 0.0, -3.0, 0.0),
        (b.pad, -2.0, -4.0, 0.0),
        (b.arp, 0.0, -4.0, 0.0),
        (b.guitar, 0.0, -4.0, 0.25),
        (b.vox, 6.0, 0.0, 0.0),
        (b.lead, 6.0, -6.0, 0.0),
        (b.choir, -4.0, -4.0, 0.0),
        (b.bells, 0.0, -6.0, 0.0),
        (b.rise, 8.0, -6.0, 0.0),
        (b.drop, 0.0, -3.0, 0.0),
    ] {
        s.trim(p, trim);
        s.mix(p.strip, fader, pan);
    }
    s.mix(d.drums, 0.0, 0.0);
    s.mix(d.music, 0.0, 0.0);
    s.mix(d.verb, -6.0, 0.0);
    s.mix(d.delay, -10.0, 0.0);
}
