//! The seed library: listing seeds, switching the one Eden loads, save backups, and cleanup.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use svr_core::output::{self, SeedMeta, ACTIVE_MOD_NAME, META_FILE};

use crate::config::AppConfig;
use crate::edition::Edition;
use crate::errors::{io_failure, sentence};

const SAVE_SEARCH_DEPTH: usize = 3;
const SAVE_SEARCH_MAX_DIRS: usize = 5000;

/// Every folder whose direct children may be seed folders.
pub struct SeedRoots {
    pub library: PathBuf,
    pub extra: Vec<PathBuf>,
}

impl SeedRoots {
    pub fn new(config: &AppConfig, app_data: &Path) -> SeedRoots {
        SeedRoots {
            library: config.library_dir.clone().unwrap_or_else(|| app_data.join("seeds")),
            extra: config.extra_seed_dirs.clone(),
        }
    }

    /// The library first, then each extra folder once with its folder name as the group.
    fn each(&self) -> Vec<(&Path, Option<String>)> {
        let mut roots: Vec<(&Path, Option<String>)> = vec![(&self.library, None)];
        for dir in &self.extra {
            if roots.iter().all(|(seen, _)| *seen != dir.as_path()) {
                let group = dir.file_name().map_or_else(|| dir.display().to_string(), |name| name.to_string_lossy().into_owned());
                roots.push((dir, Some(group)));
            }
        }
        roots
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SeedEntry {
    pub path: PathBuf,
    pub name: String,
    pub group: Option<String>,
    pub edition: Option<Edition>,
    pub title_id: String,
    pub seed: String,
    pub created_unix: u64,
    pub file_count: usize,
    pub active: bool,
    pub has_spoiler: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SwitchCheck {
    pub edition: Edition,
    pub active_seed: Option<String>,
    pub saves: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActivateResult {
    pub installed_to: PathBuf,
    pub backup_dir: Option<PathBuf>,
}

pub fn installed_dir(load_dir: &Path, edition: Edition) -> PathBuf {
    load_dir.join(edition.title_id_hex()).join(ACTIVE_MOD_NAME)
}

fn read_meta(seed_dir: &Path) -> Result<(SeedMeta, Vec<u8>), String> {
    let file = seed_dir.join(META_FILE);
    let bytes = fs::read(&file).map_err(|_| {
        format!("\"{}\" is not a randomizer seed folder: it has no {META_FILE}.", seed_dir.display())
    })?;
    let meta = serde_json::from_slice(&bytes)
        .map_err(|_| format!("The seed details in \"{}\" are damaged. Make the seed again.", file.display()))?;
    Ok((meta, bytes))
}

fn seed_edition(meta: &SeedMeta, seed_dir: &Path) -> Result<Edition, String> {
    Edition::from_id(&meta.edition).ok_or_else(|| {
        format!(
            "The seed in \"{}\" is not marked as Scarlet or Violet. Make the seed again.",
            seed_dir.display()
        )
    })
}

pub fn list_seeds(roots: &SeedRoots, load_dir: Option<&Path>) -> Vec<SeedEntry> {
    let installed = Edition::ALL
        .map(|edition| load_dir.and_then(|load| fs::read(installed_dir(load, edition).join(META_FILE)).ok()));
    let mut seeds = Vec::new();
    for (root, group) in roots.each() {
        let Ok(children) = fs::read_dir(root) else {
            continue;
        };
        for child in children.filter_map(Result::ok) {
            let path = child.path();
            let Ok((meta, bytes)) = read_meta(&path) else {
                continue;
            };
            let edition = Edition::from_id(&meta.edition);
            seeds.push(SeedEntry {
                name: child.file_name().to_string_lossy().into_owned(),
                group: group.clone(),
                edition,
                active: edition.is_some_and(|e| installed[e.index()].as_deref() == Some(bytes.as_slice())),
                has_spoiler: path.join("spoiler.html").is_file() || path.join("spoiler.txt").is_file(),
                title_id: meta.title_id,
                seed: meta.seed,
                created_unix: meta.created_unix,
                file_count: meta.file_count,
                path,
            });
        }
    }
    seeds.sort_by(|a, b| b.created_unix.cmp(&a.created_unix).then_with(|| a.name.cmp(&b.name)));
    seeds
}

pub fn check_switch(
    seed_dir: &Path,
    roots: &SeedRoots,
    load_dir: Option<&Path>,
    nand_dir: Option<&Path>,
) -> Result<SwitchCheck, String> {
    let (meta, _) = read_meta(seed_dir)?;
    let edition = seed_edition(&meta, seed_dir)?;
    let active_seed = load_dir.and_then(|load| {
        let installed = fs::read(installed_dir(load, edition).join(META_FILE)).ok()?;
        let known = list_seeds(roots, Some(load))
            .into_iter()
            .find(|seed| seed.active && seed.edition == Some(edition));
        Some(match known {
            Some(seed) => seed.name,
            None => serde_json::from_slice::<SeedMeta>(&installed)
                .map_or_else(|_| "An unknown seed".to_owned(), |meta| format!("{} {}", edition.short_name(), meta.seed)),
        })
    });
    Ok(SwitchCheck {
        edition,
        active_seed,
        saves: find_saves(nand_dir, edition),
    })
}

fn save_root(nand_dir: &Path) -> PathBuf {
    nand_dir.join("user").join("save")
}

/// Eden keeps saves at `<nand>/user/save/<save space>/<user>/<TITLEID>/`.
pub fn find_saves(nand_dir: Option<&Path>, edition: Edition) -> Vec<PathBuf> {
    let Some(nand_dir) = nand_dir else {
        return Vec::new();
    };
    let title = edition.title_id_hex();
    let mut found = Vec::new();
    let mut pending = vec![(save_root(nand_dir), 0)];
    let mut visited = 0;
    while let Some((dir, depth)) = pending.pop() {
        visited += 1;
        if visited > SAVE_SEARCH_MAX_DIRS {
            break;
        }
        let Ok(children) = fs::read_dir(&dir) else {
            continue;
        };
        for child in children.filter_map(Result::ok) {
            if !child.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            if child.file_name().to_string_lossy().eq_ignore_ascii_case(&title) {
                found.push(child.path());
            } else if depth < SAVE_SEARCH_DEPTH {
                pending.push((child.path(), depth + 1));
            }
        }
    }
    found.sort();
    found
}

/// `root/name`, or `root/name (2)` and so on when that folder (or its staging twin) exists.
pub fn free_folder(root: &Path, name: &str) -> PathBuf {
    let taken = |path: &Path| path.exists() || path.with_extension("partial").exists();
    let first = root.join(name);
    if !taken(&first) {
        return first;
    }
    (2..)
        .map(|n| root.join(format!("{name} ({n})")))
        .find(|candidate| !taken(candidate))
        .expect("an unused folder name exists")
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    let action = "back up the save to";
    fs::create_dir_all(to).map_err(|e| io_failure(action, to, &e))?;
    for entry in fs::read_dir(from).map_err(|e| io_failure("read the save folder", from, &e))? {
        let entry = entry.map_err(|e| io_failure("read the save folder", from, &e))?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|e| io_failure(action, &target, &e))?;
        }
    }
    Ok(())
}

/// Copies every save folder of the game into `backups/<TITLEID> <timestamp>/`, keeping the
/// folders' paths below `user/save`. `None` when the game has no saves.
fn back_up_saves(backups: &Path, nand_dir: Option<&Path>, edition: Edition) -> Result<Option<PathBuf>, String> {
    let saves = find_saves(nand_dir, edition);
    let Some(nand_dir) = nand_dir.filter(|_| !saves.is_empty()) else {
        return Ok(None);
    };
    let root = save_root(nand_dir);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H-%M-%S");
    let target = free_folder(backups, &format!("{} {stamp}", edition.title_id_hex()));
    for save in &saves {
        let relative = save.strip_prefix(&root).expect("saves are found below the save root");
        copy_dir(save, &target.join(relative))?;
    }
    Ok(Some(target))
}

/// Installs the seed as Eden's one active mod for its game. With `backups`, every save of the game
/// is copied there first, and nothing is installed if that copy fails.
pub fn activate_seed(
    seed_dir: &Path,
    load_dir: &Path,
    nand_dir: Option<&Path>,
    backups: Option<&Path>,
) -> Result<ActivateResult, String> {
    let (meta, _) = read_meta(seed_dir)?;
    let edition = seed_edition(&meta, seed_dir)?;
    if !seed_dir.join("romfs").is_dir() {
        return Err(format!(
            "\"{}\" has no romfs folder, so Eden has nothing to load. Make the seed again.",
            seed_dir.display()
        ));
    }
    let backup_dir = match backups {
        Some(backups) => back_up_saves(backups, nand_dir, edition)
            .map_err(|reason| format!("The seed was not switched because the save backup failed. {reason}"))?,
        None => None,
    };
    let installed_to = output::install(seed_dir, load_dir, edition.title_id()).map_err(|error| {
        format!(
            "Could not copy the seed into Eden's mod folder. Close Eden if it is running, then try again. ({})",
            sentence(error.to_string())
        )
    })?;
    Ok(ActivateResult {
        installed_to,
        backup_dir,
    })
}

pub fn deactivate(load_dir: &Path, edition: Edition) -> Result<(), String> {
    let dir = installed_dir(load_dir, edition);
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "{} Close Eden if it is running, then try again.",
            io_failure("remove the seed from", &dir, &error)
        )),
    }
}

/// Deletes a seed folder, but only one with seed details that sits directly in a seed root.
pub fn delete_seed(seed_dir: &Path, roots: &SeedRoots) -> Result<(), String> {
    let seed = fs::canonicalize(seed_dir).map_err(|_| format!("\"{}\" no longer exists.", seed_dir.display()))?;
    if !seed.join(META_FILE).is_file() {
        return Err(format!(
            "\"{}\" is not a randomizer seed folder, so it was not deleted.",
            seed_dir.display()
        ));
    }
    let inside_a_root = roots
        .each()
        .into_iter()
        .filter_map(|(root, _)| fs::canonicalize(root).ok())
        .any(|root| seed.parent() == Some(root.as_path()));
    if !inside_a_root {
        return Err(format!(
            "\"{}\" is not in the seed library or a batch folder, so it was not deleted.",
            seed_dir.display()
        ));
    }
    fs::remove_dir_all(&seed).map_err(|e| io_failure("delete", seed_dir, &e))
}

/// Opens a folder in Explorer, or a file with its default app. Explorer exits non-zero even on
/// success, so it is not waited on.
pub fn open_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("\"{}\" no longer exists.", path.display()));
    }
    let native = path.to_string_lossy().replace('/', "\\");
    Command::new("explorer.exe")
        .arg(native)
        .spawn()
        .map(drop)
        .map_err(|e| io_failure("open", path, &e))
}

pub fn launch_eden(exe: &Path, base_xci: &Path) -> Result<(), String> {
    if !exe.is_file() {
        return Err(format!(
            "Eden was not found at \"{}\". Choose eden.exe in Settings.",
            exe.display()
        ));
    }
    let mut command = Command::new(exe);
    command
        .arg("-g")
        .arg(base_xci)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().map(drop).map_err(|e| io_failure("start Eden from", exe, &e))
}
