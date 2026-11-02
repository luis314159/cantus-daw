//! The same pose gives the same pixels on every system (ADR 0031 R9, ADR 0090): this crate
//! never builds a rotation with tiny-skia's own constructors (they call the platform's
//! sine and cosine); `rig::rotation` builds the same matrix from `cantus_dsp::math`.

use cantus_dance::rig;
use std::path::Path;

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("the source folder") {
        let p = entry.expect("an entry").path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&p).expect("a source file");
            out.push((p.display().to_string(), text));
        }
    }
}

#[test]
fn no_rotation_comes_from_tiny_skia() {
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join(dir), &mut files);
    }
    assert!(files.len() > 10, "the crate's sources were read");
    let banned = ["from_rotate", "pre_rotate", "post_rotate"];
    for (path, text) in &files {
        for (n, line) in text.lines().enumerate() {
            for b in banned {
                assert!(
                    !line.contains(b),
                    "{path}:{}: `{b}` calls the platform's sin and cos — use rig::rotation",
                    n + 1
                );
            }
        }
    }
}

#[test]
// The reference is what tiny-skia computes: the platform's sin and cos, on purpose.
#[allow(clippy::disallowed_methods)]
fn the_rotation_is_tiny_skias_matrix() {
    for deg in [
        -370.0f32, -90.0, -14.0, 0.0, 7.5, 45.0, 90.0, 180.0, 225.0, 359.0,
    ] {
        let ours = rig::rotation(deg);
        let theirs = tiny_skia::Transform::from_row(
            deg.to_radians().cos(),
            deg.to_radians().sin(),
            -deg.to_radians().sin(),
            deg.to_radians().cos(),
            0.0,
            0.0,
        );
        for (a, b) in [
            (ours.sx, theirs.sx),
            (ours.ky, theirs.ky),
            (ours.kx, theirs.kx),
            (ours.sy, theirs.sy),
        ] {
            assert!((a - b).abs() < 1e-5, "{deg}°: {a} vs {b}");
        }
        let at = rig::rotation_at(deg, 0.3, -0.7);
        // The centre stays put.
        let (x, y) = (
            at.sx * 0.3 + at.kx * -0.7 + at.tx,
            at.ky * 0.3 + at.sy * -0.7 + at.ty,
        );
        assert!((x - 0.3).abs() < 1e-5 && (y + 0.7).abs() < 1e-5);
    }
}
