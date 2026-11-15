//! The unit's snapshot (`configure`) and its audio-thread calls.

use super::*;
use cantus_core::inst::sampler::split_indexed;
use cantus_core::inst::tympanum_kit as kitdef;
use cantus_dsp::pulsus::{PadSettings, LANES_MODEL, MAX_PADS};

/// Where each mic's peak sits in the telemetry: `METER_TEL + n` for `o{n}` (the views'
/// `tympanum_strips::METER_TEL`).
pub const METER_TEL: usize = 16;

/// What the spec says of the pieces (`p{n}.`) and of the bleed matrix (`b{p}.{m}.`).
struct Keys {
    tune: [f32; MAX_PIECES],
    damp: [f32; MAX_PIECES],
    level: [f32; MAX_PIECES],
    wires: [f32; MAX_PIECES],
    mute: [bool; MAX_PIECES],
    /// Where the stick lands (offset) and how far each hit strays (r40:tymp-engine).
    pos: [f32; MAX_PIECES],
    spread: [f32; MAX_PIECES],
    /// Per cell: dB, inverted, off.
    cell_db: [[f32; MAX_MICS]; MAX_PIECES],
    cell_inv: [[bool; MAX_MICS]; MAX_PIECES],
    cell_off: [[bool; MAX_MICS]; MAX_PIECES],
    mic: [[f32; 6]; MAX_MICS],
}

impl Keys {
    fn read(spec: &NativeSpec) -> Keys {
        let mut k = Keys {
            tune: [0.0; MAX_PIECES],
            damp: [0.0; MAX_PIECES],
            level: [0.0; MAX_PIECES],
            wires: [1.0; MAX_PIECES],
            mute: [false; MAX_PIECES],
            pos: [0.0; MAX_PIECES],
            spread: [cantus_core::inst::tympanum::SPREAD; MAX_PIECES],
            cell_db: [[0.0; MAX_MICS]; MAX_PIECES],
            cell_inv: [[false; MAX_MICS]; MAX_PIECES],
            cell_off: [[false; MAX_MICS]; MAX_PIECES],
            mic: [[0.0; 6]; MAX_MICS],
        };
        for (key, v) in &spec.params {
            let v = *v;
            if let Some((p, m, f)) = bleed_key(key) {
                if p < MAX_PIECES && m < MAX_MICS {
                    match f {
                        "db" => k.cell_db[p][m] = v,
                        "inv" => k.cell_inv[p][m] = v >= 0.5,
                        "on" => k.cell_off[p][m] = v < 0.5,
                        _ => {}
                    }
                }
            } else if let Some((p, f)) = split_indexed(key, b'p', MAX_PIECES) {
                match f {
                    "tune" => k.tune[p] = v,
                    "damp" => k.damp[p] = v,
                    "level" => k.level[p] = v,
                    "wires" => k.wires[p] = v,
                    "mute" => k.mute[p] = v >= 0.5,
                    "pos" => k.pos[p] = v.clamp(-1.0, 1.0),
                    "spread" => k.spread[p] = v.clamp(0.0, 1.0),
                    _ => {}
                }
            } else if let Some((m, f)) = split_indexed(key, b'm', MAX_MICS) {
                let i = match f {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    "az" => 3,
                    "el" => 4,
                    "pattern" => 5,
                    _ => continue,
                };
                k.mic[m][i] = v;
            }
        }
        k
    }
}

impl Unit {
    /// Forget the live hits counted so far — the unit's, the model's, the hybrid's: a unit
    /// at rest is as good as a new one (no allocation).
    fn forget_hits(&mut self) {
        self.live = [0; 128];
        self.model.pulsus.forget_hits();
        self.model.hybrid.forget_hits();
    }

    /// Each mic's mix, bus, power and delay from the `o{n}.` keys and the routes.
    fn configure_outputs(&mut self, spec: &NativeSpec, def: &KitDef) {
        let n = def.mics.len().min(MAX_MICS);
        self.outs.count = n;
        let mut delay = [0.0f32; MAX_MICS];
        let mut solo = [false; MAX_MICS];
        for (m, mic) in def.mics.iter().take(n).enumerate() {
            self.outs.mix[m] = OutMix::default();
            self.outs.on[m] = true;
            self.outs.bus[m] = bus_of(&spec.outputs, mic.output);
            // An output past the lines' 32 has none (`u8::MAX`).
            self.outs.out_id[m] = u8::try_from(mic.output.0)
                .ok()
                .filter(|i| usize::from(*i) < MAX_MICS)
                .unwrap_or(u8::MAX);
        }
        let amb = spec.get("amb.level");
        for (key, v) in &spec.params {
            let Some((id, f)) = output_key(key) else {
                continue;
            };
            let Some(m) = def
                .mics
                .iter()
                .take(n)
                .position(|d| usize::from(d.output.0) == id)
            else {
                continue;
            };
            let x = &mut self.outs.mix[m];
            match f {
                OutputField::Level => x.level_db = *v,
                OutputField::Pan => x.pan = *v,
                OutputField::Width => x.width = *v,
                OutputField::Invert => x.invert = *v >= 0.5,
                OutputField::Mute => x.mute = *v >= 0.5,
                OutputField::Solo => solo[m] = *v >= 0.5,
                OutputField::Delay => delay[m] = *v,
                OutputField::On => self.outs.on[m] = *v >= 0.5,
            }
        }
        let any_solo = solo[..n].iter().any(|s| *s);
        for (m, mic) in def.mics.iter().take(n).enumerate() {
            if amb != 0.0 && mic.family == OutputFamily::Rooms {
                self.outs.mix[m].level_db += amb;
            }
            if any_solo && !solo[m] {
                self.outs.mix[m].mute = true;
            }
            let fs = self.fs;
            if let Some(d) = self.outs.line(m) {
                d.set_ms(delay[m], fs);
            }
        }
    }

    /// The bleed matrix: per piece and mic, the kit's gain, the piece's level, the kit's
    /// bleed fader (a mic's own pieces untouched), the cell's dB, polarity and power.
    fn cells(spec: &NativeSpec, def: &KitDef, k: &Keys, out: &mut [[f32; MAX_MICS]; MAX_PIECES]) {
        let gain = spec.get("gain");
        let bleed = spec.get("bleed");
        // The kit's own cells, in one pass over its list (fix-e: was a search of the whole
        // list for each of the 32 × 32 cells): each piece × mic takes the first that names it.
        let mut kit_db = [[0.0f32; MAX_MICS]; MAX_PIECES];
        let mut seen = [0u32; MAX_PIECES];
        for (pp, o, d) in &def.bleed.cells {
            let p = *pp as usize;
            if p >= MAX_PIECES {
                continue;
            }
            for (m, mic) in def.mics.iter().enumerate().take(MAX_MICS) {
                if mic.output == *o && seen[p] & (1 << m) == 0 {
                    kit_db[p][m] = *d;
                    seen[p] |= 1 << m;
                }
            }
        }
        for (p, row) in out.iter_mut().enumerate() {
            for (m, cell) in row.iter_mut().enumerate() {
                let Some(mic) = def.mics.get(m) else {
                    *cell = 0.0;
                    continue;
                };
                if k.mute[p] || k.cell_off[p][m] {
                    *cell = 0.0;
                    continue;
                }
                let own = mic.main_for.is_empty() || mic.main_for.contains(&(p as u16));
                let d = gain
                    + k.level[p]
                    + if own { 0.0 } else { bleed }
                    + kit_db[p][m]
                    + k.cell_db[p][m];
                let g = db(d);
                *cell = if k.cell_inv[p][m] { -g } else { g };
            }
        }
    }

    fn configure_modelled(&mut self, spec: &NativeSpec, m: &Arc<cache::Modelled>, k: &Keys) {
        let def = &m.def;
        let snap = !self.modelled.as_ref().is_some_and(|x| Arc::ptr_eq(x, m));
        if snap {
            self.model.reset();
        }
        self.modelled = Some(m.clone());
        let seed = spec.get("seed") as u32;
        self.model
            .pulsus
            .set_kit(LANES_MODEL, seed, self.hat_cc, false);
        self.model.pulsus.chick = spec.get("hat.chick") >= 0.5;
        // The kit card: every piece tuned and ringing at once.
        let kit_tune = spec.get("tune");
        let ring = spec.get("ring").clamp(0.05, 2.0);
        let beater = spec.get("beater").clamp(0.0, 1.0);
        let mut pads = [(0u8, PadSettings::kick()); MAX_PADS];
        let n = m.pads.len().min(MAX_PADS);
        for (i, (key, piece, base)) in m.pads.iter().take(n).enumerate() {
            let p = (*piece as usize).min(MAX_PIECES - 1);
            let mut s = *base;
            s.tune += k.tune[p] + kit_tune;
            if ring != 1.0 {
                s.decay *= ring;
                s.open_decay *= ring;
                s.noise_decay *= ring;
            }
            if k.pos[p] != 0.0 {
                let to = if k.pos[p] > 0.0 { 1.0 } else { 0.0 };
                s.position += (to - s.position) * k.pos[p].abs();
            }
            s.scatter = k.spread[p];
            if s.model == 5 {
                // A measured piece: `pos` tilts it (the edge brighter, the centre fuller).
                s.position = k.pos[p];
                if beater > 0.0 && (s.measured & !cantus_dsp::pulsus::MEASURED_FOOT) % 16 == 4 {
                    // A measured kick: a plastic beater's click (2–6 kHz) — the stick's
                    // burst up to four times as strong, the attack brighter.
                    s.click *= 1.0 + 3.0 * beater;
                    s.position = (s.position + 0.5 * beater).min(1.0);
                }
            }
            if beater > 0.0 && s.voice == cantus_dsp::pulsus::DrumVoice::Kick && s.model == 4 {
                s.hardness = (s.hardness + 0.4 * beater).min(1.0);
                s.click *= 1.0 + 0.8 * beater;
                s.tone *= 1.0 + 0.6 * beater;
            }
            if k.damp[p] > 0.0 {
                s.decay *= 1.0 - 0.8 * k.damp[p];
                s.damping = (s.damping + 0.4 * k.damp[p]).min(1.0);
            }
            s.wires *= k.wires[p];
            pads[i] = (*key, s);
        }
        self.model.set_pads(&pads[..n]);
        // The player's figures, by their articulations' names.
        self.figures = [Figure::None; 128];
        for p in &def.pieces {
            for a in &p.articulations {
                self.figures[a.key as usize & 127] = match a.id.as_str() {
                    "flam" => Figure::Flam,
                    "drag" => Figure::Drag,
                    "buzz" => Figure::Buzz,
                    "roll" => Figure::Roll,
                    _ => Figure::None,
                };
            }
        }
        // The pieces and the mics where they stand now.
        let mut pieces = [PieceGeom::default(); MAX_PIECES];
        let np = def.pieces.len().min(MAX_PIECES);
        for (g, p) in pieces.iter_mut().zip(&def.pieces) {
            let a = p.place.at();
            g.pos = [a[0] / 100.0, a[1] / 100.0, a[2] / 100.0];
            g.normal = p.place.dir();
            g.dipole = match p.kind {
                PieceKind::Kick => 0.55,
                PieceKind::Snare | PieceKind::RackTom | PieceKind::FloorTom => 0.7,
                _ => 0.85,
            };
            // A cymbal is a plate as broad as the pair is wide: the overheads hear it from
            // their centre and its two sides apart (round 40: never out of phase, the
            // recordings' +0.1 … +0.4 between the capsules).
            g.width = match p.kind {
                PieceKind::HiHat
                | PieceKind::Crash
                | PieceKind::Ride
                | PieceKind::Splash
                | PieceKind::China
                | PieceKind::Stack => 0.7,
                _ => 0.0,
            };
            // A head's radius (its diameter is in cm): the beam its highs leave in.
            g.radius = p.diameter / 200.0;
        }
        let mut mics = [MicGeom::default(); MAX_MICS];
        let nm = def.mics.len().min(MAX_MICS);
        for (i, (g, d)) in mics.iter_mut().zip(&def.mics).enumerate() {
            let mv = k.mic[i];
            let a = d.place.at();
            g.pos = [
                (a[0] + mv[0]) / 100.0,
                (a[1] + mv[1]) / 100.0,
                (a[2] + mv[2]) / 100.0,
            ];
            let mut place = d.place;
            place.azimuth += mv[3];
            place.elevation = (place.elevation + mv[4]).clamp(-90.0, 90.0);
            g.aim = place.dir();
            g.pattern = if mv[5] >= 1.0 {
                (mv[5] as u8 - 1).min(4)
            } else {
                pattern_index(d.pattern)
            };
            g.spread = d.spread / 100.0;
            g.pan = d.pan;
            g.level = db(d.level_db);
            g.norm = m.norm.get(i).copied().unwrap_or(1.0);
            let (room, refl) = match d.family {
                OutputFamily::Rooms => (1.0, 2),
                OutputFamily::Overheads => (0.3, 1),
                OutputFamily::Cymbals | OutputFamily::HiHat => (0.08, 1),
                _ => (0.05, 0),
            };
            g.room = room;
            g.reflections = refl;
        }
        let room = room_geom(spec, def);
        Self::cells(spec, def, k, &mut self.model.cells);
        self.model
            .set_geometry(&pieces[..np], &mics[..nm], room, snap);
    }

    fn configure_recorded(
        &mut self,
        spec: &NativeSpec,
        def: &KitDef,
        rec: &Arc<cantus_dsp::tympanum::rec::RecKit>,
        assets: &super::super::Assets,
        k: &Keys,
    ) {
        // A pack's articulations are its own recordings: no figures played for them.
        self.figures = [Figure::None; 128];
        self.rec.set_kit(Some(rec));
        self.rec.seed = spec.get("seed") as u32;
        self.rec.gain = 1.0;
        self.rec.vary = spec.get("vary") >= 0.5;
        // The kit's own room (0) or one of Cantus's on its room mics (1–3).
        let choice = spec.get("room") as u8;
        self.room_choice = choice;
        let was = self.room_on;
        self.rooms = [false; cantus_dsp::tympanum::MAX_MICS];
        if (1..=3).contains(&choice) {
            for (i, m) in def.mics.iter().enumerate().take(self.rooms.len()) {
                self.rooms[i] = m.family == OutputFamily::Rooms;
            }
        }
        self.room_on = self.rooms.iter().any(|r| *r);
        if self.room_on {
            let g = room_geom(spec, def);
            let size = cantus_dsp::math::cbrt(g.width * g.depth * g.height);
            self.own_room.set(size, g.t60, g.t60 * 0.45);
        } else if was {
            self.own_room.clear();
            if let Some(a) = self.aula.as_deref_mut() {
                a.reset();
            }
        }
        self.rec.chick = spec.get("hat.chick") >= 0.5;
        Self::cells(spec, def, k, &mut self.rec.cells);
        for p in 0..MAX_PIECES {
            self.rec.piece_cents[p] = (k.tune[p] + spec.get("tune")) * 100.0;
            // Damping a recorded drum: its sound falls under an envelope, from none at 0 to
            // a 80 ms time constant at full.
            self.rec.piece_env_k[p] = if k.damp[p] <= 0.0 {
                1.0
            } else {
                let tau = 10.0 * cantus_dsp::math::powf(0.008, k.damp[p].min(1.0));
                cantus_dsp::math::exp(-1.0 / (tau * self.fs))
            };
        }
        // The files, by their index (dropping the last snapshot's: the sample cache keeps
        // each one, so nothing is freed here).
        self.files.clear();
        let n = rec_files(def, assets).min(MAX_FILES);
        self.files.resize(n, None);
        for (key, d) in assets.iter() {
            let Some(i) = key.strip_prefix('t').and_then(|x| x.parse::<usize>().ok()) else {
                continue;
            };
            if let Some(slot) = self.files.get_mut(i) {
                slot.clone_from(d);
            }
        }
    }
}

/// How many files the table needs: one past the highest index among the assets.
fn rec_files(_def: &KitDef, assets: &super::super::Assets) -> usize {
    assets
        .iter()
        .filter_map(|(k, _)| k.strip_prefix('t').and_then(|x| x.parse::<usize>().ok()))
        .max()
        .map_or(0, |m| m + 1)
}

/// The room a modelled kit sounds in: its own (`room` 0) or one of Cantus's, sized and
/// lengthened by `amb.size` and `amb.decay`.
fn room_geom(spec: &NativeSpec, def: &KitDef) -> RoomGeom {
    let own = RoomGeom {
        width: def.room.width / 100.0,
        depth: def.room.depth / 100.0,
        height: def.room.height / 100.0,
        back: def.room.back / 100.0,
        ..RoomGeom::default()
    };
    let mut r = match spec.get("room") as u8 {
        1 => RoomGeom {
            width: 3.0,
            depth: 3.5,
            height: 2.5,
            back: 1.2,
            reflect: 0.45,
            floor_reflect: 0.5,
            t60: 0.25,
        },
        2 => RoomGeom::default(),
        3 => RoomGeom {
            width: 14.0,
            depth: 20.0,
            height: 8.2,
            back: 4.0,
            reflect: 0.85,
            floor_reflect: 0.8,
            t60: 1.6,
        },
        _ => own,
    };
    let size = spec.get("amb.size");
    if size != 1.0 {
        let s = 0.5 + 0.5 * size.clamp(0.0, 2.0);
        r.width *= s;
        r.depth *= s;
        r.height = (r.height * s).max(2.2);
        r.back = (r.back * s).max(1.0);
    }
    r.t60 *= spec.get("amb.decay").clamp(0.05, 2.0);
    // The surfaces (0: the room's own). Wood gives back less than stone; fabric least.
    const FLOOR: [f32; 9] = [0.0, 0.72, 0.72, 0.7, 0.68, 0.7, 0.9, 0.92, 0.88];
    const WALLS: [f32; 11] = [0.0, 0.75, 0.6, 0.55, 0.78, 0.7, 0.35, 0.35, 0.85, 0.65, 0.9];
    let (f, w, g) = (
        spec.get("floor") as usize,
        spec.get("walls") as usize,
        spec.get("rug") as usize,
    );
    if let Some(&x) = FLOOR.get(f).filter(|_| f > 0) {
        r.floor_reflect = x;
    }
    if let Some(&x) = WALLS.get(w).filter(|_| w > 0) {
        r.t60 *= 0.6 + 0.6 * x / r.reflect.max(0.1);
        r.reflect = x;
    }
    // A rug under the kit soaks up the floor's reflection; "none" leaves it bare.
    if (1..=3).contains(&g) {
        r.floor_reflect *= 0.45;
    }
    r
}

impl InstrumentUnit for Unit {
    /// A copy for a checkpoint (ADR 0031's listening renders): a modelled kit's whole state;
    /// a recorded kit streams from the disk's board, which a copy cannot share — none.
    fn fork(&self) -> Option<Box<dyn InstrumentUnit>> {
        if self.mode == Mode::Recorded {
            return None;
        }
        let rec = RecEngine::new(self.fs);
        super::super::stream::hub().register_board(rec.board());
        Some(Box::new(Unit {
            fs: self.fs,
            mode: self.mode,
            rec,
            model: self.model.clone(),
            outs: self.outs.clone(),
            files: self.files.clone(),
            modelled: self.modelled.clone(),
            kit_def: self.kit_def.clone(),
            hat_cc: self.hat_cc,
            held: self.held,
            live: self.live,
            humanize: self.humanize,
            timing: self.timing,
            seed: self.seed,
            pending: self.pending,
            n_pending: self.n_pending,
            own_room: self.own_room.clone(),
            rooms: self.rooms,
            room_on: self.room_on,
            figures: self.figures,
            aula: self.aula.clone(),
            aula_room: self.aula_room,
            room_choice: self.room_choice,
            send_l: self.send_l.clone(),
            send_r: self.send_r.clone(),
        }))
    }

    fn configure(&mut self, spec: &NativeSpec, assets: &super::super::Assets) {
        super::CONFIGURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.hat_cc = spec.get("hat.cc").clamp(0.0, 127.0) as u8;
        self.humanize = spec.get("humanize").clamp(0.0, 1.0);
        self.timing = spec.get("timing").clamp(0.0, 20.0) * 0.001 * self.fs;
        self.seed = spec.get("seed") as u32;
        let keys = Keys::read(spec);
        // The kit it plays: the spec's, or — while the spec's new kit loads — the kit it
        // had, which the sample cache marks (`k:<kit>`).
        let text = assets
            .iter()
            .find_map(|(k, _)| k.strip_prefix("k:"))
            .unwrap_or_else(|| cache::kit_text(spec));
        let before = self.mode;
        if let Some(id) = text.strip_prefix("model:") {
            match cache::modelled(id) {
                Some(m) => {
                    self.configure_outputs(spec, &m.def);
                    self.configure_modelled(spec, &m, &keys);
                    // r40:drums-hyb — the hybrid's residual (none: the model alone).
                    super::super::tympanum_hybrid::configure(&mut self.model.hybrid, spec, id, &m);
                    self.mode = Mode::Modelled;
                }
                None => self.mode = Mode::Silent,
            }
        } else {
            let found = kitdef::recorded(text).and_then(|k| cache::rec_kit(&k).map(|r| (k, r)));
            match found {
                Some((kit, rec)) => {
                    self.configure_outputs(spec, &kit.def);
                    self.kit_def = Some(kit.clone());
                    self.configure_recorded(spec, &kit.def, &rec, assets, &keys);
                    self.mode = Mode::Recorded;
                }
                None => self.mode = Mode::Silent,
            }
        }
        if before != self.mode {
            if before == Mode::Recorded {
                self.rec.set_kit(None);
            }
            if before == Mode::Modelled {
                self.model.reset();
                self.modelled = None;
            }
            if self.mode == Mode::Silent {
                self.outs.count = 0;
            }
            self.outs.clear();
        }
    }

    /// fix-e — a lane moved only per-output scalars (`o{n}.*`, `amb.level`), the *Feel*
    /// card (`humanize`, `timing`) or the bleed's levels (`gain`, `bleed`, `p{n}.level`,
    /// `p{n}.mute`, `b{p}.{m}.db`/`.inv`): those alone are set — no file, path or pad
    /// rebuilt — to the very state [`Self::configure`] reaches. Anything else (the kit, its
    /// files, a piece's sound, a mic's place, the room) is a whole configure.
    fn configure_keys(
        &mut self,
        spec: &NativeSpec,
        _assets: &super::super::Assets,
        keys: &[String],
    ) -> bool {
        let (mut outs, mut cells) = (false, false);
        for k in keys {
            match scalar_key(k) {
                Some(Scalar::Output) => outs = true,
                Some(Scalar::Feel) => {}
                Some(Scalar::Cell) => cells = true,
                None => return false,
            }
        }
        // The kit as last configured (an `Arc` clone: it outlives this call).
        let (modelled, recorded) = match self.mode {
            Mode::Modelled => (self.modelled.clone(), None),
            Mode::Recorded => (None, self.kit_def.clone()),
            Mode::Silent => return false,
        };
        let def = match (&modelled, &recorded) {
            (Some(m), _) => &m.def,
            (_, Some(r)) => &r.def,
            _ => return false,
        };
        self.humanize = spec.get("humanize").clamp(0.0, 1.0);
        self.timing = spec.get("timing").clamp(0.0, 20.0) * 0.001 * self.fs;
        if outs {
            self.configure_outputs(spec, def);
        }
        if cells {
            let k = Keys::read(spec);
            if self.mode == Mode::Recorded {
                Self::cells(spec, def, &k, &mut self.rec.cells);
            } else {
                Self::cells(spec, def, &k, &mut self.model.cells);
                // The paths' gains from the cells, the geometry as it is.
                self.model.regain();
            }
        }
        true
    }

    fn note_on(&mut self, note: NoteOn) {
        self.feel(note.key, note.velocity_u8(), note.id, None);
    }

    fn note_on_seq(&mut self, note: NoteOn, ordinal: u32) {
        self.feel(note.key, note.velocity_u8(), note.id, Some(ordinal));
    }

    fn note_off(&mut self, key: u8, id: u64, _velocity: f32) {
        // A held roll ends with its note: its strokes still waiting do not come.
        if self.figures[key as usize & 127] == Figure::Roll {
            let mut i = 0;
            while i < self.n_pending {
                if self.pending[i].id == id {
                    self.pending[i] = self.pending[self.n_pending - 1];
                    self.n_pending -= 1;
                } else {
                    i += 1;
                }
            }
        }
        if self.mode == Mode::Modelled {
            self.model.pulsus.note_off(key, id);
        }
    }

    fn choke(&mut self, key: u8, id: u64) {
        match self.mode {
            Mode::Recorded => self.rec.choke_note(id),
            Mode::Modelled => self.model.pulsus.note_off(key, id),
            Mode::Silent => {}
        }
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        let Controller::Cc(n) = controller else {
            return;
        };
        if n != self.hat_cc || !value.is_finite() {
            return;
        }
        self.held = true;
        match self.mode {
            Mode::Recorded => {
                let files = &self.files;
                self.rec.pedal(value, &|i| Self::source(files, i));
            }
            Mode::Modelled => self.model.pulsus.controller(n, value),
            Mode::Silent => {}
        }
    }

    fn all_notes_off(&mut self) {
        // A stop, a seek: a hit still waiting for its moment does not come.
        self.n_pending = 0;
        if self.mode == Mode::Modelled {
            self.model.pulsus.all_notes_off();
        }
    }

    fn reset(&mut self) {
        self.rec.reset();
        self.model.reset();
        self.outs.clear();
        self.held = false;
        self.forget_hits();
        self.n_pending = 0;
        self.own_room.clear();
        if let Some(a) = self.aula.as_deref_mut() {
            a.reset();
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.process_buses(left, right, &mut |_, _, _, _| {})
    }

    fn process_buses(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        out: &mut cantus_dsp::pulsus::BusOut<'_>,
    ) -> bool {
        let n = left.len().min(right.len());
        // The meters show this block.
        self.outs.reset_peaks();
        if self.mode == Mode::Silent || self.at_rest() {
            return false;
        }
        let mut any = false;
        let mut at = 0;
        while at < n {
            // A hit the *Feel* card holds sounds on its own frame, whatever the block.
            let next = if self.n_pending > 0 {
                self.due()
            } else {
                u32::MAX
            };
            let c = (n - at).min(LANE).min(next as usize);
            for p in &mut self.pending[..self.n_pending] {
                p.left -= c as u32;
            }
            self.outs.begin(c);
            match self.mode {
                Mode::Recorded => {
                    self.rec.render(c, &mut self.outs);
                    if self.room_on {
                        let mine = self.aula_room == self.room_choice;
                        match self.aula.as_deref_mut().filter(|_| mine) {
                            // Aula's response of the room (the unit built for it).
                            Some(aula) => {
                                let (l, r) = (&mut self.send_l[..c], &mut self.send_r[..c]);
                                self.outs.room_send(&self.rooms, c, 0.5, l, r);
                                aula.process(l, r);
                                let ringing = !aula.at_rest();
                                self.outs.room_return(&self.rooms, c, l, r, ringing);
                            }
                            // Until that unit arrives: the modelled kit's late field.
                            None => self
                                .outs
                                .room_from(&mut self.own_room, &self.rooms, c, 0.35),
                        }
                    }
                }
                Mode::Modelled => self.model.render(c, &mut self.outs),
                Mode::Silent => {}
            }
            any |= self.outs.emit(c, at, left, right, out);
            at += c;
        }
        if self.at_rest() {
            // Come to rest, as good as new: the live hits counted so far are forgotten,
            // so the next one is drawn as a new unit's first (the contract's twins).
            self.forget_hits();
        }
        any
    }

    fn at_rest(&self) -> bool {
        if self.held || self.n_pending > 0 || self.outs.ringing() {
            return false;
        }
        match self.mode {
            Mode::Silent => true,
            Mode::Recorded => {
                self.rec.is_quiet()
                    && self.own_room.at_rest()
                    && self.aula.as_deref().is_none_or(|a| a.at_rest())
            }
            Mode::Modelled => self.model.at_rest(),
        }
    }

    /// Each mic's peak this block at `METER_TEL + n` (the views' meters, `o{n}`).
    fn telemetry(&self) -> crate::inst::Telemetry {
        let mut t = [0.0; crate::inst::TELEMETRY_LEN];
        let n = self.outs.count.min(cantus_dsp::tympanum::MAX_MICS);
        for (m, v) in t.iter_mut().skip(METER_TEL).take(n).enumerate() {
            *v = self.outs.peak[m];
        }
        t
    }

    fn active_voices(&self) -> usize {
        match self.mode {
            Mode::Recorded => self.rec.active_voices(),
            Mode::Modelled => self.model.pulsus.active_voices(),
            Mode::Silent => 0,
        }
    }

    fn set_offline(&mut self, offline: bool) {
        self.rec.set_blocking(offline);
    }

    fn take_late_reads(&mut self) -> u32 {
        self.rec.take_late()
    }

    fn take_cut_notes(&mut self) -> u32 {
        self.rec.take_cut()
    }

    fn take_stream_wakes(&mut self) -> u32 {
        self.rec.take_wakes()
    }
}

/// The stream reader lets go of a dropped unit's board.
impl Drop for Unit {
    fn drop(&mut self) {
        super::super::stream::hub().boards_changed();
    }
}

/// What a key the automation moves touches, when it is not the whole unit (fix-e).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Scalar {
    /// An output's mix or delay (`o{n}.*`), the room mics' level (`amb.level`).
    Output,
    /// The *Feel* card.
    Feel,
    /// The bleed matrix's levels.
    Cell,
}

fn scalar_key(key: &str) -> Option<Scalar> {
    if let Some((_, f)) = output_key(key) {
        // *On* loads and unloads files: never automated, and never this path.
        return (f != OutputField::On).then_some(Scalar::Output);
    }
    match key {
        "amb.level" => return Some(Scalar::Output),
        "humanize" | "timing" => return Some(Scalar::Feel),
        "gain" | "bleed" => return Some(Scalar::Cell),
        _ => {}
    }
    if let Some((_, _, f)) = bleed_key(key) {
        return matches!(f, "db" | "inv").then_some(Scalar::Cell);
    }
    match split_indexed(key, b'p', MAX_PIECES) {
        Some((_, "level" | "mute")) => Some(Scalar::Cell),
        _ => None,
    }
}
