//! The host's side of ARA's callbacks: the five controller interfaces a document controller
//! calls back into (audio access, archiving, content access, model updates, playback), and
//! the state they read and write.
//!
//! **Threads.** The plug-in may read audio from any of its threads (analysis runs in the
//! background), so audio sources are behind a lock and a reader holds its own `Arc` of the
//! samples; nothing here allocates on a read. Archive readers and writers live only for the
//! one call that uses them. Model update notifications are collected under a lock and taken
//! by the host's main thread.

use std::collections::HashMap;
use std::ffi::{c_void, CString};
use std::sync::{Arc, Mutex};

use cantus_core::SampleData;

use crate::sys::*;

/// Audio the plug-in may read: one recording at its own rate.
pub struct SourceAudio {
    pub data: Arc<SampleData>,
}

impl SourceAudio {
    fn channels(&self) -> usize {
        self.data.channels.max(1) as usize
    }

    fn frames(&self) -> i64 {
        (self.data.frames.len() / self.channels()) as i64
    }
}

/// What a plug-in told the host through the model update controller, keyed by our host refs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notified {
    /// Audio modifications (their host ref) whose content changed, and where: a range in
    /// modification time (seconds), `None` for all of it.
    pub modifications: Vec<(usize, Option<(f64, f64)>)>,
    /// Audio sources whose content changed (analysis results).
    pub sources: Vec<usize>,
    /// Playback regions whose content changed.
    pub regions: Vec<usize>,
    /// The plug-in's private document data changed (it wants to be saved).
    pub document: bool,
}

/// The musical context's content the host publishes: tempo and bars.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Musical {
    /// (seconds, quarters) pairs, ascending; at least two.
    pub tempo: Vec<(f64, f64)>,
    /// (numerator, denominator, position in quarters), ascending.
    pub bars: Vec<(i32, i32, f64)>,
}

/// Everything the callbacks touch. One per document; its address is every controller host
/// ref, so it is boxed and never moves.
#[derive(Default)]
pub struct HostState {
    /// Audio sources by host ref.
    pub sources: Mutex<HashMap<usize, Arc<SourceAudio>>>,
    pub notified: Mutex<Notified>,
    pub musical: Mutex<Musical>,
    /// The document archive id a restore reads (`getDocumentArchiveID`).
    pub restoring_archive_id: Mutex<Option<CString>>,
    /// The plug-in asked to start/stop playback or move the playhead: the host may honour it.
    pub playback_requests: Mutex<Vec<PlaybackRequest>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackRequest {
    Start,
    Stop,
    Position(f64),
}

unsafe fn state<'a, T>(r: *mut T) -> &'a HostState {
    &*(r as *const HostState)
}

// --- audio access ---------------------------------------------------------------------------

struct Reader {
    audio: Arc<SourceAudio>,
    f64: bool,
}

unsafe extern "C" fn create_audio_reader(
    ctrl: ARAAudioAccessControllerHostRef,
    source: ARAAudioSourceHostRef,
    use64: ARABool,
) -> ARAAudioReaderHostRef {
    let st = state(ctrl);
    let audio = st
        .sources
        .lock()
        .ok()
        .and_then(|m| m.get(&(source as usize)).cloned());
    match audio {
        Some(audio) => Box::into_raw(Box::new(Reader {
            audio,
            f64: use64 != 0,
        })) as ARAAudioReaderHostRef,
        None => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn read_audio_samples(
    _ctrl: ARAAudioAccessControllerHostRef,
    reader: ARAAudioReaderHostRef,
    pos: ARASamplePosition,
    count: ARASampleCount,
    buffers: *const *mut c_void,
) -> ARABool {
    if reader.is_null() || buffers.is_null() || count < 0 {
        return 0;
    }
    let r = &*(reader as *const Reader);
    let ch = r.audio.channels();
    let frames = r.audio.frames();
    let data = &r.audio.data.frames;
    for c in 0..ch {
        let buf = *buffers.add(c);
        if buf.is_null() {
            return 0;
        }
        for i in 0..count {
            let f = pos + i;
            let v = if (0..frames).contains(&f) {
                data[f as usize * ch + c]
            } else {
                0.0
            };
            if r.f64 {
                *(buf as *mut f64).add(i as usize) = v as f64;
            } else {
                *(buf as *mut f32).add(i as usize) = v;
            }
        }
    }
    1
}

unsafe extern "C" fn destroy_audio_reader(
    _ctrl: ARAAudioAccessControllerHostRef,
    reader: ARAAudioReaderHostRef,
) {
    if !reader.is_null() {
        drop(Box::from_raw(reader as *mut Reader));
    }
}

pub static AUDIO_ACCESS: ARAAudioAccessControllerInterface = ARAAudioAccessControllerInterface {
    structSize: std::mem::size_of::<ARAAudioAccessControllerInterface>(),
    createAudioReaderForSource: Some(create_audio_reader),
    readAudioSamples: Some(read_audio_samples),
    destroyAudioReader: Some(destroy_audio_reader),
};

// --- archiving --------------------------------------------------------------------------------

/// The bytes one store or restore call reads or writes.
pub struct ArchiveBuf {
    pub bytes: Vec<u8>,
}

unsafe extern "C" fn get_archive_size(
    _ctrl: ARAArchivingControllerHostRef,
    reader: ARAArchiveReaderHostRef,
) -> ARASize {
    (*(reader as *const ArchiveBuf)).bytes.len()
}

unsafe extern "C" fn read_bytes(
    _ctrl: ARAArchivingControllerHostRef,
    reader: ARAArchiveReaderHostRef,
    position: ARASize,
    length: ARASize,
    buffer: *mut ARAByte,
) -> ARABool {
    let a = &*(reader as *const ArchiveBuf);
    match position.checked_add(length) {
        Some(end) if end <= a.bytes.len() => {
            std::ptr::copy_nonoverlapping(a.bytes.as_ptr().add(position), buffer, length);
            1
        }
        _ => 0,
    }
}

unsafe extern "C" fn write_bytes(
    _ctrl: ARAArchivingControllerHostRef,
    writer: ARAArchiveWriterHostRef,
    position: ARASize,
    length: ARASize,
    buffer: *const ARAByte,
) -> ARABool {
    let a = &mut *(writer as *mut ArchiveBuf);
    let Some(end) = position.checked_add(length) else {
        return 0;
    };
    if a.bytes.len() < end {
        a.bytes.resize(end, 0);
    }
    std::ptr::copy_nonoverlapping(buffer, a.bytes.as_mut_ptr().add(position), length);
    1
}

unsafe extern "C" fn archiving_progress(_ctrl: ARAArchivingControllerHostRef, _value: f32) {}

unsafe extern "C" fn get_document_archive_id(
    ctrl: ARAArchivingControllerHostRef,
    _reader: ARAArchiveReaderHostRef,
) -> ARAPersistentID {
    let st = state(ctrl);
    match st
        .restoring_archive_id
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|c| c.as_ptr()))
    {
        // The CString lives in the state until the restore ends.
        Some(p) => p,
        None => std::ptr::null(),
    }
}

pub static ARCHIVING: ARAArchivingControllerInterface = ARAArchivingControllerInterface {
    structSize: std::mem::size_of::<ARAArchivingControllerInterface>(),
    getArchiveSize: Some(get_archive_size),
    readBytesFromArchive: Some(read_bytes),
    writeBytesToArchive: Some(write_bytes),
    notifyDocumentArchivingProgress: Some(archiving_progress),
    notifyDocumentUnarchivingProgress: Some(archiving_progress),
    getDocumentArchiveID: Some(get_document_archive_id),
};

// --- content access ---------------------------------------------------------------------------

enum ContentReader {
    Tempo(Vec<ARAContentTempoEntry>),
    Bars(Vec<ARAContentBarSignature>),
}

unsafe extern "C" fn musical_available(
    _ctrl: ARAContentAccessControllerHostRef,
    _ctx: ARAMusicalContextHostRef,
    kind: ARAContentType,
) -> ARABool {
    (kind == kARAContentTypeTempoEntries || kind == kARAContentTypeBarSignatures) as ARABool
}

unsafe extern "C" fn musical_grade(
    _ctrl: ARAContentAccessControllerHostRef,
    _ctx: ARAMusicalContextHostRef,
    _kind: ARAContentType,
) -> ARAContentGrade {
    kARAContentGradeAdjusted
}

unsafe extern "C" fn musical_reader(
    ctrl: ARAContentAccessControllerHostRef,
    _ctx: ARAMusicalContextHostRef,
    kind: ARAContentType,
    _range: *const ARAContentTimeRange,
) -> ARAContentReaderHostRef {
    let st = state(ctrl);
    let Ok(m) = st.musical.lock() else {
        return std::ptr::null_mut();
    };
    let reader = if kind == kARAContentTypeTempoEntries {
        ContentReader::Tempo(
            m.tempo
                .iter()
                .map(|&(t, q)| ARAContentTempoEntry {
                    timePosition: t,
                    quarterPosition: q,
                })
                .collect(),
        )
    } else if kind == kARAContentTypeBarSignatures {
        ContentReader::Bars(
            m.bars
                .iter()
                .map(|&(n, d, p)| ARAContentBarSignature {
                    numerator: n,
                    denominator: d,
                    position: p,
                })
                .collect(),
        )
    } else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(reader)) as ARAContentReaderHostRef
}

unsafe extern "C" fn source_available(
    _ctrl: ARAContentAccessControllerHostRef,
    _src: ARAAudioSourceHostRef,
    _kind: ARAContentType,
) -> ARABool {
    0
}

unsafe extern "C" fn source_grade(
    _ctrl: ARAContentAccessControllerHostRef,
    _src: ARAAudioSourceHostRef,
    _kind: ARAContentType,
) -> ARAContentGrade {
    kARAContentGradeInitial
}

unsafe extern "C" fn source_reader(
    _ctrl: ARAContentAccessControllerHostRef,
    _src: ARAAudioSourceHostRef,
    _kind: ARAContentType,
    _range: *const ARAContentTimeRange,
) -> ARAContentReaderHostRef {
    std::ptr::null_mut()
}

unsafe extern "C" fn reader_count(
    _ctrl: ARAContentAccessControllerHostRef,
    reader: ARAContentReaderHostRef,
) -> ARAInt32 {
    if reader.is_null() {
        return 0;
    }
    match &*(reader as *const ContentReader) {
        ContentReader::Tempo(v) => v.len() as ARAInt32,
        ContentReader::Bars(v) => v.len() as ARAInt32,
    }
}

unsafe extern "C" fn reader_event(
    _ctrl: ARAContentAccessControllerHostRef,
    reader: ARAContentReaderHostRef,
    index: ARAInt32,
) -> *const c_void {
    if reader.is_null() || index < 0 {
        return std::ptr::null();
    }
    let i = index as usize;
    match &*(reader as *const ContentReader) {
        ContentReader::Tempo(v) => v
            .get(i)
            .map_or(std::ptr::null(), |e| e as *const _ as *const c_void),
        ContentReader::Bars(v) => v
            .get(i)
            .map_or(std::ptr::null(), |e| e as *const _ as *const c_void),
    }
}

unsafe extern "C" fn destroy_reader(
    _ctrl: ARAContentAccessControllerHostRef,
    reader: ARAContentReaderHostRef,
) {
    if !reader.is_null() {
        drop(Box::from_raw(reader as *mut ContentReader));
    }
}

pub static CONTENT_ACCESS: ARAContentAccessControllerInterface =
    ARAContentAccessControllerInterface {
        structSize: std::mem::size_of::<ARAContentAccessControllerInterface>(),
        isMusicalContextContentAvailable: Some(musical_available),
        getMusicalContextContentGrade: Some(musical_grade),
        createMusicalContextContentReader: Some(musical_reader),
        isAudioSourceContentAvailable: Some(source_available),
        getAudioSourceContentGrade: Some(source_grade),
        createAudioSourceContentReader: Some(source_reader),
        getContentReaderEventCount: Some(reader_count),
        getContentReaderDataForEvent: Some(reader_event),
        destroyContentReader: Some(destroy_reader),
    };

// --- model updates ----------------------------------------------------------------------------

unsafe extern "C" fn analysis_progress(
    _ctrl: ARAModelUpdateControllerHostRef,
    _src: ARAAudioSourceHostRef,
    _state: ARAAnalysisProgressState,
    _value: f32,
) {
}

unsafe extern "C" fn source_changed(
    ctrl: ARAModelUpdateControllerHostRef,
    src: ARAAudioSourceHostRef,
    _range: *const ARAContentTimeRange,
    _flags: ARAContentUpdateFlags,
) {
    if let Ok(mut n) = state(ctrl).notified.lock() {
        n.sources.push(src as usize);
    }
}

unsafe extern "C" fn modification_changed(
    ctrl: ARAModelUpdateControllerHostRef,
    m: ARAAudioModificationHostRef,
    range: *const ARAContentTimeRange,
    _flags: ARAContentUpdateFlags,
) {
    let range = (!range.is_null()).then(|| {
        let r = std::ptr::read_unaligned(range);
        (r.start, r.start + r.duration)
    });
    if let Ok(mut n) = state(ctrl).notified.lock() {
        n.modifications.push((m as usize, range));
    }
}

unsafe extern "C" fn region_changed(
    ctrl: ARAModelUpdateControllerHostRef,
    r: ARAPlaybackRegionHostRef,
    _range: *const ARAContentTimeRange,
    _flags: ARAContentUpdateFlags,
) {
    if let Ok(mut n) = state(ctrl).notified.lock() {
        n.regions.push(r as usize);
    }
}

unsafe extern "C" fn document_changed(ctrl: ARAModelUpdateControllerHostRef) {
    if let Ok(mut n) = state(ctrl).notified.lock() {
        n.document = true;
    }
}

pub static MODEL_UPDATE: ARAModelUpdateControllerInterface = ARAModelUpdateControllerInterface {
    structSize: std::mem::size_of::<ARAModelUpdateControllerInterface>(),
    notifyAudioSourceAnalysisProgress: Some(analysis_progress),
    notifyAudioSourceContentChanged: Some(source_changed),
    notifyAudioModificationContentChanged: Some(modification_changed),
    notifyPlaybackRegionContentChanged: Some(region_changed),
    notifyDocumentDataChanged: Some(document_changed),
};

// --- playback -----------------------------------------------------------------------------------

unsafe fn request(ctrl: ARAPlaybackControllerHostRef, r: PlaybackRequest) {
    if let Ok(mut v) = state(ctrl).playback_requests.lock() {
        v.push(r);
    }
}

unsafe extern "C" fn start_playback(ctrl: ARAPlaybackControllerHostRef) {
    request(ctrl, PlaybackRequest::Start)
}

unsafe extern "C" fn stop_playback(ctrl: ARAPlaybackControllerHostRef) {
    request(ctrl, PlaybackRequest::Stop)
}

unsafe extern "C" fn set_position(ctrl: ARAPlaybackControllerHostRef, t: ARATimePosition) {
    request(ctrl, PlaybackRequest::Position(t))
}

unsafe extern "C" fn set_cycle(
    _ctrl: ARAPlaybackControllerHostRef,
    _s: ARATimePosition,
    _d: ARATimeDuration,
) {
}

unsafe extern "C" fn enable_cycle(_ctrl: ARAPlaybackControllerHostRef, _e: ARABool) {}

pub static PLAYBACK: ARAPlaybackControllerInterface = ARAPlaybackControllerInterface {
    structSize: std::mem::size_of::<ARAPlaybackControllerInterface>(),
    requestStartPlayback: Some(start_playback),
    requestStopPlayback: Some(stop_playback),
    requestSetPlaybackPosition: Some(set_position),
    requestSetCycleRange: Some(set_cycle),
    requestEnableCycle: Some(enable_cycle),
};

/// The host instance a document controller is created with: every controller's host ref is
/// the state's address.
pub fn host_instance(state: &HostState) -> ARADocumentControllerHostInstance {
    let r = state as *const HostState as *mut c_void;
    ARADocumentControllerHostInstance {
        structSize: std::mem::size_of::<ARADocumentControllerHostInstance>(),
        audioAccessControllerHostRef: r as _,
        audioAccessControllerInterface: &AUDIO_ACCESS,
        archivingControllerHostRef: r as _,
        archivingControllerInterface: &ARCHIVING,
        contentAccessControllerHostRef: r as _,
        contentAccessControllerInterface: &CONTENT_ACCESS,
        modelUpdateControllerHostRef: r as _,
        modelUpdateControllerInterface: &MODEL_UPDATE,
        playbackControllerHostRef: r as _,
        playbackControllerInterface: &PLAYBACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st_with(ch: u8, frames: &[f32]) -> (Box<HostState>, usize) {
        let st = Box::<HostState>::default();
        let data = Arc::new(SampleData {
            rate: 48_000,
            channels: ch,
            frames: frames.into(),
            stream: None,
        });
        st.sources
            .lock()
            .unwrap()
            .insert(7, Arc::new(SourceAudio { data }));
        (st, 7)
    }

    #[test]
    fn a_reader_reads_each_channel_and_zeros_outside_the_source() {
        let (st, src) = st_with(2, &[1.0, -1.0, 2.0, -2.0]);
        unsafe {
            let ctrl = &*st as *const HostState as ARAAudioAccessControllerHostRef;
            let r = create_audio_reader(ctrl, src as _, 0);
            assert!(!r.is_null());
            let (mut l, mut rr) = ([9.0f32; 4], [9.0f32; 4]);
            let bufs = [
                l.as_mut_ptr() as *mut c_void,
                rr.as_mut_ptr() as *mut c_void,
            ];
            assert_eq!(read_audio_samples(ctrl, r, -1, 4, bufs.as_ptr()), 1);
            assert_eq!(l, [0.0, 1.0, 2.0, 0.0]);
            assert_eq!(rr, [0.0, -1.0, -2.0, 0.0]);
            destroy_audio_reader(ctrl, r);
            assert!(
                create_audio_reader(ctrl, 99 as _, 0).is_null(),
                "an unknown source has no reader"
            );
        }
    }

    #[test]
    fn an_archive_writes_and_reads_back_and_refuses_reads_past_its_end() {
        let mut a = ArchiveBuf { bytes: Vec::new() };
        unsafe {
            let w = &mut a as *mut ArchiveBuf as ARAArchiveWriterHostRef;
            assert_eq!(
                write_bytes(std::ptr::null_mut(), w, 2, 3, b"abc".as_ptr()),
                1
            );
            assert_eq!(a.bytes, b"\0\0abc");
            let r = &a as *const ArchiveBuf as ARAArchiveReaderHostRef;
            assert_eq!(get_archive_size(std::ptr::null_mut(), r), 5);
            let mut out = [0u8; 3];
            assert_eq!(
                read_bytes(std::ptr::null_mut(), r, 2, 3, out.as_mut_ptr()),
                1
            );
            assert_eq!(&out, b"abc");
            assert_eq!(
                read_bytes(std::ptr::null_mut(), r, 4, 3, out.as_mut_ptr()),
                0
            );
        }
    }

    #[test]
    fn tempo_and_bars_are_read_as_published() {
        let st = Box::<HostState>::default();
        *st.musical.lock().unwrap() = Musical {
            tempo: vec![(0.0, 0.0), (1.0, 2.0)],
            bars: vec![(3, 4, 0.0)],
        };
        unsafe {
            let ctrl = &*st as *const HostState as ARAContentAccessControllerHostRef;
            let r = musical_reader(
                ctrl,
                std::ptr::null_mut(),
                kARAContentTypeTempoEntries,
                std::ptr::null(),
            );
            assert_eq!(reader_count(ctrl, r), 2);
            let e = &*(reader_event(ctrl, r, 1) as *const ARAContentTempoEntry);
            assert_eq!((e.timePosition, e.quarterPosition), (1.0, 2.0));
            destroy_reader(ctrl, r);
            let b = musical_reader(
                ctrl,
                std::ptr::null_mut(),
                kARAContentTypeBarSignatures,
                std::ptr::null(),
            );
            let e = &*(reader_event(ctrl, b, 0) as *const ARAContentBarSignature);
            assert_eq!((e.numerator, e.denominator), (3, 4));
            destroy_reader(ctrl, b);
        }
    }
}
