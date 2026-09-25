//! The contract every plug-in keeps, checked for every kind at once: at its default
//! settings it renders finite samples, the same at every block size. Each plug-in's own
//! tests go further; this one catches the kind that breaks the family's floor.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use support::*;

#[test]
fn every_kind_is_finite_and_block_size_independent_at_its_defaults() {
    for kind in EffectKind::ALL {
        let mut p = demo_project();
        add_effect(&mut p, kind);
        let reference = render_with(&p, 512);
        assert!(
            reference.iter().all(|x| x.is_finite()),
            "{kind:?} produced a sample that is not a number"
        );
        for block in [1usize, 64, 1000] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{kind:?} at block size {block}"),
            );
        }
    }
}
