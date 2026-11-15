//! Fractus measured (2).

use super::*;

#[test]
fn debug_splatter() {
    let tone = super::tests::sine(220.0);
    let dry: Vec<f32> = (0..96_000).map(|i| tone(i as i64)[0]).collect();
    eprintln!("dry splatter {:.1}", super::tests::splatter_db(&dry, 4_000.0, 0.5));
}

#[test]
fn debug_repeat_join() {
    use super::tests::*;
    let tone = sine(233.0);
    let mut p = Pattern::EMPTY;
    p.block(Module::Reverse, 0, 16, 1.0, 1.0);
    let s = FractusSettings {
        repeat: RepeatSettings {
            size: 0.125,
            ..RepeatSettings::default()
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &tone);
    let mut worst = (0usize, 0.0f32);
    for i in 2..96_000 {
        let d2 = (l[i] - 2.0 * l[i - 1] + l[i - 2]).abs();
        if d2 > worst.1 {
            worst = (i, d2);
        }
    }
    eprintln!("worst second difference {worst:?}");
    let mut d2: Vec<(usize, f32)> = (2..96_000).map(|i| (i, (l[i] - 2.0 * l[i - 1] + l[i - 2]).abs())).collect();
    d2.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    eprintln!("top {:?}", &d2[..12]);
    let i = worst.0;
    eprintln!("{:?}", &l[i - 5..i + 5]);
}
