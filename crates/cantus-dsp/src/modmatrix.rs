//! The modulation matrix every native instrument evaluates (ADR 0023): routes from source
//! values to target offsets, fixed capacity, no allocation.
//!
//! The model (which source tags and target keys a route names) lives in
//! `cantus_core::inst::modulation`; an instrument resolves those names to its own source and
//! target indices when it is configured, and hands this module plain numbers. A route adds
//! `shape(source) · depth · via` to its target, where depth is a fraction of the target's
//! range — the instrument scales it into the target's unit.

/// Routes an instrument can hold at once (`cantus_core::inst::modulation::MOD_SLOTS`).
pub const MAX_ROUTES: usize = 32;

/// How a source's value is shaped before it is scaled.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ModCurve {
    #[default]
    Linear,
    /// Slow start, fast finish (x², sign kept).
    Exp,
    /// Fast start, slow finish.
    Log,
    /// Smoothstep.
    S,
    /// Eight steps.
    Steps,
}

impl ModCurve {
    pub fn from_index(i: f32) -> ModCurve {
        match i.round() as i32 {
            1 => ModCurve::Exp,
            2 => ModCurve::Log,
            3 => ModCurve::S,
            4 => ModCurve::Steps,
            _ => ModCurve::Linear,
        }
    }

    /// Shape `x` (−1…1 or 0…1), keeping its sign; 0 stays 0 and ±1 stays ±1.
    #[inline]
    pub fn shape(self, x: f32) -> f32 {
        let (s, a) = (x.signum(), x.abs().min(1.0));
        let y = match self {
            ModCurve::Linear => return x,
            ModCurve::Exp => a * a,
            ModCurve::Log => 1.0 - (1.0 - a) * (1.0 - a),
            ModCurve::S => a * a * (3.0 - 2.0 * a),
            ModCurve::Steps => (a * 8.0).floor() / 8.0,
        };
        s * y
    }
}

/// One resolved connection.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ModRoute {
    /// Index into the instrument's source values.
    pub src: u16,
    /// Index into the instrument's target offsets.
    pub dst: u16,
    /// −1…1: a fraction of the target's range.
    pub depth: f32,
    /// A source whose value (0…1 or −1…1) multiplies the depth.
    pub via: Option<u16>,
    pub curve: ModCurve,
}

/// A fixed set of routes.
#[derive(Clone, Debug)]
pub struct ModMatrix {
    routes: [ModRoute; MAX_ROUTES],
    len: usize,
}

impl Default for ModMatrix {
    fn default() -> Self {
        ModMatrix {
            routes: [ModRoute::default(); MAX_ROUTES],
            len: 0,
        }
    }
}

impl ModMatrix {
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Add a route; `false` when full or when its depth is zero (a route that changes
    /// nothing is not evaluated — "a control at its default touches no sample").
    pub fn push(&mut self, route: ModRoute) -> bool {
        if self.len == MAX_ROUTES || route.depth == 0.0 {
            return false;
        }
        self.routes[self.len] = route;
        self.len += 1;
        true
    }

    pub fn routes(&self) -> &[ModRoute] {
        &self.routes[..self.len]
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether any route lands on `dst`.
    pub fn targets(&self, dst: u16) -> bool {
        self.routes().iter().any(|r| r.dst == dst)
    }

    /// Add every route's contribution into `out` (indexed by target). Sources or targets
    /// out of range are skipped rather than panicking.
    #[inline]
    pub fn apply(&self, sources: &[f32], out: &mut [f32]) {
        for r in self.routes() {
            let (Some(&x), Some(o)) = (sources.get(r.src as usize), out.get_mut(r.dst as usize))
            else {
                continue;
            };
            let via = match r.via {
                Some(v) => sources.get(v as usize).copied().unwrap_or(0.0),
                None => 1.0,
            };
            *o += r.curve.shape(x) * r.depth * via;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_keep_the_ends_and_the_sign() {
        for c in [
            ModCurve::Linear,
            ModCurve::Exp,
            ModCurve::Log,
            ModCurve::S,
            ModCurve::Steps,
        ] {
            assert_eq!(c.shape(0.0), 0.0, "{c:?}");
            assert_eq!(c.shape(1.0), 1.0, "{c:?}");
            assert_eq!(c.shape(-1.0), -1.0, "{c:?}");
            assert!(c.shape(-0.5) <= 0.0, "{c:?}");
        }
    }

    #[test]
    fn routes_add_scaled_by_depth_and_via() {
        let mut m = ModMatrix::default();
        assert!(!m.push(ModRoute {
            depth: 0.0,
            ..Default::default()
        }));
        m.push(ModRoute {
            src: 0,
            dst: 1,
            depth: 0.5,
            via: None,
            curve: ModCurve::Linear,
        });
        m.push(ModRoute {
            src: 1,
            dst: 1,
            depth: -1.0,
            via: Some(2),
            curve: ModCurve::Linear,
        });
        let mut out = [0.0; 2];
        m.apply(&[1.0, 0.5, 0.5], &mut out);
        assert_eq!(out, [0.0, 0.5 - 0.25]);
        assert!(m.targets(1) && !m.targets(0));
    }
}
