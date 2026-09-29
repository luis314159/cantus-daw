//! The repaint ticker (ADR 0024 §13): **one** place decides when the next frame is
//! drawn. Anything that moves on screen — the playhead, a meter still falling, an
//! animation, a drag — says so with [`moving`]; at the end of the frame the application
//! calls [`finish`], which asks egui for **one** repaint within 1/60 s if anything moved,
//! a single wake-up at the nearest deadline if something is merely due later (an autosave),
//! and **nothing at all** when the window is idle. Input wakes egui by itself, so an idle
//! window costs no frames.
//!
//! Views that used to call `ctx.request_repaint()` every frame should call [`moving`]
//! instead: the effect is the same while something moves, but the rate is capped at 60 Hz
//! and the reasons are counted, so a test can prove that an idle frame asks for none.

use egui::{Context, Id};
use std::time::Duration;

/// The fastest the ticker asks for frames: 60 per second (rounded up, so never faster).
pub const FRAME: Duration = Duration::from_millis(17);
/// Hosted plug-ins' main-thread callbacks are pumped at 30 Hz: often enough for CLAP's
/// `request_callback`, half the cost of the display rate.
pub const PUMP: Duration = Duration::from_millis(33);

/// Why a frame is wanted.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Reason {
    /// The transport runs: the playhead and the clock move.
    Playback = 1,
    /// A take is being recorded.
    Recording = 1 << 1,
    /// A meter still shows a level (it falls back to silence over a few frames).
    Meters = 1 << 2,
    /// An animation is running (a lamp pulsing, a panel easing).
    Animation = 1 << 3,
    /// Something is being dragged.
    Drag = 1 << 4,
    /// A note is sounding on its own (an audition), to be released on time.
    Audition = 1 << 5,
    /// Work runs in the background and its result must be picked up (an export, a scan).
    Background = 1 << 6,
    /// Hosted plug-ins are loaded and their callbacks must be served.
    Plugins = 1 << 7,
    /// A device editor shows live data (an analyser, a gain-reduction trace).
    Live = 1 << 8,
}

/// What the ticker saw in one frame.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// The reasons given this frame, as bits of [`Reason`].
    pub reasons: u16,
    /// The nearest deadline anybody asked to be woken at.
    pub wake: Option<Duration>,
    /// The repaint the ticker asked egui for, if any.
    pub requested: Option<Duration>,
}

impl Tick {
    pub fn has(&self, r: Reason) -> bool {
        self.reasons & r as u16 != 0
    }

    /// Nothing moved and nothing is due: no frame was asked for.
    pub fn idle(&self) -> bool {
        self.requested.is_none()
    }
}

#[derive(Clone, Default)]
struct Slot {
    pass: u64,
    now: Tick,
    last: Tick,
    /// Repaints requested since the context was made (for tests and the stats line).
    total: u64,
}

fn id() -> Id {
    Id::new("cantus-ticker")
}

fn with<R>(ctx: &Context, f: impl FnOnce(&mut Slot, u64) -> R) -> R {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Slot>(id());
        if s.pass != pass {
            s.pass = pass;
            s.now = Tick::default();
        }
        f(s, pass)
    })
}

/// Something on screen moves this frame: draw the next one soon (≤ 60 Hz).
pub fn moving(ctx: &Context, why: Reason) {
    with(ctx, |s, _| s.now.reasons |= why as u16);
}

/// Wake once after `after` — a deadline, not a steady tick (an autosave that falls due).
pub fn wake_after(ctx: &Context, after: Duration) {
    with(ctx, |s, _| {
        s.now.wake = Some(s.now.wake.map_or(after, |w| w.min(after)));
    });
}

/// End the frame: ask for the one repaint this frame's reasons need, and none when idle.
/// Returns what was decided.
pub fn finish(ctx: &Context) -> Tick {
    let tick = with(ctx, |s, _| {
        let mut t = s.now;
        let rate = if t.reasons == 0 {
            None
        } else if t.reasons == Reason::Plugins as u16 {
            Some(PUMP)
        } else {
            Some(FRAME)
        };
        t.requested = match (rate, t.wake) {
            (Some(r), Some(w)) => Some(r.min(w)),
            (Some(r), None) => Some(r),
            (None, w) => w,
        };
        if t.requested.is_some() {
            s.total += 1;
        }
        s.last = t;
        t
    });
    if let Some(d) = tick.requested {
        ctx.request_repaint_after(d);
    }
    tick
}

/// What the last finished frame decided.
pub fn last(ctx: &Context) -> Tick {
    ctx.data(|d| d.get_temp::<Slot>(id()).map(|s| s.last).unwrap_or_default())
}

/// Repaints the ticker has asked for since the context was made.
pub fn requests(ctx: &Context) -> u64 {
    ctx.data(|d| d.get_temp::<Slot>(id()).map_or(0, |s| s.total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, f: impl Fn(&Context)) -> (Tick, Duration) {
        let mut out = ctx.run_ui(Default::default(), |ui| {
            f(ui.ctx());
            finish(ui.ctx());
        });
        out.textures_delta.clear();
        let delay = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        (last(ctx), delay)
    }

    #[test]
    fn an_idle_frame_asks_for_no_repaint_and_a_moving_one_for_sixty_hertz() {
        let ctx = Context::default();
        // egui paints its first frames on its own account; let them pass.
        for _ in 0..3 {
            frame(&ctx, |_| {});
        }
        let (t, delay) = frame(&ctx, |_| {});
        assert!(t.idle(), "nothing moved, yet a frame was asked for: {t:?}");
        assert_eq!(
            delay,
            Duration::MAX,
            "an idle frame must not schedule another"
        );

        let (t, delay) = frame(&ctx, |c| moving(c, Reason::Playback));
        assert!(t.has(Reason::Playback));
        assert_eq!(t.requested, Some(FRAME), "never faster than 60 Hz");
        // egui reports the delay less the time the frame itself took.
        assert!(delay <= FRAME, "a frame is scheduled");

        let (t, _) = frame(&ctx, |c| wake_after(c, Duration::from_secs(5)));
        assert_eq!(
            t.requested,
            Some(Duration::from_secs(5)),
            "a deadline, once"
        );
        let (t, _) = frame(&ctx, |_| {});
        assert!(t.idle(), "the reasons do not outlive their frame");
        assert!(requests(&ctx) >= 2);
    }
}
