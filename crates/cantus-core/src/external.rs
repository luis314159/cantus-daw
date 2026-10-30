//! A plug-in made to a market standard (CLAP, VST3) in an insert slot, as the project
//! stores it (ADR 0019).
//!
//! The project keeps what finds the plug-in again on another machine (format and id),
//! what to show when it cannot (name and vendor), the plug-in's own state as an opaque
//! blob, and its **parameter values by the plug-in's ids**. The values are what undo and,
//! later, automation work on: a change is a command like any other, and the engine turns
//! the difference between two snapshots into the plug-in's parameter events. The state is
//! what the plug-in says it is when the project is saved; everything the parameters do not
//! cover (a sample it loaded, a preset name) lives there.
//!
//! A project whose plug-in is missing on this machine keeps all of this untouched: it
//! opens, the slot passes audio through as a wire, and saving writes the same data back.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The i18n key a hosted plug-in's kind is shown with when its own name is not at hand.
pub const NAME_KEY: &str = "host.plugin";

/// A plug-in standard. The host's own `cantus_host::Format` mirrors it; this one is the
/// project's, so the model depends on no host.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginFormat {
    Clap,
    Vst3,
}

impl PluginFormat {
    /// The short badge a slot shows.
    pub fn badge(self) -> &'static str {
        match self {
            PluginFormat::Clap => "CLAP",
            PluginFormat::Vst3 => "VST3",
        }
    }
}

/// A plug-in's saved state: opaque bytes, base64 in the project file. Shared, so compiling
/// a snapshot after every edit does not copy a sampler's megabytes each time.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PluginState(pub Arc<Vec<u8>>);

impl PluginState {
    pub fn new(bytes: Vec<u8>) -> Self {
        PluginState(Arc::new(bytes))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PluginState({} bytes)", self.0.len())
    }
}

impl Serialize for PluginState {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64_encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for PluginState {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        base64_decode(&text)
            .map(PluginState::new)
            .ok_or_else(|| serde::de::Error::custom("plug-in state is not base64"))
    }
}

/// Where a hosted plug-in runs (round 29, ADR 0063). Cantus's own effects and instruments
/// are never hosted, so they always run within; this is for third-party CLAP and VST3.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginIsolation {
    /// In Cantus's own process: no round trip, and a crash takes Cantus with it.
    Within,
    /// In one helper process shared by every plug-in set to it: a crash takes the helper
    /// and its other plug-ins, never Cantus.
    Together,
    /// One helper process per vendor.
    ByVendor,
    /// A helper process of its own: a crash takes only this plug-in.
    Individually,
}

impl PluginIsolation {
    pub const ALL: [PluginIsolation; 4] = [
        PluginIsolation::Within,
        PluginIsolation::Together,
        PluginIsolation::ByVendor,
        PluginIsolation::Individually,
    ];

    /// The i18n key it is shown with.
    pub fn label_key(self) -> &'static str {
        match self {
            PluginIsolation::Within => "isolation.within",
            PluginIsolation::Together => "isolation.together",
            PluginIsolation::ByVendor => "isolation.by_vendor",
            PluginIsolation::Individually => "isolation.individually",
        }
    }

    /// Whether the plug-in runs in a helper process.
    pub fn isolated(self) -> bool {
        self != PluginIsolation::Within
    }
}

/// How a hosted instrument hears per-note expression (round 33, ADR 0080).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprRoute {
    /// The format's own per-note events: CLAP note expressions, VST3 note expression.
    NoteExpression,
    /// MPE over MIDI 1.0: each note on a member channel of its own, its bend, pressure and
    /// timbre as that channel's pitch bend, channel pressure and CC 74, the zone announced
    /// with its MCM and bend range — for a plug-in that takes MIDI only.
    MpeMidi,
    /// Nothing per note: the notes alone (a plug-in that misreads either).
    Off,
}

impl ExprRoute {
    pub const ALL: [ExprRoute; 3] = [
        ExprRoute::NoteExpression,
        ExprRoute::MpeMidi,
        ExprRoute::Off,
    ];

    /// The route when the project has not chosen: the format's own events where the
    /// plug-in declares it takes them, MPE where it takes MIDI only.
    pub fn automatic(takes_note_expressions: bool, takes_midi: bool) -> ExprRoute {
        if takes_note_expressions {
            ExprRoute::NoteExpression
        } else if takes_midi {
            ExprRoute::MpeMidi
        } else {
            ExprRoute::Off
        }
    }

    /// The i18n key it is shown with.
    pub fn label_key(self) -> &'static str {
        match self {
            ExprRoute::NoteExpression => "mpe.route.note_expression",
            ExprRoute::MpeMidi => "mpe.route.mpe_midi",
            ExprRoute::Off => "mpe.route.off",
        }
    }
}
/// A hosted plug-in in an insert slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalPlugin {
    pub format: PluginFormat,
    /// The format's stable id: a CLAP plug-in id, a VST3 class id.
    pub id: String,
    /// Shown when the plug-in is missing on this machine.
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// It has a second (sidechain) audio input, so the slot can be keyed from a strip.
    #[serde(default)]
    pub sidechain: bool,
    /// The plug-in's own state, as it gave it when the project was last saved.
    #[serde(default)]
    pub state: PluginState,
    /// Parameter values by the plug-in's ids, in the plug-in's own range.
    #[serde(default, deserialize_with = "params_by_id")]
    pub params: BTreeMap<u32, f64>,
    /// Where it runs (schema 36, ADR 0063); `None` follows the machine's default, set in
    /// Settings' plug-in manager. Not written while `None`, so a project that never chose
    /// saves the same bytes as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation: Option<PluginIsolation>,
    /// How per-note expression reaches it (schema 47, ADR 0080); `None` — every project
    /// before, and the default — chooses by what the plug-in declares
    /// ([`ExprRoute::automatic`]). Not written while `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<ExprRoute>,
}

impl ExternalPlugin {
    pub fn new(format: PluginFormat, id: impl Into<String>, name: impl Into<String>) -> Self {
        ExternalPlugin {
            format,
            id: id.into(),
            name: name.into(),
            vendor: String::new(),
            sidechain: false,
            state: PluginState::default(),
            params: BTreeMap::new(),
            isolation: None,
            expression: None,
        }
    }
}

/// The values map, read whether its keys arrive as numbers or as the text JSON gives them.
/// A record inside an internally tagged enum (a track's `InstrumentSpec::External`, ADR
/// 0028) is buffered by serde before it is read, and a buffered key is text that a plain
/// `u32` key refuses.
fn params_by_id<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<u32, f64>, D::Error> {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Id(u32);
    struct IdVisitor;
    impl serde::de::Visitor<'_> for IdVisitor {
        type Value = Id;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a parameter id")
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Id, E> {
            u32::try_from(v).map(Id).map_err(E::custom)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Id, E> {
            u32::try_from(v).map(Id).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Id, E> {
            v.parse().map(Id).map_err(E::custom)
        }
    }
    impl<'de> Deserialize<'de> for Id {
        fn deserialize<D2: Deserializer<'de>>(d: D2) -> Result<Id, D2::Error> {
            d.deserialize_any(IdVisitor)
        }
    }
    let raw: BTreeMap<Id, f64> = BTreeMap::deserialize(d)?;
    Ok(raw.into_iter().map(|(k, v)| (k.0, v)).collect())
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (RFC 4648 §4).
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The inverse of [`base64_encode`]; `None` for text that is not base64. Whitespace is
/// skipped, so a hand-wrapped value still reads.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut padding = 0usize;
    for c in text.bytes() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None; // data after padding
        }
        let v = ALPHABET.iter().position(|a| *a == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    (padding <= 2).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length_and_matches_the_rfc() {
        // RFC 4648 §10.
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(plain.as_bytes()), coded);
            assert_eq!(base64_decode(coded).unwrap(), plain.as_bytes());
        }
        let all: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        for n in 0..all.len() {
            assert_eq!(base64_decode(&base64_encode(&all[..n])).unwrap(), &all[..n]);
        }
        assert!(base64_decode("Zm9v!").is_none());
        assert!(base64_decode("Zg==Zg").is_none());
    }

    #[test]
    fn a_plug_in_round_trips_through_json_with_its_state_and_values() {
        let mut p = ExternalPlugin::new(PluginFormat::Clap, "org.example.gain", "Gain");
        p.vendor = "Example".into();
        p.sidechain = true;
        p.state = PluginState::new(vec![0, 1, 2, 250, 255]);
        p.params.insert(0, -6.25);
        p.params.insert(10, 0.1 + 0.2);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"AAEC+v8=\""), "{json}");
        let back: ExternalPlugin = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.params[&10].to_bits(), (0.1f64 + 0.2).to_bits());
    }

    /// A track's hosted instrument (ADR 0028) is the same record inside an internally
    /// tagged enum, which serde buffers: its values must still come back, to the bit.
    #[test]
    fn a_hosted_instrument_round_trips_through_json() {
        let mut p = ExternalPlugin::new(
            PluginFormat::Vst3,
            "0123456789ABCDEF0123456789ABCDEF",
            "Synth",
        );
        p.state = PluginState::new(vec![7, 0, 255]);
        p.params.insert(0, -4.5);
        p.params.insert(4_000_000, 0.1 + 0.2);
        let spec = crate::InstrumentSpec::External(p);
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"type\":\"External\""), "{json}");
        let back: crate::InstrumentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        let params = &back.external().unwrap().params;
        assert_eq!(params[&4_000_000].to_bits(), (0.1f64 + 0.2).to_bits());
    }
}
