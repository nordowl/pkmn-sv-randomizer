//! The option registry and user settings.
//!
//! Every option is declared once, next to the section that reads it. The UI renders the registry,
//! settings files store `{id: value}`, and `Settings::normalized` repairs anything stale.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    Pool,
    Pokemon,
    Evolutions,
    Moves,
    Starters,
    GiftsTrades,
    Wild,
    Static,
    Bosses,
    Trainers,
    Raids,
    Items,
    Shops,
    Extras,
    Experimental,
}

impl Page {
    pub const ALL: [Page; 15] = [
        Page::Pool,
        Page::Pokemon,
        Page::Evolutions,
        Page::Moves,
        Page::Starters,
        Page::GiftsTrades,
        Page::Wild,
        Page::Static,
        Page::Bosses,
        Page::Trainers,
        Page::Raids,
        Page::Items,
        Page::Shops,
        Page::Extras,
        Page::Experimental,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Pool => "Species pool",
            Page::Pokemon => "Stats, types & abilities",
            Page::Evolutions => "Evolutions",
            Page::Moves => "Moves & TMs",
            Page::Starters => "Starters",
            Page::GiftsTrades => "Gifts & trades",
            Page::Wild => "Wild Pokémon",
            Page::Static => "Static encounters",
            Page::Bosses => "Titans & legendaries",
            Page::Trainers => "Trainers",
            Page::Raids => "Tera Raids",
            Page::Items => "Items",
            Page::Shops => "Shops",
            Page::Extras => "Other content",
            Page::Experimental => "Experimental",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Upr,
    Sv,
    Pknx,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Data,
    Test,
    Research,
}

#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
    pub help: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    Toggle { default: bool },
    Choice { default: &'static str, choices: Vec<Choice> },
    Number { default: i64, min: i64, max: i64, step: i64, unit: &'static str },
    /// A species pool override. `None` default means "use the global pool".
    Pool,
    /// One species and form, or empty for "random".
    Species,
    Text { default: &'static str, max_len: usize },
}

/// A condition that must hold for an option to apply. The UI greys out options whose
/// requirements fail and shows `reason`.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "when", rename_all = "snake_case")]
pub enum Req {
    On { id: &'static str },
    Off { id: &'static str },
    Is { id: &'static str, value: &'static str },
    Not { id: &'static str, value: &'static str },
}

#[derive(Clone, Debug, Serialize)]
pub struct OptionDef {
    pub id: &'static str,
    pub page: Page,
    pub group: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: Kind,
    pub requires: Vec<Req>,
    pub sources: Vec<Source>,
    pub status: Status,
}

impl OptionDef {
    pub fn default_value(&self) -> OptValue {
        match &self.kind {
            Kind::Toggle { default } => OptValue::Bool(*default),
            Kind::Choice { default, .. } => OptValue::Text((*default).to_owned()),
            Kind::Number { default, .. } => OptValue::Int(*default),
            Kind::Pool => OptValue::Pool(PoolFilter::inherit()),
            Kind::Species => OptValue::Species(SpeciesPick::default()),
            Kind::Text { default, .. } => OptValue::Text((*default).to_owned()),
        }
    }

    /// Returns the value if it fits this option, or None.
    fn accept(&self, value: &OptValue) -> Option<OptValue> {
        match (&self.kind, value) {
            (Kind::Toggle { .. }, OptValue::Bool(_)) => Some(value.clone()),
            (Kind::Choice { choices, .. }, OptValue::Text(t)) => {
                choices.iter().any(|c| c.value == t).then(|| value.clone())
            }
            (Kind::Number { min, max, .. }, OptValue::Int(v)) => Some(OptValue::Int((*v).clamp(*min, *max))),
            (Kind::Pool, OptValue::Pool(p)) => Some(OptValue::Pool(p.clone())),
            (Kind::Species, OptValue::Species(s)) => Some(OptValue::Species(*s)),
            (Kind::Text { max_len, .. }, OptValue::Text(t)) => {
                Some(OptValue::Text(t.chars().take(*max_len).collect()))
            }
            _ => None,
        }
    }
}

/// Builder so section files declare options compactly.
pub struct Opt(OptionDef);

impl Opt {
    pub fn new(id: &'static str, page: Page, group: &'static str, label: &'static str, kind: Kind) -> Opt {
        Opt(OptionDef {
            id,
            page,
            group,
            label,
            help: "",
            kind,
            requires: Vec::new(),
            sources: Vec::new(),
            status: Status::Data,
        })
    }

    pub fn toggle(id: &'static str, page: Page, group: &'static str, label: &'static str, default: bool) -> Opt {
        Opt::new(id, page, group, label, Kind::Toggle { default })
    }

    pub fn choice(
        id: &'static str,
        page: Page,
        group: &'static str,
        label: &'static str,
        default: &'static str,
        choices: &[(&'static str, &'static str, &'static str)],
    ) -> Opt {
        let choices = choices
            .iter()
            .map(|&(value, label, help)| Choice { value, label, help })
            .collect();
        Opt::new(id, page, group, label, Kind::Choice { default, choices })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn number(
        id: &'static str,
        page: Page,
        group: &'static str,
        label: &'static str,
        default: i64,
        min: i64,
        max: i64,
        unit: &'static str,
    ) -> Opt {
        Opt::new(id, page, group, label, Kind::Number { default, min, max, step: 1, unit })
    }

    pub fn help(mut self, help: &'static str) -> Opt {
        self.0.help = help;
        self
    }

    pub fn step(mut self, step: i64) -> Opt {
        if let Kind::Number { step: s, .. } = &mut self.0.kind {
            *s = step;
        }
        self
    }

    pub fn requires(mut self, req: Req) -> Opt {
        self.0.requires.push(req);
        self
    }

    pub fn on(self, id: &'static str) -> Opt {
        self.requires(Req::On { id })
    }

    pub fn is(self, id: &'static str, value: &'static str) -> Opt {
        self.requires(Req::Is { id, value })
    }

    pub fn not(self, id: &'static str, value: &'static str) -> Opt {
        self.requires(Req::Not { id, value })
    }

    pub fn sources(mut self, sources: &[Source]) -> Opt {
        self.0.sources = sources.to_vec();
        self
    }

    pub fn status(mut self, status: Status) -> Opt {
        self.0.status = status;
        self
    }

    pub fn build(self) -> OptionDef {
        self.0
    }
}

/// Which species a section may use. Overrides on individual sections start with `inherit = true`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolFilter {
    pub inherit: bool,
    pub generations: [bool; 9],
    /// Legendary-class species (sub-legendary, legendary, mythical, Ultra Beast) by generation.
    pub legendaries: [bool; 9],
    pub paradox: bool,
    /// Single-stage, first, middle and final evolution stages.
    pub stages: [bool; 4],
    pub alt_forms: bool,
    pub irregular_forms: bool,
    /// When any is true, only species in a ticked regional dex qualify.
    pub regional_dex: [bool; 3],
    /// National Dex numbers.
    pub banned: Vec<u16>,
    pub allowed: Vec<u16>,
}

impl PoolFilter {
    pub fn everything() -> PoolFilter {
        PoolFilter {
            inherit: false,
            generations: [true; 9],
            legendaries: [true; 9],
            paradox: true,
            stages: [true; 4],
            alt_forms: true,
            irregular_forms: false,
            regional_dex: [false; 3],
            banned: Vec::new(),
            allowed: Vec::new(),
        }
    }

    pub fn inherit() -> PoolFilter {
        PoolFilter {
            inherit: true,
            ..PoolFilter::everything()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesPick {
    /// National Dex number; None means "random".
    pub national: Option<u16>,
    pub form: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OptValue {
    Bool(bool),
    Int(i64),
    Text(String),
    Pool(PoolFilter),
    Species(SpeciesPick),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub values: BTreeMap<String, OptValue>,
}

impl Settings {
    pub fn defaults(registry: &[OptionDef]) -> Settings {
        Settings {
            values: registry
                .iter()
                .map(|def| (def.id.to_owned(), def.default_value()))
                .collect(),
        }
    }

    /// Every registry option gets a valid value: unknown ids are dropped, wrong types fall back to
    /// the default, numbers are clamped. Returns one warning per repair.
    pub fn normalized(&self, registry: &[OptionDef]) -> (Settings, Vec<String>) {
        let mut warnings = Vec::new();
        let mut values = BTreeMap::new();
        for def in registry {
            let value = match self.values.get(def.id) {
                None => def.default_value(),
                Some(v) => def.accept(v).unwrap_or_else(|| {
                    warnings.push(format!("option `{}` had an invalid value and was reset", def.id));
                    def.default_value()
                }),
            };
            values.insert(def.id.to_owned(), value);
        }
        for id in self.values.keys().filter(|id| !values.contains_key(*id)) {
            warnings.push(format!("unknown option `{id}` was ignored"));
        }
        (Settings { values }, warnings)
    }

    fn value(&self, id: &str) -> &OptValue {
        self.values
            .get(id)
            .unwrap_or_else(|| panic!("option `{id}` is not in the registry or settings were not normalized"))
    }

    pub fn on(&self, id: &str) -> bool {
        match self.value(id) {
            OptValue::Bool(b) => *b,
            other => panic!("option `{id}` is not a toggle: {other:?}"),
        }
    }

    pub fn choice(&self, id: &str) -> &str {
        match self.value(id) {
            OptValue::Text(t) => t,
            other => panic!("option `{id}` is not a choice: {other:?}"),
        }
    }

    pub fn number(&self, id: &str) -> i64 {
        match self.value(id) {
            OptValue::Int(v) => *v,
            other => panic!("option `{id}` is not a number: {other:?}"),
        }
    }

    pub fn pool(&self, id: &str) -> &PoolFilter {
        match self.value(id) {
            OptValue::Pool(p) => p,
            other => panic!("option `{id}` is not a pool: {other:?}"),
        }
    }

    pub fn species(&self, id: &str) -> SpeciesPick {
        match self.value(id) {
            OptValue::Species(s) => *s,
            other => panic!("option `{id}` is not a species pick: {other:?}"),
        }
    }

    pub fn text(&self, id: &str) -> &str {
        self.choice(id)
    }

    pub fn set(&mut self, id: &str, value: OptValue) {
        self.values.insert(id.to_owned(), value);
    }

    /// Whether every requirement of `def` holds. A disabled option behaves as if switched off.
    pub fn applies(&self, def: &OptionDef) -> bool {
        def.requires.iter().all(|req| match req {
            Req::On { id } => matches!(self.values.get(*id), Some(OptValue::Bool(true))),
            Req::Off { id } => matches!(self.values.get(*id), Some(OptValue::Bool(false))),
            Req::Is { id, value } => matches!(self.values.get(*id), Some(OptValue::Text(t)) if t == value),
            Req::Not { id, value } => !matches!(self.values.get(*id), Some(OptValue::Text(t)) if t == value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Vec<OptionDef> {
        vec![
            Opt::toggle("wild.enabled", Page::Wild, "Wild", "Randomize", false).build(),
            Opt::choice(
                "wild.mode",
                Page::Wild,
                "Wild",
                "Mode",
                "swap",
                &[("swap", "Swap", ""), ("global", "Global 1:1", "")],
            )
            .on("wild.enabled")
            .build(),
            Opt::number("wild.level", Page::Wild, "Wild", "Level", 0, -50, 100, "%").build(),
        ]
    }

    #[test]
    fn normalizing_repairs_and_reports() {
        let reg = registry();
        let mut s = Settings::default();
        s.set("wild.mode", OptValue::Text("nonsense".into()));
        s.set("wild.level", OptValue::Int(500));
        s.set("gone", OptValue::Bool(true));
        let (n, warnings) = s.normalized(&reg);
        assert_eq!(n.choice("wild.mode"), "swap");
        assert_eq!(n.number("wild.level"), 100);
        assert!(!n.on("wild.enabled"));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn requirements_follow_toggles() {
        let reg = registry();
        let mut s = Settings::defaults(&reg);
        assert!(!s.applies(&reg[1]));
        s.set("wild.enabled", OptValue::Bool(true));
        assert!(s.applies(&reg[1]));
    }

    #[test]
    fn settings_round_trip_as_json() {
        let reg = registry();
        let mut s = Settings::defaults(&reg);
        s.set("wild.enabled", OptValue::Bool(true));
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.normalized(&reg).0, s);
    }
}
