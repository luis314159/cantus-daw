use super::*;

fn env(points: &[(i64, f32)]) -> GainEnvelope {
    GainEnvelope::new(
        points
            .iter()
            .map(|(t, db)| GainPoint::new(Ticks(*t), *db))
            .collect(),
    )
}

#[test]
fn a_line_holds_its_ends_and_travels_between_its_points() {
    let e = env(&[(100, -6.0), (200, 6.0)]);
    assert_eq!(e.db_at(0.0), -6.0);
    assert_eq!(e.db_at(100.0), -6.0);
    assert_eq!(e.db_at(150.0), 0.0);
    assert_eq!(e.db_at(200.0), 6.0);
    assert_eq!(e.db_at(1e9), 6.0);
    // No points: 0 dB.
    assert_eq!(GainEnvelope::default().db_at(5.0), 0.0);
}

#[test]
fn two_points_at_one_tick_are_a_jump() {
    let e = env(&[(0, 0.0), (100, 0.0), (100, -12.0), (200, -12.0)]);
    assert_eq!(e.db_at(99.0), 0.0);
    assert_eq!(e.db_at(100.0), -12.0);
}

#[test]
fn a_shape_bends_the_line_like_automation() {
    let mut e = env(&[(0, -12.0), (100, 0.0)]);
    e.points[0].shape = Shape::Hold;
    assert_eq!(e.db_at(99.0), -12.0);
    e.points[0].shape = Shape::Curve;
    e.points[0].tension = 0.5;
    assert!(e.db_at(50.0) < -6.0, "a bend that waits");
}

#[test]
fn flat_is_every_point_at_zero() {
    assert!(GainEnvelope::default().is_flat());
    assert!(env(&[(0, 0.0), (10, 0.0)]).is_flat());
    assert!(!env(&[(0, 0.0), (10, -0.1)]).is_flat());
}

#[test]
fn validity_is_order_range_and_count() {
    assert!(env(&[(0, ENV_MIN_DB), (10, ENV_MAX_DB)]).is_valid());
    let mut e = env(&[(0, 0.0)]);
    e.points[0].db = ENV_MAX_DB + 1.0;
    assert!(!e.is_valid());
    e.points[0].db = f32::NAN;
    assert!(!e.is_valid());
    let unsorted = GainEnvelope {
        points: vec![GainPoint::new(Ticks(5), 0.0), GainPoint::new(Ticks(1), 0.0)],
    };
    assert!(!unsorted.is_valid());
    let mut t = env(&[(0, 0.0)]);
    t.points[0].tension = 2.0;
    assert!(!t.is_valid());
    let many = GainEnvelope {
        points: vec![GainPoint::new(Ticks(0), 0.0); MAX_ENV_POINTS + 1],
    };
    assert!(!many.is_valid());
}

#[test]
fn the_floor_is_silence_and_zero_is_unity_to_the_bit() {
    assert_eq!(gain_of(ENV_MIN_DB), 0.0);
    assert_eq!(gain_of(0.0), 1.0);
    assert!((gain_of(-6.0) - 0.501_187).abs() < 1e-5);
}

#[test]
fn a_glue_keeps_one_line_or_joins_each_windows_own() {
    let a = env(&[(0, -6.0), (100, -6.0)]);
    // A split's halves: the same line, kept as it is.
    let kept = GainEnvelope::glued(&[
        (Some(&a), Ticks(0), Ticks(50)),
        (Some(&a), Ticks(50), Ticks(100)),
    ]);
    assert_eq!(kept.as_ref(), Some(&a));
    // Two lines: each over its own stretch, a jump at the join.
    let b = env(&[(0, 3.0)]);
    let j = GainEnvelope::glued(&[
        (Some(&a), Ticks(0), Ticks(50)),
        (Some(&b), Ticks(50), Ticks(100)),
    ])
    .unwrap();
    assert!(j.is_valid());
    assert_eq!(j.db_at(25.0), -6.0);
    assert_eq!(j.db_at(75.0), 3.0);
    // One half with none: 0 dB there.
    let h = GainEnvelope::glued(&[
        (Some(&a), Ticks(0), Ticks(50)),
        (None, Ticks(50), Ticks(100)),
    ])
    .unwrap();
    assert_eq!(h.db_at(75.0), 0.0);
    // None anywhere: none.
    assert!(
        GainEnvelope::glued(&[(None, Ticks(0), Ticks(5)), (None, Ticks(5), Ticks(9))]).is_none()
    );
}
