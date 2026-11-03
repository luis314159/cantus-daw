//! ARA's VST3 side in the host (ADR 0097): finding a module's ARA factory (its
//! "ARA Main Factory Class"), and loading an instance bound to a document controller.

use std::path::Path;
use std::sync::Arc;

use ::vst3::Steinberg::Vst::IComponent;
use ::vst3::Steinberg::{FIDString, IPluginFactoryTrait, PClassInfo, TUID};
use ::vst3::{ComPtr, Interface};
use cantus_ara::host::Factory;
use cantus_ara::sys::{
    kARAEditorRendererRole, kARAEditorViewRole, kARAPlaybackRendererRole, ARADocumentControllerRef,
    ARAEditorViewInterface, ARAEditorViewRef, ARAPlaybackRegionRef, ARAPlugInInstanceRoleFlags,
};
use cantus_ara::vst3::{
    bind_with_roles, main_factory_get, IMainFactory, IPlugInEntryPoint2, MAIN_FACTORY_CLASS,
};

use super::module::Module;
use super::util::ok;
use crate::backend::{AudioSetup, HostError, Loaded, PluginInfo};

/// A module's ARA factory, with the module that keeps it valid.
pub struct AraModule {
    pub factory: Factory,
    /// The `IMainFactory` object: released before the module.
    _main: ComPtr<IMainFactory>,
    pub module: Arc<Module>,
}

// SAFETY: the factory is the plug-in's read-only table; the main factory object is only
// held (released on drop), never called from two threads.
unsafe impl Send for AraModule {}

fn cstr(b: &[std::ffi::c_char]) -> String {
    let bytes: Vec<u8> = b
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The class ids and names of a module's ARA main factory classes.
pub fn main_factory_classes(module: &Module) -> Vec<(TUID, String)> {
    let f = module.factory();
    let mut out = Vec::new();
    // SAFETY: main-thread factory queries into a zeroed struct.
    unsafe {
        for i in 0..f.countClasses().max(0) {
            let mut info: PClassInfo = std::mem::zeroed();
            if ok(f.getClassInfo(i, &mut info)) && cstr(&info.category) == MAIN_FACTORY_CLASS {
                out.push((info.cid, cstr(&info.name)));
            }
        }
    }
    out
}

/// Open `path` and find its ARA factory: the main factory class named `name` (several
/// plug-ins in one binary pair a main factory with a processor by name), else the first.
pub fn open(path: &Path, name: Option<&str>) -> Result<AraModule, HostError> {
    let module = super::module::open(path)?;
    let classes = main_factory_classes(&module);
    let (cid, _) = classes
        .iter()
        .find(|(_, n)| name.is_some_and(|w| w == n))
        .or_else(|| classes.first())
        .ok_or_else(|| HostError::Instantiate("not an ARA plug-in".into()))?;
    let mut obj = std::ptr::null_mut();
    // SAFETY: the factory writes an add-ref'd `IMainFactory*` on success.
    let main = unsafe {
        let r = module.factory().createInstance(
            cid.as_ptr() as FIDString,
            IMainFactory::IID.as_ptr() as FIDString,
            &mut obj,
        );
        if !ok(r) {
            return Err(HostError::Instantiate(
                "the ARA main factory refused".into(),
            ));
        }
        ComPtr::from_raw(obj as *mut IMainFactory)
            .ok_or_else(|| HostError::Instantiate("no ARA main factory".into()))?
    };
    // SAFETY: a valid main factory; its table lives as long as the module.
    let factory =
        unsafe { Factory::new(main_factory_get(&main)) }.map_err(HostError::Instantiate)?;
    Ok(AraModule {
        factory,
        _main: main,
        module,
    })
}

/// The roles Cantus knows and assigns: one instance per bound clip is its playback renderer,
/// its editor renderer and its editor view (the plug-in's own window opens from the clip).
pub const KNOWN_ROLES: ARAPlugInInstanceRoleFlags =
    kARAPlaybackRendererRole | kARAEditorRendererRole | kARAEditorViewRole;
pub const ASSIGNED_ROLES: ARAPlugInInstanceRoleFlags = KNOWN_ROLES;

/// What binding gave: the instance's editor view (null when the plug-in has none).
#[derive(Copy, Clone)]
pub struct Extension {
    pub editor_view: ARAEditorViewRef,
    pub editor_view_iface: *const ARAEditorViewInterface,
}

// SAFETY: refs into the plug-in, used only on the document thread.
unsafe impl Send for Extension {}

/// Load an instance of `info`, bound to the document controller `dc` as the playback
/// renderer, editor renderer and editor view of `regions` — bound and given its regions
/// before it is set up or activated, as ARA requires.
///
/// # Safety
/// `dc` is a live document controller of this plug-in's ARA factory, and `regions` are its
/// playback regions; both outlive the instance.
pub unsafe fn load_bound(
    info: &PluginInfo,
    setup: AudioSetup,
    dc: ARADocumentControllerRef,
    regions: &[ARAPlaybackRegionRef],
) -> Result<(Loaded, Extension), HostError> {
    let mut out = Extension {
        editor_view: std::ptr::null_mut(),
        editor_view_iface: std::ptr::null(),
    };
    let mut bind = |component: &ComPtr<IComponent>| -> Result<(), HostError> {
        let entry = component
            .cast::<IPlugInEntryPoint2>()
            .ok_or_else(|| HostError::Instantiate("no ARA 2 entry point".into()))?;
        // SAFETY: bound once, before activation; the controller outlives the instance (the
        // session drops instances before their document).
        unsafe {
            let ext = bind_with_roles(&entry, dc, KNOWN_ROLES, ASSIGNED_ROLES);
            if ext.is_null() {
                return Err(HostError::Instantiate(
                    "the plug-in refused the ARA binding".into(),
                ));
            }
            let ext = std::ptr::read_unaligned(ext);
            if ext.playbackRendererInterface.is_null() {
                return Err(HostError::Instantiate("no ARA playback renderer".into()));
            }
            let pr = std::ptr::read_unaligned(ext.playbackRendererInterface);
            let add = pr
                .addPlaybackRegion
                .ok_or_else(|| HostError::Instantiate("no addPlaybackRegion".into()))?;
            for r in regions {
                add(ext.playbackRendererRef, *r);
            }
            if !ext.editorRendererInterface.is_null() {
                let er = std::ptr::read_unaligned(ext.editorRendererInterface);
                if let Some(add) = er.addPlaybackRegion {
                    for r in regions {
                        add(ext.editorRendererRef, *r);
                    }
                }
            }
            out.editor_view = ext.editorViewRef;
            out.editor_view_iface = ext.editorViewInterface;
        }
        Ok(())
    };
    let loaded = super::main_thread::load_bound(info, setup, &mut bind)?;
    Ok((loaded, out))
}
