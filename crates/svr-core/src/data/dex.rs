use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fb::{StructValue, TableValue};

use super::facts::{self, Rarity};
use super::store::{TableStore, CATALOG, PERSONAL};
use super::DataError;

/// A species form by the game's internal species id (`DevID`), which differs from the National Dex
/// number for many Gen 9 species.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pokemon {
    pub species: u16,
    pub form: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evolution {
    pub condition: u16,
    pub parameter: u16,
    pub level: u16,
    pub into: Pokemon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormInfo {
    pub id: Pokemon,
    pub row: usize,
    pub national: u16,
    pub present: bool,
    pub has_model: bool,
    pub types: [u8; 2],
    pub abilities: [u16; 3],
    pub stats: [u8; 6],
    pub evo_stage: u8,
    pub evolutions: Vec<Evolution>,
    pub catch_rate: u8,
    pub xp_growth: u8,
    pub gender_group: u8,
    pub gender_ratio: u8,
    /// Paldea, Kitakami and Blueberry dex numbers; 0 means not in that dex.
    pub regional_dex: [u16; 3],
    pub rarity: Rarity,
    pub paradox: bool,
    pub generation: u8,
}

impl FormInfo {
    pub fn bst(&self) -> u32 {
        self.stats.iter().map(|&s| u32::from(s)).sum()
    }

    /// The randomizer only ever places forms that are flagged present and have model assets.
    pub fn playable(&self) -> bool {
        self.present && self.has_model
    }

    pub fn is_legendary_class(&self) -> bool {
        self.rarity != Rarity::Standard
    }
}

pub struct Dex {
    forms: Vec<FormInfo>,
    index: BTreeMap<Pokemon, usize>,
    pre_evolution: BTreeMap<Pokemon, Pokemon>,
}

impl Dex {
    pub fn load(store: &mut TableStore) -> Result<Dex, DataError> {
        let with_models = catalog_forms(store.get(CATALOG)?)?;
        let personal = store.get(PERSONAL)?;
        let mut forms = Vec::new();
        for (row, entry) in personal.tables("entry").enumerate() {
            forms.push(read_form(row, entry, &with_models)?);
        }
        Ok(Dex::from_forms(forms))
    }

    pub fn from_forms(forms: Vec<FormInfo>) -> Dex {
        let index = forms.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
        let mut pre_evolution = BTreeMap::new();
        for form in forms.iter().filter(|f| f.present) {
            for evo in &form.evolutions {
                pre_evolution.entry(evo.into).or_insert(form.id);
            }
        }
        Dex { forms, index, pre_evolution }
    }

    pub fn get(&self, id: Pokemon) -> Option<&FormInfo> {
        self.index.get(&id).map(|&i| &self.forms[i])
    }

    pub fn all(&self) -> &[FormInfo] {
        &self.forms
    }

    pub fn playable(&self) -> impl Iterator<Item = &FormInfo> {
        self.forms.iter().filter(|f| f.playable())
    }

    pub fn is_playable(&self, id: Pokemon) -> bool {
        self.get(id).is_some_and(FormInfo::playable)
    }

    pub fn pre_evolution(&self, id: Pokemon) -> Option<Pokemon> {
        self.pre_evolution.get(&id).copied()
    }

    /// The first form of `id`'s evolution line.
    pub fn base_of(&self, id: Pokemon) -> Pokemon {
        let mut current = id;
        let mut guard = 0;
        while let Some(prev) = self.pre_evolution(current) {
            current = prev;
            guard += 1;
            if guard > 8 {
                break;
            }
        }
        current
    }

    /// All forms reachable by evolving from `id`, including `id`.
    pub fn line_from(&self, id: Pokemon) -> BTreeSet<Pokemon> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            if let Some(info) = self.get(current) {
                stack.extend(info.evolutions.iter().map(|e| e.into));
            }
        }
        seen
    }

    pub fn is_fully_evolved(&self, id: Pokemon) -> bool {
        self.get(id)
            .is_none_or(|info| info.evolutions.iter().all(|e| !self.is_playable(e.into)))
    }
}

fn catalog_forms(catalog: &TableValue) -> Result<BTreeSet<Pokemon>, DataError> {
    let mut forms = BTreeSet::new();
    for entry in catalog.tables("Table") {
        let info = entry
            .table("Info")
            .ok_or_else(|| DataError::Shape("catalog entry without Info".into()))?;
        forms.insert(Pokemon {
            species: field_u16(info.int("SpeciesInternal"))?,
            form: field_u16(info.int("Form"))?,
        });
    }
    Ok(forms)
}

fn read_form(row: usize, entry: &TableValue, with_models: &BTreeSet<Pokemon>) -> Result<FormInfo, DataError> {
    let species = entry
        .struct_value("species")
        .ok_or_else(|| DataError::Shape(format!("personal row {row} has no species struct")))?;
    let id = Pokemon {
        species: field_u16(species.int("species"))?,
        form: field_u16(species.int("form"))?,
    };
    let national = field_u16(species.int("national_dex"))?;
    let stats = stat_block(entry.struct_value("base_stats"))?;
    let gender = entry.struct_value("gender");
    let mut evolutions = Vec::new();
    for evo in entry.vector("evolutions").unwrap_or(&[]) {
        let evo = evo
            .as_struct()
            .ok_or_else(|| DataError::Shape(format!("personal row {row} evolution is not a struct")))?;
        evolutions.push(Evolution {
            condition: field_u16(evo.int("condition"))?,
            parameter: field_u16(evo.int("parameter"))?,
            level: field_u16(evo.int("level"))?,
            into: Pokemon {
                species: field_u16(evo.int("species"))?,
                form: field_u16(evo.int("form"))?,
            },
        });
    }
    let present = entry.bool("is_present").unwrap_or(false);
    Ok(FormInfo {
        id,
        row,
        national,
        present,
        has_model: with_models.contains(&id),
        types: [field_u8(entry.int("type_1"))?, field_u8(entry.int("type_2"))?],
        abilities: [
            field_u16(entry.int("ability_1"))?,
            field_u16(entry.int("ability_2"))?,
            field_u16(entry.int("ability_hidden"))?,
        ],
        stats,
        evo_stage: field_u8(entry.int("evo_stage"))?,
        evolutions,
        catch_rate: field_u8(entry.int("catch_rate"))?,
        xp_growth: field_u8(entry.int("xp_growth"))?,
        gender_group: gender.map_or(Ok(0), |g| field_u8(g.int("group")))?,
        gender_ratio: gender.map_or(Ok(0), |g| field_u8(g.int("ratio")))?,
        regional_dex: [
            field_u16(entry.int("paldea_dex"))?,
            field_u16(entry.int("kitakami_dex"))?,
            field_u16(entry.int("blueberry_dex"))?,
        ],
        rarity: facts::rarity(national),
        paradox: facts::is_paradox(national),
        generation: facts::generation(national),
    })
}

pub const STAT_FIELDS: [&str; 6] = ["hp", "atk", "def", "spa", "spd", "spe"];

fn stat_block(block: Option<&StructValue>) -> Result<[u8; 6], DataError> {
    let mut out = [0; 6];
    if let Some(block) = block {
        for (slot, name) in out.iter_mut().zip(STAT_FIELDS) {
            *slot = field_u8(block.int(name))?;
        }
    }
    Ok(out)
}

fn field_u16(v: Result<i64, crate::fb::FieldError>) -> Result<u16, DataError> {
    let v = v.map_err(|e| DataError::Shape(e.to_string()))?;
    u16::try_from(v).map_err(|_| DataError::Shape(format!("value {v} does not fit u16")))
}

fn field_u8(v: Result<i64, crate::fb::FieldError>) -> Result<u8, DataError> {
    let v = v.map_err(|e| DataError::Shape(e.to_string()))?;
    u8::try_from(v).map_err(|_| DataError::Shape(format!("value {v} does not fit u8")))
}
