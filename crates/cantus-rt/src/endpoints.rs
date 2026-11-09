//! Changes to the system's audio endpoints, waited for instead of listed again and again
//! (round 38, the scorecard: the device watcher listed every endpoint every 1.5 s — half a
//! point of a core at rest).
//!
//! Windows keeps its endpoints under `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\
//! MMDevices\Audio`: an interface plugged or unplugged, enabled, disabled or renamed, or a
//! new default, changes a value there. [`EndpointChanges`] asks the registry to signal an
//! event on any change below that key (`RegNotifyChangeKeyValue`), and [`EndpointChanges::wait`]
//! waits for it. Elsewhere, and if the key cannot be watched, `wait` sleeps the time asked
//! and says "maybe changed": the caller then lists the devices as it always did.
//!
//! The registration belongs to the thread that made it (Windows signals the event if that
//! thread ends): make it, wait on it and drop it on one thread — the type is not `Send`.

use std::time::Duration;

/// A watch on the system's audio endpoints.
#[derive(Debug)]
pub struct EndpointChanges {
    #[cfg(windows)]
    imp: Option<win::Watch>,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl EndpointChanges {
    /// Watch them, if the system lets us (`watching` says whether it did).
    pub fn open() -> EndpointChanges {
        EndpointChanges {
            #[cfg(windows)]
            imp: win::Watch::open(),
            _not_send: std::marker::PhantomData,
        }
    }

    /// Whether changes are signalled; `false`: [`Self::wait`] only sleeps.
    pub fn watching(&self) -> bool {
        #[cfg(windows)]
        {
            self.imp.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Wait up to `timeout` for a change. `true` when something changed — or may have
    /// (not watching, or the watch failed and was given up).
    pub fn wait(&mut self, timeout: Duration) -> bool {
        #[cfg(windows)]
        if let Some(w) = self.imp.as_mut() {
            match w.wait(timeout) {
                Some(changed) => return changed,
                None => {
                    self.imp = None;
                    return true;
                }
            }
        }
        std::thread::sleep(timeout);
        true
    }
}

#[cfg(windows)]
mod win {
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_NOTIFY,
        REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    };
    use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

    #[derive(Debug)]
    pub struct Watch {
        key: HKEY,
        event: HANDLE,
        armed: bool,
    }

    impl Watch {
        pub fn open() -> Option<Watch> {
            let path: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\0"
                .encode_utf16()
                .collect();
            let mut key: HKEY = std::ptr::null_mut();
            // SAFETY: a NUL-terminated path; `key` is written by the call on success.
            let r = unsafe {
                RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_NOTIFY, &mut key)
            };
            if r != 0 {
                return None;
            }
            // SAFETY: an unnamed auto-reset event, not signalled, default security.
            let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
            if event.is_null() {
                // SAFETY: the key opened above, closed once.
                unsafe { RegCloseKey(key) };
                return None;
            }
            Some(Watch {
                key,
                event,
                armed: false,
            })
        }

        /// Ask the registry to signal the event on the next change; `false` if it refused.
        fn arm(&mut self) -> bool {
            // SAFETY: the key and the event are this watch's own, alive until it drops.
            let r = unsafe {
                RegNotifyChangeKeyValue(
                    self.key,
                    1,
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    self.event,
                    1,
                )
            };
            self.armed = r == 0;
            self.armed
        }

        /// `Some(changed)`; `None` when the registry refused the request.
        pub fn wait(&mut self, timeout: Duration) -> Option<bool> {
            if !self.armed && !self.arm() {
                return None;
            }
            let ms = timeout.as_millis().min(u32::MAX as u128 - 1) as u32;
            // SAFETY: the watch's own event handle.
            match unsafe { WaitForSingleObject(self.event, ms) } {
                WAIT_OBJECT_0 => {
                    // One notification per registration: armed again at once, so a value
                    // written while the caller settles and lists the devices is signalled
                    // to its next wait (if the registry refuses now, the next wait asks).
                    self.armed = false;
                    self.arm();
                    Some(true)
                }
                WAIT_TIMEOUT => Some(false),
                _ => None,
            }
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            // SAFETY: the key and the event this watch opened, each closed once; closing the
            // key ends any registration on it.
            unsafe {
                RegCloseKey(self.key);
                CloseHandle(self.event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_system_times_out_and_an_unwatched_one_says_maybe() {
        let mut w = EndpointChanges::open();
        let t = std::time::Instant::now();
        let changed = w.wait(Duration::from_millis(50));
        assert!(t.elapsed() >= Duration::from_millis(45));
        if !w.watching() {
            assert!(changed, "not watching: always maybe changed");
        }
    }
}
