//! Third-party plug-ins out of Cantus's process (ADR 0063, ADR 0031 phase 6).
//!
//! A plug-in set to run isolated is loaded by a [`BridgedBackend`]: one more
//! [`Backend`](crate::backend::Backend), whose [`MainThread`](crate::backend::MainThread)
//! and [`AudioThread`](crate::backend::AudioThread) are proxies — [`BridgeMain`] and
//! [`BridgeAudio`] — for the real halves, which a helper process (`cantus-sandbox`, the
//! crate of that name) loads with the very same CLAP and VST3 backends. Everything above
//! the contract — the hosted units, the session, the engine, the interface — is unchanged:
//! the engine never learns that a plug-in runs elsewhere.
//!
//! - [`slot`]: one instance's shared memory, where each block travels in lock-step.
//! - [`proto`]: the main-thread calls and the helper's news, on its standard pipes.
//! - [`sandbox`]: the helper processes — started per group, watched, killed when they hang.
//! - [`audio`] and [`main`]: the proxies.
//!
//! **Groups**: *Together* puts every such plug-in in one helper, *By vendor* one per maker,
//! *Individually* one per instance ([`group_key`]).

pub mod audio;
pub mod main;
pub mod proto;
pub mod sandbox;
pub mod slot;

pub use audio::{BridgeAudio, Status};
pub use main::BridgeMain;
pub use sandbox::{Sandbox, Sandboxes};

use crate::backend::{AudioSetup, Backend, Format, HostError, Loaded, PluginInfo};
use cantus_core::PluginIsolation;
use proto::Call;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// The helper group a plug-in runs in, or `None` for *Within Cantus*.
pub fn group_key(isolation: PluginIsolation, info: &PluginInfo) -> Option<String> {
    match isolation {
        PluginIsolation::Within => None,
        PluginIsolation::Together => Some("together".into()),
        // The maker's name as it is (case aside): makers whose names differ only in
        // punctuation are different makers.
        PluginIsolation::ByVendor => Some(format!("vendor-{}", info.vendor.to_lowercase())),
        PluginIsolation::Individually => {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            Some(format!("own-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
        }
    }
}

/// Loads a plug-in into a helper process of a group.
#[derive(Debug, Clone)]
pub struct BridgedBackend {
    pub format: Format,
    pub group: String,
    pub pool: Arc<Sandboxes>,
    /// For an offline render: blocks are waited for without a deadline.
    pub offline: bool,
    /// An offline render's stop: set, its instances stop waiting (and their helper ends).
    pub cancel: Option<Arc<AtomicBool>>,
}

impl Backend for BridgedBackend {
    fn format(&self) -> Format {
        self.format
    }

    fn search_paths(&self) -> Vec<PathBuf> {
        crate::backend_for(self.format).map_or_else(Vec::new, |b| b.search_paths())
    }

    fn is_candidate(&self, path: &Path) -> bool {
        crate::backend_for(self.format).is_some_and(|b| b.is_candidate(path))
    }

    /// Scanning has its own process already (`cantus-scan`): this delegates.
    fn scan(&self, path: &Path) -> Result<Vec<PluginInfo>, HostError> {
        crate::backend_for(self.format)
            .ok_or(HostError::Unsupported("format"))?
            .scan(path)
    }

    fn load(&self, info: &PluginInfo, setup: AudioSetup) -> Result<Loaded, HostError> {
        self.load_async(info, setup, &[], &[])
    }

    fn load_with(
        &self,
        info: &PluginInfo,
        setup: AudioSetup,
        state: &[u8],
        values: &[(u32, f64)],
    ) -> Option<Result<Loaded, HostError>> {
        Some(self.load_async(info, setup, state, values))
    }
}

impl BridgedBackend {
    /// Sends the load — the plug-in, its state and its values in one request — and returns
    /// at once: the main half says when it is loaded (`Notice::Loaded`), and the audio half
    /// passes dry until then; a render's waits for it. Nothing here waits for the helper,
    /// not even a new one's start (round 29, review isolation-5).
    fn load_async(
        &self,
        info: &PluginInfo,
        setup: AudioSetup,
        state: &[u8],
        values: &[(u32, f64)],
    ) -> Result<Loaded, HostError> {
        let sandbox = self.pool.group(&self.group)?;
        let slot = Arc::new(
            slot::Slot::create(setup.max_block)
                .map_err(|e| HostError::Load(format!("shared memory: {e}")))?,
        );
        let caller = main::new_caller(&slot, &sandbox.lobby)?;
        let closer = caller
            .closer(slot.name())
            .map_err(|e| HostError::Load(format!("shared memory: {e}")))?;
        let instance = sandbox.new_instance();
        let inbox = Arc::new(sandbox::Inbox::default());
        let status = Arc::new(Status::new(self.offline));
        sandbox.register(
            instance,
            info,
            (inbox.clone(), status.clone()),
            slot.clone(),
            closer,
        );
        let load = sandbox.post(
            Some(instance),
            Call::Load {
                instance,
                info: Box::new(info.clone()),
                sample_rate: setup.sample_rate,
                max_block: setup.max_block,
                slot: slot.name().to_string(),
                values: values.to_vec(),
            },
            state,
            sandbox::Purpose::Load,
        );
        if load == 0 {
            let why = sandbox
                .why()
                .unwrap_or_else(|| "the plug-in sandbox ended".into());
            sandbox.unregister(instance);
            return Err(HostError::Load(format!(
                "the plug-in sandbox could not start: {why}"
            )));
        }
        let main = BridgeMain::new(
            sandbox,
            self.pool.clone(),
            (instance, load),
            slot.clone(),
            (status, inbox),
            (self.offline, self.cancel.clone().unwrap_or_default()),
            setup.sample_rate,
            info.clone(),
            state,
        );
        // The first audio half reuses the caller made above, whose bells the helper opens
        // when it serves the load.
        let audio = {
            let sandbox = main.sandbox.clone();
            Box::new(BridgeAudio::new(
                (slot, main.sandbox.lobby.clone()),
                caller,
                main.status.clone(),
                (
                    main.offline_parts(),
                    setup.sample_rate,
                    self.pool.deadline_floor(),
                    0,
                ),
                Box::new(move || {
                    sandbox.post(
                        Some(instance),
                        Call::DropAudio { instance },
                        &[],
                        sandbox::Purpose::Ignore,
                    );
                }),
            ))
        };
        Ok(Loaded {
            main: Box::new(main),
            audio,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(vendor: &str) -> PluginInfo {
        PluginInfo {
            format: Format::Clap,
            id: "com.test.x".into(),
            name: "X".into(),
            vendor: vendor.into(),
            version: String::new(),
            path: PathBuf::new(),
            kind: crate::backend::PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain: false,
            editor: false,
            features: Vec::new(),
        }
    }

    /// Where each choice puts a plug-in (review #29).
    #[test]
    fn each_choice_has_its_group() {
        let a = info("Acme Audio");
        assert_eq!(group_key(PluginIsolation::Within, &a), None);
        assert_eq!(
            group_key(PluginIsolation::Together, &a),
            group_key(PluginIsolation::Together, &info("Other"))
        );
        let own1 = group_key(PluginIsolation::Individually, &a);
        let own2 = group_key(PluginIsolation::Individually, &a);
        assert!(own1.is_some() && own1 != own2, "one process per instance");
        assert_eq!(
            group_key(PluginIsolation::ByVendor, &a),
            group_key(PluginIsolation::ByVendor, &info("Acme Audio"))
        );
        assert_ne!(
            group_key(PluginIsolation::ByVendor, &a),
            group_key(PluginIsolation::ByVendor, &info("Other"))
        );
        // Makers whose names differ only in case share one; in punctuation, they do not.
        assert_eq!(
            group_key(PluginIsolation::ByVendor, &a),
            group_key(PluginIsolation::ByVendor, &info("ACME AUDIO"))
        );
        assert_ne!(
            group_key(PluginIsolation::ByVendor, &info("A&B")),
            group_key(PluginIsolation::ByVendor, &info("A-B"))
        );
    }
}
