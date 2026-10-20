//! The main-thread half: parameters, state, latency, the editor, and servicing what the
//! plug-in asked for (`idle`).

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use ::vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::{
    kCanAutomate, kIsHidden, kIsReadOnly,
};
use ::vst3::Steinberg::Vst::RestartFlags_::{
    kIoChanged, kLatencyChanged, kParamIDMappingChanged, kParamTitlesChanged, kParamValuesChanged,
    kReloadComponent,
};
use ::vst3::Steinberg::Vst::ViewType::kEditor;
use ::vst3::Steinberg::Vst::{
    IComponentTrait, IEditControllerTrait, IUnitInfo, IUnitInfoTrait, ParamID, ParameterInfo,
    String128, UnitInfo,
};
use ::vst3::Steinberg::{
    kPlatformTypeHWND, kPlatformTypeX11EmbedWindowID, FIDString, IBStream, IPlugFrame, IPlugView,
    IPlugViewTrait, ViewRect,
};
use ::vst3::{ComPtr, ComWrapper};
use raw_window_handle::RawWindowHandle;

use super::host_objects::{lock, MemoryStream, PlugFrame};
use super::instance::Instance;
use super::util::{from_char16, ok, tuid_to_string};
use super::vstpreset::Preset;
use crate::backend::{EditorSize, HostError, MainThread, Notice, ParamInfo, PluginInfo};

pub struct MainHalf {
    inst: Arc<Instance>,
    info: PluginInfo,
    /// Values the audio thread saw (host automation, the processor's own changes), for
    /// the controller.
    from_audio: rtrb::Consumer<(ParamID, f64)>,
    /// Units per parameter, for `param_text`.
    units: HashMap<ParamID, String>,
    frame: ComWrapper<PlugFrame>,
    view: Option<ComPtr<IPlugView>>,
    /// Notices found outside `idle` (a state load that changed the latency).
    pending: Vec<Notice>,
}

impl MainHalf {
    pub fn new(
        inst: Arc<Instance>,
        info: PluginInfo,
        from_audio: rtrb::Consumer<(ParamID, f64)>,
    ) -> MainHalf {
        let frame = ComWrapper::new(PlugFrame {
            requests: inst.requests.clone(),
        });
        let mut m = MainHalf {
            inst,
            info,
            from_audio,
            units: HashMap::new(),
            frame,
            view: None,
            pending: Vec::new(),
        };
        m.refresh_units();
        m
    }

    fn infos(&self) -> Vec<ParameterInfo> {
        let Some(ctrl) = self.inst.controller() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        // SAFETY: main-thread controller queries into zeroed structs.
        unsafe {
            for i in 0..ctrl.getParameterCount().max(0) {
                let mut info: ParameterInfo = std::mem::zeroed();
                if ok(ctrl.getParameterInfo(i, &mut info)) {
                    out.push(info);
                }
            }
        }
        out
    }

    fn refresh_units(&mut self) {
        self.units = self
            .infos()
            .iter()
            .map(|i| (i.id, from_char16(&i.units)))
            .collect();
    }

    /// Unit id → "Parent/Child" path, from `IUnitInfo` when the controller has it.
    fn unit_paths(&self) -> HashMap<i32, String> {
        let mut out = HashMap::new();
        let Some(units) = self.inst.controller().and_then(|c| c.cast::<IUnitInfo>()) else {
            return out;
        };
        let mut raw: HashMap<i32, (i32, String)> = HashMap::new();
        // SAFETY: main-thread queries into zeroed structs.
        unsafe {
            for i in 0..units.getUnitCount().max(0) {
                let mut u: UnitInfo = std::mem::zeroed();
                if ok(units.getUnitInfo(i, &mut u)) {
                    raw.insert(u.id, (u.parentUnitId, from_char16(&u.name)));
                }
            }
        }
        for &id in raw.keys() {
            let mut parts = Vec::new();
            let mut at = id;
            // The root unit (0) has no name worth showing; stop at it, or at a loop.
            while at != 0 && parts.len() < 16 {
                let Some((parent, name)) = raw.get(&at) else {
                    break;
                };
                if !name.is_empty() {
                    parts.push(name.clone());
                }
                at = *parent;
            }
            parts.reverse();
            out.insert(id, parts.join("/"));
        }
        out
    }

    fn plain(&self, id: ParamID, norm: f64) -> f64 {
        match self.inst.controller() {
            // SAFETY: main-thread conversion.
            Some(c) => unsafe { c.normalizedParamToPlain(id, norm) },
            None => norm,
        }
    }

    fn normalized(&self, id: ParamID, plain: f64) -> f64 {
        match self.inst.controller() {
            // SAFETY: main-thread conversion.
            Some(c) => unsafe { c.plainParamToNormalized(id, plain) },
            None => plain,
        }
    }

    /// The component's (or controller's) state as bytes; `None` for a controller that
    /// has none to give.
    fn get_state(&self, controller: bool) -> Result<Option<Vec<u8>>, HostError> {
        let stream = ComWrapper::new(MemoryStream::new());
        let s = stream
            .as_com_ref::<IBStream>()
            .ok_or_else(|| HostError::State("stream".into()))?;
        // SAFETY: main-thread state calls into a stream we own.
        let r = unsafe {
            if controller {
                match self.inst.controller() {
                    Some(c) => c.getState(s.as_ptr()),
                    None => return Ok(None),
                }
            } else {
                self.inst.parts.component.getState(s.as_ptr())
            }
        };
        if ok(r) {
            Ok(Some(stream.bytes()))
        } else if controller {
            Ok(None)
        } else {
            Err(HostError::State(
                "the component refused to save its state".into(),
            ))
        }
    }
}

impl MainThread for MainHalf {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn params(&self) -> Vec<ParamInfo> {
        let paths = self.unit_paths();
        // A parameter the controller maps a MIDI controller to (`IMidiMapping`: JUCE makes
        // 2 000 "MIDI CC" ones) is an input the notes drive, not a setting: hidden and
        // read-only, so it is neither listed in the generic editor nor kept in a project
        // (round 20, ADR 0028).
        let midi: std::collections::HashSet<u32> = self
            .inst
            .midi_map
            .iter()
            .flatten()
            .copied()
            .filter(|id| *id != super::instance::NO_MIDI_PARAM)
            .collect();
        self.infos()
            .iter()
            .map(|i| {
                let from_midi = midi.contains(&i.id);
                let default = self.plain(i.id, i.defaultNormalizedValue);
                ParamInfo {
                    id: i.id,
                    name: from_char16(&i.title),
                    group: paths.get(&i.unitId).cloned().unwrap_or_default(),
                    min: self.plain(i.id, 0.0),
                    max: self.plain(i.id, 1.0),
                    default,
                    stepped: i.stepCount > 0,
                    automatable: i.flags & kCanAutomate != 0,
                    hidden: i.flags & kIsHidden != 0 || from_midi,
                    read_only: i.flags & kIsReadOnly != 0 || from_midi,
                }
            })
            .collect()
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        let ctrl = self.inst.controller()?;
        if !self.units.contains_key(&id) {
            return None;
        }
        // SAFETY: main-thread query.
        let norm = unsafe { ctrl.getParamNormalized(id) };
        Some(self.plain(id, norm))
    }

    fn param_text(&self, id: u32, value: f64) -> Option<String> {
        let ctrl = self.inst.controller()?;
        let units = self.units.get(&id)?;
        let norm = self.normalized(id, value);
        // SAFETY: main-thread query into a zeroed string.
        let text = unsafe {
            let mut s: String128 = [0; 128];
            if !ok(ctrl.getParamStringByValue(id, norm, &mut s)) {
                return None;
            }
            from_char16(&s)
        };
        let text = text.trim().to_owned();
        Some(if units.is_empty() || text.ends_with(units.as_str()) {
            text
        } else {
            format!("{text} {units}")
        })
    }

    fn save_state(&mut self) -> Result<Vec<u8>, HostError> {
        let component = self.get_state(false)?.unwrap_or_default();
        let controller = self.get_state(true)?;
        Ok(Preset {
            class_id: self.info.id.clone(),
            component,
            controller,
        }
        .to_bytes())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError> {
        let preset = Preset::from_bytes(data)?;
        if !preset.class_id.eq_ignore_ascii_case(&self.info.id) {
            return Err(HostError::State(format!(
                "the state is for class {}, not {}",
                preset.class_id, self.info.id
            )));
        }
        let comp = ComWrapper::new(MemoryStream::from_bytes(preset.component));
        let cs = comp
            .as_com_ref::<IBStream>()
            .ok_or_else(|| HostError::State("stream".into()))?;
        // SAFETY: main-thread state calls with streams we own.
        unsafe {
            if !ok(self.inst.parts.component.setState(cs.as_ptr())) {
                return Err(HostError::State("the component refused the state".into()));
            }
            if let Some(ctrl) = self.inst.controller() {
                if self.inst.parts.is_separate() {
                    comp.rewind();
                    ctrl.setComponentState(cs.as_ptr());
                }
                if let Some(bytes) = preset.controller {
                    let cont = ComWrapper::new(MemoryStream::from_bytes(bytes));
                    if let Some(s) = cont.as_com_ref::<IBStream>() {
                        ctrl.setState(s.as_ptr());
                    }
                }
            }
        }
        // A state may bring another latency; plug-ins are not all diligent in saying so.
        let before = self.inst.latency.load(Ordering::Acquire);
        let now = self.inst.refresh_latency();
        if now != before {
            self.pending.push(Notice::LatencyChanged(now));
        }
        Ok(())
    }

    fn latency(&self) -> usize {
        self.inst.latency.load(Ordering::Acquire)
    }

    fn has_editor(&self) -> bool {
        self.info.editor && self.inst.controller().is_some()
    }

    fn open_editor(&mut self, parent: RawWindowHandle) -> Result<EditorSize, HostError> {
        if let Some(view) = &self.view {
            return Ok(view_size(view));
        }
        let (platform, handle): (FIDString, *mut c_void) = match parent {
            RawWindowHandle::Win32(h) => (kPlatformTypeHWND, h.hwnd.get() as *mut c_void),
            RawWindowHandle::Xlib(h) => (kPlatformTypeX11EmbedWindowID, h.window as *mut c_void),
            RawWindowHandle::Xcb(h) => (
                kPlatformTypeX11EmbedWindowID,
                h.window.get() as usize as *mut c_void,
            ),
            _ => return Err(HostError::Unsupported("this kind of parent window")),
        };
        let ctrl = self
            .inst
            .controller()
            .ok_or_else(|| HostError::Editor("the plug-in has no controller".into()))?;
        // SAFETY: the SDK's view sequence on the main thread: create, check the platform,
        // give it the frame, attach, ask its size.
        unsafe {
            let view = ComPtr::<IPlugView>::from_raw(ctrl.createView(kEditor))
                .ok_or_else(|| HostError::Editor("the plug-in made no editor".into()))?;
            if !ok(view.isPlatformTypeSupported(platform)) {
                return Err(HostError::Editor(
                    "the editor does not support this window".into(),
                ));
            }
            if let Some(frame) = self.frame.as_com_ref::<IPlugFrame>() {
                view.setFrame(frame.as_ptr());
            }
            if !ok(view.attached(handle, platform)) {
                view.setFrame(std::ptr::null_mut());
                return Err(HostError::Editor("the editor refused the window".into()));
            }
            let size = view_size(&view);
            self.view = Some(view);
            Ok(size)
        }
    }

    fn close_editor(&mut self) {
        if let Some(view) = self.view.take() {
            // SAFETY: detach in the SDK's order, main thread.
            unsafe {
                view.removed();
                view.setFrame(std::ptr::null_mut());
            }
        }
    }

    fn idle(&mut self) -> Vec<Notice> {
        let mut notices = std::mem::take(&mut self.pending);
        self.inst.maps.collect();
        // The controller follows what the processor was told and what it reported.
        // A program change (the program list's parameter) changes the plug-in's whole
        // state: said as such, so the project records one state step — whether or not the
        // plug-in asks for it itself (round 29, review 2 of ADR 0062, #52).
        let program = self.inst.program_param.map(|(id, _)| id);
        let mut program_changed = false;
        if let Some(ctrl) = self.inst.controller() {
            while let Ok((id, norm)) = self.from_audio.pop() {
                program_changed |= Some(id) == program;
                // SAFETY: main-thread call.
                unsafe {
                    ctrl.setParamNormalized(id, norm);
                }
            }
        }
        let (restart, dirty, resized) = {
            let mut r = lock(&self.inst.requests);
            (
                std::mem::take(&mut r.restart),
                std::mem::take(&mut r.dirty),
                r.resized.take(),
            )
        };
        if restart & kLatencyChanged != 0 {
            notices.push(Notice::LatencyChanged(self.inst.refresh_latency()));
        }
        if restart & (kParamTitlesChanged | kParamIDMappingChanged) != 0 {
            self.refresh_units();
            self.inst.maps.post(Box::new(self.inst.param_map()));
            notices.push(Notice::ParamsChanged);
        }
        if restart & kParamValuesChanged != 0 || dirty || program_changed {
            notices.push(Notice::StateDirty);
        }
        if restart & (kIoChanged | kReloadComponent) != 0 {
            notices.push(Notice::RestartNeeded);
        }
        if let Some(size) = resized {
            notices.push(Notice::EditorResized(size));
        }
        notices
    }
}

impl Drop for MainHalf {
    fn drop(&mut self) {
        self.close_editor();
    }
}

/// The view's current size (zero when it does not say).
fn view_size(view: &ComPtr<IPlugView>) -> EditorSize {
    let mut rect = ViewRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: main-thread query into a local.
    unsafe {
        view.getSize(&mut rect);
    }
    EditorSize {
        width: (rect.right - rect.left).max(0) as u32,
        height: (rect.bottom - rect.top).max(0) as u32,
    }
}

/// Load, instantiate and activate one plug-in (`Backend::load`).
pub fn load(
    info: &PluginInfo,
    setup: crate::backend::AudioSetup,
) -> Result<crate::backend::Loaded, HostError> {
    let tuid = super::util::tuid_from_string(&info.id)
        .ok_or_else(|| HostError::NotFound(info.id.clone()))?;
    let module = super::module::open(&info.path)?;
    let parts = super::instance::Parts::create(&module, &tuid)?;
    let (inst, from_ui) = Instance::activate(parts, setup)?;
    let map = inst.param_map();
    // What the file or the scan could not know, now exact.
    let mut info = info.clone();
    info.id = tuid_to_string(&tuid);
    info.main_inputs = inst.layout.buses.main_inputs;
    info.main_outputs = inst.layout.buses.main_outputs;
    info.sidechain = inst.layout.buses.sidechain.is_some();
    let inst = Arc::new(inst);
    let (to_ui, from_audio) = rtrb::RingBuffer::new(TO_UI_RING);
    let audio = super::audio_thread::AudioHalf::new(inst.clone(), map, from_ui, to_ui);
    let main = MainHalf::new(inst, info, from_audio);
    Ok(crate::backend::Loaded {
        main: Box::new(main),
        audio: Box::new(audio),
    })
}

/// Room for values going back to the controller between two `idle()` calls.
const TO_UI_RING: usize = 4096;
