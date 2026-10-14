//! Who made the content, as *About ▸ Credits* and `THIRD-PARTY-NOTICES.md` name them
//! (ADR 0042 §3): every pack of the manifest — CC BY ones with their full credit line,
//! CC0 ones as a courtesy line — and any installed pack the manifest no longer lists,
//! from its receipt.

use crate::install::Receipt;
use crate::manifest::{Attribution, Credit, Licence, Manifest, Text};

/// Whether a credit is the licence's condition or given gladly.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Courtesy {
    /// CC BY: the credit line is the licence's one condition and is shown in full.
    Required,
    /// CC0: no credit is asked; the makers are named as a courtesy.
    Courtesy,
}

/// One pack's credit.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: Text,
    /// The licence's name (`CC BY 3.0`), or the SPDX id when it is not one content may
    /// be under (an old receipt).
    pub licence: String,
    pub courtesy: Courtesy,
    /// The credit in full in English ([`Attribution::full`]) when the pack carries one,
    /// otherwise the makers joined by `; `.
    pub line: String,
    /// The SPDX id and the licence text's URL, with the credit's parts, so a view can
    /// word the credit in its own language.
    pub spdx: String,
    pub licence_url: String,
    pub attribution: Option<Attribution>,
    /// The author's own words about crediting, when the manifest quotes them.
    pub note: String,
    /// Installed from a receipt the manifest no longer lists.
    pub from_receipt: bool,
}

fn makers(credits: &[Credit]) -> String {
    credits
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Required or a courtesy, decided by the licence alone (ADR 0042): CC BY asks for
/// credit; CC0 never does, even when its author asked for a credit line. An id content
/// may not be under (an old receipt) is treated as asking for credit — never less.
fn courtesy(spdx: &str) -> Courtesy {
    match Licence::from_spdx(spdx) {
        Some(l) if !l.needs_credit() => Courtesy::Courtesy,
        _ => Courtesy::Required,
    }
}

fn licence_name(spdx: &str) -> String {
    Licence::from_spdx(spdx).map_or_else(|| spdx.to_string(), |l| l.name().to_string())
}

/// The credits of every pack in `manifest`, in its order, then of every installed pack it
/// does not list.
pub fn list(manifest: &Manifest, installed: &[Receipt]) -> Vec<Entry> {
    let mut out: Vec<Entry> = manifest
        .packs
        .iter()
        .map(|p| {
            let courtesy = courtesy(&p.licence);
            let line = p.credit_full().unwrap_or_else(|| makers(&p.credits));
            Entry {
                id: p.id.clone(),
                name: p.name.clone(),
                licence: licence_name(&p.licence),
                courtesy,
                line,
                spdx: p.licence.clone(),
                licence_url: p.licence_url.clone(),
                attribution: p.attribution.clone(),
                note: p
                    .attribution
                    .as_ref()
                    .map(|a| a.note.clone())
                    .unwrap_or_default(),
                from_receipt: false,
            }
        })
        .collect();
    for r in installed {
        if manifest.pack(&r.id).is_some() {
            continue;
        }
        let courtesy = courtesy(&r.licence);
        let line = r
            .attribution
            .as_ref()
            .map(|a| a.full(&r.licence, &r.licence_url))
            .unwrap_or_else(|| makers(&r.credits));
        out.push(Entry {
            id: r.id.clone(),
            name: r.name.clone(),
            licence: licence_name(&r.licence),
            courtesy,
            line,
            spdx: r.licence.clone(),
            licence_url: r.licence_url.clone(),
            attribution: r.attribution.clone(),
            note: r
                .attribution
                .as_ref()
                .map(|a| a.note.clone())
                .unwrap_or_default(),
            from_receipt: true,
        });
    }
    out
}
