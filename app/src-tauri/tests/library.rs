mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::TempDir;
use svr_app::edition::Edition;
use svr_app::library::{self, SeedRoots};
use svr_core::output::SeedMeta;

/// A seed folder shaped like the ones `job::run` writes, without needing the game.
fn fake_seed(root: &Path, name: &str, edition: Edition, created_unix: u64) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("romfs/arc")).unwrap();
    fs::write(dir.join("romfs/arc/data.trpfd"), name.as_bytes()).unwrap();
    fs::write(dir.join("spoiler.txt"), b"spoiler").unwrap();
    let meta = SeedMeta {
        app_version: "0.1.0".into(),
        edition: edition.id().into(),
        title_id: edition.title_id_hex(),
        seed: name.into(),
        created_unix,
        file_count: 1,
    };
    fs::write(dir.join("svr-seed.json"), serde_json::to_vec_pretty(&meta).unwrap()).unwrap();
    dir
}

#[test]
fn seeds_switch_with_a_save_backup_and_switch_off() {
    let temp = TempDir::new("library");
    let roots = SeedRoots {
        library: temp.path().join("seeds"),
        extra: vec![temp.path().join("Batch run")],
    };
    let older = fake_seed(&roots.library, "Scarlet 1", Edition::Scarlet, 100);
    let newer = fake_seed(&roots.library, "Scarlet 2", Edition::Scarlet, 200);
    let batch = fake_seed(&roots.extra[0], "Race 001", Edition::Violet, 150);
    fs::create_dir_all(roots.library.join("not a seed")).unwrap();

    let load = temp.path().join("eden/load");
    let nand = temp.path().join("eden/nand");
    let save = nand.join("user/save/0000000000000000/CD821E2A4C71B3528698147B36276F04/0100A3D008C5C000");
    fs::create_dir_all(save.join("sub")).unwrap();
    fs::write(save.join("main"), b"main save").unwrap();
    fs::write(save.join("sub/extra"), b"extra").unwrap();
    let backups = temp.path().join("app-data/save-backups");

    let check = library::check_switch(&older, &roots, Some(&load), Some(&nand)).unwrap();
    assert_eq!(check.edition, Edition::Scarlet);
    assert_eq!(check.active_seed, None);
    assert_eq!(check.saves, [save.as_path()]);

    let result = library::activate_seed(&older, &load, Some(&nand), Some(&backups)).unwrap();
    let installed = load.join("0100A3D008C5C000/SV Randomizer");
    assert_eq!(result.installed_to, installed);
    assert_eq!(fs::read(installed.join("romfs/arc/data.trpfd")).unwrap(), b"Scarlet 1");
    let backup = result.backup_dir.expect("a save existed");
    let backup_name = backup.file_name().unwrap().to_string_lossy().into_owned();
    assert!(backup.starts_with(&backups) && backup_name.starts_with("0100A3D008C5C000 "), "{backup_name}");
    let copied = backup.join("0000000000000000/CD821E2A4C71B3528698147B36276F04/0100A3D008C5C000");
    assert_eq!(fs::read(copied.join("main")).unwrap(), b"main save");
    assert_eq!(fs::read(copied.join("sub/extra")).unwrap(), b"extra");

    let seeds = library::list_seeds(&roots, Some(&load));
    let listed: Vec<(&str, Option<&str>, bool)> =
        seeds.iter().map(|s| (s.name.as_str(), s.group.as_deref(), s.active)).collect();
    assert_eq!(
        listed,
        [("Scarlet 2", None, false), ("Race 001", Some("Batch run"), false), ("Scarlet 1", None, true)]
    );
    assert!(seeds.iter().all(|s| s.has_spoiler));

    let check = library::check_switch(&newer, &roots, Some(&load), Some(&nand)).unwrap();
    assert_eq!(check.active_seed.as_deref(), Some("Scarlet 1"));

    let result = library::activate_seed(&newer, &load, Some(&nand), None).unwrap();
    assert_eq!(result.backup_dir, None);
    assert_eq!(fs::read(installed.join("romfs/arc/data.trpfd")).unwrap(), b"Scarlet 2");
    let active: Vec<String> = library::list_seeds(&roots, Some(&load))
        .into_iter()
        .filter(|s| s.active)
        .map(|s| s.name)
        .collect();
    assert_eq!(active, ["Scarlet 2"]);

    library::deactivate(&load, Edition::Scarlet).unwrap();
    assert!(!installed.exists());
    library::deactivate(&load, Edition::Scarlet).unwrap();
    assert!(load.join("0100A3D008C5C000").is_dir(), "only the mod folder is removed");

    let outside = fake_seed(temp.path(), "Loose seed", Edition::Scarlet, 1);
    assert!(library::delete_seed(&outside, &roots).is_err());
    assert!(outside.exists());
    assert!(library::delete_seed(&roots.library.join("not a seed"), &roots).is_err());
    assert!(library::delete_seed(&older.join("romfs"), &roots).is_err());
    library::delete_seed(&batch, &roots).unwrap();
    assert!(!batch.exists());
    library::delete_seed(&older, &roots).unwrap();
    assert!(!older.exists() && newer.exists());
    println!("OK library switching, backup to {}, and guarded deletes", backup.display());
}
