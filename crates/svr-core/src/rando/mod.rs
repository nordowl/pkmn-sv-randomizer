//! The randomizer: a fixed pipeline of sections over game tables, driven by `Settings`.

pub mod log;
pub mod options;
pub mod pool;
pub mod rng;
pub mod itempool;
pub mod movepool;
pub mod pick;
pub mod sections;
pub mod validate;

use std::collections::BTreeMap;

use crate::data::{DataError, Dex, Names, Pokemon, TableStore};
use crate::game::GameFiles;
use crate::text::Language;

pub use log::SpoilerLog;
pub use options::{OptValue, OptionDef, Page, PoolFilter, Settings};
pub use rng::{Rng, Seed};

pub const GLOBAL_POOL: &str = "pool.global";
pub const ALLOW_TERA_LOCKED: &str = "pool.allow_tera_locked";

#[derive(Debug, thiserror::Error)]
pub enum RandoError {
    #[error(transparent)]
    Data(#[from] DataError),
    #[error("{section}: {detail}")]
    Section { section: &'static str, detail: String },
    #[error("the species pool for {what} is empty. Loosen its filters.")]
    EmptyPool { what: String },
    #[error("validation failed: {0}")]
    Invalid(String),
    #[error("could not read game text: {0}")]
    Text(String),
}

/// One page of options plus the step that applies them. Steps run in the order of [`sections`].
pub struct Section {
    pub id: &'static str,
    pub options: fn() -> Vec<OptionDef>,
    pub run: fn(&mut Ctx) -> Result<(), RandoError>,
}

pub fn registry() -> Vec<OptionDef> {
    sections::ALL.iter().flat_map(|s| (s.options)()).collect()
}

pub struct Ctx<'g> {
    pub game: &'g GameFiles,
    pub settings: Settings,
    pub seed: Seed,
    pub store: TableStore<'g>,
    pub dex: Dex,
    pub names: Names,
    pub log: SpoilerLog,
    /// Output files that are not FlatBuffer tables from the store, such as patched scenes.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The starters the player can receive, set by the starters section for later sections such
    /// as rival teams. Order: grass slot (Sprigatito), fire slot (Fuecoco), water slot (Quaxly).
    pub starters: Option<[Pokemon; 3]>,
}

impl<'g> Ctx<'g> {
    pub fn new(game: &'g GameFiles, settings: Settings, seed: Seed) -> Result<Ctx<'g>, RandoError> {
        let mut store = TableStore::new(game);
        let dex = Dex::load(&mut store)?;
        let names = Names::load(game, Language::English).map_err(|e| RandoError::Text(e.to_string()))?;
        Ok(Ctx {
            game,
            settings,
            seed,
            store,
            dex,
            names,
            log: SpoilerLog::default(),
            files: BTreeMap::new(),
            starters: None,
        })
    }

    pub fn rng(&self, stream: &str) -> Rng {
        Rng::stream(&self.seed, stream)
    }

    /// Rebuilds the species index after a section changed the species table.
    pub fn refresh_dex(&mut self) -> Result<(), RandoError> {
        self.dex = Dex::load(&mut self.store)?;
        Ok(())
    }

    /// The species allowed by the pool override `id`, falling back to the global pool.
    pub fn pool(&self, id: &str, what: &str) -> Result<Vec<Pokemon>, RandoError> {
        let global = self.settings.pool(GLOBAL_POOL);
        let filter = pool::effective(global, self.settings.pool(id));
        let species = pool::build(&self.dex, filter, self.settings.on(ALLOW_TERA_LOCKED));
        if species.is_empty() {
            return Err(RandoError::EmptyPool { what: what.to_owned() });
        }
        Ok(species)
    }

    pub fn global_pool(&self) -> Result<Vec<Pokemon>, RandoError> {
        self.pool(GLOBAL_POOL, "the global pool")
    }
}

pub struct RandoOutput {
    pub files: BTreeMap<String, Vec<u8>>,
    pub log: SpoilerLog,
    pub settings: Settings,
    pub seed: Seed,
    pub warnings: Vec<String>,
}

pub fn randomize(game: &GameFiles, settings: &Settings, seed: Seed) -> Result<RandoOutput, RandoError> {
    let registry = registry();
    let (settings, warnings) = settings.normalized(&registry);
    let mut ctx = Ctx::new(game, settings, seed)?;
    for section in sections::ALL {
        (section.run)(&mut ctx)?;
    }
    let rules = validate::Rules::load(&mut ctx.store)?;
    let problems = validate::check(game, &ctx.dex, &rules, &ctx.store.changed(), &ctx.files);
    if !problems.is_empty() {
        return Err(RandoError::Invalid(problems.join("; ")));
    }
    let Ctx {
        settings,
        seed,
        store,
        log,
        mut files,
        ..
    } = ctx;
    for (path, bytes) in store.into_files()? {
        files.insert(path, bytes);
    }
    Ok(RandoOutput {
        files,
        log,
        settings,
        seed,
        warnings,
    })
}
