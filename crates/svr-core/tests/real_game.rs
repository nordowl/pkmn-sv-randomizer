use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths, OpenError};
use svr_core::trinity::{parse_index, path_hash, remove_from_index, ArchiveError, IndexError};

const EXPECTED_FILES: usize = 408_641;
const EXPECTED_PACKS: usize = 23_589;
const EXPECTED_INDEX_BYTES: usize = 20_266_112;
const EXPECTED_TABLES: usize = 290;
const EXPECTED_SCHEMAS: usize = 287;
const GIFTS: &str = "world/data/event/event_add_pokemon/eventAddPokemon/eventAddPokemon_array.bin";
const TRAINERS: &str = "world/data/trainer/trdata/trdata_array.bin";

struct Case {
    edition: Edition,
    build_id: &'static str,
    title_id: u64,
}

const SCARLET: usize = 0;
const VIOLET: usize = 1;
const CASES: [Case; 2] = [
    Case {
        edition: Edition::Scarlet,
        build_id: "421C5411B487EB4D049DD065FEC9547773E8E598",
        title_id: 0x0100_A3D0_08C5_C000,
    },
    Case {
        edition: Edition::Violet,
        build_id: "709BFD66115298640155FCC4979DBA151C7CC79A",
        title_id: 0x0100_8F60_08C5_E000,
    },
];

static OPENED: [OnceLock<GameFiles>; 2] = [const { OnceLock::new() }; 2];
static KEYS: OnceLock<Keys> = OnceLock::new();

struct TestEnv {
    keys: PathBuf,
    games: PathBuf,
    tables: PathBuf,
}

impl TestEnv {
    fn load(test: &str) -> Option<TestEnv> {
        let var = |name: &str| env::var_os(name).map(PathBuf::from);
        let (Some(keys), Some(games), Some(tables)) = (
            var("SVR_TEST_KEYS"),
            var("SVR_TEST_GAMES"),
            var("SVR_TEST_TABLES"),
        ) else {
            println!("SKIP {test}: set SVR_TEST_KEYS, SVR_TEST_GAMES and SVR_TEST_TABLES to run");
            return None;
        };
        Some(TestEnv {
            keys,
            games,
            tables,
        })
    }

    fn keys(&self) -> &'static Keys {
        KEYS.get_or_init(|| Keys::load(&self.keys).expect("load keys"))
    }

    fn find(&self, case: usize) -> GamePaths {
        GamePaths::find(&self.games, CASES[case].edition).expect("the base XCI and update NSP are in SVR_TEST_GAMES")
    }

    fn paths(&self, base: usize, update: usize) -> GamePaths {
        GamePaths {
            base_xci: self.find(base).base_xci,
            update_nsp: self.find(update).update_nsp,
        }
    }

    fn game(&self, case: usize) -> &'static GameFiles {
        OPENED[case].get_or_init(|| {
            let start = Instant::now();
            let game = GameFiles::open(&self.paths(case, case), self.keys()).expect("open game");
            println!(
                "TIMING open {:?}: {:.3}s (first open)",
                CASES[case].edition,
                start.elapsed().as_secs_f64()
            );
            game
        })
    }

    fn fixtures(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        collect_fixtures(&self.tables, "", &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        let count = |extension: &str| out.iter().filter(|f| f.0.ends_with(extension)).count();
        assert_eq!(count(".bin"), EXPECTED_TABLES, "fixture table count");
        assert_eq!(count(".bfbs"), EXPECTED_SCHEMAS, "fixture schema count");
        out
    }
}

fn collect_fixtures(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("read fixture dir") {
        let entry = entry.expect("read fixture entry");
        let name = entry.file_name().into_string().expect("fixture name is UTF-8");
        let path = format!("{prefix}{name}");
        if entry.file_type().expect("fixture type").is_dir() {
            collect_fixtures(&entry.path(), &format!("{path}/"), out);
        } else {
            out.push((path, fs::read(entry.path()).expect("read fixture")));
        }
    }
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[test]
fn both_games_open_with_the_expected_identity() {
    let Some(env) = TestEnv::load("both_games_open_with_the_expected_identity") else {
        return;
    };
    for (index, case) in CASES.iter().enumerate() {
        let game = env.game(index);
        assert_eq!(game.edition(), case.edition);
        assert_eq!(hex_upper(&game.build_id()), case.build_id);
        assert_eq!(game.base_title_id(), case.title_id);
        assert_eq!(game.archive().file_count(), EXPECTED_FILES);
        assert_eq!(game.archive().pack_count(), EXPECTED_PACKS);
        assert_eq!(game.raw_index().len(), EXPECTED_INDEX_BYTES);
        println!(
            "OK {:?}: build {}, title {:016X}, {} files, {} packs, index {} bytes",
            game.edition(),
            hex_upper(&game.build_id()),
            game.base_title_id(),
            game.archive().file_count(),
            game.archive().pack_count(),
            game.raw_index().len(),
        );
    }
}

#[test]
fn every_fixture_is_byte_identical_on_both_games() {
    let Some(env) = TestEnv::load("every_fixture_is_byte_identical_on_both_games") else {
        return;
    };
    let fixtures = env.fixtures();
    for (index, case) in CASES.iter().enumerate() {
        let game = env.game(index);
        let start = Instant::now();
        let mut bytes = 0;
        let mut failures = Vec::new();
        for (path, expected) in &fixtures {
            match game.read(path) {
                Ok(actual) if actual == *expected => bytes += actual.len(),
                Ok(actual) => failures.push(format!(
                    "{path}: {} bytes, expected {}",
                    actual.len(),
                    expected.len()
                )),
                Err(error) => failures.push(format!("{path}: {error}")),
            }
        }
        println!(
            "TIMING read all fixtures {:?}: {} files, {} bytes, {:.3}s",
            case.edition,
            fixtures.len(),
            bytes,
            start.elapsed().as_secs_f64()
        );
        assert!(
            failures.is_empty(),
            "{:?}: {} of {} fixtures differ:\n{}",
            case.edition,
            failures.len(),
            fixtures.len(),
            failures.join("\n")
        );
        println!("OK {:?}: all {} fixtures byte-identical", case.edition, fixtures.len());
    }
}

#[test]
fn parallel_reads_on_one_instance_stay_correct() {
    let Some(env) = TestEnv::load("parallel_reads_on_one_instance_stay_correct") else {
        return;
    };
    let fixtures = env.fixtures();
    let game = GameFiles::open(&env.paths(SCARLET, SCARLET), env.keys()).expect("open game");
    let start = Instant::now();
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let (fixtures, game) = (&fixtures, &game);
            scope.spawn(move || {
                let rotated = fixtures.iter().cycle().skip(worker * 37).take(fixtures.len());
                for (path, expected) in rotated {
                    let actual = game.read(path).expect("parallel read");
                    assert!(actual == *expected, "parallel read of {path} differs");
                }
            });
        }
    });
    println!(
        "TIMING 8 threads each reading all {} fixtures: {:.3}s",
        fixtures.len(),
        start.elapsed().as_secs_f64()
    );
}

#[test]
fn removing_paths_keeps_every_other_mapping() {
    let Some(env) = TestEnv::load("removing_paths_keeps_every_other_mapping") else {
        return;
    };
    for (index, case) in CASES.iter().enumerate() {
        let game = env.game(index);
        let raw = game.raw_index();
        let patched = remove_from_index(raw, &[GIFTS, TRAINERS]).expect("remove");
        assert_eq!(patched.len(), raw.len());

        let before = parse_index(raw).expect("parse original");
        let after = parse_index(&patched).expect("parse patched");
        assert_eq!(after.entries.len(), EXPECTED_FILES - 2);

        let removed: HashSet<u64> = [path_hash(GIFTS), path_hash(TRAINERS)].into();
        assert_eq!(before.entries.iter().filter(|e| removed.contains(&e.hash)).count(), 2);
        assert!(after.entries.iter().all(|e| !removed.contains(&e.hash)));

        let expected: Vec<_> = before
            .entries
            .iter()
            .filter(|e| !removed.contains(&e.hash))
            .copied()
            .collect();
        assert!(after.entries == expected, "order or pack mapping changed");
        assert!(after.pack_paths == before.pack_paths, "pack paths changed");

        println!(
            "OK {:?}: {} -> {} files, length {} unchanged, all other mappings identical",
            case.edition,
            before.entries.len(),
            after.entries.len(),
            patched.len()
        );

        let missing = remove_from_index(raw, &[GIFTS, "no/such/file.bin"]).unwrap_err();
        assert!(matches!(missing, IndexError::PathNotInIndex { path } if path == "no/such/file.bin"));
    }
}

#[test]
fn swapped_games_are_refused_with_the_right_error() {
    let Some(env) = TestEnv::load("swapped_games_are_refused_with_the_right_error") else {
        return;
    };
    for (base, update) in [(SCARLET, VIOLET), (VIOLET, SCARLET)] {
        let error = GameFiles::open(&env.paths(base, update), env.keys()).unwrap_err();
        println!("OK swapped: {error}");
        match error {
            OpenError::UpdateForDifferentGame {
                edition,
                expected,
                found,
                ..
            } => {
                assert_eq!(edition, CASES[base].edition);
                assert_eq!(expected, CASES[base].title_id);
                assert_eq!(found, CASES[update].title_id);
            }
            other => panic!("wrong error: {other:?}"),
        }
    }
}

#[test]
fn wrong_files_and_keys_get_plain_errors() {
    let Some(env) = TestEnv::load("wrong_files_and_keys_get_plain_errors") else {
        return;
    };
    let paths = env.paths(SCARLET, SCARLET);

    let swapped_slots = GamePaths {
        base_xci: paths.update_nsp.clone(),
        update_nsp: paths.base_xci.clone(),
    };
    let error = GameFiles::open(&swapped_slots, env.keys()).unwrap_err();
    println!("OK nsp as xci: {error}");
    assert!(matches!(error, OpenError::NotScarletVioletXci { found_title_id: None, .. }));

    let xci_as_update = GamePaths {
        base_xci: paths.base_xci.clone(),
        update_nsp: paths.base_xci.clone(),
    };
    let error = GameFiles::open(&xci_as_update, env.keys()).unwrap_err();
    println!("OK xci as update: {error}");
    assert!(matches!(error, OpenError::NotAnUpdate { .. }));

    let absent = GamePaths {
        base_xci: env.games.join("does-not-exist.xci"),
        update_nsp: paths.update_nsp.clone(),
    };
    let error = GameFiles::open(&absent, env.keys()).unwrap_err();
    println!("OK absent file: {error}");
    assert!(matches!(error, OpenError::Unreadable { .. }));

    let empty_keys_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("empty.keys");
    fs::write(&empty_keys_path, "").expect("write empty keys file");
    let no_keys = Keys::load(&empty_keys_path).expect("load empty keys");
    let error = GameFiles::open(&paths, &no_keys).unwrap_err();
    println!("OK no keys: {error}");
    assert!(matches!(&error, OpenError::MissingKey { name } if name == "header_key"));

    let zero_keys_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("zero.keys");
    fs::write(&zero_keys_path, format!("header_key = {}\n", "00".repeat(32))).expect("write keys");
    let zero_keys = Keys::load(&zero_keys_path).expect("load zero keys");
    let error = GameFiles::open(&paths, &zero_keys).unwrap_err();
    println!("OK wrong header key: {error}");
    assert!(matches!(error, OpenError::Corrupt { .. }));

    let dlc = fs::read_dir(&env.games)
        .expect("read SVR_TEST_GAMES")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().to_uppercase().contains("0100A3D008C5D001"));
    if let Some(dlc) = dlc {
        let dlc_as_update = GamePaths {
            base_xci: paths.base_xci.clone(),
            update_nsp: dlc,
        };
        let error = GameFiles::open(&dlc_as_update, env.keys()).unwrap_err();
        println!("OK DLC as update: {error}");
        assert!(matches!(error, OpenError::NotAnUpdate { .. }));
    }
}

#[test]
fn nonexistent_paths_are_clean_errors() {
    let Some(env) = TestEnv::load("nonexistent_paths_are_clean_errors") else {
        return;
    };
    let game = env.game(SCARLET);
    assert!(!game.contains("no/such/file.bin"));
    let error = game.read("no/such/file.bin").unwrap_err();
    println!("OK nonexistent archive path: {error}");
    assert!(matches!(error, ArchiveError::NotFound { .. }));

    let error = game.romfs_file("no/such/file.bin").unwrap_err();
    println!("OK nonexistent RomFS path: {error}");
    assert!(game.contains(GIFTS));
}

#[test]
fn loose_romfs_files_are_readable() {
    let Some(env) = TestEnv::load("loose_romfs_files_are_readable") else {
        return;
    };
    let game = env.game(SCARLET);
    let index = game.romfs_file("arc/data.trpfd").expect("read loose index");
    assert!(index == game.raw_index(), "RomFS index differs from raw_index");
    println!("OK RomFS arc/data.trpfd is {} bytes and equals raw_index", index.len());
}
