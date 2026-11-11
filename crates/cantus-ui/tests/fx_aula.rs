//! Aula's editor (round 39, ADR 0112): with a file playing, the list of Cantus's own
//! responses says plainly that none of them is — to the eye and to a screen reader — and the
//! file reads as the source in use; with none, the list names the room playing.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::aula;
use cantus_ui::CantusApp;
use support::*;

fn with_aula() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Aula);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

/// The editor painted once with the screen reader on: its painted words and every label
/// the screen reader is given.
fn painted(app: &mut CantusApp) -> (String, Vec<String>) {
    let ctx = ctx();
    ctx.enable_accesskit();
    let mut text = String::new();
    let mut labels: Vec<String> = Vec::new();
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            cantus_ui::views::fx::window(app, &ctx);
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes).to_lowercase();
        // A label's words are the node's label, or its value for plain text.
        if let Some(u) = out.platform_output.accesskit_update {
            labels.extend(u.nodes.iter().flat_map(|(_, n)| {
                [n.label(), n.value()]
                    .into_iter()
                    .flatten()
                    .map(|l| l.to_lowercase())
                    .collect::<Vec<_>>()
            }));
        }
    }
    (text, labels)
}

#[test]
fn a_file_playing_is_the_source_shown_and_spoken_and_no_room_is() {
    let (mut a, s0, fx) = with_aula();
    // Cantus's own: the room playing is named, and spoken.
    let (text, labels) = painted(&mut a);
    assert!(text.contains("concert hall"), "{text}");
    assert!(
        labels
            .iter()
            .any(|l| l == "cantus's own response: concert hall"),
        "{labels:?}"
    );
    // A file: no room is shown or spoken as playing; the file is.
    aula::set_file(&mut a, s0, fx, Some("C:/IRs/York Minster.wav".into()));
    let (text, labels) = painted(&mut a);
    assert!(text.contains("none: a file is playing"), "{text}");
    assert!(text.contains("york minster"), "{text}");
    assert!(
        !text.contains("concert hall"),
        "a room reads as playing beside the file: {text}"
    );
    assert!(
        labels
            .iter()
            .any(|l| l == "cantus's own response: none. the file york minster is playing"),
        "{labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.contains("concert hall")),
        "{labels:?}"
    );
    assert_eq!(
        aula::source_words(&a.project().effect(s0, fx).unwrap().spec),
        "Cantus's own response: none. The file York Minster is playing"
    );
    // Back to Cantus's own: the room again.
    aula::set_file(&mut a, s0, fx, None);
    let (text, _) = painted(&mut a);
    assert!(text.contains("concert hall") && !text.contains("a file is playing"));
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (key, values) in cantus_ui::views::fx::factory_presets(EffectKind::Aula) {
        for (k, _) in *values {
            assert!(EffectKind::Aula.param(k).is_some(), "{key}: {k}");
        }
        for l in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            assert!(cantus_ui::i18n::has(l, key), "{key} in {l:?}");
        }
    }
    assert_eq!(
        cantus_ui::views::fx::factory_presets(EffectKind::Aula).len(),
        12
    );
}
