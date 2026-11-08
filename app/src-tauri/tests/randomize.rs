mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use common::{real_games, real_inputs, TempDir};
use svr_app::edition::Edition;
use svr_app::games::OpenedGame;
use svr_app::job::{self, BatchRequest, JobInput, RandoEvent, RandomizeRequest};
use svr_app::library::{self, SeedRoots};
use svr_app::settings;
use svr_core::container::Keys;
use svr_core::rando::{self, OptValue, PoolFilter, Settings, GLOBAL_POOL};

fn kinds(events: &[RandoEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| match event {
            RandoEvent::Started { total } => format!("started {total}"),
            RandoEvent::SeedStarted { position, .. } => format!("seed_started {position}"),
            RandoEvent::SeedStage { position, stage } => format!("seed_stage {position} {stage:?}"),
            RandoEvent::SeedDone { position, .. } => format!("seed_done {position}"),
            RandoEvent::SeedFailed { position, error, .. } => format!("seed_failed {position}: {error}"),
        })
        .collect()
}

#[test]
fn a_seed_is_written_installed_and_removed() {
    let Some((keys, games)) = real_inputs("a_seed_is_written_installed_and_removed") else {
        return;
    };
    let keys = Keys::load(&keys).expect("load keys");
    let scarlet = real_games(&games).scarlet;
    let game = OpenedGame::open(scarlet.base_xci, scarlet.update_nsp, &keys).expect("open Scarlet");
    let registry = rando::registry();
    let temp = TempDir::new("job");
    let library_dir = temp.path().join("library");

    let mut settings = Settings::defaults(&registry);
    let mut pool = PoolFilter::everything();
    pool.legendaries = [false; 9];
    settings.set(GLOBAL_POOL, OptValue::Pool(pool));
    let request = RandomizeRequest {
        edition: Edition::Scarlet,
        settings: settings.clone(),
        seed: "svr-app-test".to_owned(),
        batch: None,
    };
    let input = JobInput {
        request: &request,
        game: &game,
        registry: &registry,
        library_dir: &library_dir,
        next_batch_index: 4,
    };
    let mut events = Vec::new();
    let started = Instant::now();
    let summary = job::run(&input, &AtomicBool::new(false), &mut |event| events.push(event)).unwrap();
    println!("OK one seed in {:?}", started.elapsed());

    assert_eq!(
        kinds(&events),
        [
            "started 1",
            "seed_started 0",
            "seed_stage 0 Randomizing",
            "seed_stage 0 Logging",
            "seed_stage 0 Writing",
            "seed_done 0",
        ]
    );
    let folder = library_dir.join("Scarlet svr-app-test");
    let RandoEvent::SeedDone { name, seed, folder: done_folder, warnings, .. } = &events[5] else {
        unreachable!()
    };
    assert_eq!((name.as_str(), seed.as_str()), ("Scarlet svr-app-test", "svr-app-test"));
    assert_eq!(done_folder, &folder);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((summary.total, summary.done, summary.failed, summary.cancelled), (1, 1, 0, false));
    assert_eq!(summary.folders, [folder.as_path()]);
    assert_eq!(summary.next_batch_index, 4);

    for file in ["romfs/arc/data.trpfd", "svr-seed.json", "spoiler.txt", "spoiler.html", "settings.json"] {
        assert!(folder.join(file).is_file(), "missing {file}");
    }
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(folder.join("svr-seed.json")).unwrap()).unwrap();
    assert_eq!(meta["edition"], "scarlet");
    assert_eq!(meta["title_id"], "0100A3D008C5C000");
    assert_eq!(meta["seed"], "svr-app-test");
    let reloaded = settings::load_file(&folder.join("settings.json"), &registry).unwrap();
    assert_eq!(reloaded.settings, settings.normalized(&registry).0);
    let spoiler = fs::read_to_string(folder.join("spoiler.txt")).unwrap();
    assert!(spoiler.contains("Seed: svr-app-test") && spoiler.contains("No legendaries"), "{spoiler}");
    println!("OK spoiler.txt:\n{spoiler}");
    let html = fs::read_to_string(folder.join("spoiler.html")).unwrap();
    assert!(html.contains("id=\"search\"") && html.contains("Species pool"));

    install_and_remove(&temp.path().join("eden"), &library_dir, &folder);

    let again = job::run(&input, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!(again.folders, [library_dir.join("Scarlet svr-app-test (2)")]);
    assert!(folder.join("svr-seed.json").is_file(), "the first seed is kept");

    batch_numbering(&game, &registry, temp.path());
}

fn install_and_remove(eden: &Path, library_dir: &Path, seed: &Path) {
    let load = eden.join("load");
    let nand = eden.join("nand");
    let save = nand.join("user/save/0000000000000000/0123456789ABCDEF0123456789ABCDEF/0100a3d008c5c000");
    fs::create_dir_all(&save).unwrap();
    fs::write(save.join("main"), b"save data").unwrap();
    let backups = eden.join("backups");

    let result = library::activate_seed(seed, &load, Some(&nand), Some(&backups)).unwrap();
    let installed = load.join("0100A3D008C5C000").join("SV Randomizer");
    assert_eq!(result.installed_to, installed);
    assert!(installed.join("romfs/arc/data.trpfd").is_file());
    let backup = result.backup_dir.expect("the save was backed up");
    assert!(backup.starts_with(&backups));
    let copied = backup.join("0000000000000000/0123456789ABCDEF0123456789ABCDEF/0100a3d008c5c000/main");
    assert_eq!(fs::read(copied).unwrap(), b"save data");

    let roots = SeedRoots {
        library: library_dir.to_owned(),
        extra: Vec::new(),
    };
    let seeds = library::list_seeds(&roots, Some(&load));
    let active: Vec<&str> = seeds.iter().filter(|s| s.active).map(|s| s.name.as_str()).collect();
    assert_eq!(active, ["Scarlet svr-app-test"]);

    library::deactivate(&load, Edition::Scarlet).unwrap();
    assert!(!installed.exists());
    assert!(library::list_seeds(&roots, Some(&load)).iter().all(|s| !s.active));
    println!("OK installed into {}, backed up to {}", installed.display(), backup.display());
}

fn batch_numbering(game: &OpenedGame, registry: &[svr_core::rando::OptionDef], root: &Path) {
    let output_dir = root.join("batch out");
    let request = RandomizeRequest {
        edition: Edition::Scarlet,
        settings: Settings::defaults(registry),
        seed: "race".to_owned(),
        batch: Some(BatchRequest {
            count: 2,
            prefix: "Race ".to_owned(),
            start_index: 9,
            output_dir: Some(output_dir.clone()),
        }),
    };
    let input = JobInput {
        request: &request,
        game,
        registry,
        library_dir: &root.join("library"),
        next_batch_index: 1,
    };

    let cancelled = job::run(&input, &AtomicBool::new(true), &mut |_| {}).unwrap();
    assert!(cancelled.cancelled && cancelled.done == 0 && cancelled.next_batch_index == 9);

    let mut events = Vec::new();
    let summary = job::run(&input, &AtomicBool::new(false), &mut |event| events.push(event)).unwrap();
    let seeds: Vec<(String, String)> = events
        .iter()
        .filter_map(|event| match event {
            RandoEvent::SeedDone { name, seed, .. } => Some((name.clone(), seed.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        seeds,
        [("Race 009".to_owned(), "race-9".to_owned()), ("Race 010".to_owned(), "race-10".to_owned())]
    );
    assert_eq!(summary.next_batch_index, 11);
    assert!(output_dir.join("Race 010").join("svr-seed.json").is_file());
    println!("OK batch wrote {:?}", summary.folders);
}
