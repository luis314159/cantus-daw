//! ARA's VST3 binding (`ARAVST3.h`): the three COM interfaces a VST3 plug-in exposes for
//! ARA, written by hand because the header needs the VST3 SDK to parse. The IIDs are the
//! header's (`DECLARE_CLASS_IID`), built with `vst3::uid`, which lays them out as the SDK
//! does on each system.
//!
//! - [`IMainFactory`]: a factory class of category [`MAIN_FACTORY_CLASS`]; its
//!   `getFactory` returns the plug-in's `ARAFactory`.
//! - [`IPlugInEntryPoint`] / [`IPlugInEntryPoint2`]: implemented by the audio processor
//!   component; the host binds the instance to a document controller with
//!   `bindToDocumentControllerWithRoles` **before** the first `setActive`, `setState`,
//!   `getProcessContextRequirements` or view.
//!
//! Each interface carries what the `vst3` crate's generated ones carry: the host calls
//! them through a smart pointer (`ComPtr<IMainFactory>`), and a plug-in written in Rust
//! (the repository's test plug-in) implements them on a `vst3::Class`.

use std::ffi::c_void;

use vst3::com_scrape_types::{
    self, Class, Construct, Guid, Inherits, Interface, SmartPtr, Unknown, Wrapper,
};
use vst3::Steinberg::{FUnknown, FUnknownVtbl, TUID};

use crate::sys::{
    ARADocumentControllerRef, ARAFactory, ARAPlugInExtensionInstance, ARAPlugInInstanceRoleFlags,
};

/// The category of the factory class that is the plug-in's `IMainFactory`.
pub const MAIN_FACTORY_CLASS: &str = "ARA Main Factory Class";

pub const IMAIN_FACTORY_IID: TUID = vst3::uid(0xDB2A1669, 0xFAFD42A5, 0xA82F864F, 0x7B6872EA);
pub const IPLUGIN_ENTRY_POINT_IID: TUID = vst3::uid(0x12814E54, 0xA1CE4076, 0x82B96813, 0x16950BD6);
pub const IPLUGIN_ENTRY_POINT2_IID: TUID =
    vst3::uid(0xCD9A5913, 0xC9EB46D7, 0x96CA53AD, 0xD1DB89F5);

const fn guid(t: TUID) -> Guid {
    let mut g = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        g[i] = t[i] as u8;
        i += 1;
    }
    g
}

unsafe fn query(this: *mut c_void, iid: &Guid) -> Option<*mut c_void> {
    let ptr = this as *mut FUnknown;
    let mut obj = std::ptr::null_mut();
    let r = ((*(*ptr).vtbl).queryInterface)(ptr, iid.as_ptr() as *const TUID, &mut obj);
    (r == vst3::Steinberg::kResultOk).then_some(obj)
}

unsafe fn add_ref(this: *mut c_void) -> usize {
    let ptr = this as *mut FUnknown;
    ((*(*ptr).vtbl).addRef)(ptr) as usize
}

unsafe fn release(this: *mut c_void) -> usize {
    let ptr = this as *mut FUnknown;
    ((*(*ptr).vtbl).release)(ptr) as usize
}

/// Declares one interface: its layout, `Unknown`/`Interface`/`Inherits<FUnknown>`.
macro_rules! interface {
    ($name:ident, $vtbl:ident, $iid:expr) => {
        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct $name {
            pub vtbl: *const $vtbl,
        }
        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}
        unsafe impl Inherits<FUnknown> for $name {}
        impl Unknown for $name {
            unsafe fn query_interface(this: *mut Self, iid: &Guid) -> Option<*mut c_void> {
                query(this as *mut c_void, iid)
            }
            unsafe fn add_ref(this: *mut Self) -> usize {
                add_ref(this as *mut c_void)
            }
            unsafe fn release(this: *mut Self) -> usize {
                release(this as *mut c_void)
            }
        }
        unsafe impl Interface for $name {
            type Vtbl = $vtbl;
            const IID: Guid = guid($iid);
            fn inherits(iid: &Guid) -> bool {
                iid == &Self::IID || FUnknown::inherits(iid)
            }
        }
    };
}

// --- IMainFactory -------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct IMainFactoryVtbl {
    pub base: FUnknownVtbl,
    pub getFactory: unsafe extern "system" fn(this: *mut IMainFactory) -> *const ARAFactory,
}
interface!(IMainFactory, IMainFactoryVtbl, IMAIN_FACTORY_IID);

/// What a plug-in implements for [`IMainFactory`].
pub trait IMainFactoryTrait {
    unsafe fn getFactory(&self) -> *const ARAFactory;
}

/// The host's call.
pub unsafe fn main_factory_get<P>(p: &P) -> *const ARAFactory
where
    P: SmartPtr,
    P::Target: Inherits<IMainFactory>,
{
    let ptr = p.ptr() as *mut IMainFactory;
    ((*(*ptr).vtbl).getFactory)(ptr)
}

unsafe impl<C, W, const OFFSET: isize> Construct<C, W, OFFSET> for IMainFactory
where
    C: IMainFactoryTrait + Class,
    W: Wrapper<C>,
{
    const OBJ: Self = IMainFactory {
        vtbl: &IMainFactoryVtbl {
            base: FUnknown::make_vtbl::<C, W, OFFSET>(),
            getFactory: main_factory_get_factory::<C, W, OFFSET>,
        },
    };
}

unsafe extern "system" fn main_factory_get_factory<C, W, const OFFSET: isize>(
    this: *mut IMainFactory,
) -> *const ARAFactory
where
    C: IMainFactoryTrait + Class,
    W: Wrapper<C>,
{
    let header = (this as *mut u8).offset(-OFFSET) as *mut com_scrape_types::Header<C>;
    (*W::data_from_header(header)).getFactory()
}

// --- IPlugInEntryPoint --------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct IPlugInEntryPointVtbl {
    pub base: FUnknownVtbl,
    pub getFactory: unsafe extern "system" fn(this: *mut IPlugInEntryPoint) -> *const ARAFactory,
    /// Deprecated since ARA 2.0; never called by Cantus.
    pub bindToDocumentController: unsafe extern "system" fn(
        this: *mut IPlugInEntryPoint,
        dc: ARADocumentControllerRef,
    )
        -> *const ARAPlugInExtensionInstance,
}
interface!(
    IPlugInEntryPoint,
    IPlugInEntryPointVtbl,
    IPLUGIN_ENTRY_POINT_IID
);

pub trait IPlugInEntryPointTrait {
    unsafe fn getFactory(&self) -> *const ARAFactory;
}

pub unsafe fn entry_point_get_factory<P>(p: &P) -> *const ARAFactory
where
    P: SmartPtr,
    P::Target: Inherits<IPlugInEntryPoint>,
{
    let ptr = p.ptr() as *mut IPlugInEntryPoint;
    ((*(*ptr).vtbl).getFactory)(ptr)
}

unsafe impl<C, W, const OFFSET: isize> Construct<C, W, OFFSET> for IPlugInEntryPoint
where
    C: IPlugInEntryPointTrait + Class,
    W: Wrapper<C>,
{
    const OBJ: Self = IPlugInEntryPoint {
        vtbl: &IPlugInEntryPointVtbl {
            base: FUnknown::make_vtbl::<C, W, OFFSET>(),
            getFactory: entry_get_factory::<C, W, OFFSET>,
            bindToDocumentController: entry_bind_v1,
        },
    };
}

unsafe extern "system" fn entry_get_factory<C, W, const OFFSET: isize>(
    this: *mut IPlugInEntryPoint,
) -> *const ARAFactory
where
    C: IPlugInEntryPointTrait + Class,
    W: Wrapper<C>,
{
    let header = (this as *mut u8).offset(-OFFSET) as *mut com_scrape_types::Header<C>;
    (*W::data_from_header(header)).getFactory()
}

/// ARA 1's bind: refused (a plug-in written here is ARA 2 only).
unsafe extern "system" fn entry_bind_v1(
    _this: *mut IPlugInEntryPoint,
    _dc: ARADocumentControllerRef,
) -> *const ARAPlugInExtensionInstance {
    std::ptr::null()
}

// --- IPlugInEntryPoint2 -------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct IPlugInEntryPoint2Vtbl {
    pub base: FUnknownVtbl,
    pub bindToDocumentControllerWithRoles:
        unsafe extern "system" fn(
            this: *mut IPlugInEntryPoint2,
            dc: ARADocumentControllerRef,
            known: ARAPlugInInstanceRoleFlags,
            assigned: ARAPlugInInstanceRoleFlags,
        ) -> *const ARAPlugInExtensionInstance,
}
interface!(
    IPlugInEntryPoint2,
    IPlugInEntryPoint2Vtbl,
    IPLUGIN_ENTRY_POINT2_IID
);

pub trait IPlugInEntryPoint2Trait {
    unsafe fn bindToDocumentControllerWithRoles(
        &self,
        dc: ARADocumentControllerRef,
        known: ARAPlugInInstanceRoleFlags,
        assigned: ARAPlugInInstanceRoleFlags,
    ) -> *const ARAPlugInExtensionInstance;
}

pub unsafe fn bind_with_roles<P>(
    p: &P,
    dc: ARADocumentControllerRef,
    known: ARAPlugInInstanceRoleFlags,
    assigned: ARAPlugInInstanceRoleFlags,
) -> *const ARAPlugInExtensionInstance
where
    P: SmartPtr,
    P::Target: Inherits<IPlugInEntryPoint2>,
{
    let ptr = p.ptr() as *mut IPlugInEntryPoint2;
    ((*(*ptr).vtbl).bindToDocumentControllerWithRoles)(ptr, dc, known, assigned)
}

unsafe impl<C, W, const OFFSET: isize> Construct<C, W, OFFSET> for IPlugInEntryPoint2
where
    C: IPlugInEntryPoint2Trait + Class,
    W: Wrapper<C>,
{
    const OBJ: Self = IPlugInEntryPoint2 {
        vtbl: &IPlugInEntryPoint2Vtbl {
            base: FUnknown::make_vtbl::<C, W, OFFSET>(),
            bindToDocumentControllerWithRoles: entry2_bind::<C, W, OFFSET>,
        },
    };
}

unsafe extern "system" fn entry2_bind<C, W, const OFFSET: isize>(
    this: *mut IPlugInEntryPoint2,
    dc: ARADocumentControllerRef,
    known: ARAPlugInInstanceRoleFlags,
    assigned: ARAPlugInInstanceRoleFlags,
) -> *const ARAPlugInExtensionInstance
where
    C: IPlugInEntryPoint2Trait + Class,
    W: Wrapper<C>,
{
    let header = (this as *mut u8).offset(-OFFSET) as *mut com_scrape_types::Header<C>;
    (*W::data_from_header(header)).bindToDocumentControllerWithRoles(dc, known, assigned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The IIDs as `ARAVST3.h` declares them, in the SDK's byte order for this system.
    #[test]
    fn iids_are_the_headers() {
        assert_eq!(
            IMainFactory::IID,
            guid(vst3::uid(0xDB2A1669, 0xFAFD42A5, 0xA82F864F, 0x7B6872EA))
        );
        assert_ne!(IPlugInEntryPoint::IID, IPlugInEntryPoint2::IID);
        assert!(IPlugInEntryPoint2::inherits(&FUnknown::IID));
    }
}
