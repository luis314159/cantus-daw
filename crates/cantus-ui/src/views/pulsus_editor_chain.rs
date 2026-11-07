//! The selected pad's chain (Pulsus III, ADR 0100 §6): its inserts in order — each with its
//! switch, its editor and its removal — and *Add insert ▾*. A chain lives on its pad (the
//! desk does not show it); the pad's grid cell marks a pad that has one.

use super::*;
use cantus_core::EffectKind;

/// The rows a chain shows: one per insert, then the add row.
pub(super) fn chain_height(s: &Style, app: &CantusApp, pad: &DrumPad) -> f32 {
    let n = cantus_core::padout::chain_effects(app.project(), pad).len();
    (parts::CARD_CHROME + parts::ROW * (n as f32 + 1.0) + 6.0 * n as f32) * s.k
}

/// The kinds a chain offers to add: every built-in insert offered for a new one — not the
/// retired Reverb (r37:fixes; Spatium is the reverb) — and a hosted plug-in goes on a chain
/// from the device picker, *More…*, once the pad has one.
pub(crate) fn kinds() -> Vec<EffectKind> {
    EffectKind::ALL
        .iter()
        .copied()
        .filter(|k| *k != EffectKind::External && k.offered())
        .collect()
}

pub(super) fn chain_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Output);
    let cr = parts::card(ui, rect, s, &look(Src::Key("padout.chain"), None), true);
    let k = s.k;
    let h = parts::ROW * k;
    let effects: Vec<cantus_core::EffectSlot> =
        cantus_core::padout::chain_effects(app.project(), pad).to_vec();
    let chain = cantus_core::padout::chain_of(app.project(), pad);
    let mut y = cr.body.top();
    for e in &effects {
        let row = Rect::from_min_size(pos2(cr.body.left(), y), vec2(cr.body.width(), h));
        let name = t(e.spec.kind.name_key());
        let on = !e.bypass;
        let lamp = Rect::from_min_size(row.min, vec2(160.0 * k, h));
        if parts::led(ui, lamp, s, on, Src::Str(&name), &name).clicked() {
            if let Some(strip) = chain {
                app.exec(Command::SetEffectBypass {
                    strip,
                    effect: e.id,
                    bypass: on,
                });
            }
        }
        let open = Rect::from_min_size(pos2(lamp.right() + 6.0 * k, row.top()), vec2(80.0 * k, h));
        if parts::press(
            ui,
            open,
            s,
            Src::Key("padout.chain_open"),
            &crate::i18n::tf("padout.chain_open_tip", &[("name", &name)]),
        )
        .clicked()
        {
            if let Some(strip) = chain {
                app.open_effect_editor(strip, e.id);
            }
        }
        let del = Rect::from_min_size(pos2(open.right() + 6.0 * k, row.top()), vec2(80.0 * k, h));
        if parts::press(
            ui,
            del,
            s,
            Src::Key("padout.chain_remove"),
            &crate::i18n::tf("padout.chain_remove_tip", &[("name", &name)]),
        )
        .clicked()
        {
            app.remove_from_pad_chain(track, pad.id, e.id);
        }
        y += h + 6.0 * k;
    }
    let add = Rect::from_min_size(pos2(cr.body.left(), y), vec2(fields_w(s), h));
    let kinds = kinds();
    let names: Vec<String> = kinds.iter().map(|k| t(k.name_key())).collect();
    let labels: Vec<Src<'_>> = names.iter().map(|n| Src::Str(n)).collect();
    let id = egui::Id::new(("pulsus-chain-add", track.0, pad.id.0));
    let (_, c) = parts::field(
        ui,
        add,
        s,
        &t("padout.chain_add"),
        false,
        false,
        &t("padout.chain_add"),
    );
    let items = parts::radio_items(&labels, None);
    if let Some(i) = parts::dropdown(ui, id, add, s, &items, c.open) {
        if let Some(kind) = kinds.get(i) {
            app.add_to_pad_chain(track, pad.id, *kind);
        }
    }
    // Once the pad has a chain: the device picker, plug-ins included, onto it.
    if let Some(strip) = chain {
        let more = Rect::from_min_size(pos2(add.right() + 6.0 * k, add.top()), vec2(120.0 * k, h));
        if parts::press(
            ui,
            more,
            s,
            Src::Key("padout.chain_browse"),
            &t("padout.chain_browse_tip"),
        )
        .clicked()
        {
            app.open_picker(crate::picker::PickPurpose::AddEffect { strip });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::kinds;
    use cantus_core::EffectKind;

    /// r37:fixes — the chain offers what a strip offers: Spatium is the reverb, the older
    /// Reverb only stays for the projects that hold one.
    #[test]
    fn a_pads_chain_offers_no_retired_reverb() {
        let k = kinds();
        assert!(!k.contains(&EffectKind::Reverb));
        assert!(k.contains(&EffectKind::Space) && k.contains(&EffectKind::Delay));
        assert!(k.iter().all(|x| x.offered()));
    }
}
