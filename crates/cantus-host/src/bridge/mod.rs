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
use proto::{Answer, Call};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The helper group a plug-in runs in, or `None` for *Within Cantus*.
pub fn group_key(isolation: PluginIsolation, info: &PluginInfo) -> Option<String> {
    match isolation {
        PluginIsolation::Within => None,
        PluginIsolation::Together => Some("together".into()),
        PluginIsolation::ByVendor => {
            let vendor: String = info
                .vendor
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect();
            Some(format!("vendor-{}", vendor.to_lowercase()))
        }
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
        sandbox.register(instance, inbox.clone(), slot.clone(), closer);
        let answer = sandbox.call(
            Some(instance),
            Call::Load {
                instance,
                info: info.clone(),
                sample_rate: setup.sample_rate,
                max_block: setup.max_block,
                slot: slot.name().to_string(),
            },
            &[],
            sandbox::LOAD_TIMEOUT,
        );
        let (described, first_state): (proto::Described, Vec<u8>) = match answer {
            Ok((Answer::Loaded(d), state)) => (d, state),
            Ok(_) => {
                sandbox.unregister(instance);
                return Err(HostError::Load(
                    "the plug-in sandbox did not load it".into(),
                ));
            }
            Err(e) => {
                sandbox.unregister(instance);
                return Err(e);
            }
        };
        let latency = described.latency;
        let status = Arc::new(Status::default());
        let main = BridgeMain::new(
            sandbox,
            self.pool.clone(),
            instance,
            slot.clone(),
            status,
            inbox,
            self.offline,
            setup.sample_rate,
            described,
            first_state,
        );
        // The first audio half reuses the caller made above, whose bells the helper opened.
        let audio = {
            let sandbox = main.sandbox.clone();
            Box::new(BridgeAudio::new(
                (slot, main.sandbox.lobby.clone()),
                caller,
                main.status.clone(),
                (
                    self.offline,
                    setup.sample_rate,
                    self.pool.deadline_floor(),
                    latency,
                ),
                Box::new(move || sandbox.tell(Call::DropAudio { instance }, &[])),
            ))
        };
        Ok(Loaded {
            main: Box::new(main),
            audio,
        })
    }
}
