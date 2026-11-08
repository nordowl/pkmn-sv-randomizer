#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use svr_core::game::{Edition, GamePaths};

/// The game files in the `SVR_TEST_GAMES` folder, found by the title IDs in their names.
pub struct RealGames {
    pub scarlet: GamePaths,
    pub violet: GamePaths,
    /// Scarlet's DLC license file, which is not a game update.
    pub scarlet_dlc: PathBuf,
}

pub fn real_games(games: &Path) -> RealGames {
    let find = |edition| GamePaths::find(games, edition).expect("a base XCI and 4.0.0 update NSP per game in SVR_TEST_GAMES");
    let scarlet_dlc = std::fs::read_dir(games)
        .expect("read SVR_TEST_GAMES")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().to_uppercase().contains("0100A3D008C5D001"))
        .expect("Scarlet's DLC file in SVR_TEST_GAMES");
    RealGames { scarlet: find(Edition::Scarlet), violet: find(Edition::Violet), scarlet_dlc }
}

/// A fresh folder under the system temp dir, removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("svr-app-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The keys file and games folder from `SVR_TEST_KEYS` and `SVR_TEST_GAMES`, or `None` after
/// printing SKIP.
pub fn real_inputs(test: &str) -> Option<(PathBuf, PathBuf)> {
    match (std::env::var_os("SVR_TEST_KEYS"), std::env::var_os("SVR_TEST_GAMES")) {
        (Some(keys), Some(games)) => Some((keys.into(), games.into())),
        _ => {
            println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES to run");
            None
        }
    }
}
