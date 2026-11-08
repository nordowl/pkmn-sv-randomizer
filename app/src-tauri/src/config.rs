use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::edition::Edition;
use crate::errors::io_failure;

/// Persisted in the app's own config dir. `None` means "detect it".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub eden_exe: Option<PathBuf>,
    pub eden_data_dir: Option<PathBuf>,
    pub keys_file: Option<PathBuf>,
    pub load_dir: Option<PathBuf>,
    pub nand_dir: Option<PathBuf>,
    pub game_dirs: Option<Vec<PathBuf>>,
    pub scarlet: GameFileOverride,
    pub violet: GameFileOverride,
    pub library_dir: Option<PathBuf>,
    pub extra_seed_dirs: Vec<PathBuf>,
    pub next_batch_index: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameFileOverride {
    pub base: Option<PathBuf>,
    pub update: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> AppConfig {
        AppConfig {
            eden_exe: None,
            eden_data_dir: None,
            keys_file: None,
            load_dir: None,
            nand_dir: None,
            game_dirs: None,
            scarlet: GameFileOverride::default(),
            violet: GameFileOverride::default(),
            library_dir: None,
            extra_seed_dirs: Vec::new(),
            next_batch_index: 1,
        }
    }
}

fn clean_path(path: Option<PathBuf>) -> Option<PathBuf> {
    let text = path?.to_string_lossy().trim().trim_matches('"').to_owned();
    (!text.is_empty()).then(|| PathBuf::from(text.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn clean_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for path in paths.into_iter().filter_map(|path| clean_path(Some(path))) {
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

impl GameFileOverride {
    fn cleaned(self) -> GameFileOverride {
        GameFileOverride {
            base: clean_path(self.base),
            update: clean_path(self.update),
        }
    }
}

impl AppConfig {
    pub fn game(&self, edition: Edition) -> &GameFileOverride {
        match edition {
            Edition::Scarlet => &self.scarlet,
            Edition::Violet => &self.violet,
        }
    }

    /// Blank fields from the settings form mean "detect it", and duplicate folders collapse.
    pub fn cleaned(self) -> AppConfig {
        AppConfig {
            eden_exe: clean_path(self.eden_exe),
            eden_data_dir: clean_path(self.eden_data_dir),
            keys_file: clean_path(self.keys_file),
            load_dir: clean_path(self.load_dir),
            nand_dir: clean_path(self.nand_dir),
            game_dirs: self.game_dirs.map(clean_paths).filter(|dirs| !dirs.is_empty()),
            scarlet: self.scarlet.cleaned(),
            violet: self.violet.cleaned(),
            library_dir: clean_path(self.library_dir),
            extra_seed_dirs: clean_paths(self.extra_seed_dirs),
            next_batch_index: self.next_batch_index,
        }
    }
}

pub struct LoadedConfig {
    pub config: AppConfig,
    /// The file exists but could not be read or parsed. It is left alone until the user saves.
    pub broken: bool,
}

pub fn load(path: &Path) -> LoadedConfig {
    let parsed = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<AppConfig>(&bytes).ok(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(AppConfig::default()),
        Err(_) => None,
    };
    match parsed {
        Some(config) => LoadedConfig {
            config: config.cleaned(),
            broken: false,
        },
        None => LoadedConfig {
            config: AppConfig::default(),
            broken: true,
        },
    }
}

/// Writes through a temporary file so a crash never leaves half a config behind.
pub fn save(path: &Path, config: &AppConfig) -> Result<(), String> {
    let action = "save the app settings to";
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_failure(action, parent, &e))?;
    }
    let json = serde_json::to_vec_pretty(config).expect("the app config serializes");
    let staging = path.with_extension("json.tmp");
    fs::write(&staging, json).map_err(|e| io_failure(action, &staging, &e))?;
    fs::rename(&staging, path).map_err(|e| io_failure(action, path, &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_fields_mean_detect() {
        let config = AppConfig {
            keys_file: Some(PathBuf::from("  ")),
            game_dirs: Some(vec![PathBuf::from(""), PathBuf::from("D:/Games"), PathBuf::from("D:/Games")]),
            scarlet: GameFileOverride {
                base: Some(PathBuf::from("\"D:/a.xci\"")),
                update: None,
            },
            ..AppConfig::default()
        }
        .cleaned();
        assert_eq!(config.keys_file, None);
        assert_eq!(config.game_dirs, Some(vec![PathBuf::from("D:/Games")]));
        assert_eq!(config.scarlet.base, Some(PathBuf::from("D:/a.xci")));
    }

    #[test]
    fn partial_json_fills_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"keys_file":"K:/prod.keys"}"#).unwrap();
        assert_eq!(config.keys_file, Some(PathBuf::from("K:/prod.keys")));
        assert_eq!(config.next_batch_index, 1);
    }
}
