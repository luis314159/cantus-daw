//! Saltator's stage (round 35, ADR 0087): the character dancing live in the editor, on a
//! small theatre drawn in the kit — a floor that catches the light, a spotlight that
//! follows it and swells on the beat, footlights along the front, and the words a
//! performer's monitor would show: the bar and the beat, the move playing, who dances.
//!
//! The character is drawn by the one renderer at the display's physical pixels
//! (`DanceTexture`), and only when its pose or size changed.

use crate::app::CantusApp;
use crate::dance_live::{DanceLive, LiveDancer};
use crate::dance_texture::DanceTexture;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::TrackId;
use cantus_dance::render::{Background, Stage};
use egui::{Align2, Pos2, Rect, Vec2};

/// Where the floor line sits in the display (a share of its height): a dancer's `y` at its
/// default (0.85) stands on it.
pub const FLOOR: f32 = 0.85;

/// The stage's state: its texture and what it last drew.
#[derive(Default)]
pub struct StageState {
    tex: Option<DanceTexture>,
    shown: Option<(TrackId, String, cantus_dance::Pose, [u32; 2])>,
    /// Redraws of the character (the tests' and the budget's measure).
    pub draws: u64,
}

/// The dancer on `track`, as the screen draws it now.
pub fn live(app: &CantusApp, track: TrackId) -> Option<LiveDancer> {
    DanceLive::dancers(app)
        .into_iter()
        .find(|d| d.track == track)
}

/// Draw the character into the stage's texture (when anything changed) and give its id.
pub fn character_texture(
    st: &mut StageState,
    ctx: &egui::Context,
    track: TrackId,
    dancer: &LiveDancer,
    px: [u32; 2],
) -> Option<egui::TextureId> {
    let id = dancer.character.info().id.clone();
    let same = st
        .shown
        .as_ref()
        .is_some_and(|(t, c, p, sz)| *t == track && *c == id && *p == dancer.pose && *sz == px);
    let tex = st
        .tex
        .get_or_insert_with(|| DanceTexture::new("dancer-editor-stage"));
    if same {
        return tex.id();
    }
    st.shown = Some((track, id, dancer.pose, px));
    st.draws += 1;
    let mut stage = Stage::new(px[0], px[1], Background::Transparent);
    // The editor's stage is a close-up: the character stands taller than on the
    // workspace's stage, its feet on the floor line.
    stage.standard_height = 0.62;
    let mut pose = dancer.pose;
    pose.y = FLOOR + (pose.y - 0.85);
    tex.draw(ctx, &stage, &[(dancer.character.as_ref(), pose)])
}

/// The theatre behind the character: the back wall, the floor, the spotlight on the
/// character's place, the footlights. `accent` (0…1) swells the light on the beat.
pub fn backdrop(b: &mut Brush, rect: Rect, s: &Style, x: f32, accent: f32, playing: bool) {
    let t = &s.t;
    let ink = crate::kit::skin::display_ink(s);
    let ground = ink.ground;
    let light = ink.trace;
    let gold = s.skin.accent2_in(t.dark).unwrap_or(t.highlight);
    let floor_y = rect.top() + rect.height() * FLOOR;
    // The back wall, a little lighter high up where the lamps are.
    b.rect(
        Rect::from_min_max(rect.min, Pos2::new(rect.right(), floor_y)),
        Fill::V(mix(ground, light, 0.10), ground),
    );
    // The floor: a darker plane catching the light at its front edge.
    b.rect(
        Rect::from_min_max(Pos2::new(rect.left(), floor_y), rect.max),
        Fill::V(mix(ground, light, 0.14), mix(ground, t.dark_shade, 0.35)),
    );
    // The spotlight: a cone from above and a pool on the floor, where it stands.
    let cx = rect.left() + rect.width() * x.clamp(0.0, 1.0);
    let swell = if playing { 0.75 + 0.25 * accent } else { 0.7 };
    let top_w = rect.width() * 0.05;
    let pool_w = rect.height() * 0.42;
    b.polygon(
        &[
            Pos2::new(cx - top_w, rect.top()),
            Pos2::new(cx + top_w, rect.top()),
            Pos2::new(cx + pool_w, floor_y),
            Pos2::new(cx - pool_w, floor_y),
        ],
        with_alpha(light, 0.07 * swell),
    );
    b.radial(
        rect,
        Pos2::new(cx, floor_y),
        Vec2::new(pool_w * 1.15, rect.height() * 0.07),
        &[
            (0.0, with_alpha(light, 0.42 * swell)),
            (0.6, with_alpha(light, 0.14 * swell)),
            (1.0, with_alpha(light, 0.0)),
        ],
        8,
    );
    // The floor's edge, lit.
    b.hline(
        rect.left(),
        rect.right(),
        floor_y,
        s.s(1.0),
        with_alpha(light, 0.35),
    );
    // Footlights along the front, glowing brighter on a strong beat.
    let n = 9;
    let y = rect.bottom() - rect.height() * 0.045;
    for i in 0..n {
        let u = (i as f32 + 0.5) / n as f32;
        let p = Pos2::new(rect.left() + rect.width() * u, y);
        let glow = if playing { 0.25 + 0.55 * accent } else { 0.2 };
        b.radial(
            rect,
            p,
            Vec2::splat(s.s(9.0)),
            &[(0.0, with_alpha(gold, glow)), (1.0, with_alpha(gold, 0.0))],
            6,
        );
        b.circle(
            p,
            s.s(1.6),
            Fill::Solid(with_alpha(gold, 0.55 + 0.45 * glow)),
        );
    }
}

/// The monitor's words over the stage: bar and beat at the top left, the move playing at
/// the top right, who dances at the bottom left (and a line when it could not be found).
pub fn hud(b: &mut Brush, rect: Rect, s: &Style, d: &LiveDancer) {
    let ink = crate::kit::skin::display_ink(s);
    let pad = s.s(8.0);
    let row = s.s(16.0);
    let top = Rect::from_min_size(
        rect.min + Vec2::splat(pad),
        Vec2::new(rect.width() * 0.5 - pad, row),
    );
    let beat = if d.pose.playing {
        tf(
            "dance.hud.bar_beat",
            &[
                ("bar", &(d.pose.bar + 1)),
                ("beat", &(d.pose.beat_in_bar + 1)),
            ],
        )
    } else {
        t("dance.hud.stopped")
    };
    let _ = ktext::paint(
        b,
        top,
        &Text::new(&beat, Role::Value, ink.text)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(ktext::Elide::End)),
        s.tk(),
    );
    let mv = Rect::from_min_max(
        Pos2::new(rect.center().x, rect.top() + pad),
        Pos2::new(rect.right() - pad, rect.top() + pad + row),
    );
    let _ = ktext::paint(
        b,
        mv,
        &Text::new(&t(d.pose.mv.label_key()), Role::Label, ink.trace)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(ktext::Elide::End)),
        s.tk(),
    );
    let who = super::character_name(&d.character);
    let line = match (&d.missing, d.loading) {
        (Some(id), _) => tf(
            "dance.hud.missing",
            &[("name", &id), ("why", &d.why.clone().unwrap_or_default())],
        ),
        (None, true) => t("dance.hud.loading"),
        (None, false) => who,
    };
    let bottom = Rect::from_min_max(
        Pos2::new(rect.left() + pad, rect.bottom() - pad - row),
        Pos2::new(rect.right() - pad, rect.bottom() - pad),
    );
    let _ = ktext::paint(
        b,
        bottom,
        &Text::new(
            &line,
            Role::Body,
            if d.missing.is_some() {
                s.t.highlight
            } else {
                ink.text
            },
        )
        .caps(false)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(ktext::Elide::End)),
        s.tk(),
    );
}
