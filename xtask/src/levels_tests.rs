//! The v2/v3 tasks' own logic: the null-hash comparison and the paired A/B table.

use crate::null_hashes::{differences, items};
use crate::perf::{ab_table, points, Point};
use cantus_launch::Level;

const V2: &str = "# cantus null hashes; build x86-64-v2; 3 items
aaaa  10  math.sin.f32
bbbb  20  fx.porta.heavy
cccc  30  demo.house
";

#[test]
fn equal_lists_are_equal_whatever_the_header_says() {
    let v3 = V2.replace("x86-64-v2", "x86-64-v3");
    assert!(differences(V2, &v3).is_empty());
    assert_eq!(items(V2).len(), 3);
    assert_eq!(items(V2)[1], ("fx.porta.heavy".to_string(), "bbbb 20".to_string()));
}

#[test]
fn each_item_that_differs_is_named() {
    let v3 = V2
        .replace("bbbb  20", "bbbc  20")
        .replace("cccc  30  demo.house", "cccc  31  demo.house");
    assert_eq!(differences(V2, &v3), ["fx.porta.heavy", "demo.house"]);
    let fewer = V2.replace("cccc  30  demo.house\n", "");
    assert_eq!(differences(V2, &fewer), ["demo.house (only in the first)"]);
    assert_eq!(differences(&fewer, V2), ["demo.house (only in the second)"]);
}

fn report(engine: f64, fit: f64) -> String {
    format!(
        r#"{{"schema":1,"suite":"full","machine":{{"class":"x"}},"metrics":[
        {{"id":"engine.s64.b128","unit":"µs/block","better":"lower","gate":true,
          "summary":{{"median":{engine},"p10":0,"p90":0,"min":0,"max":0,"runs":3}}}},
        {{"id":"fx.terminus.heavy.fit128","unit":"instances","better":"higher","gate":false,
          "summary":{{"median":{fit},"p10":0,"p90":0,"min":0,"max":0,"runs":3}}}}]}}"#
    )
}

#[test]
fn the_paired_table_takes_the_median_of_per_pair_ratios() {
    let p = points(&report(100.0, 10.0));
    assert_eq!(
        p[0],
        Point {
            id: "engine.s64.b128".into(),
            unit: "µs/block".into(),
            lower_is_better: true,
            median: 100.0,
        }
    );
    assert!(!p[1].lower_is_better);
    // Three pairs: B/A = 0.8, 0.9, 1.2 → median 0.9; B better (lower) in two.
    let pairs: Vec<_> = [(100.0, 80.0), (200.0, 180.0), (100.0, 120.0)]
        .iter()
        .map(|(a, b)| (points(&report(*a, 10.0)), points(&report(*b, 12.0))))
        .collect();
    let t = ab_table(Level::V2, Level::V3, &pairs);
    let row = t.lines().find(|l| l.contains("engine.s64.b128")).unwrap();
    assert!(row.contains("**0.900×**"), "{row}");
    assert!(row.contains("0.80–1.20"), "{row}");
    assert!(row.contains("2 of 3"), "{row}");
    // Higher is better for instances: 12 > 10 in all three pairs.
    let row = t.lines().find(|l| l.contains("fit128")).unwrap();
    assert!(row.contains("**1.200×**") && row.contains("3 of 3"), "{row}");
    assert!(t.starts_with("| Metric | Unit | x86-64-v2 | x86-64-v3 |"));
}
