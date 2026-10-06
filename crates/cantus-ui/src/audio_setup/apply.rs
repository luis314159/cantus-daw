//! Reading the engine, and changing the device: one path for the popover, the first-run
//! setup, the fix buttons and plug-and-play.

use super::problems::Fix;
use super::{interfaces, latency, names, plug, FirstRun, Live, Notice};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::settings::AudioSettings;
use std::time::{Duration, Instant};

/// What the engine says now (or the demo's numbers). `peaks` reads the input meters,
/// which empties them for everyone else this frame: only a view that shows them asks.
pub fn live(app: &mut CantusApp, peaks: bool) -> Live {
    if let Some(d) = &app.audio_setup.demo {
        return d.clone();
    }
    let e = &mut app.engine;
    let stats = *e.stats();
    let block = if stats.block_frames > 0 {
        stats.block_frames as u32
    } else {
        e.requested_buffer().unwrap_or(0)
    };
    Live {
        live: e.is_live(),
        driver: e.driver_name().to_string(),
        device: e.device_name.clone(),
        wanted: app
            .settings
            .audio
            .device
            .clone()
            .unwrap_or_else(|| e.device_name.clone()),
        input: e.input_device_name().map(str::to_string),
        rate: e.sample_rate,
        block,
        in_channels: e.input_channel_count(),
        out_channels: e.output_channel_count(),
        peaks: if peaks {
            e.input_peaks().to_vec()
        } else {
            Vec::new()
        },
        latency: e.device_latency(),
        lost: e.device_lost(),
        stalled: e.output_stalled(),
        dropouts: e.xruns() + stats.overruns + e.input_dropped(),
        input_error: e.input_error().map(str::to_string),
        open_error: app.audio_error().map(str::to_string),
    }
}

/// The interface in use, by key: the scan's interface the settings point at, or the
/// running device's adapter when there is no scan yet.
pub fn current_key(app: &CantusApp) -> Option<String> {
    let device = match &app.audio_setup.demo {
        Some(d) => d.device.clone(),
        None if app.engine.is_live() => app.engine.device_name.clone(),
        None => return app.settings.audio_setup.last.clone(),
    };
    if let Some(i) = app
        .audio_setup
        .scan
        .as_ref()
        .and_then(|s| interfaces::current(&s.interfaces, &app.settings.audio, &device))
    {
        return Some(i.key.clone());
    }
    Some(interfaces::adapter_of(&device)).filter(|k| !k.is_empty())
}

/// The input device of the interface in use, on the system's driver.
pub fn own_input(app: &CantusApp) -> Option<String> {
    let key = current_key(app)?;
    let scan = app.audio_setup.scan.as_ref()?;
    scan.interface(&key)?.inputs.first().cloned()
}

/// Use these settings: store them for the interface they belong to and open the device.
pub fn apply(app: &mut CantusApp, audio: AudioSettings) {
    if app.is_recording() {
        app.set_error(t("devices.busy_recording"));
        return;
    }
    let mut audio = audio;
    // The offset belongs to the interface: a new interface starts from its own.
    audio.record_offset_frames = audio.record_offset_frames.clamp(-48_000, 48_000);
    app.settings.audio = audio.clone();
    app.audio_setup.dropouts.reset();
    if app.audio_setup.offline {
        app.audio_setup.applied.push(audio.clone());
        app.store_settings();
    } else {
        app.restart_audio();
    }
    app.audio_setup.plug.lost = None;
    app.audio_setup
        .notices
        .retain(|(n, _)| !matches!(n, Notice::Lost(_)));
    if let Some(key) = key_for(app, &audio) {
        app.settings.audio_setup.remember(&key, &audio);
        app.store_settings();
    }
    apply_routing(app);
}

/// The interface a configuration names (the scan's, else the device's adapter).
fn key_for(app: &CantusApp, audio: &AudioSettings) -> Option<String> {
    let running = if app.engine.is_live() {
        app.engine.device_name.clone()
    } else {
        String::new()
    };
    if let Some(i) = app
        .audio_setup
        .scan
        .as_ref()
        .and_then(|s| interfaces::current(&s.interfaces, audio, &running))
    {
        return Some(i.key.clone());
    }
    let device = audio.device.clone().unwrap_or(running);
    Some(interfaces::adapter_of(&device)).filter(|k| !k.is_empty())
}

/// Switch to an interface: its remembered settings, or the suggestion for it.
pub fn choose_interface(app: &mut CantusApp, key: &str) {
    let Some(iface) = app
        .audio_setup
        .scan
        .as_ref()
        .and_then(|s| s.interface(key))
        .cloned()
    else {
        return;
    };
    let audio = app
        .settings
        .audio_setup
        .recall(key)
        .cloned()
        .unwrap_or_else(|| interfaces::suggest(&iface, cantus_engine::devices::ASIO_BUILT));
    apply(app, audio);
    app.audio_setup
        .notices
        .retain(|(n, _)| n != &Notice::Connected(key.to_string()));
    app.set_status(tf("devices.switched", &[("name", &key)]));
}

/// The lost interface is back: open it again as it was.
pub fn reconnect(app: &mut CantusApp, key: &str) {
    let audio = app
        .settings
        .audio_setup
        .recall(key)
        .cloned()
        .unwrap_or_else(|| app.settings.audio.clone());
    apply(app, audio);
    app.audio_setup.notice(Notice::Reconnected(key.to_string()));
    app.set_status(tf("devices.notice.reconnected", &[("name", &key)]));
}

/// Apply a problem's fix.
pub fn fix(app: &mut CantusApp, f: &Fix) {
    let mut a = app.settings.audio.clone();
    match f {
        Fix::RestartAudio => {}
        Fix::UseDefault => {
            // The lost interface stays remembered: it is reopened when it returns.
            let lost = app.audio_setup.plug.lost.clone();
            a = AudioSettings {
                record_offset_frames: 0,
                ..Default::default()
            };
            apply(app, a);
            app.audio_setup.plug.lost = lost;
            return;
        }
        Fix::RunAt(r) => a.sample_rate = Some(*r),
        Fix::UseInput(name) => a.input_device = Some(name.clone()),
        Fix::RaiseBuffer(b) => a.buffer_frames = Some(*b),
    }
    apply(app, a);
}

/// Hand the interface's routing to the engine: the main pair, the cue, the tone.
pub fn apply_routing(app: &mut CantusApp) {
    let p = current_key(app)
        .and_then(|k| app.settings.audio_setup.profile(&k).cloned())
        .unwrap_or_default();
    app.engine.set_main_output(p.main);
    app.engine.set_cue_output(p.cue.map(|c| (c, p.cue_gain())));
    app.engine.set_test_tone(app.audio_setup.tone);
    names::set_inputs(&p.inputs);
}

/// Play the test tone on a pair (`None` stops it).
pub fn set_tone(app: &mut CantusApp, pair: Option<u16>) {
    app.audio_setup.tone = pair;
    app.engine.set_test_tone(pair);
}

/// Open or close the popover (the chip's click). Opening it scans the devices.
pub fn toggle_popover(app: &mut CantusApp) {
    let s = &mut app.audio_setup;
    s.popover = !s.popover;
    s.pop_list = None;
    if s.popover {
        s.request_scan();
    }
}

/// Open the first-run setup.
pub fn open_first_run(app: &mut CantusApp) {
    app.audio_setup.first_run = Some(FirstRun::default());
    app.audio_setup.popover = false;
    app.audio_setup.request_scan();
}

/// Close it: the tone stops, and it is not opened by itself again.
pub fn close_first_run(app: &mut CantusApp) {
    app.audio_setup.first_run = None;
    set_tone(app, None);
    app.settings.audio_setup.setup_done = true;
    app.store_settings();
}

/// At start: a person who has never set audio up gets the first-run setup.
pub fn arm_first_run(app: &mut CantusApp) {
    let s = &app.settings;
    if !s.audio_setup.setup_done && s.audio == AudioSettings::default() {
        open_first_run(app);
    }
}

/// The round trip a buffer of `b` frames should give on this device, in ms.
pub fn estimate_ms(live: &Live, b: u32) -> f64 {
    let rate = if live.rate > 0.0 { live.rate } else { 48_000.0 };
    let over = if live.live && live.block > 0 && live.latency.round_trip() > 0 {
        latency::overhead(live.latency, live.block)
    } else {
        0
    };
    latency::estimate_ms(b, rate, over)
}

/// Once a frame, before anything is drawn: take a scan in, watch the devices, count
/// dropouts, notice a device the engine lost, keep the channel names current. Allocates
/// nothing in the common case (the frame budget holds it).
pub fn tick(app: &mut CantusApp) {
    app.audio_setup.poll_scan();
    if app.engine.is_live() && !app.audio_setup.offline {
        app.audio_setup.plug.start();
    }
    plug::poll(app);
    let now = Instant::now();
    let (total, lost) = match &app.audio_setup.demo {
        Some(d) => (d.dropouts, d.lost),
        None => {
            let e = &app.engine;
            (
                e.xruns() + e.stats().overruns + e.input_dropped(),
                e.device_lost(),
            )
        }
    };
    app.audio_setup.dropouts.update(total, now);
    if lost && app.audio_setup.plug.lost.is_none() {
        if let Some(k) = current_key(app) {
            app.audio_setup.plug.lost = Some(k.clone());
            app.audio_setup.notice(Notice::Lost(k));
        }
    }
    // Cards that only report fade away; the others wait for an answer.
    app.audio_setup.notices.retain(|(n, at)| match n {
        Notice::Reconnected(_) => now.duration_since(*at) < Duration::from_secs(6),
        _ => true,
    });
    // A device opened (at start, after a restart): its routing and channel names —
    // looked up again only when the device changes.
    if app.audio_setup.names_for != app.engine.device_name {
        app.audio_setup.names_for = app.engine.device_name.clone();
        apply_routing(app);
    }
}

/// Put the interface's channel names where the input menus read them.
pub fn sync_names(app: &mut CantusApp) {
    let names = current_key(app)
        .and_then(|k| {
            app.settings
                .audio_setup
                .profile(&k)
                .map(|p| p.inputs.clone())
        })
        .unwrap_or_default();
    names::set_inputs(&names);
}

/// Finish the first-run setup. When an input moved, add an audio track recording from it
/// — armed, selected, named after the channel when it has a name — so the next thing is
/// pressing record. One undo step.
pub fn finish_first_run(app: &mut CantusApp, heard: Option<usize>) {
    close_first_run(app);
    let Some(ch) = heard else {
        return;
    };
    let mut name = String::new();
    if !names::write_input(ch, false, &mut name) {
        let n = app
            .project()
            .tracks
            .iter()
            .filter(|t| t.input.is_some())
            .count()
            + 1;
        name = tf("rec.audio_track_name", &[("n", &n)]);
    }
    let mut track = app.bus.new_track(name);
    track.input = Some(cantus_core::TrackInput::mono(ch as u16));
    track.icon = Some(cantus_core::TrackIcon::Strings);
    track.armed = true;
    let id = track.id;
    let index = app
        .selected_track
        .and_then(|s| app.project().tracks.iter().position(|t| t.id == s))
        .map(|i| i + 1)
        .unwrap_or(app.project().tracks.len());
    if app.exec(cantus_core::Command::InsertTrack {
        index,
        track: Box::new(track),
    }) {
        app.select_track(id);
        app.set_status(tf("devices.first.ready", &[("n", &(ch + 1))]));
    }
}
