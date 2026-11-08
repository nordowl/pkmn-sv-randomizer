use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::Serialize;
use svr_core::container::Keys;
use svr_core::data::{Dex, TableStore};
use svr_core::game::{GameFiles, GamePaths, OpenError};
use svr_core::rando::options::{Kind, OptValue, OptionDef, PoolFilter, Settings};
use svr_core::rando::{pool, ALLOW_TERA_LOCKED, GLOBAL_POOL};
use svr_core::text::{GameText, Language};

use crate::edition::Edition;
use crate::errors::sentence;

/// A base game and update that opened together, plus species data built on first use.
pub struct OpenedGame {
    pub edition: Edition,
    pub base: PathBuf,
    pub update: PathBuf,
    pub files: GameFiles,
    species: OnceLock<Result<Species, String>>,
}

/// Opened games by `Edition::index`. Cleared whenever the app config changes.
pub type GameCache = [Option<Arc<OpenedGame>>; 2];

impl OpenedGame {
    pub fn open(base: PathBuf, update: PathBuf, keys: &Keys) -> Result<OpenedGame, OpenError> {
        let paths = GamePaths {
            base_xci: base,
            update_nsp: update,
        };
        let files = GameFiles::open(&paths, keys)?;
        Ok(OpenedGame {
            edition: Edition::from_core(files.edition()),
            base: paths.base_xci,
            update: paths.update_nsp,
            files,
            species: OnceLock::new(),
        })
    }

    pub fn species(&self) -> Result<&Species, String> {
        self.species
            .get_or_init(|| Species::load(&self.files))
            .as_ref()
            .map_err(Clone::clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SpeciesEntry {
    pub national: u16,
    pub form: u16,
    pub species_id: u16,
    pub name: String,
    pub generation: u8,
    pub legendary: bool,
    pub paradox: bool,
    pub stage: u8,
    pub regional: [bool; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PoolCounts {
    pub counts: BTreeMap<String, usize>,
    pub total: usize,
}

pub struct Species {
    pub entries: Vec<SpeciesEntry>,
    dex: Dex,
}

impl Species {
    fn load(game: &GameFiles) -> Result<Species, String> {
        let unreadable = |detail: String| {
            format!(
                "Could not read the Pokémon list from the game: {}",
                sentence(detail)
            )
        };
        let mut store = TableStore::new(game);
        let dex = Dex::load(&mut store).map_err(|e| unreadable(e.to_string()))?;
        let names = GameText::load(game, Language::English, "monsname")
            .map_err(|e| unreadable(e.to_string()))?;
        let entries = dex
            .playable()
            .map(|form| SpeciesEntry {
                national: form.national,
                form: form.id.form,
                species_id: form.id.species,
                name: names
                    .line(usize::from(form.id.species))
                    .unwrap_or_else(|| format!("No. {}", form.national)),
                generation: form.generation,
                legendary: form.is_legendary_class(),
                paradox: form.paradox,
                stage: pool::stage_slot(&dex, form) as u8,
                regional: form.regional_dex.map(|number| number != 0),
            })
            .collect();
        Ok(Species { entries, dex })
    }

    /// The name for a National Dex number, preferring the exact form.
    pub fn name_of(&self, national: u16, form: u16) -> Option<&str> {
        let entries = &self.entries;
        entries
            .iter()
            .find(|e| e.national == national && e.form == form)
            .or_else(|| entries.iter().find(|e| e.national == national))
            .map(|e| e.name.as_str())
    }

    /// Forms allowed by every `Kind::Pool` option after inheriting from the global pool.
    pub fn pool_counts(&self, registry: &[OptionDef], settings: &Settings) -> PoolCounts {
        let (settings, _) = settings.normalized(registry);
        let everything = PoolFilter::everything();
        let global = match settings.values.get(GLOBAL_POOL) {
            Some(OptValue::Pool(filter)) => filter,
            _ => &everything,
        };
        let allow_tera_locked = matches!(settings.values.get(ALLOW_TERA_LOCKED), Some(OptValue::Bool(true)));
        let counts = registry
            .iter()
            .filter(|def| matches!(def.kind, Kind::Pool))
            .map(|def| {
                let filter = pool::effective(global, settings.pool(def.id));
                let count = pool::build(&self.dex, filter, allow_tera_locked).len();
                (def.id.to_owned(), count)
            })
            .collect();
        PoolCounts {
            counts,
            total: self.dex.playable().count(),
        }
    }
}
