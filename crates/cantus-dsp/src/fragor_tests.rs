use super::*;

fn sine_gain(p: &mut Pedal, rate: f64, hz: f64, amp: f64) -> f64 {
    let n = (rate * 0.3) as usize;
    let mut peak = 0.0f64;
    for i in 0..n {
        let x = amp * crate::math::sin(std::f64::consts::TAU * hz * i as f64 / rate);
        let y = p.tick(x);
        if i > n / 2 {
            peak = peak.max(y.abs());
        }
    }
    peak / amp
}

#[test]
fn explore() {
    let rate = 192_000.0;
    for m in Model::ALL {
        let mut p = Pedal::new(m, rate);
        let ports: Vec<f64> = (0..p_parts(m)).flat_map(|i| p.part(i).ports().to_vec()).collect();
        let g = sine_gain(&mut p, rate, 1000.0, 1e-4);
        p.snap();
        let g2 = sine_gain(&mut p, rate, 1000.0, 0.1);
        println!("{m:?}: ports {ports:?} small {:.2} dB, 100 mV {:.2} dB", 20.0 * g.log10(), 20.0 * g2.log10());
    }
}

fn p_parts(m: Model) -> usize {
    match m {
        Model::Silicon => 2,
        Model::Muff => 3,
        _ => 1,
    }
}

#[test]
fn explore_cpu() {
    let rate = 192_000.0;
    for m in Model::ALL {
        let mut p = Pedal::new(m, rate);
        let n = rate as usize;
        let xs: Vec<f64> = (0..n).map(|i| {
            let ph = i as f64 / rate;
            0.2 * crate::math::exp(-ph * 2.0) * crate::math::sin(std::f64::consts::TAU * 220.0 * ph)
        }).collect();
        let it0 = dk::ITERS.load(std::sync::atomic::Ordering::Relaxed);
        let t = std::time::Instant::now();
        let mut acc = 0.0;
        for x in &xs {
            acc += p.tick(*x);
        }
        let dt = t.elapsed().as_secs_f64();
        let it = dk::ITERS.load(std::sync::atomic::Ordering::Relaxed) - it0;
        println!("  iterations per tick per part-call {:.2}", it as f64 / n as f64);
        println!("{m:?}: {:.0} ns/tick ({:.1} % of a core at 48 kHz x4 mono) {acc:.3}", dt / n as f64 * 1e9, dt / (rate / 192_000.0) * 100.0);
    }
}
