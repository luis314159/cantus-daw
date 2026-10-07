//! A plausible studio for screenshots and tests (round 20): a Behringer UMC204HD with its
//! ASIO driver, the laptop's own Realtek audio, a pair of Bluetooth headphones and a
//! screen — and an engine running on the UMC at 48 kHz, 128 samples, a guitar on input 1.
//! Nothing here touches the machine's sound card: the setup is `offline`.

use super::{AudioSetup, Live, Scan};
use crate::app::CantusApp;
use crate::settings::AudioSettings;
use cantus_engine::{DeviceLatency, DriverInfo, InputDeviceInfo};

pub const UMC: &str = "UMC204HD 192k";
pub const UMC_OUT: &str = "Speakers (UMC204HD 192k)";
pub const UMC_IN: &str = "Line (UMC204HD 192k)";
pub const REALTEK_OUT: &str = "Speakers (Realtek(R) Audio)";
pub const REALTEK_IN: &str = "Microphone Array (Realtek(R) Audio)";

/// The machine's endpoints, as the system driver lists them.
pub fn scan(with_umc: bool) -> Scan {
    let mut outputs = vec![
        REALTEK_OUT.to_string(),
        "Headphones (WH-1000XM4 Stereo)".to_string(),
        "LG ULTRAGEAR (NVIDIA High Definition Audio)".to_string(),
    ];
    let mut inputs = vec![InputDeviceInfo {
        name: REALTEK_IN.into(),
        channels: 2,
        is_default: true,
    }];
    if with_umc {
        outputs.insert(0, UMC_OUT.into());
        inputs.insert(
            0,
            InputDeviceInfo {
                name: UMC_IN.into(),
                channels: 2,
                is_default: false,
            },
        );
    }
    Scan::of(
        vec![
            DriverInfo {
                name: "WASAPI".into(),
                is_default: true,
            },
            DriverInfo {
                name: "ASIO".into(),
                is_default: false,
            },
        ],
        outputs,
        inputs,
        vec!["UMC ASIO Driver".into(), "Realtek ASIO".into()],
    )
}

/// The engine's numbers on the UMC: running, a guitar on input 1.
pub fn live_on_umc() -> Live {
    Live {
        live: true,
        driver: "WASAPI".into(),
        device: UMC_OUT.into(),
        wanted: UMC_OUT.into(),
        input: Some(UMC_IN.into()),
        rate: 48_000.0,
        block: 128,
        in_channels: 2,
        out_channels: 4,
        peaks: vec![0.42, 0.02],
        latency: DeviceLatency {
            input: 158,
            output: 172,
        },
        lost: false,
        stalled: false,
        state: cantus_engine::DeviceState::Running,
        asked_buffer: None,
        rate_refused: None,
        dropouts: 0,
        input_error: None,
        open_error: None,
    }
}

/// Put `app` in the demo studio: offline, the scan above, the engine on the UMC, the
/// settings pointing at it and remembered for it.
pub fn setup(app: &mut CantusApp) {
    app.audio_setup = AudioSetup {
        offline: true,
        scan: Some(scan(true)),
        demo: Some(live_on_umc()),
        ..Default::default()
    };
    let audio = AudioSettings {
        driver: None,
        device: Some(UMC_OUT.into()),
        input_device: Some(UMC_IN.into()),
        sample_rate: Some(48_000),
        buffer_frames: Some(128),
        record_offset_frames: 0,
        workers: None,
    };
    app.settings.audio = audio.clone();
    app.settings.audio_setup.remember(UMC, &audio);
    app.settings.audio_setup.setup_done = true;
}
