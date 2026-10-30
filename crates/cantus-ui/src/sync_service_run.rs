//! The sync thread: every millisecond, the source's messages to its follower, Link's
//! session read, the timeline published, the engine's proposals handed to Link.

use super::{Cfg, EngineSide, Status, SyncSource};
use cantus_engine::midi::SyncMsg;
use cantus_engine::transport::sync::{
    self, clock::ClockFollower, mtc::MtcChaser, Proposal, SourceKind, Timeline,
};
use cantus_link::{Config, Link};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A start the user asks for while Link is followed alone is put this far ahead (µs), so
/// the session's beat 0 lands where the engine can still start on it.
const START_AHEAD_US: i64 = 30_000;
/// After Link fails to open, it is tried again this often (µs).
const RETRY_US: i64 = 5_000_000;

pub(super) fn run(
    mut rx: rtrb::Consumer<SyncMsg>,
    cfg: Arc<Mutex<Cfg>>,
    engine: Arc<Mutex<Option<EngineSide>>>,
    status: Arc<Status>,
    stop: Arc<AtomicBool>,
    tempo_req: Arc<Mutex<Option<f64>>>,
) {
    let mut link: Option<Link> = None;
    let mut link_retry = i64::MIN;
    let mut clock = ClockFollower::new();
    let mut chase = MtcChaser::new();
    let mut last = Cfg::default();
    let mut heard_at = i64::MIN;
    while !stop.load(Ordering::Relaxed) {
        // Every millisecond while a source is followed; at rest, twenty.
        let idle = last.source == SyncSource::None;
        std::thread::sleep(Duration::from_millis(if idle { 20 } else { 1 }));
        let c = cfg.lock().map(|c| c.clone()).unwrap_or_default();
        let now = sync::host_micros();
        if c.source != last.source || c.port != last.port {
            clock = ClockFollower::new();
            chase = MtcChaser::new();
        }
        // The source port's clock or code, in the order it came.
        while let Ok(m) = rx.pop() {
            if Some(usize::from(m.port)) != c.port {
                continue;
            }
            heard_at = m.at;
            match c.source {
                SyncSource::Clock => clock.feed(m.at, m.msg()),
                SyncSource::Mtc => chase.feed(m.at, m.msg()),
                _ => {}
            }
        }
        status
            .receiving
            .store(now.saturating_sub(heard_at) < 500_000, Ordering::Relaxed);
        // Link lives while it is the source.
        if c.source == SyncSource::Link {
            if link.is_none() && now >= link_retry {
                let bpm = if c.project_bpm > 0.0 {
                    c.project_bpm
                } else {
                    120.0
                };
                let l = Link::new(bpm, sync::host_micros, Config::default());
                match l.enable(true) {
                    Ok(()) => link = Some(l),
                    Err(e) => {
                        log::warn!("link: {e}");
                        status.link_failed.store(true, Ordering::Relaxed);
                        link_retry = now + RETRY_US;
                    }
                }
            }
        } else if link.is_some() {
            link = None;
        }
        // On but speaking on no interface (none could be opened): said in the top bar and
        // Settings while it keeps looking (round 2, D6).
        if let Some(l) = &link {
            status
                .link_failed
                .store(l.interfaces_open() == 0, Ordering::Relaxed);
        } else if c.source != SyncSource::Link {
            status.link_failed.store(false, Ordering::Relaxed);
        }
        if let Some(bpm) = tempo_req.lock().ok().and_then(|mut r| r.take()) {
            if let Some(l) = &link {
                l.set_tempo(bpm, now);
            }
        }
        let tl = match c.source {
            SyncSource::None => Timeline::default(),
            SyncSource::Link => match &link {
                Some(l) => {
                    let s = l.session();
                    status.peers.store(s.peers as u32, Ordering::Relaxed);
                    status
                        .link_tempo
                        .store(s.tempo.to_bits(), Ordering::Relaxed);
                    Timeline {
                        kind: SourceKind::Link,
                        tempo: s.tempo,
                        host: now,
                        position: s.beat_at(now),
                        rate: 1.0,
                        playing: s.playing,
                        quantum: c.quantum,
                        start_stop: c.start_stop,
                        epoch: 0,
                    }
                }
                None => Timeline::default(),
            },
            SyncSource::Clock => clock.timeline(now),
            SyncSource::Mtc => {
                let t = chase.timeline(now);
                status
                    .code_s
                    .store(t.seconds_at(now).to_bits(), Ordering::Relaxed);
                status
                    .code_rate
                    .store(u32::from(chase.rate().code()), Ordering::Relaxed);
                t
            }
        };
        status.link_on.store(link.is_some(), Ordering::Relaxed);
        if link.is_none() {
            status.peers.store(0, Ordering::Relaxed);
        }
        if let Ok(mut e) = engine.lock() {
            if let Some(side) = e.as_mut() {
                side.shared.timeline.publish(&tl);
                while let Ok(p) = side.proposals.pop() {
                    let Some(l) = &link else { continue };
                    match p {
                        Proposal::Play { at } => {
                            let at = at.max(now) + START_AHEAD_US;
                            if l.session().peers == 0 {
                                // Alone: the session's bar starts where ours does.
                                l.request_beat_at(0.0, at, c.quantum);
                            }
                            l.set_playing(true, at);
                        }
                        Proposal::Stop { at } => l.set_playing(false, at),
                        Proposal::Tempo { bpm, at } => l.set_tempo(bpm, at),
                    }
                }
            }
        }
        last = c;
    }
}
