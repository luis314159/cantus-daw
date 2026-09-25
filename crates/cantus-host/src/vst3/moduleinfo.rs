//! `moduleinfo.json`: what a bundle says about itself, read without loading its binary.
//!
//! Since SDK 3.7.5 a bundle may carry `Contents/Resources/moduleinfo.json` (before 3.7.8:
//! `Contents/moduleinfo.json`), written by the SDK's `moduleinfotool` from the factory.
//! The file is JSON5 (comments and trailing commas allowed); it is turned into JSON here and
//! read with `serde_json`. It names every class with its category, sub-categories, vendor
//! and version, which is everything a [`PluginInfo`] needs except the buses and whether
//! there is an editor; those are filled in from the sub-categories here and made exact when
//! the plug-in is loaded ([`crate::MainThread::info`]).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::scan::{kind_from_subcategories, AUDIO_MODULE_CLASS};
use super::util::tuid_from_string;
use crate::backend::{Format, PluginInfo, PluginKind};

/// Where the file is in a bundle, newest location first.
pub fn location(bundle: &Path) -> Option<PathBuf> {
    [
        bundle
            .join("Contents")
            .join("Resources")
            .join("moduleinfo.json"),
        bundle.join("Contents").join("moduleinfo.json"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Every audio module class the file lists, or `None` when there is no readable file.
pub fn read(bundle: &Path) -> Option<Vec<PluginInfo>> {
    let text = std::fs::read_to_string(location(bundle)?).ok()?;
    parse(&text, bundle)
}

/// Parse the text of a `moduleinfo.json` for the bundle at `path`.
pub fn parse(text: &str, path: &Path) -> Option<Vec<PluginInfo>> {
    let json: Value = serde_json::from_str(&json5_to_json(text)).ok()?;
    let factory_vendor = json
        .get("Factory Info")
        .and_then(|f| f.get("Vendor"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let module_version = json.get("Version").and_then(Value::as_str).unwrap_or("");
    let classes = json.get("Classes")?.as_array()?;
    let mut out = Vec::new();
    for class in classes {
        let s = |key: &str| class.get(key).and_then(Value::as_str).unwrap_or("");
        if s("Category") != AUDIO_MODULE_CLASS {
            continue;
        }
        let Some(tuid) = tuid_from_string(s("CID")) else {
            continue;
        };
        let features: Vec<String> = match class.get("Sub Categories") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            // Some writers keep the factory's `Fx|EQ` form.
            Some(Value::String(s)) => s.split('|').map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        let kind = kind_from_subcategories(&features);
        let vendor = match s("Vendor") {
            "" => factory_vendor,
            v => v,
        };
        let version = match s("Version") {
            "" => module_version,
            v => v,
        };
        let instrument = kind == PluginKind::Instrument;
        out.push(PluginInfo {
            format: Format::Vst3,
            id: super::util::tuid_to_string(&tuid),
            name: s("Name").to_owned(),
            vendor: vendor.to_owned(),
            version: version.to_owned(),
            path: path.to_path_buf(),
            kind,
            // The file does not describe buses or the editor: assume the common case
            // (stereo, an editor) until a load says otherwise.
            main_inputs: if instrument { 0 } else { 2 },
            main_outputs: 2,
            sidechain: false,
            editor: true,
            features,
        });
    }
    Some(out)
}

/// JSON5 → JSON for what `moduleinfo.json` files use: `//` and `/* */` comments and
/// trailing commas. Strings are copied untouched.
pub fn json5_to_json(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                out.push(c);
                i += 1;
                while i < chars.len() {
                    out.push(chars[i]);
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        out.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    if chars[i - 1] == '"' {
                        break;
                    }
                }
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            ',' => {
                // Drop the comma when only whitespace or comments lead to a closer.
                let mut j = i + 1;
                loop {
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if chars.get(j) == Some(&'/') && chars.get(j + 1) == Some(&'/') {
                        while j < chars.len() && chars[j] != '\n' {
                            j += 1;
                        }
                    } else if chars.get(j) == Some(&'/') && chars.get(j + 1) == Some(&'*') {
                        j += 2;
                        while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '/') {
                            j += 1;
                        }
                        j += 2;
                    } else {
                        break;
                    }
                }
                if !matches!(chars.get(j), Some('}') | Some(']')) {
                    out.push(',');
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        // written by moduleinfotool
        "Name": "helloworld",
        "Version": "3.7.5.0",
        "Factory Info": {
            "Vendor": "Steinberg Media Technologies",
            "URL": "http://www.steinberg.net", /* the site */
            "E-Mail": "mailto:info@steinberg.de",
            "Flags": { "Unicode": true, "Classes Discardable": false, },
        },
        "Classes": [
            {
                "CID": "BD58B550F9E5634E9D2EFF39EA0927B1",
                "Category": "Audio Module Class",
                "Name": "Hello // World",
                "Vendor": "",
                "Version": "1.0.0.1",
                "SDKVersion": "VST 3.7.5",
                "Sub Categories": [ "Fx", "EQ", ],
                "Class Flags": 1,
                "Cardinality": 2147483647,
                "Snapshots": [ ],
            },
            {
                "CID": "E8E5CEAA51C8F14ABC3E6E1B8E9F3D6A",
                "Category": "Component Controller Class",
                "Name": "Hello World Controller",
            },
            {
                "CID": "1A2B3C4D1A2B3C4D1A2B3C4D1A2B3C4D",
                "Category": "Audio Module Class",
                "Name": "Synth",
                "Vendor": "Someone",
                "Sub Categories": [ "Instrument", "Synth" ],
            },
        ],
        "Compatibility": [ ],
    }"#;

    #[test]
    fn reads_audio_module_classes_from_json5() {
        let infos = parse(SAMPLE, Path::new("X.vst3")).expect("parses");
        assert_eq!(infos.len(), 2, "the controller class is not a plug-in");
        let fx = &infos[0];
        assert_eq!(fx.id, "BD58B550F9E5634E9D2EFF39EA0927B1");
        assert_eq!(
            fx.name, "Hello // World",
            "comments are not stripped inside strings"
        );
        assert_eq!(
            fx.vendor, "Steinberg Media Technologies",
            "the factory's vendor"
        );
        assert_eq!(fx.version, "1.0.0.1");
        assert_eq!(fx.kind, PluginKind::Effect);
        assert_eq!(fx.features, vec!["Fx".to_owned(), "EQ".to_owned()]);
        assert_eq!((fx.main_inputs, fx.main_outputs), (2, 2));
        let synth = &infos[1];
        assert_eq!(synth.kind, PluginKind::Instrument);
        assert_eq!(
            synth.version, "3.7.5.0",
            "the module's version when the class has none"
        );
        assert_eq!((synth.main_inputs, synth.main_outputs), (0, 2));
    }

    #[test]
    fn broken_files_are_none() {
        assert!(parse("{ \"Classes\": [ }", Path::new("X.vst3")).is_none());
        assert!(parse("{}", Path::new("X.vst3")).is_none());
    }
}
