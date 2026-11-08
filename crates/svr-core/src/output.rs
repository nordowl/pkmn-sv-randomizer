//! Turns randomizer output into a LayeredFS mod folder that Eden loads over the game.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::game::GameFiles;
use crate::trinity::remove_from_index;

pub const INDEX_PATH: &str = "arc/data.trpfd";
pub const META_FILE: &str = "svr-seed.json";
/// The single Eden mod folder an active seed is copied into, so only one seed is ever enabled.
pub const ACTIVE_MOD_NAME: &str = "SV Randomizer";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedMeta {
    pub app_version: String,
    pub edition: String,
    pub title_id: String,
    pub seed: String,
    pub created_unix: u64,
    pub file_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("could not write the mod folder {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("could not patch the archive index: {0}")]
    Index(String),
    #[error("refusing to write {0}: output paths must be relative and stay inside romfs")]
    BadPath(String),
    #[error("the folder {root} is too deep: the longest mod file would need {length} characters, but Eden skips files whose full path is longer than {limit}. Choose a shorter folder, such as one near the drive root.")]
    PathTooLong { root: PathBuf, length: usize, limit: usize },
}

/// Eden on Windows cannot open longer paths; it silently leaves those mod files out, and the game
/// then finds neither the original nor the replacement.
pub const MAX_FULL_PATH: usize = 259;

/// Fails if any `romfs/<file>` under `root` would exceed Windows' path limit.
pub fn check_path_lengths<'a>(root: &Path, files: impl IntoIterator<Item = &'a str>) -> Result<(), OutputError> {
    let base = root.join("romfs").to_string_lossy().chars().count() + 1;
    let longest = files.into_iter().map(|f| base + f.chars().count()).max().unwrap_or(0);
    if longest > MAX_FULL_PATH {
        return Err(OutputError::PathTooLong {
            root: root.to_owned(),
            length: longest,
            limit: MAX_FULL_PATH,
        });
    }
    Ok(())
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> OutputError + '_ {
    move |source| OutputError::Io {
        path: path.to_owned(),
        source,
    }
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Writes `files` under `dir/romfs/` plus a patched archive index that sends the game to them.
/// The folder is built next to `dir` and renamed into place, so a failed run leaves no half mod.
pub fn write_mod(
    dir: &Path,
    game: &GameFiles,
    files: &BTreeMap<String, Vec<u8>>,
    meta: &SeedMeta,
    extras: &[(&str, &[u8])],
) -> Result<(), OutputError> {
    for path in files.keys() {
        if !safe_relative(path) || path == INDEX_PATH {
            return Err(OutputError::BadPath(path.clone()));
        }
    }
    check_path_lengths(dir, files.keys().map(String::as_str).chain([INDEX_PATH]))?;
    let indexed: Vec<&str> = files.keys().map(String::as_str).filter(|p| game.contains(p)).collect();
    let index = remove_from_index(game.raw_index(), &indexed).map_err(|e| OutputError::Index(e.to_string()))?;

    let staging = dir.with_extension("partial");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(io_at(&staging))?;
    }
    let romfs = staging.join("romfs");
    let write = |relative: &str, bytes: &[u8]| -> Result<(), OutputError> {
        let target = romfs.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_at(parent))?;
        }
        fs::write(&target, bytes).map_err(io_at(&target))
    };
    write(INDEX_PATH, &index)?;
    for (path, bytes) in files {
        write(path, bytes)?;
    }
    let meta_json = serde_json::to_vec_pretty(meta).expect("seed metadata serializes");
    fs::write(staging.join(META_FILE), meta_json).map_err(io_at(&staging))?;
    for (name, bytes) in extras {
        if !safe_relative(name) || name.contains('/') {
            return Err(OutputError::BadPath((*name).to_owned()));
        }
        fs::write(staging.join(name), bytes).map_err(io_at(&staging))?;
    }

    if dir.exists() {
        fs::remove_dir_all(dir).map_err(io_at(dir))?;
    }
    fs::rename(&staging, dir).map_err(io_at(dir))
}

/// Copies a seed's `romfs` into Eden's mod folder for the game, replacing any active seed.
pub fn install(seed_dir: &Path, eden_load_dir: &Path, title_id: u64) -> Result<PathBuf, OutputError> {
    let target = eden_load_dir.join(format!("{title_id:016X}")).join(ACTIVE_MOD_NAME);
    let source = seed_dir.join("romfs");
    let mut relative = Vec::new();
    collect_relative(&source, &source, &mut relative)?;
    check_path_lengths(&target, relative.iter().map(String::as_str))?;
    if target.exists() {
        fs::remove_dir_all(&target).map_err(io_at(&target))?;
    }
    copy_tree(&source, &target.join("romfs"))?;
    fs::copy(seed_dir.join(META_FILE), target.join(META_FILE)).map_err(io_at(&target))?;
    Ok(target)
}

fn collect_relative(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), OutputError> {
    for entry in fs::read_dir(dir).map_err(io_at(dir))? {
        let path = entry.map_err(io_at(dir))?.path();
        if path.is_dir() {
            collect_relative(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), OutputError> {
    fs::create_dir_all(to).map_err(io_at(to))?;
    for entry in fs::read_dir(from).map_err(io_at(from))? {
        let entry = entry.map_err(io_at(from))?;
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if entry.file_type().map_err(io_at(&path))?.is_dir() {
            copy_tree(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).map_err(io_at(&path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_over_the_windows_limit_are_refused() {
        let file = "world/scene/parts/event/event_scenario/main_scenario/common_0060_/common_0060_always_0.trsog";
        assert!(check_path_lengths(Path::new("C:/Users/x/AppData/Roaming/eden/load/0100A3D008C5C000/SV Randomizer"), [file]).is_ok());
        let deep = "C:/Users/Player/AppData/Local/Temp/some-tool/a-very-long-project-folder-name-for-testing/4f1c2b7e-0d3a-4e5b-9c8d-7a6b5c4d3e2f/workspace/eden-portable/user/load/0100A3D008C5C000/SV Randomizer Boot Test";
        let err = check_path_lengths(Path::new(deep), [file]).unwrap_err();
        assert!(err.to_string().contains("too deep"), "{err}");
    }

    #[test]
    fn only_plain_relative_paths_are_allowed() {
        assert!(safe_relative("world/data/x.bin"));
        assert!(!safe_relative("../x.bin"));
        assert!(!safe_relative("/abs"));
        assert!(!safe_relative("a//b"));
        assert!(!safe_relative("C:/x"));
        assert!(!safe_relative("a\\b"));
    }
}
