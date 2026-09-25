//! The state blob: a `.vstpreset` file in memory.
//!
//! A VST3 plug-in has two states — the component's (`IComponent::getState`, what it
//! sounds like) and the controller's (`IEditController::getState`, what its editor looks
//! like) — and the SDK already defines a container for the pair: the preset file
//! (`public.sdk/source/vst/vstpresetfile.h`). Using it as the opaque blob means a project
//! stores exactly what the plug-in wrote, and the same bytes open as a preset in any host.
//!
//! ```text
//! 0   'VST3'                      4 bytes
//! 4   version (1)                 int32 LE
//! 8   class id, 32 ASCII hex      32 bytes
//! 40  offset of the chunk list    int64 LE
//! 48  chunk data …
//!     'List'                      4 bytes
//!     entry count                 int32 LE
//!     per entry: id (4 bytes: 'Comp' | 'Cont' | …), offset int64 LE, size int64 LE
//! ```
//!
//! Cantus writes `Comp` then `Cont` (the latter only when the controller has a state) and
//! reads any chunk order, ignoring chunks it does not know (`Info`, `Prog`).

use crate::backend::HostError;

const HEADER: &[u8; 4] = b"VST3";
const LIST: &[u8; 4] = b"List";
pub const COMPONENT: [u8; 4] = *b"Comp";
pub const CONTROLLER: [u8; 4] = *b"Cont";
const VERSION: i32 = 1;
const HEADER_SIZE: usize = 48;

/// The two states of a plug-in and its class id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preset {
    /// 32 hex digits, as [`super::util::tuid_to_string`] prints them.
    pub class_id: String,
    pub component: Vec<u8>,
    pub controller: Option<Vec<u8>>,
}

impl Preset {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut chunks: Vec<([u8; 4], &[u8])> = vec![(COMPONENT, &self.component)];
        if let Some(c) = &self.controller {
            chunks.push((CONTROLLER, c));
        }
        let data_len: usize = chunks.iter().map(|(_, d)| d.len()).sum();
        let mut out = Vec::with_capacity(HEADER_SIZE + data_len + 8 + 20 * chunks.len());
        out.extend_from_slice(HEADER);
        out.extend_from_slice(&VERSION.to_le_bytes());
        let mut id = [b'0'; 32];
        for (dst, src) in id.iter_mut().zip(self.class_id.bytes()) {
            *dst = src;
        }
        out.extend_from_slice(&id);
        let list_offset = (HEADER_SIZE + data_len) as i64;
        out.extend_from_slice(&list_offset.to_le_bytes());
        let mut entries = Vec::new();
        for (id, data) in &chunks {
            entries.push((*id, out.len() as i64, data.len() as i64));
            out.extend_from_slice(data);
        }
        out.extend_from_slice(LIST);
        out.extend_from_slice(&(entries.len() as i32).to_le_bytes());
        for (id, offset, size) in entries {
            out.extend_from_slice(&id);
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Preset, HostError> {
        let bad = |why: &str| HostError::State(format!("not a VST3 state: {why}"));
        if bytes.len() < HEADER_SIZE || &bytes[0..4] != HEADER {
            return Err(bad("no VST3 header"));
        }
        let class_id = String::from_utf8_lossy(&bytes[8..40]).into_owned();
        let list = i64::from_le_bytes(bytes[40..48].try_into().expect("8 bytes"));
        let list = usize::try_from(list).map_err(|_| bad("chunk list offset"))?;
        let list_end = list
            .checked_add(8)
            .ok_or_else(|| bad("chunk list offset"))?;
        if list_end > bytes.len() || &bytes[list..list + 4] != LIST {
            return Err(bad("no chunk list"));
        }
        let count = i32::from_le_bytes(bytes[list + 4..list + 8].try_into().expect("4 bytes"));
        let mut preset = Preset {
            class_id,
            ..Preset::default()
        };
        let mut has_component = false;
        for i in 0..usize::try_from(count).map_err(|_| bad("entry count"))? {
            let at = list_end + i * 20;
            let entry = bytes
                .get(at..at + 20)
                .ok_or_else(|| bad("chunk list cut short"))?;
            let id: [u8; 4] = entry[0..4].try_into().expect("4 bytes");
            let offset = i64::from_le_bytes(entry[4..12].try_into().expect("8 bytes"));
            let size = i64::from_le_bytes(entry[12..20].try_into().expect("8 bytes"));
            let (Ok(offset), Ok(size)) = (usize::try_from(offset), usize::try_from(size)) else {
                return Err(bad("chunk bounds"));
            };
            let data = offset
                .checked_add(size)
                .and_then(|end| bytes.get(offset..end))
                .ok_or_else(|| bad("chunk outside the data"))?;
            if id == COMPONENT {
                preset.component = data.to_vec();
                has_component = true;
            } else if id == CONTROLLER {
                preset.controller = Some(data.to_vec());
            }
        }
        if !has_component {
            return Err(bad("no component state"));
        }
        Ok(preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_matches_the_sdk_layout() {
        let p = Preset {
            class_id: "6E33225254224A00AA69301AF318797D".into(),
            component: vec![1, 2, 3],
            controller: Some(vec![9; 5]),
        };
        let bytes = p.to_bytes();
        assert_eq!(&bytes[0..4], b"VST3");
        assert_eq!(&bytes[8..40], p.class_id.as_bytes());
        assert_eq!(
            i64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            48 + 3 + 5
        );
        assert_eq!(&bytes[48..51], &[1, 2, 3]);
        assert_eq!(bytes.len(), 48 + 8 + 8 + 2 * 20);
        assert_eq!(Preset::from_bytes(&bytes).unwrap(), p);

        let no_controller = Preset {
            controller: None,
            ..p.clone()
        };
        assert_eq!(
            Preset::from_bytes(&no_controller.to_bytes()).unwrap(),
            no_controller
        );
    }

    #[test]
    fn rejects_what_is_not_a_preset() {
        assert!(Preset::from_bytes(b"").is_err());
        assert!(Preset::from_bytes(&[0u8; 64]).is_err());
        let mut bytes = Preset::default().to_bytes();
        let n = bytes.len();
        bytes[n - 8] = 0xFF; // a chunk that claims to be larger than the file
        assert!(Preset::from_bytes(&bytes).is_err());
    }
}
