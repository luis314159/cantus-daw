//! ADR 0089's measured prototypes (not product code). `import-proto <what> <args>`.
mod bones;
mod gen;
mod lottie_mini;
mod raster;
mod sprite;

use std::time::Instant;

fn mean_ms(n: u32, mut f: impl FnMut()) -> f64 {
    f(); // warm
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed().as_secs_f64() * 1000.0 / f64::from(n)
}

fn velato(dir: &str) {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .map(|p| (p.file_name().unwrap().to_string_lossy().to_string(), std::fs::read(&p).unwrap()))
        .collect();
    files.push(("ours:character".into(), gen::lottie_character(1)));
    for (name, bytes) in gen::lottie_hostile() {
        files.push((name, bytes));
    }
    std::panic::set_hook(Box::new(|_| {}));
    for (name, bytes) in &files {
        let t = Instant::now();
        let r = std::panic::catch_unwind(|| velato::Composition::from_slice(bytes).map(|_| ()));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let what = match r {
            Ok(Ok(())) => "ok".to_string(),
            Ok(Err(e)) => format!("error: {e}"),
            Err(p) => format!(
                "PANIC: {}",
                p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default()
            ),
        };
        println!("velato {name:32} {ms:7.3} ms  {what}");
    }
}

fn lottie(dir: &str) {
    for copies in [1u32, 4, 16] {
        let bytes = gen::lottie_character(copies);
        let parse = mean_ms(50, || {
            let _ = lottie_mini::parse(&bytes).unwrap();
        });
        let a = lottie_mini::parse(&bytes).unwrap();
        let mut pm = tiny_skia::Pixmap::new(600, 600).unwrap();
        let s = 600.0 / a.w.max(a.h);
        let base = tiny_skia::Transform::from_scale(s, s);
        let mut f = 0.0f32;
        let mut calls = 0;
        let draw = mean_ms(200, || {
            pm.fill(tiny_skia::Color::TRANSPARENT);
            calls = a.draw(f, base, &mut pm);
            f = (f + 1.7) % a.op;
        });
        println!(
            "ours lottie x{copies:<3} {:6} bytes, {:3} layers, {} markers: parse {parse:.3} ms, frame 600x600 {draw:.3} ms ({calls} paints)",
            bytes.len(),
            a.layers.len(),
            a.markers.len()
        );
        if copies == 1 {
            pm.fill(tiny_skia::Color::TRANSPARENT);
            a.draw(45.0, base, &mut pm);
            pm.save_png(format!("{dir}/ours-lottie-character.png")).unwrap();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(3).cloned().unwrap_or_else(|| ".".into());
    match args.get(1).map(String::as_str) {
        Some("velato") => velato(&args[2]),
        Some("lottie") => lottie(&out),
        Some("sprite") => sprite::run(&out),
        Some("bones") => bones::run(&out),
        Some("own") => bones::run_own(&out),
        Some("write") => gen::write_all(&args[2]),
        _ => eprintln!("import-proto velato <dir> | lottie - <out> | sprite - <out> | bones - <out> | write <dir>"),
    }
}
