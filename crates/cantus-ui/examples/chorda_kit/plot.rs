//! Small line plots as PNG for the comparison's report: axes, a grid, labelled ticks, lines
//! and a legend, drawn into an RGBA buffer with a 5×7 bitmap font (no dependency but `png`).

use cantus_dsp::math;
use std::path::Path;

pub type Rgb = [u8; 3];
pub const REFERENCE: Rgb = [40, 40, 40];
pub const MODEL1: Rgb = [214, 120, 30];
pub const MODEL2: Rgb = [30, 100, 200];
pub const GREEN: Rgb = [40, 150, 70];
pub const PURPLE: Rgb = [140, 60, 170];
const GRID: Rgb = [225, 225, 225];
const AXIS: Rgb = [120, 120, 120];
const TEXT: Rgb = [30, 30, 30];

fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x11, 0x1F, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        ',' => [0, 0, 0, 0, 0x0C, 0x04, 0x08],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        '/' => [0, 0x01, 0x02, 0x04, 0x08, 0x10, 0],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '=' => [0, 0, 0x1F, 0, 0x1F, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0x1F],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '\'' => [0x0C, 0x04, 0x08, 0, 0, 0, 0],
        _ => [0; 7],
    }
}

pub struct Plot {
    w: usize,
    h: usize,
    px: Vec<Rgb>,
    /// The frame's pixels: left, top, right, bottom.
    box_: (usize, usize, usize, usize),
    x: (f64, f64, bool),
    y: (f64, f64),
    legend: usize,
}

impl Plot {
    /// A plot `w × h` px titled `title`; x from `x0` to `x1` (log if `logx`), y `y0 … y1`.
    pub fn new(w: usize, h: usize, title: &str, x: (f64, f64, bool), y: (f64, f64)) -> Plot {
        let mut p = Plot {
            w,
            h,
            px: vec![[255, 255, 255]; w * h],
            box_: (70, 40, w - 20, h - 50),
            x,
            y,
            legend: 0,
        };
        p.text(70, 12, title, TEXT, 2);
        p
    }

    fn put(&mut self, x: i64, y: i64, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn text(&mut self, x: usize, y: usize, s: &str, c: Rgb, scale: usize) {
        for (i, ch) in s.chars().enumerate() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let px = x + i * 6 * scale + col * scale + dx;
                                let py = y + row * scale + dy;
                                self.put(px as i64, py as i64, c);
                            }
                        }
                    }
                }
            }
        }
    }

    fn sx(&self, v: f64) -> f64 {
        let (a, b, log) = self.x;
        let t = if log {
            (math::ln(v.max(1e-9)) - math::ln(a)) / (math::ln(b) - math::ln(a))
        } else {
            (v - a) / (b - a)
        };
        self.box_.0 as f64 + t * (self.box_.2 - self.box_.0) as f64
    }

    fn sy(&self, v: f64) -> f64 {
        let t = (v - self.y.0) / (self.y.1 - self.y.0);
        self.box_.3 as f64 - t * (self.box_.3 - self.box_.1) as f64
    }

    /// Grid lines and labels at `xs` and `ys` (shown as given, `fmt` per axis), the axes'
    /// names.
    pub fn axes(&mut self, xs: &[(f64, String)], ys: &[(f64, String)], xname: &str, yname: &str) {
        let (l, t, r, b) = self.box_;
        for (v, s) in xs {
            let x = self.sx(*v).round() as i64;
            for y in t..=b {
                self.put(x, y as i64, GRID);
            }
            let w = s.len() * 6;
            self.text((x as usize).saturating_sub(w / 2), b + 6, s, TEXT, 1);
        }
        for (v, s) in ys {
            let y = self.sy(*v).round() as i64;
            for x in l..=r {
                self.put(x as i64, y, GRID);
            }
            self.text(
                l.saturating_sub(s.len() * 6 + 4),
                (y as usize).saturating_sub(3),
                s,
                TEXT,
                1,
            );
        }
        for x in l..=r {
            self.put(x as i64, b as i64, AXIS);
            self.put(x as i64, t as i64, AXIS);
        }
        for y in t..=b {
            self.put(l as i64, y as i64, AXIS);
            self.put(r as i64, y as i64, AXIS);
        }
        let w = xname.len() * 6;
        self.text((l + r) / 2 - w / 2, b + 22, xname, TEXT, 1);
        self.text(4, t.saturating_sub(12), yname, TEXT, 1);
    }

    /// A polyline through `pts` (NaN points break it), `thick` px wide.
    pub fn line(&mut self, pts: &[(f64, f64)], c: Rgb, thick: i64) {
        let clip = |v: f64, lo: f64, hi: f64| v.clamp(lo, hi);
        let (yl, yh) = (self.box_.1 as f64, self.box_.3 as f64);
        let mut last: Option<(f64, f64)> = None;
        for (x, y) in pts {
            if !x.is_finite() || !y.is_finite() {
                last = None;
                continue;
            }
            let p = (self.sx(*x), clip(self.sy(*y), yl, yh));
            if let Some(q) = last {
                let n = ((p.0 - q.0).abs().max((p.1 - q.1).abs()) as usize).max(1);
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let (x, y) = (q.0 + (p.0 - q.0) * t, q.1 + (p.1 - q.1) * t);
                    for d in 0..thick {
                        for e in 0..thick {
                            self.put(x as i64 + d - thick / 2, y as i64 + e - thick / 2, c);
                        }
                    }
                }
            } else {
                for d in -1..=1 {
                    for e in -1..=1 {
                        self.put(p.0 as i64 + d, p.1 as i64 + e, c);
                    }
                }
            }
            last = Some(p);
        }
    }

    /// A horizontal reference line at `y`.
    pub fn hline(&mut self, y: f64, c: Rgb) {
        let yy = self.sy(y).round() as i64;
        for x in self.box_.0..=self.box_.2 {
            if x % 6 < 3 {
                self.put(x as i64, yy, c);
            }
        }
    }

    /// One more entry of the legend (top right, inside the frame).
    pub fn key(&mut self, label: &str, c: Rgb) {
        let x = self.box_.2 - 230;
        let y = self.box_.1 + 8 + self.legend * 16;
        for dx in 0..24 {
            for dy in 0..3 {
                self.put((x + dx) as i64, (y + 4 + dy) as i64, c);
            }
        }
        self.text(x + 30, y + 1, label, TEXT, 1);
        self.legend += 1;
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), self.w as u32, self.h as u32);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().map_err(|e| e.to_string())?;
        let data: Vec<u8> = self.px.iter().flat_map(|p| p.iter().copied()).collect();
        w.write_image_data(&data).map_err(|e| e.to_string())
    }
}

/// Frequency ticks for a log axis, 31.5 Hz … 16 kHz.
pub fn hz_ticks() -> Vec<(f64, String)> {
    [31.5, 63.0, 125.0, 250.0, 500.0, 1e3, 2e3, 4e3, 8e3, 16e3]
        .iter()
        .map(|f| {
            let s = if *f >= 1000.0 {
                format!("{}K", (*f / 1000.0) as i64)
            } else {
                format!("{}", *f as i64)
            };
            (*f, s)
        })
        .collect()
}

/// Ticks every `step` from `a` to `b`.
pub fn lin_ticks(a: f64, b: f64, step: f64) -> Vec<(f64, String)> {
    let mut v = Vec::new();
    let mut x = (a / step).ceil() * step;
    while x <= b + 1e-9 {
        v.push((x, format!("{}", (x * 100.0).round() / 100.0)));
        x += step;
    }
    v
}

/// Key ticks: the C's from C1 to C8, named.
pub fn key_ticks() -> Vec<(f64, String)> {
    (1..=8)
        .map(|o| ((12 + 12 * o) as f64, format!("C{o}")))
        .collect()
}
