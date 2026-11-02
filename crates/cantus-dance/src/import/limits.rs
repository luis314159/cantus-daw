//! Every size an import may reach, checked before memory is given to it (ADR 0089 §5).
//!
//! A JSON file is measured by [`prescan`] — its bytes, how deep it nests and how many
//! values it holds — before `serde_json` builds anything from it; images are measured from
//! their headers before they are decoded; a zip from its central directory and again while
//! it inflates. The counts of what a format builds (layers, keyframes, bones…) are checked
//! as the model is made, by a [`Budget`].

use super::ImportError;

/// The bounds of ADR 0089 §5. One table; tests check each at its edge.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Limits {
    /// A JSON document's bytes.
    pub json_bytes: usize,
    /// How deep arrays and objects nest.
    pub json_depth: usize,
    /// How many values (numbers, strings, arrays, objects…) a document holds.
    pub json_values: usize,
    /// A zip package (`.lottie`): its bytes, entries, and bytes once inflated, in all.
    pub zip_bytes: u64,
    pub zip_entries: usize,
    pub zip_inflated: u64,
    /// An image file's bytes, its side, and the pixels of every image of a character.
    pub image_bytes: usize,
    pub image_side: u32,
    pub image_pixels_total: u64,
    /// Lottie.
    pub layers: usize,
    pub precomp_depth: usize,
    pub shape_items: usize,
    pub keyframes: usize,
    pub path_vertices: usize,
    pub timeline_frames: f64,
    pub repeater_copies: usize,
    pub masks: usize,
    /// Markers (each one a clip the card lists and the box samples).
    pub markers: usize,
    /// Sprite sheets.
    pub frames: usize,
    pub tags: usize,
    /// DragonBones.
    pub bones: usize,
    pub slots: usize,
    pub mesh_vertices: usize,
    pub mesh_triangles: usize,
    pub animations: usize,
    pub armature_depth: usize,
    /// DragonBones: slots, image corners, mesh vertices and triangles one frame may draw,
    /// every nested armature counted each time it is shown.
    pub frame_work: u64,
    /// A name or a text, in bytes (longer ones are cut).
    pub name_bytes: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        json_bytes: 16 << 20,
        json_depth: 96,
        json_values: 4_000_000,
        zip_bytes: 32 << 20,
        zip_entries: 256,
        zip_inflated: 64 << 20,
        image_bytes: 32 << 20,
        image_side: 4096,
        image_pixels_total: 32 << 20,
        layers: 2000,
        precomp_depth: 16,
        shape_items: 50_000,
        keyframes: 200_000,
        path_vertices: 500_000,
        timeline_frames: 100_000.0,
        repeater_copies: 1000,
        masks: 256,
        markers: 256,
        frames: 4096,
        tags: 256,
        bones: 1000,
        slots: 1000,
        mesh_vertices: 65_536,
        mesh_triangles: 131_072,
        animations: 256,
        armature_depth: 4,
        frame_work: 2_000_000,
        name_bytes: 256,
    };
}

impl Default for Limits {
    fn default() -> Limits {
        Limits::DEFAULT
    }
}

/// What [`prescan`] measured.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct JsonSize {
    pub bytes: usize,
    pub depth: usize,
    pub values: usize,
}

/// Measure a JSON document without building it: bytes, nesting and values, refused past
/// the limits. It does not validate the JSON (the parser does); it only makes sure parsing
/// it cannot take more than the limits allow.
pub fn prescan(bytes: &[u8], limits: &Limits) -> Result<JsonSize, ImportError> {
    if bytes.len() > limits.json_bytes {
        return Err(ImportError::TooLarge(format!(
            "{} bytes of JSON, at most {}",
            bytes.len(),
            limits.json_bytes
        )));
    }
    let (mut depth, mut max_depth, mut values) = (0usize, 0usize, 0usize);
    let mut in_string = false;
    let mut escaped = false;
    // A value starts at `{`, `[`, `"`, or the first byte of a number/literal after a
    // separator; counting starts and commas closely enough bounds the parser's nodes.
    let mut in_scalar = false;
    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => {
                in_string = true;
                in_scalar = false;
                values += 1;
            }
            b'{' | b'[' => {
                in_scalar = false;
                values += 1;
                depth += 1;
                max_depth = max_depth.max(depth);
                if depth > limits.json_depth {
                    return Err(ImportError::TooLarge(format!(
                        "JSON nested more than {} deep",
                        limits.json_depth
                    )));
                }
            }
            b'}' | b']' => {
                in_scalar = false;
                depth = depth.saturating_sub(1);
            }
            b',' | b':' | b' ' | b'\n' | b'\r' | b'\t' => in_scalar = false,
            _ => {
                if !in_scalar {
                    values += 1;
                    in_scalar = true;
                }
            }
        }
        if values > limits.json_values {
            return Err(ImportError::TooLarge(format!(
                "more than {} values in the JSON",
                limits.json_values
            )));
        }
    }
    Ok(JsonSize {
        bytes: bytes.len(),
        depth: max_depth,
        values,
    })
}

/// Measure, then parse.
pub fn parse_json(bytes: &[u8], limits: &Limits) -> Result<serde_json::Value, ImportError> {
    prescan(bytes, limits)?;
    serde_json::from_slice(bytes).map_err(|e| ImportError::Invalid(format!("JSON: {e}")))
}

/// Counts what a model builds against its limits ("more than 2000 layers").
#[derive(Debug)]
pub struct Budget {
    used: Vec<(&'static str, usize, usize)>,
}

impl Budget {
    pub fn new() -> Budget {
        Budget { used: Vec::new() }
    }

    /// Take `n` of `what` (bounded by `max`); refused once past it.
    pub fn take(&mut self, what: &'static str, n: usize, max: usize) -> Result<(), ImportError> {
        let entry = match self.used.iter_mut().find(|e| e.0 == what) {
            Some(e) => e,
            None => {
                self.used.push((what, 0, max));
                self.used.last_mut().ok_or(ImportError::Unsupported)?
            }
        };
        entry.1 = entry.1.saturating_add(n);
        if entry.1 > entry.2 {
            return Err(ImportError::TooLarge(format!(
                "more than {} {what}",
                entry.2
            )));
        }
        Ok(())
    }

    pub fn used(&self, what: &str) -> usize {
        self.used
            .iter()
            .find(|e| e.0 == what)
            .map(|e| e.1)
            .unwrap_or(0)
    }
}

impl Default for Budget {
    fn default() -> Budget {
        Budget::new()
    }
}

/// A name cut to the limit at a character boundary.
pub fn name(s: &str, limits: &Limits) -> String {
    if s.len() <= limits.name_bytes {
        return s.to_string();
    }
    let mut end = limits.name_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_at_the_limit_passes_and_one_past_is_refused() {
        let l = Limits::DEFAULT;
        let ok = format!("{}{}", "[".repeat(l.json_depth), "]".repeat(l.json_depth));
        assert_eq!(prescan(ok.as_bytes(), &l).unwrap().depth, l.json_depth);
        let deep = format!(
            "{}{}",
            "[".repeat(l.json_depth + 1),
            "]".repeat(l.json_depth + 1)
        );
        assert!(matches!(
            prescan(deep.as_bytes(), &l),
            Err(ImportError::TooLarge(_))
        ));
    }

    #[test]
    fn brackets_inside_strings_do_not_count() {
        let l = Limits::DEFAULT;
        let s = format!("{{\"a\":\"{}\\\"{}\"}}", "[".repeat(500), "{".repeat(500));
        let m = prescan(s.as_bytes(), &l).unwrap();
        assert_eq!(m.depth, 1);
        assert_eq!(m.values, 3);
    }

    #[test]
    fn bytes_and_values_are_bounded() {
        let l = Limits {
            json_bytes: 100,
            json_values: 10,
            ..Limits::DEFAULT
        };
        assert!(prescan(&[b' '; 101], &l).is_err());
        assert!(prescan(b"[1,2,3,4,5,6,7,8,9]", &l).is_ok());
        assert!(prescan(b"[1,2,3,4,5,6,7,8,9,10]", &l).is_err());
    }

    #[test]
    fn a_budget_refuses_one_past() {
        let mut b = Budget::new();
        b.take("layers", 2000, 2000).unwrap();
        assert!(b.take("layers", 1, 2000).is_err());
        assert_eq!(b.used("bones"), 0);
    }

    #[test]
    fn names_are_cut_at_a_character_boundary() {
        let l = Limits {
            name_bytes: 5,
            ..Limits::DEFAULT
        };
        assert_eq!(name("añoñoño", &l), "año");
    }
}
