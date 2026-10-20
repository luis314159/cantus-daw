//! The views. Each is a plain function `show(app, ui)`; state that must survive between
//! frames lives in a small struct owned by `CantusApp`.
//!
//! The window is a workspace (ADR 0024 §5): every view is a tab of the dock
//! ([`workspace`]), under the toolbar ([`transport`]) and over the status bar
//! ([`workspace_status`]).

pub mod about;
pub mod appearance;
pub mod appearance_file;
pub mod appearance_look;
pub mod appearance_panels;
pub mod arrangement;
pub mod audio_art;
pub mod audio_chip;
pub mod audio_drivers;
pub mod audio_firstrun;
pub mod audio_notice;
pub mod audio_parts;
pub mod audio_popover;
pub mod audio_popover_lists;
pub mod audio_routing;
pub mod audio_setup;
pub mod auto_curve;
pub mod auto_editor;
pub mod bars;
pub mod channel_rack;
pub mod clip_browser;
pub mod clips_chrome;
pub mod comp_editor;
pub mod controllers;
pub mod demos;
pub mod device_chrome;
/// A drum kit's row names in the interface's language (round 28).
pub mod drum_names;
pub mod eq_editor;
pub mod export;
pub mod export_dialog;
pub mod export_rows;
pub mod fx;
pub mod hint;
pub mod inspector;
pub mod inst;
pub mod interop;
pub mod link_picker;
pub mod mixer;
pub mod modulator;
pub mod piano_roll;
pub mod picker;
pub mod picker_cover;
pub mod picker_details;
pub mod picker_grid;
pub mod picker_keys;
pub mod picker_layout;
pub mod picker_panel;
pub mod picker_side;
pub mod picker_snap;
pub mod picker_thumbs;
pub mod pitch;
pub mod pitch_lane;
pub mod plugin_notice;
pub mod plugins;
pub mod pulsus_editor;
pub mod record_options;
pub mod recovery;
pub mod route_menu;
pub mod routing;
pub mod samples_browser;
pub mod samples_content;
pub mod samples_paint;
pub mod settings_audio;
pub mod transport;
pub mod unsaved;
pub mod update;
pub mod verse;
pub mod warp;
pub mod warp_quantise;
pub mod workspace;
pub mod workspace_device;
pub mod workspace_frame;
pub mod workspace_settings;
pub mod workspace_skin;
pub mod workspace_status;
pub mod workspace_style;
pub mod workspace_ticker;
pub mod workspace_widgets;

use cantus_core::{Ticks, PPQ};

pub fn ticks_to_beats(t: Ticks) -> f32 {
    t.0 as f32 / PPQ as f32
}

pub fn beats_to_ticks(b: f32) -> Ticks {
    Ticks((b * PPQ as f32).round() as i64)
}

/// Grid choices for snapping (label key, ticks).
pub const GRIDS: [(&str, i64); 8] = [
    ("grid.bar", PPQ * 4),
    ("grid.half", PPQ * 2),
    ("grid.quarter", PPQ),
    ("grid.eighth", PPQ / 2),
    ("grid.sixteenth", PPQ / 4),
    ("grid.thirtysecond", PPQ / 8),
    ("grid.triplet8", PPQ / 3),
    ("grid.triplet16", PPQ / 6),
];
