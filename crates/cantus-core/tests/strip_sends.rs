//! A strip's sends keep their order through an undo (round 24, found by the mixer-routing
//! study): removing a send from the middle of the list and undoing it puts it back where
//! it was, so the project is the one before the removal (ADR 0004: an exact inverse).

use cantus_core::*;

#[test]
fn undoing_the_removal_of_a_middle_send_puts_it_back_in_its_place() {
    let mut p = Project::new("sends");
    let a = p.make_strip("A");
    let targets: Vec<MixerStrip> = (0..3).map(|i| p.make_strip(format!("R{i}"))).collect();
    let ids: Vec<StripId> = targets.iter().map(|s| s.id).collect();
    let a_id = a.id;
    p.strips.push(a);
    p.strips.extend(targets);
    let mut bus = CommandBus::new(p);
    for (i, to) in ids.iter().enumerate() {
        bus.execute(
            Command::SetStripSend {
                id: a_id,
                send: Send {
                    to: *to,
                    level: 0.2 + i as f32 * 0.2,
                    enabled: true,
                },
            },
            Origin::User,
        )
        .unwrap();
    }
    let before = bus.project().clone();
    bus.execute(
        Command::RemoveStripSend {
            id: a_id,
            to: ids[1],
        },
        Origin::User,
    )
    .unwrap();
    bus.undo();
    let order: Vec<StripId> = bus
        .project()
        .strip(a_id)
        .unwrap()
        .sends
        .iter()
        .map(|s| s.to)
        .collect();
    assert_eq!(order, ids, "the send is back in the middle");
    assert!(
        bus.project() == &before,
        "the project is the one before the removal"
    );
}
