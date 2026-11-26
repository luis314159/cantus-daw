//! r42:arcus-polish3 — the concert light for the realistic figures (the owner, 2026-11-25:
//! "Aún le falta lo de la luz, compara con las imágenes reales"; earlier "una iluminación
//! más de presentación de orquesta, algo dramático, elegante y profesional, con sombras y
//! contrastes"). Measured against the photographs he chose (ref-04, ref-06, ref-10:
//! `tools/arcus-figures/light_measure.py`, the bands in `tests/arcus_light_measure.rs`):
//!
//! - **the exposure is the musician's**: a stage spot high and to the side from the
//!   audience (the key), narrow, so the face, the hands and the violin are bright and warm
//!   while the hall falls dark; the face has a lit side and a shadow side;
//! - **a back light** from above and behind on each side: the rim on the hair, the
//!   shoulders, the sleeves and the violin's edge;
//! - **real cast shadows** under a small source (the violin on the neck and the chest);
//! - **a filmic curve** with deep blacks, bright highlights and a warm grade;
//! - **the hall** behind: out of focus, warm, dark — players, stands, their lights.
//!
//! One set of numbers lights both pictures (the Photographic bake and the live arms, and
//! the Drawn picture's own graphic version of it). `CANTUS_CONCERT="key_az=-40,exposure=1.4"`
//! overrides any of them for a process (design sheets: the light tuned without a rebuild;
//! arrays as `a/b/c`). Never set by the app.

use crate::scene::geo::{v, V3};

/// The concert light's numbers. Angles in degrees round the subject from the camera
/// (azimuth: 0 the camera's side, positive toward the camera's right, ±180 behind the
/// subject; elevation over the horizontal), distances in cm.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Concert {
    /// The key: a stage spot (its place, its half size — the penumbra —, its light at the
    /// subject, its warmth 0 white … 1 tungsten, its pool's radius at the subject and how
    /// far past it the light still falls, as a multiple).
    pub key_az: f32,
    pub key_el: f32,
    pub key_dist: f32,
    pub key_size: f32,
    pub key_e: f32,
    pub key_warm: f32,
    pub key_r: f32,
    pub key_feather: f32,
    /// How bright its face looks in a gloss (the varnish's spot highlight).
    pub key_face: f32,
    /// The fill from the audience's side (its way, its light).
    pub fill_az: f32,
    pub fill_el: f32,
    pub fill: f32,
    /// The ambient left in the dark.
    pub amb: f32,
    /// The two back lights (above and behind, one each side): their ways, their light,
    /// their warmth, their faces in a gloss.
    pub back_az: f32,
    pub back_el: f32,
    pub back_e: f32,
    pub back2_az: f32,
    pub back2_el: f32,
    pub back2_e: f32,
    pub back_warm: f32,
    pub back_face: f32,
    /// Skin: its terminator's wrap, the camera side's soft fill on it, the warm light under
    /// it where the key falls off, its share of the rim, its albedo's grade (linear gains).
    pub skin_wrap: f32,
    pub skin_front: f32,
    pub skin_sss: f32,
    pub skin_rim: f32,
    pub skin_gain: [f32; 3],
    /// Hair: its terminator's wrap and its share of the rim.
    pub hair_wrap: f32,
    pub hair_rim: f32,
    /// Black cloth: the velvet's back light along its edge and its pile; the wool's and the
    /// velvet's broad sheen from the key along their folds (strength, tightness).
    pub velvet_back: f32,
    pub velvet_pile: f32,
    pub velvet_spec: f32,
    /// The back lights' line along black cloth's edge (its sheen at grazing).
    pub cloth_rim: f32,
    pub wool_spec: f32,
    pub cloth_pow: f32,
    /// The picture: exposure, the S-curve's contrast (1 none), the blacks' toe, the
    /// saturation, the warm grade (linear gains), the vignette, the hall's blur (× the
    /// portrait's), the hall's light and its bokeh's.
    pub exposure: f32,
    pub contrast: f32,
    pub toe: f32,
    pub sat: f32,
    pub grade: [f32; 3],
    pub vig: f32,
    pub blur: f32,
    pub hall: f32,
    pub bokeh: f32,
    /// The orchestra's own wash (× the hall's).
    pub rows: f32,
    /// Wool's pale sheen where it turns from the eye (× the studio's).
    pub wool_sheen: f32,
    /// A violin's varnish under the spots: its colour (gains over the wood's), its coat,
    /// its roughness.
    pub varnish: [f32; 3],
    pub varnish_coat: f32,
    pub varnish_rough: f32,
    /// The Drawn picture's stage: its hall's brightness, its ambient, its key, the
    /// terminator's width (n·l), the rim's strength.
    pub d_hall: f32,
    pub d_amb: f32,
    pub d_key: f32,
    pub d_term: f32,
    pub d_rim: f32,
}

/// The concert light as tuned against ref-04, ref-06 and ref-10.
pub const CONCERT: Concert = Concert {
    key_az: -45.0,
    key_el: 45.0,
    key_dist: 420.0,
    key_size: 12.0,
    key_e: 1.75,
    key_warm: 0.22,
    key_r: 70.0,
    key_feather: 2.2,
    key_face: 30.0,
    fill_az: 25.0,
    fill_el: 15.0,
    fill: 0.32,
    amb: 0.025,
    back_az: 150.0,
    back_el: 25.0,
    back_e: 1.5,
    back2_az: -150.0,
    back2_el: 30.0,
    back2_e: 0.9,
    back_warm: 0.25,
    back_face: 12.0,
    skin_wrap: 0.1,
    skin_front: 0.02,
    skin_sss: 0.2,
    skin_rim: 0.5,
    skin_gain: [1.0, 0.93, 0.86],
    hair_wrap: 0.3,
    hair_rim: 1.0,
    velvet_back: 1.3,
    velvet_pile: 0.45,
    velvet_spec: 0.08,
    cloth_rim: 0.35,
    wool_spec: 0.05,
    cloth_pow: 16.0,
    exposure: 1.1,
    contrast: 1.25,
    toe: 0.0,
    sat: 1.05,
    grade: [1.03, 1.0, 0.93],
    vig: 0.45,
    blur: 2.2,
    hall: 0.6,
    bokeh: 4.0,
    rows: 3.0,
    wool_sheen: 0.35,
    varnish: [0.40, 0.17, 0.08],
    varnish_coat: 0.7,
    varnish_rough: 0.18,
    d_hall: 1.0,
    d_amb: 0.035,
    d_key: 1.2,
    d_term: 0.5,
    d_rim: 0.5,
};

static TUNED: std::sync::OnceLock<Concert> = std::sync::OnceLock::new();

/// The concert light this process draws with: [`CONCERT`], or `CANTUS_CONCERT`'s overrides.
pub fn get() -> Concert {
    *TUNED.get_or_init(|| match std::env::var("CANTUS_CONCERT") {
        Ok(s) => tuned(CONCERT, &s),
        Err(_) => CONCERT,
    })
}

/// `base` with `spec`'s `name=value` pairs (comma-separated; arrays `a/b/c`) set; an
/// unknown name or a bad value is left out.
pub fn tuned(base: Concert, spec: &str) -> Concert {
    let mut c = base;
    for kv in spec.split(',') {
        let Some((k, val)) = kv.split_once('=') else {
            continue;
        };
        let nums: Vec<f32> = val
            .split('/')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        let Some(&x) = nums.first() else {
            continue;
        };
        let arr = |a: &mut [f32; 3]| {
            if nums.len() == 3 {
                *a = [nums[0], nums[1], nums[2]];
            }
        };
        match k.trim() {
            "key_az" => c.key_az = x,
            "key_el" => c.key_el = x,
            "key_dist" => c.key_dist = x,
            "key_size" => c.key_size = x,
            "key_e" => c.key_e = x,
            "key_warm" => c.key_warm = x,
            "key_r" => c.key_r = x,
            "key_feather" => c.key_feather = x,
            "key_face" => c.key_face = x,
            "fill_az" => c.fill_az = x,
            "fill_el" => c.fill_el = x,
            "fill" => c.fill = x,
            "amb" => c.amb = x,
            "back_az" => c.back_az = x,
            "back_el" => c.back_el = x,
            "back_e" => c.back_e = x,
            "back2_az" => c.back2_az = x,
            "back2_el" => c.back2_el = x,
            "back2_e" => c.back2_e = x,
            "back_warm" => c.back_warm = x,
            "back_face" => c.back_face = x,
            "skin_wrap" => c.skin_wrap = x,
            "skin_front" => c.skin_front = x,
            "skin_sss" => c.skin_sss = x,
            "skin_rim" => c.skin_rim = x,
            "skin_gain" => arr(&mut c.skin_gain),
            "hair_wrap" => c.hair_wrap = x,
            "hair_rim" => c.hair_rim = x,
            "velvet_back" => c.velvet_back = x,
            "velvet_pile" => c.velvet_pile = x,
            "velvet_spec" => c.velvet_spec = x,
            "cloth_rim" => c.cloth_rim = x,
            "wool_spec" => c.wool_spec = x,
            "cloth_pow" => c.cloth_pow = x,
            "exposure" => c.exposure = x,
            "contrast" => c.contrast = x,
            "toe" => c.toe = x,
            "sat" => c.sat = x,
            "grade" => arr(&mut c.grade),
            "vig" => c.vig = x,
            "blur" => c.blur = x,
            "hall" => c.hall = x,
            "bokeh" => c.bokeh = x,
            "rows" => c.rows = x,
            "wool_sheen" => c.wool_sheen = x,
            "d_hall" => c.d_hall = x,
            "varnish" => arr(&mut c.varnish),
            "varnish_coat" => c.varnish_coat = x,
            "varnish_rough" => c.varnish_rough = x,
            "d_amb" => c.d_amb = x,
            "d_key" => c.d_key = x,
            "d_term" => c.d_term = x,
            "d_rim" => c.d_rim = x,
            _ => {}
        }
    }
    c
}

/// The way from `aim` toward a light at azimuth `az` and elevation `el` (degrees) round the
/// subject as the camera at `eye` sees it (unit).
pub fn way(aim: V3, eye: V3, az: f32, el: f32) -> V3 {
    let mut t = v(eye.x - aim.x, 0.0, eye.z - aim.z);
    if t.len() < 1e-3 {
        t = v(0.0, 0.0, 1.0);
    }
    let toward = t.norm();
    // The camera's right as it looks at the subject (toward is from the subject to it).
    let right = v(toward.z, 0.0, -toward.x);
    let (a, e) = (az.to_radians(), el.to_radians());
    let h = toward * a.cos() + right * a.sin();
    (h * e.cos() + v(0.0, 1.0, 0.0) * e.sin()).norm()
}

/// A light's colour: white toward tungsten by `warm` (linear gains, the brightest 1).
pub fn warmth(warm: f32) -> [f32; 3] {
    let w = warm.clamp(0.0, 1.0);
    [1.0, 1.0 - 0.2 * w, 1.0 - 0.42 * w]
}

impl Concert {
    /// The key's place for a subject at `aim` seen from `eye` (cm).
    pub fn key_pos(&self, aim: V3, eye: V3) -> V3 {
        aim + way(aim, eye, self.key_az, self.key_el) * self.key_dist
    }

    /// The key's light at the subject (linear).
    pub fn key_light(&self) -> [f32; 3] {
        warmth(self.key_warm).map(|c| c * self.key_e)
    }

    /// The fill's way and light.
    pub fn fill_light(&self, aim: V3, eye: V3) -> (V3, [f32; 3]) {
        (
            way(aim, eye, self.fill_az, self.fill_el),
            warmth(0.3).map(|c| c * self.fill),
        )
    }

    /// The back lights' ways and lights.
    pub fn backs(&self, aim: V3, eye: V3) -> [(V3, [f32; 3]); 2] {
        let c = warmth(self.back_warm);
        [
            (
                way(aim, eye, self.back_az, self.back_el),
                c.map(|x| x * self.back_e),
            ),
            (
                way(aim, eye, self.back2_az, self.back2_el),
                c.map(|x| x * self.back2_e),
            ),
        ]
    }

    /// The ambient's light (linear): the hall's warm dark.
    pub fn ambient(&self) -> [f32; 3] {
        [self.amb, self.amb * 0.93, self.amb * 0.84]
    }

    /// Scene light (linear) to the display's linear 0 … 1: the exposure and the warm grade,
    /// the filmic shoulder (Narkowicz's ACES fit) on the luminance, an S-curve for contrast
    /// round the display's middle, the blacks' toe, the hue kept with the saturation, the
    /// brightest rolling to white as film's do.
    pub fn tone(&self, c: [f32; 3]) -> [f32; 3] {
        let g = self.grade;
        let c = [
            c[0].max(0.0) * g[0] * self.exposure,
            c[1].max(0.0) * g[1] * self.exposure,
            c[2].max(0.0) * g[2] * self.exposure,
        ];
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        if l <= 1e-7 {
            return [0.0; 3];
        }
        let curve = |x: f32| {
            let a = aces(x);
            let s = a.powf(1.0 / 2.2);
            let p = self.contrast.max(0.2);
            let (u, w) = (s.powf(p), (1.0 - s).max(0.0).powf(p));
            let s = u / (u + w).max(1e-6);
            ((s.powf(2.2) - self.toe) / (1.0 - self.toe)).clamp(0.0, 1.0)
        };
        let lm = curve(l);
        let k = lm / l;
        let kept: [f32; 3] =
            std::array::from_fn(|i| (lm + (c[i] * k - lm) * self.sat).clamp(0.0, 1.0));
        let per = c.map(curve);
        let w = ((lm - 0.6) / 0.4).clamp(0.0, 1.0);
        std::array::from_fn(|i| kept[i] + (per[i] - kept[i]) * w * w)
    }
}

/// The filmic curve (Narkowicz's fit of ACES).
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuned_sets_names_and_arrays() {
        let c = tuned(
            CONCERT,
            "key_az=-30, exposure=2,grade=1/0.9/0.8,nope=3,key_el=x",
        );
        assert_eq!(c.key_az, -30.0);
        assert_eq!(c.exposure, 2.0);
        assert_eq!(c.grade, [1.0, 0.9, 0.8]);
        assert_eq!(c.key_el, CONCERT.key_el);
    }

    #[test]
    fn ways_round_the_subject() {
        let aim = v(0.0, 100.0, 0.0);
        let eye = v(0.0, 100.0, 400.0);
        let w = way(aim, eye, 0.0, 0.0);
        assert!((w.z - 1.0).abs() < 1e-5, "{w:?}");
        // Positive azimuth: the camera's right (+x when it looks down -z).
        assert!(way(aim, eye, 90.0, 0.0).x > 0.99);
        assert!(way(aim, eye, 180.0, 0.0).z < -0.99);
        assert!(way(aim, eye, 0.0, 90.0).y > 0.99);
    }

    #[test]
    fn tone_is_monotonic_black_to_white() {
        let c = CONCERT;
        let mut last = -1.0;
        for i in 0..200 {
            let x = i as f32 * 0.05;
            let t = c.tone([x; 3])[1];
            assert!(t >= last - 1e-6, "{x}: {t} < {last}");
            last = t;
        }
        assert_eq!(c.tone([0.0; 3]), [0.0; 3]);
        assert!(c.tone([50.0; 3])[0] > 0.97);
    }
}
