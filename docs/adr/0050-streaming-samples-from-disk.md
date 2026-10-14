# ADR 0050 — Exemplar streams long samples from disk, and loads off the interface's thread

**Status:** Accepted · **Date:** 2026-10-13, amended 2026-10-14 after the review · **Round:** 26 (sampler) · **Extends:** ADR 0005
(real-time rules), ADR 0023 (native instruments: Exemplar), ADR 0025 §9 (sources loaded in the
background), ADR 0030 (content), ADR 0031 (offline = real time, parallel = serial), ADR 0038
(voices on several threads)

> Numbering: written as 0050 on `r26/sampler`; the integrator renumbers it if another branch
> took it. Measurements: `docs/round26/sampler.md`.

## Context

The Salamander Grand Piano (CC BY 3.0, 48 kHz/24-bit FLAC, 641 files, 1.89 hours of stereo
audio) installs in one step since round 26 (ADR 0042, `content.md`). Exemplar decoded every
sample to `f32` and built a half-rate mip level for every zone (`levels_needed` keeps an
octave of modulation headroom, so every zone of the piano gets level 1). Measured by the
content builder: **8 velocity layers: 16.9 s to load, 1 914 MiB; 16 layers: 32.4 s,
3 732 MiB** — all of it on the interface's thread, the window frozen and no progress shown
(the integrator's review, item 15). A first-rank sampler (Kontakt's DFD, Logic's Sampler,
HALion, Bitwig) keeps only the head of each sample in memory, streams the rest, and loads in
seconds without freezing anything.

What a streamed voice must keep from what Exemplar is:

- **The bits.** A voice reads its sample through the windowed-sinc interpolator, the mip
  levels and the half-band decimator (`exemplar_resample.rs`), choosing between a vectorised
  dot product over a contiguous window and a tap-by-tap path. Streamed playback has to equal
  in-memory playback *to the bit*, so the same path must be taken with the same numbers.
- **The real-time rules** (ADR 0005): the audio thread never allocates, locks, blocks or
  does I/O; `cantus-engine` and `cantus-dsp` are `forbid(unsafe_code)`.
- **Offline = real time and parallel = serial** (ADR 0031, 0038): a bounce is the same as
  playing, and an instrument's layers may render on any thread of the pool.

## Decision

### 1. What is streamed

A sample is streamed when (a) the engine streams at all (`SampleCache::set_streaming`; on in
the application with or without a device, off in tests and tools unless they ask), (b) it is
longer than its head plus one block, and (c) **no zone of the instrument loops it or plays it
backwards**. Looped and
reversed samples stay in memory (their loops can jump anywhere and they are almost always
short); everything else — a piano's notes, releases, resonances, pedal noises — streams.
A sample used by a kit pad, an audio clip or another instrument is loaded in full for those,
as before (they keep their own maps).

### 2. The compact store: a cache file at the sample's own depth

The first time a streamed file is loaded it is decoded once (FLAC, WAV, AIFF, Ogg) and
written to a **cache file** holding level 0 at the narrowest *exact* depth: the decoder's
`f32` values are tested — every value an integer multiple of 2⁻¹⁵ in range is `i16`, of 2⁻²³
is `i24`, otherwise `f32` — so converting back (`k · 2⁻²³`) gives the very same bits. The
Salamander's files are all 24-bit: 6 bytes per stereo frame instead of 8 (a negative zero has
no integer form: a file holding one stays `f32`).

**What names a cache file is the source as it is**: its path, size, modification time
(nanoseconds from 1970, negative before it) and a fingerprint of its first and last 4 KiB,
taken **before** the source is decoded and checked again after — a file replaced while it was
read plays what was read and is not cached. **The limit of that identity, exactly**: a file
replaced by another of the same size and time is told apart when its first or last 4 KiB
differ; one changed only between them, its size and time kept (a tool that restores
timestamps), has the same identity and its old cache file opens. Reading the whole source at
every open would undo streaming, so the cache file (format 3) also stores **a hash of the
whole source**, computed by the writer beside the loads, and a stream opened from its cache
file is checked beside the reader, paced (~500 MB/s at most), **once per file as it is per
session** — a stream opened again (a preset switched back, an undo) is not hashed again
(third review #5): a source found changed has
its stream let go (its notes end), its cache file deleted, the status line says so once, and
the next snapshot decodes it anew (second review #9). The header (magic, version, that identity, the
source's path, channels, rate, frames, depth, one checksum per 4 096-frame block, and a
checksum of the header itself) is checked on open; every block's checksum is checked when it
is read, by the loader for the heads and by the reader for every block, so a file whose data
a crash lost is never played. **A block the reader cannot read is recovered in the session**
(second review #3, #5, #15): the source is decoded again and the stream plays on from that
decode, then from a cache file written anew — a damaged file is deleted first; a file that
only failed to read (a busy drive) is opened again and **the block that failed** read again,
once per stream, before anything is decoded: an error that comes back (a bad sector) decodes
the source (fourth review #5). **Recoveries run one at a time** in a queue, each decode counted against the
writers' 1 GiB bound before it is made, and every decode is written out, so none stays in
memory for the session (third review #4) — while a real-time
voice that reaches the block first fades and is counted (at most that one note) and an
offline voice waits for the decode, so an export equals the undamaged render. Only when the
source cannot be read either (or is another file now) does the stream fail: its voices end,
the sample cache drops it and loads the file again (reporting it missing), an export fails
naming the file (`RenderError::Unreadable`, and `StemsError::Unreadable` for stems — never
a bounce silently cut short: third review #3), and the status line says it once. A file is written beside its name and renamed into
place, and a thread of its own flushes it to the disk at once (`sync_all`, and its folder on
Unix) — flushing before the rename doubled a first load on this machine (1 GB written:
9.1 s → 20.7 s), and the block checksums already keep a file a crash left short from ever
being played; a failure leaves nothing behind. **A first load plays at once from its decode**: the stream reads level 0
from the decoded file in memory while one writer thread writes its cache file, then switches
to the file — the same values (the depth is exact), so nothing heard changes at the switch —
and the decode is let go. Two writer threads hold at most 1 GiB of decoded files; a loader
waits while they hold more, so a first load's memory is bounded. A cache that cannot be written (a
full disk, a read-only folder) does not fail the sample: it keeps playing from its decode in
memory, and the status line says so once.

**The flush follows the rename** (accepted after the review): flushing each file before its
rename more than doubled a first load on this machine (writing 1 GB of cache: 9.1 s without
a flush, 20.7 s with one before each rename), and the block checksums already guarantee that
a file a crash left short is never played — its damaged blocks are detected when read, the
file is deleted and the source decoded again.

**The folder is kept under a size limit** (Settings, *Sample cache*: **automatic** by
default — the smallest of 16 GB, a tenth of its disk and what leaves 5 GB free, the numbers
shown — or a size chosen; the size it holds now, *Clear cache*); no cache file is written
that would leave its disk under 5 GB free (it plays from memory and says so — second review
#10). A sweep beside the window — at start, after every batch of loads, after a pack is
removed — deletes stray `.part` files older than ten minutes, files that are not ours or are
damaged, entries whose source is gone for good or changed, then the least recently used
(opening an entry touches it) until the folder fits. **Gone for good** means changed, or not
there while the folder it was in is (and holds something): a source whose drive is unplugged
or whose share is offline is not gone — its entry is left to the size limit — and a removed
pack's folder is named to the sweep and remembered beside the cache (`removed-roots.txt`)
until no entry under it is left, so an entry a stream held at the removal goes at a later
sweep (second review #7, third #2). **A file a stream reads is never
deleted** (second review #6): a stream holds its cache file open with a shared lock while
**the handle budget** allows — a quarter of the process's open-files limit, at most 1 024;
the limit itself is raised to the system's hard limit at start on Unix (`cantus_rt::files`;
a Finder-started macOS app begins at 256) — and past the budget the least recently read
stream's handle is closed and opened again (the lock taken again) when it is read; a file
gone by then is recovered like a damaged one (third review #1). The sweep deletes only a file it can
lock exclusively — no stream of this or another Cantus holds it —, that the hub does not name
at that moment, and that nothing touched within the minute before the sweep began, all
checked again at each deletion. One sweep at a time per folder, across processes (a lock
file), and a file another sweep already took counts as deleted (#8).

**Mip levels are not stored.** Level *k* is computed from level *k − 1* by the same half-band
filter as before (`halve`, now `halve_span` over any range, which `halve` itself calls, so
the two cannot differ): the reader computes a block of level 1 from the level-0 frames it
covers plus the filter's reach (79 frames each side). The Salamander never needs it unless
it is played more than an octave above a sample's root; storing it would add 1.3 GB of disk
for the whole piano. Heads of every level are computed at load.

### 3. One stream per file, heads and blocks

- **A stream is a file as it is** (its path and identity, §2), whatever zones play it and
  however: tune, transpose, bend, a zone's range, start or the velocity's start change nothing
  about it (an edit keeps every zone playing, and makes no new stream). It holds **every mip
  level the file has** (the in-memory rule: while the level below holds 32 frames or more);
  each zone keeps the levels its own range needs (`Source::limit_levels`, from the same
  `max_shift` the in-memory mip-maps are sized by), so a zone plays exactly as its in-memory
  sample would, even when zones of different ranges share the file.
- **The stream travels with its head** (`SampleData::stream`, an opaque handle in
  `cantus-core`): no registry to fill. It is released when nothing holds it any more — the
  sample cache buries the streams no snapshot asks for and lets go of each once it is their
  last holder (`stream::release`), never on the audio thread; a stream asked for again before
  that comes back without a load.
- **Blocks** of 4 096 frames (per level). A streamed sample's level *k* keeps its first
  `16 384 / 2^k` frames in memory (`Source::levels[k]`, an `Arc<[f32]>` as before, now a
  prefix); the rest comes in blocks.
- **Later starts are kept ready**: a zone that starts past its head (its `start`, the
  velocity's start) says on its instrument's *start board* which blocks a note may start in
  (and two more), and at how many levels: every mip level a note of the zone can start at
  (its top key, the tunes, the transpose and a full bend up), the same time ahead at each.
  The reader keeps them in the pool while the zone exists. A zone pins at most
  `count × levels` blocks (`count` ≤ 255, `levels` ≤ 8, 32 KiB each); one that starts a block
  past its head and is never played an octave up pins 3.
- **The head** is sized from the worst-case latency of the reader (§4), measured on this
  machine under the stress test (`docs/round26/sampler.md`): a voice may consume up to two
  frames of a level per output frame (the resampler keeps the local ratio below 2), so the
  head must cover `2 × rate × latency` frames plus the reader's sleep (§4). The default is
  **16 384 frames** at level 0 (341 ms at 48 kHz at unity, 171 ms at the fastest). It is one
  constant (`stream::HEAD_FRAMES`).

### 4. The reader thread, the pool and the hand-off

- **One reader thread** for the process (`cantus_engine::inst::stream`), plain `std` file
  I/O like the existing loader threads — no `unsafe`, no OS call outside `std`, so nothing
  moves into `cantus-rt`.
- **The pool** (`cantus_dsp::exemplar::stream::Pool`): fixed slots of one block each (4 096
  stereo frames, 32 KiB), in chunks of 256 slots that the reader adds when it runs short (up
  to a cap), never freed while the process runs. Each slot's samples are `AtomicU32` (the
  bits of an `f32`) behind a **sequence number** — a seqlock (Boehm, *Can seqlocks get along
  with programming language memory models?*, 2012): the writer makes the number odd, writes
  with relaxed stores after a release fence, and publishes an even number with a release
  store; a reader loads the number (acquire), copies with relaxed loads, fences (acquire)
  and loads the number again — the copy is good only when both equal the number the block
  table promised. Everything is safe Rust; a torn copy is detected, never used.
- **Block tables**: each streamed sample has, per level, one `AtomicU64` per block — 0, or
  the slot and the sequence number it was published with. Voices playing the same sample
  read the same blocks: **one read serves every voice** of a sample (a repeated note, the
  pedal's restrikes, a loop played again).
- **What the reader reads**: every Exemplar has a **board** of one `AtomicU64` per layer,
  where a streamed layer publishes (sample id, level, block) before it renders; the reader
  keeps its own copies of the streams (refreshed only when they change) and only weak ones
  of the boards — upgraded while it reads them — so an instrument's boards go with it, and
  an Exemplar zeroes its cells as it goes (second review #1); looks
  every millisecond while voices read, every 20 ms while streams exist but nothing reads, and
  not at all — parked until a stream is made — when there is none (and at once when an
  offline render wakes it); reads for each cursor the first blocks after the head as soon as the
  voice starts, then always the next three blocks, nearest first — and, when the voice's
  local ratio is near the resampler's hysteresis edges (above 1.6 or below 1.1), the block
  at the same place in the neighbouring mip level, so a pitch change that crosses an octave
  finds it there (reading it always doubled the disk's work for a level the piano almost
  never plays) — and evicts the least recently wanted block nobody's cursor wants.
- **The voice's window**: each layer owns a private buffer of 1 024 frames (allocated with
  the instrument, never on the audio thread). Before each run it copies, from the head or
  from the pool, the frames the run can touch (the interpolator's taps and the decimator's
  history included) plus a fade's worth more, only when the window does not hold them
  already; the resampler then reads the head or the window as plain slices — the same
  vectorised paths, the same bits.
- **Any ratio the reader can serve** (the review's 1 and 3): a run whose reads would not fit
  the window (Draft far above the root, a 96 kHz file, past the top mip level, a wide bend)
  is cut shorter, down to one output frame; when even one frame's reads are wider than the
  window, a read it misses is copied from the pool on the spot into the same dot product —
  as many frames as the blocks there cover, down to one (second review #17). A wide span is
  never a late read; only a block that is not in the pool is (and a block evicted under a
  wide read is counted as late too).
- **Keeping up with fast voices** (third review #8): a voice wakes the reader when it needs
  it sooner than the reader's next look (which Windows' timer makes up to 16 ms) — see
  *the voices' wake* below — and asks for as many blocks ahead as one audio
  callback of up to 1 024 frames reads at its speed; a zone played more than 8 frames of its
  file a frame keeps the blocks right after its head ready too (Draft: level 0 only, the
  only one it reads). **Beyond what the reader serves** — the general rule: a voice whose
  local ratio (its file's rate over the output's, times 2^(semitones/12), over 2^level)
  exceeds a quarter of what the reader keeps ready per millisecond of output (3 blocks ×
  4 096 frames / 4 / (rate/1000): 64 at 48 kHz, 70 at 44.1 kHz, 32 at 96 kHz) — in practice
  Draft, which reads level 0 only: about 72 semitones up for a file at the output's rate,
  48 for a 192 kHz file at 48 kHz — is decided at its start to **play its head and end
  there**, alike in real time and offline: never late, never an offline stall. Such a note
  is **counted** (`EngineStats::stream_cut`), said once a session on the status line,
  counted on the status bar, and reported with an export's result (`Rendered::cut_notes`,
  `StemsDone::cut_notes`): Normal quality, or a sample kept in memory, plays it whole
  (second review #17, third #8, #9).
- **Voices let go**: an ended voice's playhead and a removed zone's source drop their
  stream at once (on the audio thread: never the last holder — the hub's entry and the
  sample cache's head are — so never a free, counted by `rt_memory`), so a stream nothing
  plays is released while its instrument lives (second review #2).
- **Reading in place** from a pool block, without the copy, was considered for the unity case:
  the slots are atomics behind a seqlock, so reading them in place would mean scalar atomic
  loads in the dot product and no way to reject a torn read after it was played — the copy
  stays, done in bulk only when the window runs out.

### 5. A late read in real time: a 5 ms fade, counted and shown

Because the window always holds a fade's worth more than the next control step needs, a
voice whose next block is not in the pool when it needs it is **faded out over 5 ms** with
audio it already has (never a click, never a silent gap that differs unnoticed) and ended;
the engine counts it in `EngineStats::stream_late` — next to the overruns, the dropped
events and the capture overruns — and the status line's engine figures name it. A real-time
render that counted none is the same, bit for bit, as the in-memory render.

### 6. Offline waits

An offline render (`render.rs`, the stems) sets its units *offline*
(`InstrumentUnit::set_offline`): a voice whose block is not there waits — it wakes the reader
and sleeps a few tens of microseconds, on whichever thread renders it, at most the pool's
patience (10 s by the clock, not by a count of sleeps) and never while it already fades — so **offline equals
real time to the bit whenever real time had no late read**, and parallel equals serial
(every layer's copy is its own; the pool is read-only to voices). A voice that still ends
early offline (a source that cannot be read at all, a disk slower than the patience) fails
the render, naming the file: an export is whole or it is not made.

**A zone's start past its head is read before its zone table is used** (second review #16):
`attach`, on the compile thread — the interface's — computes the start pins every configured
Exemplar will publish (a superset: every level its zones keep) and hands them to the reader
on a board of the sample cache's **without waiting** (third review #6). **Only that
Exemplar's zone table waits** (fourth review #2–#4): the snapshot goes to the engine at once
— mute, solo, transport, arming, tempo, effects, every other edit — with the tracks whose new
starts are not read yet keeping the zones (parameters, texts and samples) they had; the
newest snapshot follows from `EngineHandle::flush_pending`, every frame and without compiling
anything, once their blocks are in the pool — at most `HOLD_BOUND` (250 ms) from the first
change that waited, a bound later changes never restart; reached, the tables go anyway and
are counted (`tables_late`), their notes late if they come before the blocks (counted too).
A recording started meanwhile sees the armed tracks of what was sent. `configure` wakes the
reader too. A note in the very block a zone's start changes in plays as in memory; a new
instrument's first note may be late on a slow disk (counted).
**Playing voices come first** (third review #7): the reader reads at most two start blocks a
look before it looks at the cursors again. **A start's mip levels are built in one chain**
(fourth review #6): the level-0 span its deepest level covers read once and halved level by
level over just the spans the levels above need — the very numbers a block alone gets —
one start a look, after the cursors; a block of that span that fails to read is recovered
as any other — the very block found and read again, then the source decoded — never the span
read again look after look (last review #1). A full-range zone's start edit (7 levels, 4 blocks) reads
1.07 M level-0 frames for a deepest span of 1.05 M at every start; reading each block's levels
down to the file read 0.47–1.43 M depending on the start.

**The voices' wake — a scoped exception to ADR 0005 rule 3** (as amended by ADR 0031 §2.4;
fourth review #1). A streamed voice that publishes a new cursor calls the reader's
`Thread::unpark` — a non-blocking wake system call when the reader is parked:
`WakeByAddressSingle` on Windows, `futex(FUTEX_WAKE | FUTEX_PRIVATE)` on Linux,
`semaphore_signal` (through `dispatch_semaphore_signal`) on macOS; no lock, no allocation, no
lazy initialisation (the parker is built with the thread; the hub is a plain load). **Only
when the reader is needed sooner than its next look**: the voice reads fast (`ahead` > 1
block), it moved to another mip level, or a block it will reach before the reader's next
look — the current one and the `READ_AHEAD + ahead − 1` after it, past its head: those its
previous cursor already asked for — is not in the pool (the one newly wanted at the far end
is a look or more away). A voice crossing blocks the reader already keeps ready makes none:
measured with the real reader, a fresh note at unity at real-time pace, 0 wakes in 3 s (31
blocks crossed) on a normal disk and on one slowed to 2 ms a block — the earlier rule, which
also counted the far block, woke once a block (last review #2). At most one per voice per
control step (32 frames), so per callback at most `voices × ⌈frames / 32⌉`, and in practice
one per voice per block it reaches before the reader. **Counted**: `EngineStats::stream_wakes`
(per unit, summed as `stream_late` is). The RTSan gate planned by ADR 0031 lists it as a
permitted, scoped exception: `Thread::unpark` from `Playhead::publish` (`cantus-dsp`
`exemplar_window.rs`), through `Pool::wake`.

### 7. Loading off the interface's thread, with progress

- In a live engine (`SampleCache::set_background`, on since ADR 0025 §9 for audio clips),
  **an instrument's samples load on loader threads too** — as many as the machine has cores
  less two, at most eight, **decoding in parallel**. `attach` hands back what is ready and
  queues the rest; a zone whose sample has not arrived is silent, and the snapshot is sent
  again as samples arrive (`poll_loaded`, every frame).
- **Notes before the sample is there**: a note that starts while its zone has no sample
  plays nothing for that zone and does not start it later — the same rule as a missing
  file. The rule is deterministic, and nothing renders differently offline: **an export
  asked for while samples load waits for them** — beside the window: the export dialog's
  progress says "Waiting for samples" and how far, *Stop* drops it, and it starts once they
  are in; the loads it waited for reach the live engine as well.
- **The real window does this from its first snapshot** (second review #4, #11, #12): the
  desktop app goes `run` → `boot::Boot` → `CantusApp::assemble`, and `assemble` starts the
  window's services before the first snapshot — the cache in the data folder with the
  Settings limit and its start sweep, loading and streaming beside the window with or
  without an audio device, the built-in sounds computed on a thread.
- **Progress**: `SampleCache::progress()` — files done of the files asked for in the current
  batch, instruments' and audio clips' alike (second review #13) — drawn by the kit on the status line ("Loading Salamander Grand Piano… 120 of 397
  samples") and in the instrument window's header, in both languages; the window stays
  responsive throughout.
- **A streamed zone's waveform** in the editor is read on a thread of its own (from the
  stream's cache file when there is one, else decoded), kept only while that zone is shown,
  never in the engine's cache.
- **Undo during a load**: a file no snapshot asks for any more is taken off the queue at the
  next `attach`; a file being decoded finishes and is kept in the cache (a redo then finds
  it), and the progress counts only what is still wanted. Nothing is left half-written: a
  cache file is written beside its name and renamed into place.

### 8. Alternatives rejected (with numbers)

- **Everything in memory as `i24`/`i16`**, no streaming: 8 layers ≈ 1 GB, 16 ≈ 1.96 GB plus
  mips — still gigabytes, still decoded at every load.
- **Streaming the FLAC files themselves**: FLAC seeks by frame index only after parsing; a
  block would cost a decode of up to 4 096 frames per voice on the reader (≈ 3–5× the I/O
  thread's work) and the seek tables vary by encoder. The cache costs 1.96 GB of disk for the
  whole library (half that for 8 layers) and makes a block one positioned read.
- **Storing the mip levels**: +1.3 GB of disk for a level the piano almost never plays.
- **A pool guarded by `UnsafeCell` in `cantus-rt`** (zero-copy reads from shared slots): the
  voice would read the slots directly, but pinning a block from the audio thread needs a CAS
  per block and per voice, `cantus-dsp` would depend on `cantus-rt`, and `cantus-rt` would
  pass its 2 000-line ceiling (ADR 0031 §2.7). The private window costs one copy per frame and
  keeps every crate as it is.
- **Message passing of `Arc<[f32]>` blocks to the audio thread** (rtrb in, garbage back):
  safe too, but the voices on worker threads (ADR 0038) could not see arrivals without the
  main thread, and an offline voice could not wait on its own.
- **Releasing a pedal-noise zone when its controller leaves the range**: kept only for zones
  that loop (§ ADR 0023's amendment): SFZ players let a non-looping controller-triggered
  sample play out, and Salamander's pedal-up noise silences the pedal-down one by its class.

## Measured (the Salamander, `docs/round26/sampler.md`)

| | Before | After |
|---|---|---|
| 8 layers: load / memory | 16.9 s (test build) / 7.8 s (release) on the interface's thread, 1 914 MiB | first load 10.0 s beside the window (decode 1.8 s + writing 1 GB at this machine's 110 MB/s), later 0.28 s / 99 MiB + 16 MiB pool |
| 16 layers: load / memory | 32.4 s (test build) / 3 732 MiB | later 0.47 s / 159 MiB + 16 MiB pool |
| Import (SFZ + metadata) | 7.9 s | 0.1 s |
| Reader: 40 new voices' first blocks after the head | — | 18–21 ms (the head lasts 171 ms at twice the rate) |
| CPU per voice, every block in the pool (quiet machine, best of 15) | — | resampled +3.5 %, unity +11 % (0.007 % of a core) |

The levels (triad LUFS, riff peak) are the in-memory ones to the decimal.

## Consequences

- The Salamander loads in seconds and holds megabytes, not gigabytes (`sampler.md` has the
  numbers before and after); the first load decodes in parallel off the interface's thread
  and writes the cache; later loads read heads only.
- The cache folder (`stream::set_cache_dir`; the app passes its data folder, tests a
  temporary one of their own — a debug build refuses to stream into the shared fallback
  `%TEMP%/cantus-stream-cache`, so a test that forgets fails instead of leaving files for
  every later run: `inst_stream_fallback.rs`) stays under its limit (Settings); it is a cache — clearing it costs a decode
  at the next load.
- A disk too slow for the voices asked of it is heard as voices fading early and is counted;
  the head and the lookahead are constants to tune with the stress test.
- Each Exemplar allocates its layers' windows (1 024 frames × 2 channels × 256 layers = 2 MiB)
  with the instrument.

## What the tests hold

- `cantus-dsp` (`exemplar_stream_tests.rs`): streamed playback equals in-memory playback to
  the bit — at unity (the exact path), transposed (the resampler, across block edges and the
  head's end), with a mip level, on several threads (the parallel render); a late block in
  real time ends the voice with a 5 ms fade and counts one late read; a torn block (its
  sequence number moved) is never used; the seqlock pool hands back exactly what was written.
- `cantus-dsp`: extreme ratios (Draft five octaves up, a 96 kHz file, past the top mip
  level, a +24-semitone bend) stream exactly as in memory, never late, in real time and
  offline; an offline wait for a block that never comes is bounded.
- `cantus-dsp` (resample): `halve_span` over any range equals the same frames of `halve`.
- `cantus-engine` (`stream_cache` tests, `inst_stream_review.rs`): -0.0 round-trips; a source
  replaced with the same size and time is told apart by its ends (and one changed only in its
  middle by the background check: `inst_stream_review2.rs`); a pre-1970 file streams; a damaged block
  or header is refused, the voice ends, the file goes and the next load decodes again; the
  sweep removes stale parts, gone sources and the least recently used, never a file in use;
  edits keep every streamed zone playing with the same stream; a stream nothing holds is
  released; late starts and shared files stream exactly as in memory; a late start is kept
  ready in real time; an unwritable cache plays from memory and says so once; loads an
  export waited for reach the next poll.
- `cantus-engine` (`stream.rs` tests and `inst_stream.rs`): the cache file round-trips at the
  exact depth (i16, i24, f32 chosen from the data) and is refused when its source changed;
  a streamed instrument renders offline exactly as in memory; real time driven at real-time
  pace with the real reader thread has no late read and the same bits; under a stress test
  (many voices, a slowed reader) late reads are counted and every voice ends without a click;
  the audio thread allocates nothing while streaming (`rt_memory.rs`).
- `cantus-engine` (background loading): `attach` returns at once with the zones silent and a
  progress count, the snapshot is complete after the loads; a path no longer wanted leaves
  the queue; an offline render waits for the loads.
- `cantus-engine` (`inst_stream_review2.rs`, the second review): a damaged block is decoded
  again in the session and every export equals memory; live, at most one note fades and the
  next plays as in memory; an unreadable source fails the export naming the file and is
  reported missing; a dropped unit's boards go and are zeroed; a stream is released while
  its unit lives (a zone removed, a voice ended); a note in the block of a start edit plays
  in real time as in memory; Draft at an extreme ratio plays its head alike offline and in
  real time, and a bend that widens the reads mid-voice streams offline as in memory with
  the real reader; the loading progress counts audio clips; a source changed in its middle
  is found out. `stream_cache` tests: a sweep never deletes a file a stream holds (another
  process's lock), nor one touched while it runs, nor an entry whose drive is away; sweeps at
  once delete only the excess. `stream.rs`: the automatic limit.
- `cantus-engine` (the third review): `inst_stream_fds.rs` — three times the handle budget's
  files stream on (Windows with a budget of 8; Linux, run in WSL, with `RLIMIT_NOFILE`
  lowered to 128), the handles stay within it and an unrelated file still opens;
  `inst_stream_review2.rs` — stems fail naming an unreadable sample and leave no file; the
  whole-source check runs once per file; six damaged files are recovered one at a time; a
  start drag never blocks the compile (the slowest attach of a 40-step drag on a 5 ms-a-block
  disk: 34 µs) and a snapshot waits beside for its starts; start edits never make a voice
  playing another sample late (10 ms-a-block disk); fresh fast notes (Draft 16–48 frames a
  frame, Normal) stream in real time as in memory; an export counts the notes that played
  their start only. `stream_cache` tests: a removed pack's entry held at removal goes at a
  later sweep. `cantus-rt`: the open-files limit is raised (and can be lowered for tests).
  `cantus-ui`: a cut note is told once a session (`stream_cut_status.rs`) and an export's
  result counts it; Exemplar's memory chip shows its tip on hover (`inst_sampler_footer.rs`);
  the automatic cache limit reads whole in Spanish and the knob says *Auto*.
- `cantus-engine` and `cantus-dsp` (the fourth review): a voice at unity crossing resident
  blocks wakes the reader not once, one without its blocks does and is counted
  (`a_voice_wakes_the_reader_only_when_it_is_needed`; `rt_memory`: `stream_wakes` counted);
  a two-second start drag on a 20 ms-a-block disk never delays a mute and lands its zone table
  within 250 ms of the first change that waited
  (`a_start_drag_never_holds_back_the_rest_of_the_project`); a recording started meanwhile records the armed track; a block that never reads
  is decoded from the source, at most two recoveries; a start's mip levels come from one read
  (four starts, 1.07 M frames each, every level as in memory). `cantus-ui`: a waiting zone
  table recompiles nothing and an effect added with it reaches the engine at once
  (`stream_cut_status.rs`); the boot test proves assemble returns while the decoder is held,
  by a gate, not a clock.
- `cantus-ui` (`boot_window.rs`): the boot's `assemble` starts the cache in the data folder
  (the test's own), the Settings limit, the start sweep and loading beside the window before
  the first snapshot; a source check keeps them there and every test that builds the real
  app off the user's folder.
- `cantus-ui`: choosing the piano returns within a frame's budget with a slow decoder; the
  progress is drawn on the status line and in the instrument window in both languages; undo
  during the load leaves no progress and no error; the export waits for the load.
