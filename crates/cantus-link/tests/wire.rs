//! The wire, against byte vectors written out from the SDK's definitions (ADR 0078 §5):
//! `discovery/v1/Messages.hpp` (header), `discovery/Payload.hpp` (entries: key, size,
//! value), `link/Timeline.hpp` + `Tempo.hpp` + `Beats.hpp` (`tmln`: µs per beat, µbeats,
//! µs), `link/SessionId.hpp` (`sess`), `link/StartStopState.hpp` (`stst`: bool, µbeats,
//! µs), `link/EndpointV4.hpp` (`mep4`: address, port), `link/v1/Messages.hpp` and
//! `link/PayloadEntries.hpp` (`__ht`, `__gt`, `_pgt`), every number big-endian
//! (`discovery/NetworkByteStreamSerializable.hpp`). `tests/sdk_vectors.rs` checks the same
//! states against the bytes the real SDK writes, when it was built.

use cantus_link::timeline::{Beats, StartStop, Tempo, Timeline};
use cantus_link::wire::{self, PeerState};
use std::net::{Ipv4Addr, SocketAddrV4};

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// The state every vector here encodes: 120 BPM (500 000 µs per beat), beat 8 at
/// 1 000 000 µs, playing from beat 4 decided at 2 000 000 µs, measured at 192.168.1.20:52000.
fn state() -> PeerState {
    PeerState {
        node: [1, 2, 3, 4, 5, 6, 7, 8],
        session: [9, 10, 11, 12, 13, 14, 15, 16],
        timeline: Timeline {
            tempo: Tempo::new(120.0),
            beat_origin: Beats::new(8.0),
            time_origin: 1_000_000,
        },
        start_stop: StartStop {
            playing: true,
            beats: Beats::new(4.0),
            timestamp: 2_000_000,
        },
        measurement: Some(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 20), 52000)),
    }
}

const ALIVE: &str = "
    5f 61 73 64 70 5f 76 01   01 05 00 00   01 02 03 04 05 06 07 08
    74 6d 6c 6e 00 00 00 18   00 00 00 00 00 07 a1 20   00 00 00 00 00 7a 12 00   00 00 00 00 00 0f 42 40
    73 65 73 73 00 00 00 08   09 0a 0b 0c 0d 0e 0f 10
    73 74 73 74 00 00 00 11   01   00 00 00 00 00 3d 09 00   00 00 00 00 00 1e 84 80
    6d 65 70 34 00 00 00 06   c0 a8 01 14 cb 20";

#[test]
fn an_alive_message_is_byte_for_byte_the_sdks() {
    let w = wire::encode_discovery(wire::ALIVE, 5, &state());
    assert_eq!(w.finish().unwrap(), hex(ALIVE).as_slice());
    let d = wire::decode_discovery(&hex(ALIVE)).unwrap();
    assert_eq!((d.kind, d.ttl, d.group), (wire::ALIVE, 5, 0));
    assert_eq!(d.state, Some(state()));
}

#[test]
fn a_byebye_carries_no_state_and_no_time_to_live() {
    let w = wire::encode_discovery(wire::BYEBYE, 5, &state());
    let want = hex("5f 61 73 64 70 5f 76 01  03 00 00 00  01 02 03 04 05 06 07 08");
    assert_eq!(w.finish().unwrap(), want.as_slice());
}

#[test]
fn unknown_entries_are_skipped_and_short_ones_refused() {
    let mut b = hex(ALIVE);
    // Link Audio's `aep4`, which this crate does not read.
    b.extend(hex("61 65 70 34 00 00 00 06  0a 00 00 01 12 34"));
    assert_eq!(wire::decode_discovery(&b).unwrap().state, Some(state()));
    let cut = &hex(ALIVE)[..hex(ALIVE).len() - 3];
    assert_eq!(
        wire::decode_discovery(cut).unwrap_err(),
        wire::WireError::Malformed
    );
    assert_eq!(
        wire::decode_discovery(b"_asdp_v\x02rest of it...").unwrap_err(),
        wire::WireError::Header
    );
}

#[test]
fn ping_and_pong_are_the_sdks() {
    let ping = wire::encode_ping(0x0102_0304_0506, Some(0x0a0b));
    let want = hex("5f 6c 69 6e 6b 5f 76 01  01
         5f 5f 68 74 00 00 00 08  00 00 01 02 03 04 05 06
         5f 70 67 74 00 00 00 08  00 00 00 00 00 00 0a 0b");
    assert_eq!(ping.finish().unwrap(), want.as_slice());
    // The pong: the session, the ghost time, then the ping's payload echoed.
    let pong = wire::encode_pong(&[9; 8], 77, &want[9..]);
    let mut pw = hex("5f 6c 69 6e 6b 5f 76 01  02
         73 65 73 73 00 00 00 08  09 09 09 09 09 09 09 09
         5f 5f 67 74 00 00 00 08  00 00 00 00 00 00 00 4d");
    pw.extend_from_slice(&want[9..]);
    assert_eq!(pong.finish().unwrap(), pw.as_slice());
    let m = wire::decode_measure(&pw).unwrap();
    assert_eq!(m.kind, wire::PONG);
    assert_eq!(m.session, Some([9; 8]));
    assert_eq!(
        (m.ghost_time, m.host_time, m.prev_ghost_time),
        (77, 0x0102_0304_0506, 0x0a0b)
    );
}

#[test]
fn tempo_travels_as_whole_microseconds_per_beat() {
    // 128 BPM is 468 750 µs per beat exactly; 133 is 451 127.8… → 451 128.
    assert_eq!(Tempo::new(128.0).micros_per_beat(), 468_750);
    assert_eq!(Tempo::new(133.0).micros_per_beat(), 451_128);
    let back = Tempo::from_micros_per_beat(451_128).bpm();
    assert!((back - 133.0).abs() < 1e-3);
}
