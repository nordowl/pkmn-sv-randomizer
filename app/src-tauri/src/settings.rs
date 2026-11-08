//! Settings as data: the registry payload, lenient parsing of stored values, and settings files.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};
use svr_core::rando::options::{OptValue, OptionDef, Page, Settings};

use crate::errors::io_failure;
use crate::presets::{self, Preset};

const FILE_FORMAT: &str = "svr-settings";
const FILE_VERSION: u64 = 1;

#[derive(Clone, Debug, Serialize)]
pub struct PageInfo {
    pub id: Page,
    pub title: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct RegistryInfo {
    pub pages: Vec<PageInfo>,
    pub options: Vec<OptionDef>,
    pub defaults: Settings,
    pub presets: Vec<Preset>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Normalized {
    pub settings: Settings,
    pub warnings: Vec<String>,
}

pub fn registry_info(registry: &[OptionDef]) -> RegistryInfo {
    RegistryInfo {
        pages: Page::ALL
            .into_iter()
            .map(|page| PageInfo {
                id: page,
                title: page.title(),
            })
            .collect(),
        options: registry.to_vec(),
        defaults: Settings::defaults(registry),
        presets: presets::all(registry),
    }
}

pub fn normalize(registry: &[OptionDef], settings: &Settings) -> Normalized {
    let (settings, warnings) = settings.normalized(registry);
    Normalized { settings, warnings }
}

/// Reads stored values one by one, so one unreadable value only resets that option.
pub fn from_json_values(registry: &[OptionDef], values: Map<String, Value>) -> Normalized {
    let mut settings = Settings::default();
    let mut warnings = Vec::new();
    for (id, value) in values {
        match serde_json::from_value::<OptValue>(value) {
            Ok(value) => settings.set(&id, value),
            Err(_) if registry.iter().any(|def| def.id == id) => {
                warnings.push(format!("option `{id}` had an unreadable value and was reset"));
            }
            Err(_) => warnings.push(format!("unknown option `{id}` was ignored")),
        }
    }
    let mut normalized = normalize(registry, &settings);
    warnings.append(&mut normalized.warnings);
    normalized.warnings = warnings;
    normalized
}

/// The normalized values that differ from their defaults.
pub fn changed_values(registry: &[OptionDef], settings: &Settings) -> BTreeMap<String, OptValue> {
    let (settings, _) = settings.normalized(registry);
    let defaults = Settings::defaults(registry);
    settings
        .values
        .into_iter()
        .filter(|(id, value)| defaults.values.get(id) != Some(value))
        .collect()
}

#[derive(Serialize)]
struct SettingsFile<'a> {
    format: &'static str,
    version: u64,
    values: &'a BTreeMap<String, OptValue>,
}

/// `{ "format": "svr-settings", "version": 1, "values": {...} }`, pretty printed.
pub fn file_bytes(settings: &Settings) -> Vec<u8> {
    let file = SettingsFile {
        format: FILE_FORMAT,
        version: FILE_VERSION,
        values: &settings.values,
    };
    let mut bytes = serde_json::to_vec_pretty(&file).expect("settings serialize");
    bytes.push(b'\n');
    bytes
}

pub fn save_file(path: &Path, registry: &[OptionDef], settings: &Settings) -> Result<(), String> {
    let (settings, _) = settings.normalized(registry);
    fs::write(path, file_bytes(&settings)).map_err(|e| io_failure("save the settings to", path, &e))
}

pub fn load_file(path: &Path, registry: &[OptionDef]) -> Result<Normalized, String> {
    let bytes = fs::read(path).map_err(|e| io_failure("open the settings file", path, &e))?;
    parse_file(&bytes, registry).map_err(|reason| format!("\"{}\" {reason}", path.display()))
}

/// Accepts the full file format and a bare `{ "values": {...} }`.
fn parse_file(bytes: &[u8], registry: &[OptionDef]) -> Result<Normalized, &'static str> {
    const NOT_SETTINGS: &str = "is not a randomizer settings file. Choose a .json file saved from this app.";
    let Ok(Value::Object(mut file)) = serde_json::from_slice::<Value>(bytes) else {
        return Err(NOT_SETTINGS);
    };
    if file.get("format").is_some_and(|format| format != FILE_FORMAT) {
        return Err(NOT_SETTINGS);
    }
    let Some(Value::Object(values)) = file.remove("values") else {
        return Err(NOT_SETTINGS);
    };
    let mut normalized = from_json_values(registry, values);
    if file.get("version").and_then(Value::as_u64).is_some_and(|version| version > FILE_VERSION) {
        normalized.warnings.insert(
            0,
            "This file comes from a newer version of the randomizer, so some of its options may have been skipped.".to_owned(),
        );
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use svr_core::rando::{self, ALLOW_TERA_LOCKED, GLOBAL_POOL};

    #[test]
    fn files_round_trip_and_bare_values_load() {
        let registry = rando::registry();
        let mut settings = Settings::defaults(&registry);
        settings.set(ALLOW_TERA_LOCKED, OptValue::Bool(true));
        let loaded = parse_file(&file_bytes(&settings), &registry).unwrap();
        assert_eq!(loaded.settings, settings);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);

        let bare = format!(r#"{{"values":{{"{ALLOW_TERA_LOCKED}":true,"gone.option":3}}}}"#);
        let loaded = parse_file(bare.as_bytes(), &registry).unwrap();
        assert_eq!(loaded.settings, settings);
        assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);
    }

    #[test]
    fn a_broken_value_resets_only_that_option() {
        let registry = rando::registry();
        let text = format!(r#"{{"values":{{"{GLOBAL_POOL}":{{"inherit":3}},"{ALLOW_TERA_LOCKED}":true}}}}"#);
        let loaded = parse_file(text.as_bytes(), &registry).unwrap();
        assert!(loaded.settings.on(ALLOW_TERA_LOCKED));
        assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);
    }

    #[test]
    fn other_json_is_refused() {
        let registry = rando::registry();
        assert!(parse_file(br#"{"format":"something-else","values":{}}"#, &registry).is_err());
        assert!(parse_file(b"[1,2]", &registry).is_err());
        assert!(parse_file(b"not json", &registry).is_err());
    }
}
