//! Finds Eden, its folders and keys, and pairs each edition's base game with its 4.0.0 update.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use svr_core::container::Keys;
use svr_core::game::{GameFiles, GamePaths, OpenError};
use svr_core::output::ACTIVE_MOD_NAME;

use crate::config::{AppConfig, GameFileOverride};
use crate::edition::Edition;
use crate::games::{GameCache, OpenedGame};

const VIRTUAL_GAME_DIRS: [&str; 3] = ["SDMC", "UserNAND", "SysNAND"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathSource {
    Override,
    Detected,
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedPath {
    pub path: Option<PathBuf>,
    pub source: PathSource,
    pub exists: bool,
}

impl ResolvedPath {
    fn at(path: &Path, source: PathSource) -> ResolvedPath {
        ResolvedPath {
            path: Some(path.to_owned()),
            source,
            exists: path.exists(),
        }
    }

    fn overridden(path: &Path) -> ResolvedPath {
        ResolvedPath::at(path, PathSource::Override)
    }

    /// Read from Eden's own config, or found on disk.
    fn detected(path: &Path) -> ResolvedPath {
        ResolvedPath::at(path, PathSource::Detected)
    }

    /// The conventional location: detected when something is there, a default guess otherwise.
    fn conventional(path: &Path) -> ResolvedPath {
        let source = if path.exists() {
            PathSource::Detected
        } else {
            PathSource::Default
        };
        ResolvedPath::at(path, source)
    }

    fn unknown() -> ResolvedPath {
        ResolvedPath {
            path: None,
            source: PathSource::Default,
            exists: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EdenInfo {
    pub exe: ResolvedPath,
    pub data_dir: ResolvedPath,
    pub portable: bool,
    pub keys: ResolvedPath,
    pub load_dir: ResolvedPath,
    pub nand_dir: ResolvedPath,
    pub game_dirs: Vec<PathBuf>,
    pub game_dirs_source: PathSource,
}

pub struct EdenScan {
    pub info: EdenInfo,
    /// Mod names switched off in Eden's add-on list, by title ID.
    disabled_mods: BTreeMap<u64, Vec<String>>,
}

impl EdenScan {
    pub fn mod_disabled(&self, edition: Edition) -> bool {
        self.disabled_mods
            .get(&edition.title_id())
            .is_some_and(|names| names.iter().any(|name| name == ACTIVE_MOD_NAME))
    }

    pub fn load_dir(&self) -> Result<&Path, String> {
        self.info.load_dir.path.as_deref().ok_or_else(|| {
            "Eden's mod folder is unknown. Choose Eden's data folder or its load folder in Settings.".to_owned()
        })
    }

    pub fn nand_dir(&self) -> Option<&Path> {
        self.info.nand_dir.path.as_deref()
    }
}

pub fn scan_eden(config: &AppConfig, appdata: Option<&Path>) -> EdenScan {
    let exe = match &config.eden_exe {
        Some(path) => ResolvedPath::overridden(path),
        None => ResolvedPath::unknown(),
    };
    let portable_dir = exe
        .path
        .as_deref()
        .and_then(Path::parent)
        .map(|dir| dir.join("user"))
        .filter(|dir| dir.is_dir());
    let data_dir = match (&config.eden_data_dir, &portable_dir, appdata) {
        (Some(path), _, _) => ResolvedPath::overridden(path),
        (None, Some(portable), _) => ResolvedPath::detected(portable),
        (None, None, Some(appdata)) => ResolvedPath::conventional(&appdata.join("eden")),
        (None, None, None) => ResolvedPath::unknown(),
    };
    let portable = portable_dir.is_some() && data_dir.path == portable_dir;
    let data = data_dir.path.clone();

    let ini = data
        .as_ref()
        .and_then(|dir| fs::read_to_string(dir.join("config").join("qt-config.ini")).ok())
        .map(|text| Ini::parse(&text))
        .unwrap_or_default();

    let storage_dir = |chosen: &Option<PathBuf>, key: &str, fallback: &str| match chosen {
        Some(path) => ResolvedPath::overridden(path),
        None => match ini.get("Data Storage", key).filter(|value| !value.is_empty()) {
            Some(value) => ResolvedPath::detected(&native_path(value)),
            None => data
                .as_ref()
                .map_or_else(ResolvedPath::unknown, |dir| ResolvedPath::conventional(&dir.join(fallback))),
        },
    };
    let load_dir = storage_dir(&config.load_dir, "load_directory", "load");
    let nand_dir = storage_dir(&config.nand_dir, "nand_directory", "nand");

    let keys = match &config.keys_file {
        Some(path) => ResolvedPath::overridden(path),
        None => data.as_ref().map_or_else(ResolvedPath::unknown, |dir| {
            ResolvedPath::conventional(&dir.join("keys").join("prod.keys"))
        }),
    };

    let detected_dirs = game_dirs_from(&ini);
    let (game_dirs, game_dirs_source) = match &config.game_dirs {
        Some(dirs) => (dirs.clone(), PathSource::Override),
        None if !detected_dirs.is_empty() => (detected_dirs, PathSource::Detected),
        None => (Vec::new(), PathSource::Default),
    };

    EdenScan {
        info: EdenInfo {
            exe,
            data_dir,
            portable,
            keys,
            load_dir,
            nand_dir,
            game_dirs,
            game_dirs_source,
        },
        disabled_mods: disabled_mods_from(&ini),
    }
}

pub fn load_keys(keys: &ResolvedPath) -> Result<Keys, String> {
    let Some(path) = &keys.path else {
        return Err("Eden's data folder was not found, so there is no keys file. Choose your prod.keys file in Settings.".to_owned());
    };
    if !path.is_file() {
        return Err(format!(
            "No keys file was found at \"{}\". Copy your prod.keys there, or choose where it is in Settings.",
            path.display()
        ));
    }
    Keys::load(path).map_err(|error| error.to_string())
}

// ---- Eden's qt-config.ini ----

/// Qt's INI flavour: `%20` in section names, `\`-separated keys, optionally quoted values.
#[derive(Default)]
struct Ini {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Ini {
    fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut section = String::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                section = name.replace("%20", " ");
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                ini.sections
                    .entry(section.clone())
                    .or_default()
                    .insert(key.trim().to_owned(), ini_value(value.trim()));
            }
        }
        ini
    }

    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.get(key).map(String::as_str)
    }
}

fn ini_value(raw: &str) -> String {
    let inner = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(raw);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('\\', Some(&next @ ('\\' | '"'))) => {
                out.push(next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

fn native_path(value: &str) -> PathBuf {
    PathBuf::from(value.replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn game_dirs_from(ini: &Ini) -> Vec<PathBuf> {
    let Some(ui) = ini.sections.get("UI") else {
        return Vec::new();
    };
    let mut numbered: Vec<(u32, PathBuf)> = ui
        .iter()
        .filter_map(|(key, value)| {
            let index = key.strip_prefix(r"Paths\gamedirs\")?.strip_suffix(r"\path")?.parse().ok()?;
            let real = !value.is_empty() && !VIRTUAL_GAME_DIRS.contains(&value.as_str());
            real.then(|| (index, native_path(value)))
        })
        .collect();
    numbered.sort_by_key(|(index, _)| *index);
    numbered.into_iter().map(|(_, dir)| dir).collect()
}

/// `[DisabledAddOns]` lists `N\title_id=<decimal>` and `N\disabled\K\d=<mod name>` per entry N.
fn disabled_mods_from(ini: &Ini) -> BTreeMap<u64, Vec<String>> {
    let Some(section) = ini.sections.get("DisabledAddOns") else {
        return BTreeMap::new();
    };
    let mut titles: BTreeMap<u32, u64> = BTreeMap::new();
    let mut names: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (key, value) in section {
        let Some((entry, rest)) = key.split_once('\\') else {
            continue;
        };
        let Ok(entry) = entry.parse::<u32>() else {
            continue;
        };
        if rest == "title_id" {
            if let Ok(title_id) = value.parse() {
                titles.insert(entry, title_id);
            }
        } else if let Some(item) = rest.strip_prefix(r"disabled\") {
            if item.contains('\\') && !item.ends_with(r"\default") {
                names.entry(entry).or_default().push(value.clone());
            }
        }
    }
    let mut disabled: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for (entry, title_id) in titles {
        disabled
            .entry(title_id)
            .or_default()
            .extend(names.remove(&entry).unwrap_or_default());
    }
    disabled
}

// ---- Pairing game files ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    Ready,
    Error,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GameStatus {
    pub edition: Edition,
    pub title_id: String,
    pub state: GameState,
    pub base: Option<PathBuf>,
    pub update: Option<PathBuf>,
    pub message: Option<String>,
    pub mod_disabled: bool,
}

pub enum GameSlot {
    Ready(Arc<OpenedGame>),
    Error {
        base: Option<PathBuf>,
        update: Option<PathBuf>,
        message: String,
    },
    Missing {
        message: String,
    },
}

impl GameSlot {
    pub fn ready(&self) -> Option<&Arc<OpenedGame>> {
        match self {
            GameSlot::Ready(game) => Some(game),
            _ => None,
        }
    }

    pub fn status(&self, edition: Edition, mod_disabled: bool) -> GameStatus {
        let (state, base, update, message) = match self {
            GameSlot::Ready(game) => (GameState::Ready, Some(game.base.clone()), Some(game.update.clone()), None),
            GameSlot::Error { base, update, message } => {
                (GameState::Error, base.clone(), update.clone(), Some(message.clone()))
            }
            GameSlot::Missing { message } => (GameState::Missing, None, None, Some(message.clone())),
        };
        GameStatus {
            edition,
            title_id: edition.title_id_hex(),
            state,
            base,
            update,
            message,
            mod_disabled,
        }
    }
}

#[derive(Default)]
struct Scanned {
    bases: Vec<(Edition, PathBuf)>,
    updates: Vec<PathBuf>,
    /// Why the first unidentifiable .xci could not be checked.
    base_problem: Option<String>,
}

/// Pairs both editions. Overrides in `config` replace scanning for that edition, and a cached game
/// whose files are still candidates is reused instead of reopened.
pub fn find_games(
    config: &AppConfig,
    game_dirs: &[PathBuf],
    keys: Result<&Keys, &str>,
    cache: &GameCache,
) -> [GameSlot; 2] {
    let keys = match keys {
        Ok(keys) => keys,
        Err(reason) => {
            return Edition::ALL.map(|edition| GameSlot::Error {
                base: config.game(edition).base.clone(),
                update: config.game(edition).update.clone(),
                message: format!("The game files can be checked once your keys file loads. {reason}"),
            })
        }
    };
    let needs_scan = Edition::ALL.iter().any(|&edition| {
        let chosen = config.game(edition);
        chosen.base.is_none() || chosen.update.is_none()
    });
    let scanned = if needs_scan {
        scan_game_dirs(game_dirs, keys)
    } else {
        Scanned::default()
    };
    Edition::ALL.map(|edition| {
        pair(edition, config.game(edition), &scanned, game_dirs, keys, cache[edition.index()].as_ref())
    })
}

fn scan_game_dirs(game_dirs: &[PathBuf], keys: &Keys) -> Scanned {
    let mut scanned = Scanned::default();
    for dir in game_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .map(|entry| entry.path())
            .collect();
        files.sort();
        for file in files {
            match extension(&file).as_deref() {
                Some("xci") => match identify_base(&file, keys) {
                    Ok(edition) => scanned.bases.push((edition, file)),
                    Err(OpenError::NotScarletVioletXci { .. }) => {}
                    Err(error) => {
                        scanned.base_problem.get_or_insert_with(|| error.to_string());
                    }
                },
                Some("nsp") => scanned.updates.push(file),
                _ => {}
            }
        }
    }
    scanned
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

/// Which edition a base .xci is, without needing its update.
fn identify_base(path: &Path, keys: &Keys) -> Result<Edition, OpenError> {
    // Core opens the base before the update, and an .xci is never an .nsp, so passing the .xci as
    // its own update stops right after the base is identified with an error naming its edition.
    let probe = GamePaths {
        base_xci: path.to_owned(),
        update_nsp: path.to_owned(),
    };
    match GameFiles::open(&probe, keys) {
        Ok(game) => Ok(Edition::from_core(game.edition())),
        Err(OpenError::NotAnUpdate { edition, .. }) => Ok(Edition::from_core(edition)),
        Err(error) => Err(error),
    }
}

fn pair(
    edition: Edition,
    chosen: &GameFileOverride,
    scanned: &Scanned,
    game_dirs: &[PathBuf],
    keys: &Keys,
    cached: Option<&Arc<OpenedGame>>,
) -> GameSlot {
    let problem = |base: &Path, message: String| GameSlot::Error {
        base: Some(base.to_owned()),
        update: chosen.update.clone(),
        message,
    };
    let bases: Vec<PathBuf> = match &chosen.base {
        Some(base) => match identify_base(base, keys) {
            Ok(found) if found == edition => vec![base.clone()],
            Ok(found) => {
                let message = format!(
                    "\"{}\" is {found}, not {edition}. Choose the {edition} .xci.",
                    base.display()
                );
                return problem(base, message);
            }
            Err(error) => return problem(base, error.to_string()),
        },
        None => scanned
            .bases
            .iter()
            .filter(|(found, _)| *found == edition)
            .map(|(_, path)| path.clone())
            .collect(),
    };
    let Some(first_base) = bases.first() else {
        return GameSlot::Missing {
            message: missing_message(edition, game_dirs, scanned.base_problem.as_deref()),
        };
    };
    let updates = match &chosen.update {
        Some(update) => vec![update.clone()],
        None => scanned.updates.clone(),
    };
    if let Some(game) = cached.filter(|game| bases.contains(&game.base) && updates.contains(&game.update)) {
        return GameSlot::Ready(game.clone());
    }

    let mut best: Option<OpenError> = None;
    for base in &bases {
        for update in &updates {
            match OpenedGame::open(base.clone(), update.clone(), keys) {
                Ok(game) => return GameSlot::Ready(Arc::new(game)),
                Err(error) => {
                    let base_failed = failed_file(&error).is_none_or(|path| path == base);
                    if best.as_ref().is_none_or(|kept| usefulness(&error) > usefulness(kept)) {
                        best = Some(error);
                    }
                    if base_failed {
                        break;
                    }
                }
            }
        }
    }

    let failed_update = best
        .as_ref()
        .and_then(failed_file)
        .filter(|path| !bases.iter().any(|base| base == path))
        .map(Path::to_owned);
    let message = match best {
        Some(error) if chosen.update.is_some() || usefulness(&error) >= 2 => error.to_string(),
        _ => no_update_message(edition, first_base, game_dirs),
    };
    GameSlot::Error {
        base: Some(first_base.clone()),
        update: chosen.update.clone().or(failed_update),
        message,
    }
}

/// How much an error tells the user. Other games' updates and DLC are expected noise while pairing.
fn usefulness(error: &OpenError) -> u8 {
    match error {
        OpenError::UpdateForDifferentGame { .. } => 0,
        OpenError::NotAnUpdate { .. } => 1,
        OpenError::WrongVersion { .. } => 3,
        _ => 2,
    }
}

fn failed_file(error: &OpenError) -> Option<&Path> {
    match error {
        OpenError::MissingKey { .. } => None,
        OpenError::Unreadable { path, .. }
        | OpenError::NotScarletVioletXci { path, .. }
        | OpenError::NotAnUpdate { path, .. }
        | OpenError::UpdateForDifferentGame { path, .. }
        | OpenError::WrongVersion { path, .. }
        | OpenError::NoTicket { path }
        | OpenError::Corrupt { path, .. } => Some(path),
    }
}

fn folder_list(dirs: &[PathBuf]) -> String {
    let quoted: Vec<String> = dirs.iter().map(|dir| format!("\"{}\"", dir.display())).collect();
    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

fn missing_message(edition: Edition, game_dirs: &[PathBuf], problem: Option<&str>) -> String {
    let mut message = if game_dirs.is_empty() {
        format!("No game folders are set up, so {edition} was not found. Add your games folder in Eden, or choose the {edition} .xci in Settings.")
    } else {
        format!(
            "No {edition} cartridge dump (.xci) was found in {}. Put the .xci in one of these folders, or choose it in Settings.",
            folder_list(game_dirs)
        )
    };
    if let Some(problem) = problem {
        message.push_str(" One file could not be checked: ");
        message.push_str(problem);
    }
    message
}

fn no_update_message(edition: Edition, base: &Path, game_dirs: &[PathBuf]) -> String {
    let found = format!("Found {edition} at \"{}\", but", base.display());
    if game_dirs.is_empty() {
        format!("{found} no game folders are set up to look for its 4.0.0 update. Choose the update .nsp in Settings.")
    } else {
        format!(
            "{found} no {edition} 4.0.0 update (.nsp) is in {}. Put the update .nsp beside it, or choose it in Settings.",
            folder_list(game_dirs)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INI: &str = r#"[DisabledAddOns]
size=2
1\title_id\default=false
1\title_id=72237707933630464
1\disabled\size=1
1\disabled\1\d\default=false
1\disabled\1\d=SV Randomizer
2\title_id=72057594037930019
2\disabled\size=0

[Data%20Storage]
nand_directory\default=false
nand_directory="D:/Eden Data/nand"
load_directory=D:/Eden Data/load

[UI]
Paths\gamedirs\size=3
Paths\gamedirs\1\path=SDMC
Paths\gamedirs\3\path="E:/More \"Games\""
Paths\gamedirs\2\path=D:/Games
Paths\gamedirs\2\deep_scan=false
"#;

    #[test]
    fn qt_ini_values_are_read() {
        let ini = Ini::parse(INI);
        assert_eq!(ini.get("Data Storage", "nand_directory"), Some("D:/Eden Data/nand"));
        assert_eq!(ini.get("Data Storage", "load_directory"), Some("D:/Eden Data/load"));
        assert_eq!(
            game_dirs_from(&ini),
            vec![PathBuf::from("D:/Games"), PathBuf::from("E:/More \"Games\"")]
        );
    }

    #[test]
    fn disabled_add_ons_are_keyed_by_title() {
        let disabled = disabled_mods_from(&Ini::parse(INI));
        assert_eq!(disabled.get(&0x0100_A3D0_08C5_C000), Some(&vec!["SV Randomizer".to_owned()]));
        assert_eq!(disabled.get(&72057594037930019), Some(&Vec::new()));
    }

    #[test]
    fn folder_lists_read_naturally() {
        let dirs = [PathBuf::from("A"), PathBuf::from("B"), PathBuf::from("C")];
        assert_eq!(folder_list(&dirs), "\"A\", \"B\" and \"C\"");
        assert_eq!(folder_list(&dirs[..1]), "\"A\"");
    }
}
