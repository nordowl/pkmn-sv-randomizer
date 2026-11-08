use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use svr_core::rando::options::{OptValue, OptionDef, Settings};

const PRESETS_JSON: &str = include_str!("presets.json");

#[derive(Deserialize)]
struct PresetDef {
    id: String,
    name: String,
    description: String,
    values: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Defaults with the preset applied, already normalized.
    pub settings: Settings,
}

pub fn all(registry: &[OptionDef]) -> Vec<Preset> {
    let defs: Vec<PresetDef> = serde_json::from_str(PRESETS_JSON).expect("presets.json is valid");
    defs.into_iter()
        .map(|def| Preset {
            settings: apply(registry, &def.values),
            id: def.id,
            name: def.name,
            description: def.description,
        })
        .collect()
}

/// Defaults, then every preset value whose option exists, then normalized. Presets name options
/// that other sections may not have registered yet, so unknown ids are skipped without a warning.
pub fn apply(registry: &[OptionDef], values: &Map<String, Value>) -> Settings {
    let mut settings = Settings::defaults(registry);
    for (id, value) in values {
        if !registry.iter().any(|def| def.id == id) {
            continue;
        }
        if let Ok(value) = serde_json::from_value::<OptValue>(value.clone()) {
            settings.set(id, value);
        }
    }
    settings.normalized(registry).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use svr_core::rando::{self, ALLOW_TERA_LOCKED};

    #[test]
    fn every_preset_parses_and_has_text() {
        let presets = all(&rando::registry());
        let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["balanced", "chaos", "hard", "light"]);
        assert!(presets.iter().all(|p| !p.name.is_empty() && !p.description.is_empty()));
    }

    #[test]
    fn preset_values_fit_the_options_they_name() {
        let registry = rando::registry();
        let defs: Vec<PresetDef> = serde_json::from_str(PRESETS_JSON).unwrap();
        for def in defs {
            let mut settings = Settings::default();
            for (id, value) in def.values.iter().filter(|(id, _)| registry.iter().any(|d| d.id == *id)) {
                let parsed = serde_json::from_value::<OptValue>(value.clone());
                settings.set(id, parsed.unwrap_or_else(|e| panic!("{}: {id} is not an option value: {e}", def.id)));
            }
            let warnings = settings.normalized(&registry).1;
            assert!(warnings.is_empty(), "preset {} has values its options reject: {warnings:?}", def.id);
        }
    }

    #[test]
    fn unknown_ids_are_skipped() {
        let registry = rando::registry();
        let values: Map<String, Value> = serde_json::from_str(&format!(
            r#"{{"trainers.not_yet_written": true, "{ALLOW_TERA_LOCKED}": true}}"#
        ))
        .unwrap();
        let settings = apply(&registry, &values);
        assert!(settings.on(ALLOW_TERA_LOCKED));
        assert!(!settings.values.contains_key("trainers.not_yet_written"));
        assert_eq!(settings.normalized(&registry).1, Vec::<String>::new());
    }
}
