//! The COM objects the host gives a plug-in on the main thread: the host context
//! (`IHostApplication`, with `IMessage`/`IAttributeList` for the plug-in's own
//! component ⇄ controller messages), memory streams for state (`IBStream`), the component
//! handler that receives the controller's edits and restart requests, and the editor's
//! frame (`IPlugFrame`).

use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::{Arc, Mutex, MutexGuard};

use ::vst3::Steinberg::Vst::IAttributeList_::AttrID;
use ::vst3::Steinberg::Vst::{
    IAttributeList, IAttributeListTrait, IComponentHandler, IComponentHandler2,
    IComponentHandler2Trait, IComponentHandlerTrait, IHostApplication, IHostApplicationTrait,
    IMessage, IMessageTrait, IPlugInterfaceSupport, IPlugInterfaceSupportTrait, ParamID,
    ParamValue, String128, TChar,
};
use ::vst3::Steinberg::{
    kInvalidArgument, kNoInterface, kNotImplemented, kResultFalse, kResultOk, tresult, FIDString,
    FUnknown, IBStream, IBStreamTrait, IBStream_::IStreamSeekMode_, IPlugFrame, IPlugFrameTrait,
    IPlugView, IPlugViewTrait, ISizeableStream, ISizeableStreamTrait, TBool, ViewRect, TUID,
};
use ::vst3::{Class, ComWrapper, Interface};

use super::util::{char16_len, to_char16};
use crate::backend::EditorSize;

/// Lock a mutex, surviving a poisoned one (a panic elsewhere must not take the plug-in's
/// callbacks down with it).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn same_id(a: *const TUID, b: &[u8; 16]) -> bool {
    // SAFETY: the caller hands a pointer to a 16-byte id, as the SDK requires.
    !a.is_null() && unsafe { std::slice::from_raw_parts(a as *const u8, 16) } == b
}

// ---------------------------------------------------------------------------------------
// IHostApplication

/// The host context given to the factory and to `initialize`.
pub struct HostApplication;

impl HostApplication {
    pub fn new() -> Self {
        HostApplication
    }
}

impl Class for HostApplication {
    type Interfaces = (IHostApplication, IPlugInterfaceSupport);
}

impl IHostApplicationTrait for HostApplication {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        to_char16("Cantus", &mut *name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if obj.is_null() {
            return kInvalidArgument;
        }
        *obj = std::ptr::null_mut();
        let unknown = if same_id(cid, &IMessage::IID) {
            ComWrapper::new(HostMessage::new()).to_com_ptr::<FUnknown>()
        } else if same_id(cid, &IAttributeList::IID) {
            ComWrapper::new(HostAttributes::new()).to_com_ptr::<FUnknown>()
        } else {
            None
        };
        match unknown {
            Some(u) => {
                let ptr = u.as_ptr();
                ((*(*ptr).vtbl).queryInterface)(ptr, iid as *const TUID, obj)
            }
            None => kNoInterface,
        }
    }
}

impl IPlugInterfaceSupportTrait for HostApplication {
    unsafe fn isPlugInterfaceSupported(&self, iid: *const TUID) -> tresult {
        use ::vst3::Steinberg::Vst::{
            IAudioProcessor, IComponent, IConnectionPoint, IEditController, IEventList,
            IParamValueQueue, IParameterChanges, IUnitInfo,
        };
        let supported: [&[u8; 16]; 12] = [
            &IComponent::IID,
            &IAudioProcessor::IID,
            &IEditController::IID,
            &IConnectionPoint::IID,
            &IUnitInfo::IID,
            &IComponentHandler::IID,
            &IComponentHandler2::IID,
            &IParameterChanges::IID,
            &IParamValueQueue::IID,
            &IEventList::IID,
            &IMessage::IID,
            &IPlugFrame::IID,
        ];
        if supported.iter().any(|id| same_id(iid, id)) {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

// ---------------------------------------------------------------------------------------
// IMessage, IAttributeList

pub struct HostMessage {
    id: Mutex<Option<CString>>,
    attributes: ComWrapper<HostAttributes>,
}

impl HostMessage {
    fn new() -> Self {
        HostMessage {
            id: Mutex::new(None),
            attributes: ComWrapper::new(HostAttributes::new()),
        }
    }
}

impl Class for HostMessage {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for HostMessage {
    unsafe fn getMessageID(&self) -> FIDString {
        lock(&self.id)
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr())
    }

    unsafe fn setMessageID(&self, id: FIDString) {
        *lock(&self.id) = (!id.is_null()).then(|| CStr::from_ptr(id).to_owned());
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        // Not add-ref'd: the message owns its list (as the SDK's own host does).
        self.attributes
            .as_com_ref::<IAttributeList>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
}

enum Attr {
    Int(i64),
    Float(f64),
    String(Vec<TChar>),
    Binary(Vec<u8>),
}

pub struct HostAttributes {
    values: Mutex<HashMap<CString, Attr>>,
}

impl HostAttributes {
    fn new() -> Self {
        HostAttributes {
            values: Mutex::new(HashMap::new()),
        }
    }

    unsafe fn set(&self, id: AttrID, value: Attr) -> tresult {
        if id.is_null() {
            return kInvalidArgument;
        }
        lock(&self.values).insert(CStr::from_ptr(id).to_owned(), value);
        kResultOk
    }
}

impl Class for HostAttributes {
    type Interfaces = (IAttributeList,);
}

impl IAttributeListTrait for HostAttributes {
    unsafe fn setInt(&self, id: AttrID, value: i64) -> tresult {
        self.set(id, Attr::Int(value))
    }

    unsafe fn getInt(&self, id: AttrID, value: *mut i64) -> tresult {
        if id.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        match lock(&self.values).get(CStr::from_ptr(id)) {
            Some(Attr::Int(v)) => {
                *value = *v;
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setFloat(&self, id: AttrID, value: f64) -> tresult {
        self.set(id, Attr::Float(value))
    }

    unsafe fn getFloat(&self, id: AttrID, value: *mut f64) -> tresult {
        if id.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        match lock(&self.values).get(CStr::from_ptr(id)) {
            Some(Attr::Float(v)) => {
                *value = *v;
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setString(&self, id: AttrID, string: *const TChar) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let len = char16_len(string);
        let s = std::slice::from_raw_parts(string, len).to_vec();
        self.set(id, Attr::String(s))
    }

    unsafe fn getString(&self, id: AttrID, string: *mut TChar, size_in_bytes: u32) -> tresult {
        if id.is_null() || string.is_null() {
            return kInvalidArgument;
        }
        let capacity = size_in_bytes as usize / std::mem::size_of::<TChar>();
        if capacity == 0 {
            return kInvalidArgument;
        }
        match lock(&self.values).get(CStr::from_ptr(id)) {
            Some(Attr::String(s)) => {
                let n = s.len().min(capacity - 1);
                std::ptr::copy_nonoverlapping(s.as_ptr(), string, n);
                *string.add(n) = 0;
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setBinary(&self, id: AttrID, data: *const c_void, size_in_bytes: u32) -> tresult {
        let bytes = if data.is_null() || size_in_bytes == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(data as *const u8, size_in_bytes as usize).to_vec()
        };
        self.set(id, Attr::Binary(bytes))
    }

    unsafe fn getBinary(
        &self,
        id: AttrID,
        data: *mut *const c_void,
        size_in_bytes: *mut u32,
    ) -> tresult {
        if id.is_null() || data.is_null() || size_in_bytes.is_null() {
            return kInvalidArgument;
        }
        match lock(&self.values).get(CStr::from_ptr(id)) {
            Some(Attr::Binary(b)) => {
                // The pointer stays valid while the attribute is not replaced, as in the
                // SDK's own host.
                *data = b.as_ptr() as *const c_void;
                *size_in_bytes = b.len() as u32;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
}

// ---------------------------------------------------------------------------------------
// IBStream

/// A growable in-memory stream: what `getState` writes into and `setState` reads from.
pub struct MemoryStream {
    inner: Mutex<(Vec<u8>, usize)>,
}

impl MemoryStream {
    pub fn new() -> Self {
        Self::from_bytes(Vec::new())
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        MemoryStream {
            inner: Mutex::new((bytes, 0)),
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        lock(&self.inner).0.clone()
    }

    /// Back to the start, to be read again.
    pub fn rewind(&self) {
        lock(&self.inner).1 = 0;
    }
}

impl Class for MemoryStream {
    type Interfaces = (IBStream, ISizeableStream);
}

impl IBStreamTrait for MemoryStream {
    unsafe fn read(&self, buffer: *mut c_void, num_bytes: i32, num_read: *mut i32) -> tresult {
        if buffer.is_null() || num_bytes < 0 {
            return kInvalidArgument;
        }
        let mut guard = lock(&self.inner);
        let (data, pos) = &mut *guard;
        let n = (num_bytes as usize).min(data.len().saturating_sub(*pos));
        std::ptr::copy_nonoverlapping(data.as_ptr().add(*pos), buffer as *mut u8, n);
        *pos += n;
        if !num_read.is_null() {
            *num_read = n as i32;
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, num_bytes: i32, written: *mut i32) -> tresult {
        if buffer.is_null() || num_bytes < 0 {
            return kInvalidArgument;
        }
        let mut guard = lock(&self.inner);
        let (data, pos) = &mut *guard;
        let n = num_bytes as usize;
        let end = *pos + n;
        if data.len() < end {
            data.resize(end, 0);
        }
        std::ptr::copy_nonoverlapping(buffer as *const u8, data.as_mut_ptr().add(*pos), n);
        *pos = end;
        if !written.is_null() {
            *written = n as i32;
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: i64, mode: i32, result: *mut i64) -> tresult {
        let mut guard = lock(&self.inner);
        let (data, cur) = &mut *guard;
        let base = match mode as u32 {
            m if m == IStreamSeekMode_::kIBSeekSet as u32 => 0,
            m if m == IStreamSeekMode_::kIBSeekCur as u32 => *cur as i64,
            m if m == IStreamSeekMode_::kIBSeekEnd as u32 => data.len() as i64,
            _ => return kInvalidArgument,
        };
        let target = base + pos;
        if target < 0 {
            return kResultFalse;
        }
        *cur = target as usize;
        if !result.is_null() {
            *result = target;
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut i64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        *pos = lock(&self.inner).1 as i64;
        kResultOk
    }
}

impl ISizeableStreamTrait for MemoryStream {
    unsafe fn getStreamSize(&self, size: *mut i64) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        *size = lock(&self.inner).0.len() as i64;
        kResultOk
    }

    unsafe fn setStreamSize(&self, size: i64) -> tresult {
        if size < 0 {
            return kInvalidArgument;
        }
        lock(&self.inner).0.resize(size as usize, 0);
        kResultOk
    }
}

// ---------------------------------------------------------------------------------------
// IComponentHandler, IPlugFrame

/// An edit the controller reported, value normalized (the audio thread converts it).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Edit {
    Begin(ParamID),
    Value(ParamID, ParamValue),
    End(ParamID),
}

/// What the plug-in asked for between two `idle()` calls. Written by the plug-in's calls
/// into the handler and the frame (UI thread), read by the main-thread half.
#[derive(Default)]
pub struct Requests {
    /// `RestartFlags` accumulated.
    pub restart: i32,
    pub dirty: bool,
    pub resized: Option<EditorSize>,
    /// The controller's edits, to the audio thread. VST3 leaves it to the host to send the
    /// editor's changes to the processor; the audio half applies each value at the start
    /// of its next block and reports the gesture in that block's `ParamOut`, the channel
    /// the contract gives an editor's gestures.
    pub to_audio: Option<rtrb::Producer<Edit>>,
    /// Edits the ring to the audio thread had no room for.
    pub dropped: usize,
}

pub type SharedRequests = Arc<Mutex<Requests>>;

pub struct ComponentHandler {
    pub requests: SharedRequests,
}

impl ComponentHandler {
    fn send(&self, edit: Edit) {
        let mut r = lock(&self.requests);
        let sent = r.to_audio.as_mut().is_some_and(|p| p.push(edit).is_ok());
        if !sent {
            r.dropped += 1;
        }
    }
}

impl Class for ComponentHandler {
    type Interfaces = (IComponentHandler, IComponentHandler2);
}

impl IComponentHandlerTrait for ComponentHandler {
    unsafe fn beginEdit(&self, id: ParamID) -> tresult {
        self.send(Edit::Begin(id));
        kResultOk
    }

    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        self.send(Edit::Value(id, value));
        kResultOk
    }

    unsafe fn endEdit(&self, id: ParamID) -> tresult {
        self.send(Edit::End(id));
        kResultOk
    }

    unsafe fn restartComponent(&self, flags: i32) -> tresult {
        lock(&self.requests).restart |= flags;
        kResultOk
    }
}

impl IComponentHandler2Trait for ComponentHandler {
    unsafe fn setDirty(&self, state: TBool) -> tresult {
        if state != 0 {
            lock(&self.requests).dirty = true;
        }
        kResultOk
    }

    unsafe fn requestOpenEditor(&self, _name: FIDString) -> tresult {
        kNotImplemented
    }

    unsafe fn startGroupEdit(&self) -> tresult {
        kResultOk
    }

    unsafe fn finishGroupEdit(&self) -> tresult {
        kResultOk
    }
}

pub struct PlugFrame {
    pub requests: SharedRequests,
    /// The timers and descriptors the editor registered through `IRunLoop` (Linux, ADR
    /// 0065), served by the main half's `idle`.
    #[cfg(target_os = "linux")]
    pub run_loop: Mutex<crate::runloop::RunLoop<run_loop::Handler>>,
}

impl PlugFrame {
    pub fn new(requests: SharedRequests) -> PlugFrame {
        PlugFrame {
            requests,
            #[cfg(target_os = "linux")]
            run_loop: Mutex::default(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl Class for PlugFrame {
    type Interfaces = (IPlugFrame,);
}

/// On Linux the frame is also the editor's run loop (`Steinberg::Linux::IRunLoop`): the
/// SDK's editors ask the frame for it, there being no system loop to hang on (ADR 0065).
#[cfg(target_os = "linux")]
impl Class for PlugFrame {
    type Interfaces = (IPlugFrame, ::vst3::Steinberg::Linux::IRunLoop);
}

#[cfg(target_os = "linux")]
pub mod run_loop {
    use super::{lock_run_loop, PlugFrame};
    use crate::runloop::FD_READ;
    use ::vst3::Steinberg::Linux::{
        FileDescriptor, IEventHandler, IEventHandlerTrait, IRunLoopTrait, ITimerHandler,
        ITimerHandlerTrait, TimerInterval,
    };
    use ::vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultOk, tresult};
    use ::vst3::{ComPtr, ComRef};

    /// Who a timer or a descriptor calls back.
    #[derive(Clone)]
    pub enum Handler {
        Timer(ComPtr<ITimerHandler>),
        Event(ComPtr<IEventHandler>),
    }

    impl Handler {
        fn is_timer(&self, p: *mut ITimerHandler) -> bool {
            matches!(self, Handler::Timer(t) if t.as_ptr() == p)
        }
        fn is_event(&self, p: *mut IEventHandler) -> bool {
            matches!(self, Handler::Event(e) if e.as_ptr() == p)
        }

        /// Call it back: a timer fired, or `fd` is ready.
        ///
        /// # Safety
        /// On the main thread, with no lock of the run loop held.
        pub unsafe fn call(&self, fd: i32) {
            match self {
                Handler::Timer(t) => t.onTimer(),
                Handler::Event(e) => e.onFDIsSet(fd),
            }
        }
    }

    impl IRunLoopTrait for PlugFrame {
        unsafe fn registerEventHandler(
            &self,
            handler: *mut IEventHandler,
            fd: FileDescriptor,
        ) -> tresult {
            let Some(h) = ComRef::from_raw(handler) else {
                return kInvalidArgument;
            };
            let added = lock_run_loop(self).add_fd(fd, FD_READ, Handler::Event(h.to_com_ptr()));
            if added {
                kResultOk
            } else {
                kResultFalse
            }
        }

        unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
            if handler.is_null() {
                return kInvalidArgument;
            }
            match lock_run_loop(self).remove_fds_where(|h| h.is_event(handler)) {
                0 => kResultFalse,
                _ => kResultOk,
            }
        }

        unsafe fn registerTimer(
            &self,
            handler: *mut ITimerHandler,
            milliseconds: TimerInterval,
        ) -> tresult {
            let Some(h) = ComRef::from_raw(handler) else {
                return kInvalidArgument;
            };
            let period = std::time::Duration::from_millis(milliseconds);
            lock_run_loop(self).add_timer(period, Handler::Timer(h.to_com_ptr()));
            kResultOk
        }

        unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
            if handler.is_null() {
                return kInvalidArgument;
            }
            match lock_run_loop(self).remove_timers_where(|h| h.is_timer(handler)) {
                0 => kResultFalse,
                _ => kResultOk,
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn lock_run_loop(
    frame: &PlugFrame,
) -> MutexGuard<'_, crate::runloop::RunLoop<run_loop::Handler>> {
    frame.run_loop.lock().unwrap_or_else(|e| e.into_inner())
}

impl IPlugFrameTrait for PlugFrame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        if view.is_null() || new_size.is_null() {
            return kInvalidArgument;
        }
        let rect = &*new_size;
        let size = EditorSize {
            width: (rect.right - rect.left).max(0) as u32,
            height: (rect.bottom - rect.top).max(0) as u32,
        };
        lock(&self.requests).resized = Some(size);
        // The host's window follows from the notice on the next idle; the view is told
        // now, as the SDK's own host does from inside `resizeView`.
        if let Some(view) = ::vst3::ComRef::from_raw(view) {
            view.onSize(new_size);
        }
        kResultOk
    }
}
