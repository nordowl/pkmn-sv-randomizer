//! Plain-language lines for the options a user changed from their defaults.

use serde::Serialize;
use svr_core::rando::options::{Kind, OptValue, OptionDef, Page, PoolFilter, Settings, SpeciesPick};
use svr_core::rando::GLOBAL_POOL;

use crate::games::Species;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeLine {
    pub id: String,
    pub page: Page,
    pub group: String,
    pub label: String,
    pub value: String,
    pub default: String,
}

const STAGES: [&str; 4] = ["single-stage", "first-stage", "middle-stage", "final-stage"];
const REGIONAL_DEXES: [&str; 3] = ["Paldea", "Kitakami", "Blueberry"];

/// `species` resolves names when that edition's game is loaded; otherwise dex numbers are shown.
pub fn describe_changes(registry: &[OptionDef], settings: &Settings, species: Option<&Species>) -> Vec<ChangeLine> {
    let (settings, _) = settings.normalized(registry);
    registry
        .iter()
        .filter_map(|def| {
            let value = settings.values.get(def.id)?;
            let default = def.default_value();
            if *value == default {
                return None;
            }
            let shown = describe_value(def, value, species);
            let shown_default = describe_value(def, &default, species);
            (shown != shown_default).then(|| ChangeLine {
                id: def.id.to_owned(),
                page: def.page,
                group: def.group.to_owned(),
                label: def.label.to_owned(),
                value: shown,
                default: shown_default,
            })
        })
        .collect()
}

fn describe_value(def: &OptionDef, value: &OptValue, species: Option<&Species>) -> String {
    match (&def.kind, value) {
        (Kind::Toggle { .. }, OptValue::Bool(on)) => if *on { "On" } else { "Off" }.to_owned(),
        (Kind::Choice { choices, .. }, OptValue::Text(chosen)) => choices
            .iter()
            .find(|choice| choice.value == chosen)
            .map_or_else(|| chosen.clone(), |choice| choice.label.to_owned()),
        (Kind::Number { unit, .. }, OptValue::Int(number)) => with_unit(*number, unit),
        (Kind::Pool, OptValue::Pool(filter)) => describe_pool(filter, def.id == GLOBAL_POOL),
        (Kind::Species, OptValue::Species(pick)) => describe_species(*pick, species),
        (Kind::Text { .. }, OptValue::Text(text)) if text.is_empty() => "(empty)".to_owned(),
        (Kind::Text { .. }, OptValue::Text(text)) => text.clone(),
        (_, other) => format!("{other:?}"),
    }
}

fn with_unit(number: i64, unit: &str) -> String {
    if unit.is_empty() {
        number.to_string()
    } else if unit.chars().any(char::is_alphanumeric) {
        format!("{number} {unit}")
    } else {
        format!("{number}{unit}")
    }
}

fn describe_species(pick: SpeciesPick, species: Option<&Species>) -> String {
    let Some(national) = pick.national else {
        return "Random".to_owned();
    };
    let name = species
        .and_then(|species| species.name_of(national, pick.form))
        .map_or_else(|| format!("No. {national}"), str::to_owned);
    if pick.form == 0 {
        name
    } else {
        format!("{name} (form {})", pick.form)
    }
}

/// What a pool leaves out, in plain words. The global pool has nothing to inherit from.
pub fn describe_pool(filter: &PoolFilter, is_global: bool) -> String {
    if filter.inherit && !is_global {
        return "Uses the global pool".to_owned();
    }
    let mut parts: Vec<String> = Vec::new();
    let generations_off = numbers_where(|gen| !filter.generations[gen]);
    if generations_off.len() == 9 {
        parts.push("no generations at all".to_owned());
    } else if !generations_off.is_empty() {
        parts.push(format!("no Gen {} Pokémon", or_list(&generations_off)));
    }
    let legendaries_off = numbers_where(|gen| filter.generations[gen] && !filter.legendaries[gen]);
    if !legendaries_off.is_empty() {
        if legendaries_off.len() == 9 - generations_off.len() {
            parts.push("no legendaries".to_owned());
        } else {
            parts.push(format!("no legendaries from Gen {}", or_list(&legendaries_off)));
        }
    }
    if !filter.paradox {
        parts.push("no Paradox Pokémon".to_owned());
    }
    let stages_off: Vec<String> = STAGES
        .iter()
        .zip(filter.stages)
        .filter(|(_, allowed)| !allowed)
        .map(|(stage, _)| (*stage).to_owned())
        .collect();
    if !stages_off.is_empty() {
        parts.push(format!("no {} Pokémon", or_list(&stages_off)));
    }
    if !filter.alt_forms {
        parts.push("no alternate forms".to_owned());
    }
    if filter.irregular_forms {
        parts.push("irregular forms allowed".to_owned());
    }
    let dexes: Vec<String> = REGIONAL_DEXES
        .iter()
        .zip(filter.regional_dex)
        .filter(|(_, ticked)| *ticked)
        .map(|(dex, _)| (*dex).to_owned())
        .collect();
    if !dexes.is_empty() {
        let noun = if dexes.len() == 1 { "dex" } else { "dexes" };
        parts.push(format!("only Pokémon in the {} {noun}", and_list(&dexes)));
    }
    if !filter.banned.is_empty() {
        parts.push(format!("{} species banned", filter.banned.len()));
    }
    if !filter.allowed.is_empty() {
        parts.push(format!("{} species always allowed", filter.allowed.len()));
    }
    match parts.split_first_mut() {
        None => "All species".to_owned(),
        Some((first, _)) => {
            *first = capitalize(first);
            parts.join("; ")
        }
    }
}

fn numbers_where(test: impl Fn(usize) -> bool) -> Vec<String> {
    (0..9).filter(|&gen| test(gen)).map(|gen| (gen + 1).to_string()).collect()
}

fn joined(items: &[String], last_word: &str) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} {last_word} {last}", rest.join(", ")),
    }
}

fn or_list(items: &[String]) -> String {
    joined(items, "or")
}

fn and_list(items: &[String]) -> String {
    joined(items, "and")
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use svr_core::rando::options::Opt;
    use svr_core::rando::{self, ALLOW_TERA_LOCKED};

    #[test]
    fn pools_read_as_plain_words() {
        let mut filter = PoolFilter::everything();
        assert_eq!(describe_pool(&filter, true), "All species");
        assert_eq!(describe_pool(&PoolFilter::inherit(), false), "Uses the global pool");
        assert_eq!(describe_pool(&PoolFilter::inherit(), true), "All species");

        filter.generations[0] = false;
        filter.generations[1] = false;
        filter.legendaries = [false; 9];
        filter.paradox = false;
        filter.stages[2] = false;
        filter.regional_dex = [true, true, false];
        filter.banned = vec![25, 133];
        assert_eq!(
            describe_pool(&filter, true),
            "No Gen 1 or 2 Pokémon; no legendaries; no Paradox Pokémon; no middle-stage Pokémon; \
             only Pokémon in the Paldea and Kitakami dexes; 2 species banned"
        );

        let mut some = PoolFilter::everything();
        some.legendaries[2] = false;
        some.legendaries[6] = false;
        assert_eq!(describe_pool(&some, false), "No legendaries from Gen 3 or 7");
    }

    #[test]
    fn only_changed_options_are_listed() {
        let registry = rando::registry();
        let mut settings = Settings::defaults(&registry);
        assert!(describe_changes(&registry, &settings, None).is_empty());
        settings.set(ALLOW_TERA_LOCKED, OptValue::Bool(true));
        let lines = describe_changes(&registry, &settings, None);
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].value.as_str(), lines[0].default.as_str()), ("On", "Off"));
    }

    #[test]
    fn values_of_every_kind_render() {
        let number = Opt::number("x.level", Page::Wild, "G", "Level", 0, -50, 100, "%").build();
        assert_eq!(describe_value(&number, &OptValue::Int(30), None), "30%");
        let levels = Opt::number("x.levels", Page::Wild, "G", "Levels", 0, 0, 9, "levels").build();
        assert_eq!(describe_value(&levels, &OptValue::Int(3), None), "3 levels");
        let pick = Opt::new("x.starter", Page::Starters, "G", "Starter", Kind::Species).build();
        assert_eq!(describe_value(&pick, &OptValue::Species(SpeciesPick::default()), None), "Random");
        let chosen = SpeciesPick {
            national: Some(25),
            form: 0,
        };
        assert_eq!(describe_value(&pick, &OptValue::Species(chosen), None), "No. 25");
        let mode = Opt::choice("x.mode", Page::Wild, "G", "Mode", "a", &[("a", "Area 1:1", ""), ("b", "Global", "")]).build();
        assert_eq!(describe_value(&mode, &OptValue::Text("b".into()), None), "Global");
    }
}
