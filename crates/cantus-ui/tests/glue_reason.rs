//! Round 31's fix-round review (finding 7): *Glue* says that note clips play differently
//! (baked and not, other grooves) only when the glue is a glue of notes. A selection that
//! also holds an audio clip takes the audio path and is refused for its own reasons — the
//! message never blames grooves or baking for it.

mod support;

use cantus_core::*;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::CantusApp;

/// One track: two note clips side by side, the second played past the note effects, and
/// an audio clip after them.
fn mixed() -> (CantusApp, TrackId, [ClipId; 3]) {
    let mut p = Project::empty("Glue");
    let mut t = p.make_track("Mixed");
    let a = p.make_clip("a", Ticks::ZERO, Ticks::beats(4));
    let mut b = p.make_clip("b", Ticks::beats(4), Ticks::beats(4));
    b.skip_note_fx = true;
    let src = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: src,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 10,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("c", src, Ticks::beats(8), Ticks::beats(4));
    let ids = [a.id, b.id, c.id];
    t.clips.extend([a, b, c]);
    let tid = t.id;
    p.tracks.push(t);
    (support::app(p), tid, ids)
}

fn select(app: &mut CantusApp, t: TrackId, ids: &[ClipId]) {
    app.select_clip(t, ids[0]);
    for id in &ids[1..] {
        app.toggle_clip_selection(t, *id);
    }
}

#[test]
fn a_selection_with_audio_is_not_said_to_differ_in_its_notes() {
    i18n::set_thread_locale(Locale::En);
    let differs = i18n::t("notefx.glue_differs");
    let (mut app, t, ids) = mixed();
    select(&mut app, t, &ids);
    app.glue_clips();
    assert_ne!(app.status.text, differs, "refused for another reason");
    // Notes alone that play differently: said.
    let (mut app, t, ids) = mixed();
    select(&mut app, t, &ids[..2]);
    app.glue_clips();
    assert_eq!(app.status.text, differs);
}
