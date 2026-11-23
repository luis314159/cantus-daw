//! A fake `LIBMTS` for `cantus-rt`'s MTS-ESP tests (ADR 0156): the same C symbols as
//! ODDSound's binary-only library (`research/instruments/microtuning-verified.md` §3.2),
//! with its rules as the headers state them — one master at a time, a 12-TET table at A440
//! until a master writes one, the master's table read by every client, unmapped keys
//! filtered. Built by the tests themselves (`rustc --crate-type cdylib`, no dependencies);
//! never shipped. `FAKE_MTS_*` are the tests' own levers: the inter-process flag a crashed
//! master leaves, and a reset between tests. r42:tuning-face

#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Once;

struct Table(UnsafeCell<[f64; 128]>);
// SAFETY: a test fake — the tests take turns; a torn read would show as a wrong pitch.
unsafe impl Sync for Table {}

static TABLE: Table = Table(UnsafeCell::new([0.0; 128]));
static NAME: Table = Table(UnsafeCell::new([0.0; 128])); // 1 024 bytes of name
static INIT: Once = Once::new();
static MASTER: AtomicBool = AtomicBool::new(false);
static IPC: AtomicBool = AtomicBool::new(false);
static CLIENTS: AtomicI32 = AtomicI32::new(0);
static FILTER: [AtomicBool; 128] = [const { AtomicBool::new(false) }; 128];

fn equal() {
    // SAFETY: tests take turns; the table is ours.
    let t = unsafe { &mut *TABLE.0.get() };
    for (k, v) in t.iter_mut().enumerate() {
        *v = 440.0 * 2f64.powf((k as f64 - 69.0) / 12.0);
    }
}

fn init() {
    INIT.call_once(equal);
}

fn name_bytes() -> &'static mut [u8; 1024] {
    // SAFETY: the 128 doubles are 1 024 bytes; tests take turns.
    unsafe { &mut *(NAME.0.get() as *mut [u8; 1024]) }
}

#[no_mangle]
pub extern "C" fn MTS_RegisterClient() {
    init();
    CLIENTS.fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn MTS_DeregisterClient() {
    CLIENTS.fetch_sub(1, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn MTS_HasMaster() -> bool {
    MASTER.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn MTS_HasIPC() -> bool {
    IPC.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn MTS_GetNumClients() -> i32 {
    CLIENTS.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn MTS_ShouldFilterNote(note: c_char, _channel: c_char) -> bool {
    MASTER.load(Ordering::SeqCst) && FILTER[(note as u8 & 127) as usize].load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn MTS_GetTuningTable() -> *const f64 {
    init();
    TABLE.0.get() as *const f64
}

#[no_mangle]
pub extern "C" fn MTS_GetScaleName() -> *const c_char {
    name_bytes().as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn MTS_RegisterMaster(_data: *mut c_void) {
    init();
    MASTER.store(true, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn MTS_DeregisterMaster() {
    MASTER.store(false, Ordering::SeqCst);
    // A client reverts to 12-TET when the master goes (ODDSound's header).
    equal();
    MTS_ClearNoteFilter();
    name_bytes()[0] = 0;
}

#[no_mangle]
pub extern "C" fn MTS_Reinitialize() {
    MTS_DeregisterMaster();
}

#[no_mangle]
pub unsafe extern "C" fn MTS_SetNoteTunings(hz: *const f64) {
    // SAFETY: the caller passes 128 doubles (the API's contract).
    let src = unsafe { std::slice::from_raw_parts(hz, 128) };
    // SAFETY: tests take turns.
    unsafe { (*TABLE.0.get()).copy_from_slice(src) };
}

#[no_mangle]
pub extern "C" fn MTS_SetNoteTuning(hz: f64, note: c_char) {
    // SAFETY: tests take turns.
    unsafe { (*TABLE.0.get())[(note as u8 & 127) as usize] = hz };
}

#[no_mangle]
pub unsafe extern "C" fn MTS_SetScaleName(name: *const c_char) {
    // SAFETY: the caller passes a NUL-terminated string.
    let s = unsafe { std::ffi::CStr::from_ptr(name) }.to_bytes();
    let out = name_bytes();
    let n = s.len().min(out.len() - 1);
    out[..n].copy_from_slice(&s[..n]);
    out[n] = 0;
}

#[no_mangle]
pub extern "C" fn MTS_FilterNote(on: bool, note: c_char, _channel: c_char) {
    FILTER[(note as u8 & 127) as usize].store(on, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn MTS_ClearNoteFilter() {
    for f in &FILTER {
        f.store(false, Ordering::SeqCst);
    }
}

/// The tests' lever: the library shares its state between processes (a crashed master may
/// have left its flag set).
#[no_mangle]
pub extern "C" fn FAKE_MTS_SetIPC(on: bool) {
    IPC.store(on, Ordering::SeqCst);
}

/// The tests' lever: another program registers as the master (or goes).
#[no_mangle]
pub extern "C" fn FAKE_MTS_SetOtherMaster(on: bool) {
    if on {
        MTS_RegisterMaster(std::ptr::null_mut());
    } else {
        MTS_DeregisterMaster();
    }
}
