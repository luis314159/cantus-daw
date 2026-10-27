//! What a chord or a scale stage made of each note it was given, so the note's off
//! releases exactly what its on sounded — whatever the settings did in between.

/// How many notes a stage remembers at once.
pub(super) const ENTRIES: usize = 128;

#[derive(Copy, Clone, Debug)]
pub(super) struct Entry {
    used: bool,
    tag: u64,
    pitch: u8,
    pub n: u8,
    pub pitches: [u8; 8],
}

const EMPTY: Entry = Entry {
    used: false,
    tag: 0,
    pitch: 0,
    n: 0,
    pitches: [0; 8],
};

#[derive(Debug)]
pub(super) struct Table {
    entries: [Entry; ENTRIES],
    count: usize,
}

impl Table {
    pub fn new() -> Table {
        Table {
            entries: [EMPTY; ENTRIES],
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        if self.count > 0 {
            self.entries = [EMPTY; ENTRIES];
            self.count = 0;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Remember that note `(tag, pitch)` sounded `out`; `false` when there is no room (its
    /// off then passes as it is).
    pub fn insert(&mut self, tag: u64, pitch: u8, out: &[u8]) -> bool {
        // A note on again before its off: the newer one is what its off will release.
        let slot = self
            .entries
            .iter()
            .position(|e| e.used && e.tag == tag && e.pitch == pitch)
            .or_else(|| self.entries.iter().position(|e| !e.used));
        let Some(i) = slot else {
            return false;
        };
        let e = &mut self.entries[i];
        if !e.used {
            self.count += 1;
        }
        *e = Entry {
            used: true,
            tag,
            pitch,
            n: out.len().min(8) as u8,
            pitches: [0; 8],
        };
        e.pitches[..out.len().min(8)].copy_from_slice(&out[..out.len().min(8)]);
        true
    }

    /// What note `(tag, pitch)` sounded, forgotten.
    pub fn take(&mut self, tag: u64, pitch: u8) -> Option<Entry> {
        let i = self
            .entries
            .iter()
            .position(|e| e.used && e.tag == tag && e.pitch == pitch)?;
        let e = self.entries[i];
        self.entries[i] = EMPTY;
        self.count -= 1;
        Some(e)
    }
}
