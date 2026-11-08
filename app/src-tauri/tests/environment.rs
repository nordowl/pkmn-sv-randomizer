mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use common::{real_games, real_inputs, TempDir};
use svr_app::config::{AppConfig, GameFileOverride};
use svr_app::describe::describe_changes;
use svr_app::detect::{find_games, GameState, PathSource};
use svr_app::edition::Edition;
use svr_app::state::{AppDirs, AppState};
use svr_core::container::Keys;
use svr_core::rando::{OptValue, PoolFilter, Settings, GLOBAL_POOL};

/// A fake `%APPDATA%` whose `eden/config/qt-config.ini` sends the load folder elsewhere and lists
/// `games` beside Eden's virtual folders. Scarlet has the randomizer mod switched off.
fn fake_appdata(root: &Path, games: &Path) -> PathBuf {
    let appdata = root.join("appdata");
    let config_dir = appdata.join("eden").join("config");
    fs::create_dir_all(&config_dir).unwrap();
    let load = root.join("eden-load").to_string_lossy().replace('\\', "/");
    let games = games.to_string_lossy().replace('\\', "/");
    let ini = format!(
        "[DisabledAddOns]\nsize=1\n1\\title_id\\default=false\n1\\title_id={scarlet}\n1\\disabled\\size=1\n\
         1\\disabled\\1\\d\\default=false\n1\\disabled\\1\\d=SV Randomizer\n\n\
         [Data%20Storage]\nload_directory\\default=false\nload_directory={load}\n\n\
         [UI]\nPaths\\gamedirs\\size=3\nPaths\\gamedirs\\1\\path=SDMC\nPaths\\gamedirs\\2\\path=UserNAND\n\
         Paths\\gamedirs\\3\\path=\"{games}\"\nPaths\\gamedirs\\3\\deep_scan=false\n",
        scarlet = Edition::Scarlet.title_id(),
    );
    fs::write(config_dir.join("qt-config.ini"), ini).unwrap();
    appdata
}

fn state_for(root: &Path, appdata: PathBuf) -> AppState {
    AppState::new(AppDirs {
        config_file: root.join("app-config").join("config.json"),
        app_data: root.join("app-data"),
        appdata: Some(appdata),
    })
}

/// Keeps detection away from the real Eden install beside the default eden.exe.
fn isolated_config(root: &Path) -> AppConfig {
    AppConfig {
        eden_exe: Some(root.join("Eden").join("eden.exe")),
        ..AppConfig::default()
    }
}

#[test]
fn fake_eden_without_games_explains_what_is_missing() {
    let temp = TempDir::new("env-empty");
    let games = temp.path().join("games");
    fs::create_dir_all(&games).unwrap();
    fs::write(games.join("not a game.zip"), b"zip").unwrap();
    let appdata = fake_appdata(temp.path(), &games);
    let state = state_for(temp.path(), appdata.clone());

    let env = state.save_config(isolated_config(temp.path())).unwrap();
    let data = appdata.join("eden");
    assert_eq!(env.eden.data_dir.path.as_deref(), Some(data.as_path()));
    assert_eq!(env.eden.data_dir.source, PathSource::Detected);
    assert!(!env.eden.portable);
    assert_eq!(env.eden.load_dir.path, Some(temp.path().join("eden-load")));
    assert_eq!(env.eden.load_dir.source, PathSource::Detected);
    assert_eq!(env.eden.nand_dir.path, Some(data.join("nand")));
    assert_eq!(env.eden.nand_dir.source, PathSource::Default);
    assert_eq!(env.eden.game_dirs, [games.as_path()]);
    assert_eq!(env.eden.keys.path, Some(data.join("keys").join("prod.keys")));
    assert!(!env.eden.keys.exists);
    let keys_error = env.keys_error.clone().expect("keys are missing");
    assert!(keys_error.contains("prod.keys"), "{keys_error}");
    assert!(env.games.iter().all(|game| game.state == GameState::Error));
    assert!(env.games[0].mod_disabled && !env.games[1].mod_disabled);
    assert_eq!(env.library_dir, temp.path().join("app-data").join("seeds"));

    let keys_dir = data.join("keys");
    fs::create_dir_all(&keys_dir).unwrap();
    fs::write(keys_dir.join("prod.keys"), "# a keys file with no keys in it\n").unwrap();
    let env = state.environment();
    assert_eq!(env.keys_error, None);
    assert_eq!(env.eden.keys.source, PathSource::Detected);
    for game in &env.games {
        assert_eq!(game.state, GameState::Missing);
        let message = game.message.as_deref().unwrap();
        assert!(message.contains(".xci") && message.contains(&games.display().to_string()), "{message}");
    }
    println!("OK fake Eden detected; keys error: {keys_error}");
    println!("OK missing game: {}", env.games[0].message.as_deref().unwrap());
}

#[test]
fn fake_eden_is_detected_and_both_games_pair() {
    let Some((keys, games)) = real_inputs("fake_eden_is_detected_and_both_games_pair") else {
        return;
    };
    let temp = TempDir::new("env-real");
    let appdata = fake_appdata(temp.path(), &games);
    let state = state_for(temp.path(), appdata.clone());

    let env = state.save_config(isolated_config(temp.path())).unwrap();
    let data = appdata.join("eden");
    assert_eq!(env.eden.data_dir.path.as_deref(), Some(data.as_path()));
    assert_eq!(env.eden.load_dir.path, Some(temp.path().join("eden-load")));
    assert_eq!(env.eden.game_dirs, [games.as_path()]);
    assert_eq!(env.eden.game_dirs_source, PathSource::Detected);
    assert_eq!(env.eden.keys.path, Some(data.join("keys").join("prod.keys")));
    assert!(!env.eden.keys.exists);
    assert!(env.keys_error.is_some());
    assert!(env.games.iter().all(|game| game.state == GameState::Error));

    let started = Instant::now();
    let env = state
        .save_config(AppConfig {
            keys_file: Some(keys),
            ..isolated_config(temp.path())
        })
        .unwrap();
    let first = started.elapsed();
    assert_eq!(env.keys_error, None);
    assert_eq!(env.eden.keys.source, PathSource::Override);
    let files = real_games(&games);
    let expected = [&files.scarlet, &files.violet];
    for (status, paths) in env.games.iter().zip(expected) {
        assert_eq!(status.state, GameState::Ready, "{:?}: {:?}", status.edition, status.message);
        assert_eq!(status.base.as_ref(), Some(&paths.base_xci));
        assert_eq!(status.update.as_ref(), Some(&paths.update_nsp));
        assert_eq!(status.message, None);
    }
    assert_eq!(env.games[0].title_id, "0100A3D008C5C000");
    assert_eq!(env.games[1].title_id, "01008F6008C5E000");
    assert!(env.games[0].mod_disabled && !env.games[1].mod_disabled);

    let started = Instant::now();
    let again = state.environment();
    let cached = started.elapsed();
    assert!(again.games.iter().all(|game| game.state == GameState::Ready));
    println!("OK both editions paired in {first:?}; detecting again with the cache took {cached:?}");

    let started = Instant::now();
    let violet = state.game(Edition::Violet).unwrap();
    let species = violet.species().unwrap();
    let loaded = started.elapsed();
    let pikachu = species.entries.iter().find(|e| e.national == 25 && e.form == 0).unwrap();
    assert_eq!(pikachu.name, "Pikachu");
    assert_eq!((pikachu.stage, pikachu.legendary, pikachu.regional[0]), (2, false, true));
    let koraidon = species.entries.iter().find(|e| e.national == 1007).unwrap();
    assert!(koraidon.legendary && koraidon.generation == 9);

    let mut settings = Settings::defaults(&state.registry);
    let all = species.pool_counts(&state.registry, &settings);
    let mut pool = PoolFilter::everything();
    pool.legendaries = [false; 9];
    settings.set(GLOBAL_POOL, OptValue::Pool(pool));
    let no_legendaries = species.pool_counts(&state.registry, &settings);
    assert_eq!(all.total, species.entries.len());
    assert!(all.counts[GLOBAL_POOL] < all.total, "Koraidon and Miraidon never join a pool");
    assert!(no_legendaries.counts[GLOBAL_POOL] < all.counts[GLOBAL_POOL]);
    let changes = describe_changes(&state.registry, &settings, Some(species));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].value, "No legendaries");
    println!(
        "OK Violet species list built in {loaded:?}: {} playable forms, global pool {} by default, {} without legendaries",
        all.total, all.counts[GLOBAL_POOL], no_legendaries.counts[GLOBAL_POOL]
    );
}

#[test]
fn pairing_problems_are_explained() {
    let Some((keys, games)) = real_inputs("pairing_problems_are_explained") else {
        return;
    };
    let keys = Keys::load(&keys).unwrap();
    let temp = TempDir::new("pairing");
    let empty = temp.path().join("no updates here");
    fs::create_dir_all(&empty).unwrap();
    let files = real_games(&games);
    let chosen = |base: &Path, update: Option<&Path>| GameFileOverride {
        base: Some(base.to_path_buf()),
        update: update.map(Path::to_path_buf),
    };
    let messages = |config: &AppConfig| -> Vec<(GameState, String)> {
        let slots = find_games(config, std::slice::from_ref(&empty), Ok(&keys), &[None, None]);
        Edition::ALL
            .map(|edition| {
                let status = slots[edition.index()].status(edition, false);
                (status.state, status.message.unwrap_or_default())
            })
            .into()
    };

    let found = messages(&AppConfig {
        scarlet: chosen(&files.scarlet.base_xci, None),
        violet: chosen(&files.scarlet.base_xci, Some(&files.violet.update_nsp)),
        ..AppConfig::default()
    });
    assert_eq!(found[0].0, GameState::Error);
    assert!(found[0].1.contains("no Pokémon Scarlet 4.0.0 update"), "{}", found[0].1);
    assert!(found[1].1.contains("is Pokémon Scarlet, not Pokémon Violet"), "{}", found[1].1);

    let found = messages(&AppConfig {
        scarlet: chosen(&files.scarlet.base_xci, Some(&files.scarlet_dlc)),
        violet: chosen(&files.violet.base_xci, Some(&files.scarlet.update_nsp)),
        ..AppConfig::default()
    });
    assert!(found[0].1.contains("is not a game update"), "{}", found[0].1);
    assert!(found[1].1.contains("is an update for a different game"), "{}", found[1].1);
    for (state, message) in &found {
        println!("OK {state:?}: {message}");
    }
}
