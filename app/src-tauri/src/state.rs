//! Everything the commands share: the config, the opened games and the one running job.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use svr_core::rando::{self, OptionDef};

use crate::config::{self, AppConfig};
use crate::detect::{self, EdenInfo, EdenScan, GameStatus};
use crate::edition::Edition;
use crate::games::{GameCache, OpenedGame};
use crate::library::SeedRoots;

pub struct AppDirs {
    pub config_file: PathBuf,
    pub app_data: PathBuf,
    /// `%APPDATA%`, where Eden keeps its data unless it runs portable.
    pub appdata: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Environment {
    pub config: AppConfig,
    pub eden: EdenInfo,
    pub keys_error: Option<String>,
    pub library_dir: PathBuf,
    pub games: [GameStatus; 2],
}

struct StoredConfig {
    config: AppConfig,
    /// The file on disk could not be read; it is only replaced when the user saves.
    broken_on_disk: bool,
}

pub struct AppState {
    pub dirs: AppDirs,
    pub registry: Vec<OptionDef>,
    config: Mutex<StoredConfig>,
    games: Mutex<GameCache>,
    job_running: AtomicBool,
    pub cancel: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct JobGuard<'a>(&'a AtomicBool);

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl AppState {
    pub fn new(dirs: AppDirs) -> AppState {
        let loaded = config::load(&dirs.config_file);
        AppState {
            dirs,
            registry: rando::registry(),
            config: Mutex::new(StoredConfig {
                config: loaded.config,
                broken_on_disk: loaded.broken,
            }),
            games: Mutex::new([None, None]),
            job_running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        }
    }

    pub fn config(&self) -> AppConfig {
        lock(&self.config).config.clone()
    }

    pub fn seed_roots(&self) -> SeedRoots {
        SeedRoots::new(&self.config(), &self.dirs.app_data)
    }

    pub fn eden(&self) -> EdenScan {
        detect::scan_eden(&self.config(), self.dirs.appdata.as_deref())
    }

    pub fn environment(&self) -> Environment {
        let config = self.config();
        let scan = detect::scan_eden(&config, self.dirs.appdata.as_deref());
        let keys = detect::load_keys(&scan.info.keys);
        let mut cache = lock(&self.games);
        let slots = detect::find_games(&config, &scan.info.game_dirs, keys.as_ref().map_err(String::as_str), &cache);
        *cache = [slots[0].ready().cloned(), slots[1].ready().cloned()];
        let games = Edition::ALL.map(|edition| slots[edition.index()].status(edition, scan.mod_disabled(edition)));
        Environment {
            library_dir: SeedRoots::new(&config, &self.dirs.app_data).library,
            config,
            eden: scan.info,
            keys_error: keys.err(),
            games,
        }
    }

    /// The user's own save: always written, even over a config file that could not be read.
    pub fn save_config(&self, config: AppConfig) -> Result<Environment, String> {
        let config = config.cleaned();
        config::save(&self.dirs.config_file, &config)?;
        *lock(&self.config) = StoredConfig {
            config,
            broken_on_disk: false,
        };
        *lock(&self.games) = [None, None];
        Ok(self.environment())
    }

    /// Bookkeeping the app changes by itself. Kept in memory only while the file on disk is broken.
    pub fn update_config(&self, change: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
        let mut stored = lock(&self.config);
        change(&mut stored.config);
        if stored.broken_on_disk {
            return Ok(());
        }
        config::save(&self.dirs.config_file, &stored.config)
    }

    pub fn cached_game(&self, edition: Edition) -> Option<Arc<OpenedGame>> {
        lock(&self.games)[edition.index()].clone()
    }

    /// The opened game, detecting it first when needed; otherwise why it is not ready.
    pub fn game(&self, edition: Edition) -> Result<Arc<OpenedGame>, String> {
        if let Some(game) = self.cached_game(edition) {
            return Ok(game);
        }
        let environment = self.environment();
        self.cached_game(edition).ok_or_else(|| {
            environment.games[edition.index()]
                .message
                .clone()
                .unwrap_or_else(|| format!("{edition} is not ready yet. Check the game files in Settings."))
        })
    }

    pub fn begin_job(&self) -> Result<JobGuard<'_>, String> {
        self.job_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "A randomizer run is already going. Wait for it to finish, or cancel it first.".to_owned())?;
        self.cancel.store(false, Ordering::SeqCst);
        Ok(JobGuard(&self.job_running))
    }
}
