use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::load_table;
use svr_core::fb::TableValue;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::rando::{self, OptValue, RandoOutput, Seed, Settings};

fn scarlet(test: &str) -> Option<GameFiles> {
    let (Some(keys), Some(games)) = (std::env::var_os("SVR_TEST_KEYS"), std::env::var_os("SVR_TEST_GAMES")) else {
        println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES");
        return None;
    };
    let games = PathBuf::from(games);
    let paths = GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES");
    Some(GameFiles::open(&paths, &Keys::load(Path::new(&keys)).unwrap()).unwrap())
}

/// Default settings with `overrides` applied.
fn settings(overrides: &[(&str, OptValue)]) -> Settings {
    let mut s = Settings::defaults(&rando::registry());
    for (id, value) in overrides {
        assert!(s.values.contains_key(*id), "unknown option {id}");
        s.set(id, value.clone());
    }
    s
}

fn on(id: &str) -> (&str, OptValue) {
    (id, OptValue::Bool(true))
}

fn run(game: &GameFiles, settings: &Settings, seed: &str) -> RandoOutput {
    rando::randomize(game, settings, Seed(seed.into())).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
}

/// A table as the seed wrote it, or the vanilla table when the seed left it alone.
fn output_table(game: &GameFiles, out: &RandoOutput, path: &str) -> TableValue {
    let vanilla = load_table(game, path).unwrap();
    match out.files.get(path) {
        Some(bytes) => svr_core::fb::parse(vanilla.schema(), bytes).unwrap(),
        None => vanilla,
    }
}

// ---- bosses ----

mod bosses {
    use std::collections::{BTreeMap, BTreeSet};

    use svr_core::data::store::{load_table, ITEMS, MOVES, PERSONAL};
    use svr_core::data::{scene, Dex, Pokemon, TableStore};
    use svr_core::fb::TableValue;
    use svr_core::game::GameFiles;
    use svr_core::rando::{OptValue, PoolFilter, RandoOutput, Settings};
    use svr_core::text::{GameText, Language};

    use super::{on, output_table, run, scarlet, settings};

    const BATTLES: &str = "world/data/battle/eventBattlePokemon/eventBattlePokemon_array.bin";
    const RELEASE: &str = "world/data/event/s2_sub_012_legend_poke/release_poke/release_poke_array.bin";
    const UNTOUCHED: [&str; 3] = ["lastbattle_", "sub018_", "coin_976_"];
    /// Spoiler group label, battle count from FEATURE_LIST section 11, and scene files (both versions).
    const GROUPS: [(&str, &str, usize, usize); 10] = [
        ("bosses.titans", "Titans", 16, 64),
        ("bosses.ruin", "Treasures of Ruin", 4, 8),
        ("bosses.area_zero", "Area Zero ambushes", 11, 8),
        ("bosses.story", "Lechonk and Houndoom", 2, 4),
        ("bosses.sunflora", "Sunflora gym test", 5, 2),
        ("bosses.kitakami", "Kitakami story", 21, 52),
        ("bosses.kitakami_side", "Kitakami side quest", 2, 4),
        ("bosses.indigo", "Indigo Disk story", 12, 18),
        ("bosses.snacksworth", "Snacksworth legendaries", 25, 50),
        ("bosses.mythicals", "Meloetta and Pecharunt", 2, 6),
    ];
    const WONDER_GUARD: u16 = 25;
    const ESCAPES: [u16; 2] = [193, 194];
    const ENDS_BATTLE: [u16; 5] = [18, 46, 100, 509, 525];
    const FAINTS_SELF: [u16; 8] = [120, 153, 195, 262, 361, 461, 515, 802];
    const RISKY_ITEMS: [u16; 3] = [542, 547, 1119];
    const MANUAL: i64 = 1;
    const STELLAR: i64 = 101;
    const OGERPON_GEMS: [i64; 4] = [13, 12, 11, 7];

    #[derive(Clone, Debug)]
    struct Mon {
        label: String,
        catchable: bool,
        species: Pokemon,
        level: i64,
        manual: bool,
        moves: Vec<u16>,
        item: u16,
        gem: i64,
        tokusei: i64,
        raw: String,
    }

    fn mons(t: &TableValue) -> Vec<Mon> {
        t.tables("values")
            .map(|row| {
                let p = row.table("pokeData").unwrap();
                let int = |f: &str| p.int(f).unwrap();
                Mon {
                    label: row.string("label").unwrap().to_owned(),
                    catchable: row.bool("eventEncount").unwrap(),
                    species: Pokemon {
                        species: int("devId") as u16,
                        form: int("formId") as u16,
                    },
                    level: int("level"),
                    manual: int("wazaType") == MANUAL,
                    moves: (1..=4)
                        .filter_map(|k| p.table(&format!("waza{k}")))
                        .map(|w| w.int("wazaId").unwrap() as u16)
                        .filter(|&m| m != 0)
                        .collect(),
                    item: int("item") as u16,
                    gem: int("gemType"),
                    tokusei: int("tokusei"),
                    raw: format!("{p:?}"),
                }
            })
            .collect()
    }

    fn untouched(label: &str) -> bool {
        UNTOUCHED.iter().any(|p| label.starts_with(p))
    }

    fn found(bytes: &[u8]) -> BTreeMap<Pokemon, usize> {
        scene::swap_species(bytes, &BTreeMap::new()).unwrap().found
    }

    fn log_rows<'a>(out: &'a RandoOutput, title: &str) -> &'a [Vec<String>] {
        out.log
            .sections
            .iter()
            .filter(|s| s.title == "Titans & legendaries")
            .flat_map(|s| &s.tables)
            .find(|t| t.title == title)
            .map_or(&[], |t| &t.rows)
    }

    fn boss_files(out: &RandoOutput) -> BTreeMap<String, Vec<u8>> {
        out.files
            .iter()
            .filter(|(p, _)| p.as_str() == BATTLES || p.as_str() == RELEASE || p.contains("/scene/") || p.contains("/obj_template/"))
            .map(|(p, b)| (p.clone(), b.clone()))
            .collect()
    }

    fn abilities(game: &GameFiles, out: &RandoOutput) -> BTreeMap<Pokemon, [u16; 3]> {
        output_table(game, out, PERSONAL)
            .tables("entry")
            .map(|e| {
                let id = e.struct_value("species").unwrap();
                let p = Pokemon {
                    species: id.int("species").unwrap() as u16,
                    form: id.int("form").unwrap() as u16,
                };
                let a = |f: &str| e.int(f).unwrap() as u16;
                (p, [a("ability_1"), a("ability_2"), a("ability_hidden")])
            })
            .collect()
    }

    fn reachable(abilities: [u16; 3], tokusei: i64) -> Vec<u16> {
        let slots: &[usize] = match tokusei {
            0 => &[0, 1],
            2 => &[0],
            3 => &[1],
            4 => &[2],
            _ => &[0, 1, 2],
        };
        slots.iter().map(|&s| abilities[s]).collect()
    }

    /// Every level-up move of `p` at or below `level`, from the seed's learnsets.
    fn learnable(dex: &Dex, personal: &TableValue, p: Pokemon, level: i64) -> BTreeSet<u16> {
        let row = dex.get(p).unwrap().row;
        personal
            .tables("entry")
            .nth(row)
            .unwrap()
            .vector("levelup_moves")
            .unwrap_or(&[])
            .iter()
            .filter_map(|m| m.as_struct())
            .filter(|m| m.int("level").unwrap() <= level)
            .map(|m| m.int("move").unwrap() as u16)
            .collect()
    }

    /// The four moves the game picks for a battle on wazaType DEFAULT.
    fn game_pick(dex: &Dex, personal: &TableValue, p: Pokemon, level: i64) -> Vec<u16> {
        let row = dex.get(p).unwrap().row;
        let mut learned: Vec<u16> = Vec::new();
        for m in personal.tables("entry").nth(row).unwrap().vector("levelup_moves").unwrap_or(&[]) {
            let m = m.as_struct().unwrap();
            let id = m.int("move").unwrap() as u16;
            if m.int("level").unwrap() <= level && id != 0 && !learned.contains(&id) {
                learned.push(id);
            }
        }
        learned[learned.len().saturating_sub(4)..].to_vec()
    }

    fn form_item(dex: &Dex, p: Pokemon) -> Option<u16> {
        svr_core::rando::itempool::form_item(dex.get(p).unwrap().national, p.form)
    }

    /// Must-win fights never reach Wonder Guard, no fight reaches an escape ability, no changed
    /// fight can use a move or item that ends the battle or knocks the user out, and a form that
    /// needs an item holds it.
    fn assert_winnable(game: &GameFiles, out: &RandoOutput, vanilla: &[Mon], after: &[Mon]) {
        let abilities = abilities(game, out);
        let dex = Dex::load(&mut TableStore::new(game)).unwrap();
        let personal = output_table(game, out, PERSONAL);
        let vanilla_personal = load_table(game, PERSONAL).unwrap();
        for (old, new) in vanilla.iter().zip(after) {
            if untouched(&new.label) {
                continue;
            }
            let reach = reachable(abilities[&new.species], new.tokusei);
            assert!(!reach.iter().any(|a| ESCAPES.contains(a)), "{} can flee: {reach:?}", new.label);
            if !new.catchable {
                assert!(!reach.contains(&WONDER_GUARD), "must-win {} has Wonder Guard", new.label);
            }
            let (moves, games_own) = if new.manual {
                (new.moves.clone(), if new.moves == old.moves { old.moves.clone() } else { Vec::new() })
            } else {
                (game_pick(&dex, &personal, new.species, new.level), game_pick(&dex, &vanilla_personal, old.species, old.level))
            };
            for m in moves.iter().filter(|m| !games_own.contains(m)) {
                assert!(!ENDS_BATTLE.contains(m) && !FAINTS_SELF.contains(m), "{} can use risky move {m}", new.label);
            }
            if new.item != old.item {
                assert!(!RISKY_ITEMS.contains(&new.item), "{} holds battle-ending item {}", new.label, new.item);
            }
            if new.species != old.species {
                if let Some(item) = form_item(&dex, new.species) {
                    assert_eq!(new.item, item, "{}: {:?} must hold its form item", new.label, new.species);
                }
            }
        }
    }

    fn pool(filter: PoolFilter) -> OptValue {
        OptValue::Pool(filter)
    }

    /// Every species but the ones the scripted battles start with, so every fight has to change.
    fn without_originals(game: &GameFiles, vanilla: &[Mon]) -> PoolFilter {
        let dex = Dex::load(&mut TableStore::new(game)).unwrap();
        let mut banned: Vec<u16> = vanilla.iter().map(|m| dex.get(m.species).unwrap().national).collect();
        banned.sort_unstable();
        banned.dedup();
        PoolFilter { banned, ..PoolFilter::everything() }
    }

    fn all_groups_on(extra: &[(&str, OptValue)], filter: PoolFilter) -> Settings {
        let mut overrides: Vec<(&str, OptValue)> = GROUPS.iter().map(|g| on(g.0)).collect();
        overrides.push(("bosses.pool", pool(filter)));
        overrides.extend(extra.iter().cloned());
        settings(&overrides)
    }

    #[test]
    fn bosses_defaults_change_nothing() {
        let Some(game) = scarlet("bosses defaults") else { return };
        let out = run(&game, &settings(&[]), "bosses-default");
        assert!(boss_files(&out).is_empty(), "{:?}", boss_files(&out).keys().collect::<Vec<_>>());
        let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
        let personal = load_table(&game, PERSONAL).unwrap();
        let move_names = GameText::load(&game, Language::English, "wazaname").unwrap();
        let games_own: Vec<String> = mons(&load_table(&game, BATTLES).unwrap())
            .iter()
            .filter(|m| !m.manual && !untouched(&m.label))
            .filter_map(|m| {
                let risky: Vec<String> = game_pick(&dex, &personal, m.species, m.level)
                    .into_iter()
                    .filter(|x| ENDS_BATTLE.contains(x) || FAINTS_SELF.contains(x))
                    .map(|x| move_names.line(usize::from(x)).unwrap_or_default())
                    .collect();
                (!risky.is_empty()).then(|| format!("{} {risky:?}", m.label))
            })
            .collect();
        println!("OK default settings leave scripted battles, scenes and Snacksworth's list alone");
        println!("   the game itself picks risky moves for: {games_own:?}");
    }

    #[test]
    fn bosses_every_group_matches_its_cutscenes() {
        let Some(game) = scarlet("bosses groups") else { return };
        let vanilla = mons(&load_table(&game, BATTLES).unwrap());
        let s = all_groups_on(&[], without_originals(&game, &vanilla));
        let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
        let mut seen = Vec::new();
        let mut alt_forms = Vec::new();
        for seed in ["bosses-1", "bosses-2"] {
            let out = run(&game, &s, seed);
            assert_eq!(boss_files(&out), boss_files(&run(&game, &s, seed)), "seed {seed} is not reproducible");
            let after = mons(&output_table(&game, &out, BATTLES));
            let personal = output_table(&game, &out, PERSONAL);
            let battle_log = log_rows(&out, "Scripted battles");
            let group_of: BTreeMap<&str, &str> = battle_log.iter().map(|r| (r[1].as_str(), r[0].as_str())).collect();

            let mut by_group: BTreeMap<&str, BTreeMap<u16, Pokemon>> = BTreeMap::new();
            for (old, new) in vanilla.iter().zip(&after) {
                if untouched(&old.label) {
                    assert_eq!(old.raw, new.raw, "{} must stay vanilla", old.label);
                    assert!(!group_of.contains_key(old.label.as_str()));
                    continue;
                }
                assert_ne!(old.species, new.species, "{} kept its species", old.label);
                assert!(dex.is_playable(new.species), "{}: {:?} has no model", new.label, new.species);
                assert_eq!(old.level, new.level, "{}: level", new.label);
                assert!(new.manual, "{}: species changed without legal moves", new.label);
                let legal = learnable(&dex, &personal, new.species, new.level);
                assert!(!new.moves.is_empty() && new.moves.iter().all(|m| legal.contains(m)), "{}: illegal moves {:?}", new.label, new.moves);
                let group = group_of.get(old.label.as_str()).unwrap_or_else(|| panic!("{} is not logged", old.label));
                let previous = by_group.entry(group).or_default().insert(old.species.species, new.species);
                assert!(previous.is_none_or(|p| p == new.species), "{group}: one original species got two replacements");
            }
            for &(_, label, battles, _) in &GROUPS {
                let count = battle_log.iter().filter(|r| r[0] == label).count();
                assert_eq!(count, battles, "{label}: battle count");
            }
            assert_winnable(&game, &out, &vanilla, &after);

            let scene_log = log_rows(&out, "Cutscene models");
            let mut patched: BTreeMap<&str, BTreeSet<u16>> = BTreeMap::new();
            for &(_, label, _, files) in &GROUPS {
                let rows: Vec<&Vec<String>> = scene_log.iter().filter(|r| r[0] == label).collect();
                assert_eq!(rows.len(), files, "{label}: scene files changed");
                let map = &by_group[label];
                for row in rows {
                    let path = &row[1];
                    let before = found(&game.read(path).unwrap());
                    let mut expected: BTreeMap<Pokemon, usize> = BTreeMap::new();
                    for (p, n) in &before {
                        *expected.entry(map.get(&p.species).copied().unwrap_or(*p)).or_default() += n;
                        if map.contains_key(&p.species) {
                            patched.entry(label).or_default().insert(p.species);
                        }
                    }
                    let shown = found(&out.files[path]);
                    assert_eq!(shown, expected, "{path}: scene and battle disagree");
                    for p in shown.keys().filter(|p| p.form != 0 && map.values().any(|n| n == *p)) {
                        alt_forms.push(format!("{} shows {:?}", path.rsplit('/').next().unwrap(), p));
                    }
                }
                let unpatched: Vec<u16> = map.keys().filter(|s| !patched.get(label).is_some_and(|p| p.contains(s))).copied().collect();
                assert!(unpatched.is_empty(), "{label}: battles without a cutscene model: {unpatched:?}");
            }

            let release_before = load_table(&game, RELEASE).unwrap();
            let release_after = output_table(&game, &out, RELEASE);
            let legends = &by_group["Snacksworth legendaries"];
            for (old, new) in release_before.tables("values").zip(release_after.tables("values")) {
                let old = old.int("ReleasePokeId").unwrap() as u16;
                let new = new.int("ReleasePokeId").unwrap() as u16;
                assert_eq!(new, legends[&old].species, "Snacksworth's list disagrees with the battle");
            }

            if seed == "bosses-1" {
                for &(_, label, _, _) in &GROUPS {
                    println!("== {label}");
                    for row in battle_log.iter().filter(|r| r[0] == label) {
                        println!("   battle {:<32} {}", row[1], row[2]);
                    }
                    for row in scene_log.iter().filter(|r| r[0] == label) {
                        println!("   {:<17} {} : {}", row[2], row[1], row[3]);
                    }
                }
                for row in log_rows(&out, "Snacksworth's list") {
                    println!("   Snacksworth list {} -> {}", row[0], row[1]);
                }
            }
            seen.push(boss_files(&out));
        }
        assert_ne!(seen[0], seen[1], "different seeds gave the same bosses");
        assert!(!alt_forms.is_empty(), "no replacement showed an alternate form in a cutscene");
        println!("OK {} cutscene objects show alternate forms, e.g. {:?}", alt_forms.len(), &alt_forms[..alt_forms.len().min(3)]);
        println!("OK every group: battles, cutscenes and Snacksworth's list agree for 2 seeds");
    }

    #[test]
    fn bosses_details_follow_their_switches() {
        let Some(game) = scarlet("bosses details") else { return };
        let vanilla = mons(&load_table(&game, BATTLES).unwrap());
        let s = settings(&[
            ("bosses.level_percent", OptValue::Int(50)),
            on("bosses.random_moves"),
            on("bosses.random_items"),
            on("bosses.random_tera"),
        ]);
        let moves = load_table(&game, MOVES).unwrap();
        let power: Vec<i64> = moves.tables("Table").map(|m| m.int("Power").unwrap()).collect();
        let usable: Vec<bool> = moves.tables("Table").map(|m| m.bool("CanUseMove").unwrap()).collect();
        let item_names = GameText::load(&game, Language::English, "itemname").unwrap();
        let harmful = ["Toxic Orb", "Flame Orb", "Iron Ball", "Lucky Egg", "Everstone", "Red Card", "Eject Button", "Eject Pack"];
        let holdable: BTreeSet<u16> = load_table(&game, ITEMS)
            .unwrap()
            .tables("values")
            .filter(|i| i.bool("SetToPoke").unwrap_or(false) && i.int("ItemType").unwrap() != 7)
            .map(|i| i.int("Id").unwrap() as u16)
            .filter(|&id| !harmful.contains(&item_names.line(usize::from(id)).unwrap_or_default().as_str()))
            .collect();
        let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
        let mut seen = Vec::new();
        for seed in ["details-1", "details-2"] {
            let out = run(&game, &s, seed);
            assert_eq!(boss_files(&out), boss_files(&run(&game, &s, seed)), "seed {seed} is not reproducible");
            assert!(boss_files(&out).keys().all(|p| p == BATTLES), "detail options must not touch scenes");
            let after = mons(&output_table(&game, &out, BATTLES));
            let mut gems = BTreeSet::new();
            for (old, new) in vanilla.iter().zip(&after) {
                if untouched(&old.label) {
                    assert_eq!(old.raw, new.raw, "{} must stay vanilla", old.label);
                    continue;
                }
                assert_eq!(old.species, new.species, "{}: species changed with groups off", old.label);
                assert_eq!(new.level, ((old.level * 150 + 50) / 100).clamp(1, 100), "{}: level", old.label);
                assert!(new.manual && new.moves.len() == 4, "{}: {:?}", new.label, new.moves);
                assert!(new.moves.iter().all(|&m| usable[usize::from(m)]), "{}: unusable move", new.label);
                assert!(new.moves.iter().any(|&m| power[usize::from(m)] > 1), "{}: no attack", new.label);
                if form_item(&dex, new.species).is_some() {
                    assert_eq!(new.item, old.item, "{}: a form that needs its item kept it", new.label);
                } else {
                    assert!(holdable.contains(&new.item), "{}: item {} does nothing when held", new.label, new.item);
                }
                match old.gem {
                    STELLAR => assert_eq!(new.gem, STELLAR, "{}: Stellar fight lost Stellar", new.label),
                    _ if old.label.starts_with("SDC01_kamenoni") => {
                        assert_eq!(new.gem, OGERPON_GEMS[usize::from(new.species.form % 4)], "{}: Ogerpon Tera", new.label)
                    }
                    _ if old.label.contains("kodaikame") => assert_eq!(new.gem, old.gem, "{}: Terapagos Tera", new.label),
                    _ => assert!((2..=19).contains(&new.gem), "{}: Tera {}", new.label, new.gem),
                }
                gems.insert(new.gem);
            }
            assert_winnable(&game, &out, &vanilla, &after);
            assert!(gems.len() > 10, "Tera types barely vary: {gems:?}");
            if seed == "details-1" {
                for row in log_rows(&out, "Scripted battles").iter().take(4) {
                    println!("   {} {} | lv {} | {} | {} | {}", row[0], row[1], row[3], row[4], row[5], row[6]);
                }
            }
            seen.push(boss_files(&out));
        }
        assert_ne!(seen[0], seen[1], "different seeds gave the same details");
        println!("OK level +50%, random moves, items and Tera follow their switches for 2 seeds");
    }

    #[test]
    fn bosses_lower_levels_never_pick_risky_moves() {
        let Some(game) = scarlet("bosses levels") else { return };
        let vanilla = mons(&load_table(&game, BATTLES).unwrap());
        let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
        let personal = load_table(&game, PERSONAL).unwrap();
        let s = settings(&[("bosses.level_percent", OptValue::Int(-50))]);
        let out = run(&game, &s, "levels-1");
        let after = mons(&output_table(&game, &out, BATTLES));
        assert_winnable(&game, &out, &vanilla, &after);
        let mut guarded = Vec::new();
        for (old, new) in vanilla.iter().zip(&after) {
            if untouched(&old.label) {
                continue;
            }
            assert_eq!(new.level, ((old.level * 50 + 50) / 100).clamp(1, 100), "{}: level", old.label);
            let game_would = game_pick(&dex, &personal, old.species, new.level);
            let games_own = game_pick(&dex, &personal, old.species, old.level);
            let risky = |m: &u16| (ENDS_BATTLE.contains(m) || FAINTS_SELF.contains(m)) && !games_own.contains(m);
            if !old.manual && game_would.iter().any(risky) {
                assert!(new.manual, "{} would pick a risky move at level {}", new.label, new.level);
                guarded.push(new.label.clone());
            }
        }
        assert!(!guarded.is_empty(), "-50% should reach a level where some battle picks a risky move");
        let unguarded = run(&game, &settings(&[("bosses.level_percent", OptValue::Int(-50)), ("bosses.no_softlock", OptValue::Bool(false))]), "levels-1");
        let left: Vec<String> = mons(&output_table(&game, &unguarded, BATTLES))
            .iter()
            .filter(|m| guarded.contains(&m.label))
            .filter(|m| !m.manual)
            .map(|m| m.label.clone())
            .collect();
        assert_eq!(left, guarded, "with the guard off these battles keep game-picked moves");
        println!("OK level -50%: game-picked moves rewritten for {guarded:?}");
    }

    #[test]
    fn bosses_softlock_guard_is_load_bearing() {
        let Some(game) = scarlet("bosses softlock") else { return };
        let names = GameText::load(&game, Language::English, "tokusei").unwrap();
        assert_eq!(names.line(usize::from(WONDER_GUARD)).unwrap(), "Wonder Guard");
        assert_eq!(names.line(193).unwrap(), "Wimp Out");
        assert_eq!(names.line(194).unwrap(), "Emergency Exit");
        let move_names = GameText::load(&game, Language::English, "wazaname").unwrap();
        let listed: Vec<String> = ENDS_BATTLE.iter().chain(&FAINTS_SELF).map(|&m| move_names.line(usize::from(m)).unwrap()).collect();
        assert_eq!(
            listed,
            [
                "Whirlwind", "Roar", "Teleport", "Circle Throw", "Dragon Tail", "Self-Destruct", "Explosion", "Perish Song",
                "Memento", "Healing Wish", "Lunar Dance", "Final Gambit", "Misty Explosion"
            ]
        );
        let item_names = GameText::load(&game, Language::English, "itemname").unwrap();
        let items: Vec<String> = RISKY_ITEMS.iter().map(|&i| item_names.line(usize::from(i)).unwrap()).collect();
        assert_eq!(items, ["Red Card", "Eject Button", "Eject Pack"]);

        let vanilla = mons(&load_table(&game, BATTLES).unwrap());
        let random_abilities = [on("pokemon.abilities"), on("pokemon.abilities_allow_wonder_guard")];
        for seed in ["guard-1", "guard-2"] {
            let shuffled = run(&game, &settings(&random_abilities), seed);
            let risky: BTreeSet<u16> = abilities(&game, &shuffled)
                .iter()
                .filter(|(_, a)| a.iter().any(|a| *a == WONDER_GUARD || ESCAPES.contains(a)))
                .map(|(p, _)| p.species)
                .collect();
            let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
            let risky_dex: BTreeSet<u16> = dex.playable().filter(|f| risky.contains(&f.id.species)).map(|f| f.national).collect();
            let narrow = PoolFilter {
                generations: [false; 9],
                allowed: risky_dex.iter().copied().chain([25]).collect(),
                ..PoolFilter::everything()
            };
            let reaching = |guard: bool| {
                let mut extra = random_abilities.to_vec();
                extra.push(("bosses.no_softlock", OptValue::Bool(guard)));
                let out = run(&game, &all_groups_on(&extra, narrow.clone()), seed);
                let after = mons(&output_table(&game, &out, BATTLES));
                let table = abilities(&game, &out);
                let count = after
                    .iter()
                    .filter(|m| !untouched(&m.label))
                    .filter(|m| {
                        let reach = reachable(table[&m.species], m.tokusei);
                        reach.iter().any(|a| ESCAPES.contains(a)) || (!m.catchable && reach.contains(&WONDER_GUARD))
                    })
                    .count();
                (out, after, count)
            };
            let (out, after, guarded) = reaching(true);
            assert_winnable(&game, &out, &vanilla, &after);
            assert_eq!(guarded, 0);
            let (_, _, unguarded) = reaching(false);
            assert!(unguarded > 0, "with the guard off the risky pool should reach a fight");
            println!(
                "OK {seed}: {} species get Wonder Guard or an escape ability; guard on 0 fights reach one, guard off {unguarded}",
                risky_dex.len()
            );
        }
    }
}

// ---- raids ----

mod raids {
    use std::collections::{BTreeMap, BTreeSet};

    use svr_core::data::store::{load_table, ITEMS, PERSONAL};
    use svr_core::data::{Dex, Names, Pokemon, TableStore};
    use svr_core::fb::TableValue;
    use svr_core::game::GameFiles;
    use svr_core::rando::{OptValue, PoolFilter, RandoOutput, Settings};
    use svr_core::text::Language;

    use super::{on, output_table, run, scarlet, settings};

    const REGIONS: [(&str, &str); 3] = [("paldea", ""), ("kitakami", "su1_"), ("blueberry", "su2_")];
    const FIXED: &str = "world/data/raid/raid_fixed_reward_item/raid_fixed_reward_item_array.bin";
    const LOTTERY: &str = "world/data/raid/raid_lottery_reward_item/raid_lottery_reward_item_array.bin";
    const MANUAL: i64 = 1;
    const WAZA_ACTION: i64 = 3;
    const RARE: i64 = 2;

    fn boss_table(prefix: &str, tier: usize) -> String {
        format!("world/data/raid/{prefix}raid_enemy_0{tier}/{prefix}raid_enemy_0{tier}_array.bin")
    }

    fn all_raid_tables() -> Vec<String> {
        let mut out: Vec<String> = REGIONS
            .iter()
            .flat_map(|(_, prefix)| (1..=6).map(|tier| boss_table(prefix, tier)))
            .collect();
        out.extend([FIXED.to_owned(), LOTTERY.to_owned()]);
        out
    }

    fn number(id: &str, v: i64) -> (&str, OptValue) {
        (id, OptValue::Int(v))
    }

    fn text<'a>(id: &'a str, v: &str) -> (&'a str, OptValue) {
        (id, OptValue::Text(v.into()))
    }

    #[derive(Debug, PartialEq)]
    struct Boss {
        species: Pokemon,
        level: u16,
        waza_type: i64,
        moves: Vec<u16>,
        gem: i64,
        rare: i64,
        cued: Vec<u16>,
        penalty: u16,
    }

    fn int(t: &TableValue, field: &str) -> i64 {
        t.int(field).unwrap()
    }

    fn bosses(t: &TableValue) -> Vec<Boss> {
        t.tables("values")
            .map(|row| {
                let info = row.table("raidEnemyInfo").unwrap();
                let poke = info.table("bossPokePara").unwrap();
                let desc = info.table("bossDesc").unwrap();
                Boss {
                    species: Pokemon {
                        species: int(poke, "devId") as u16,
                        form: int(poke, "formId") as u16,
                    },
                    level: int(poke, "level") as u16,
                    waza_type: int(poke, "wazaType"),
                    moves: (1..=4)
                        .filter_map(|i| poke.table(&format!("waza{i}")))
                        .map(|w| int(w, "wazaId") as u16)
                        .filter(|&m| m != 0)
                        .collect(),
                    gem: int(poke, "gemType"),
                    rare: int(poke, "rareType"),
                    cued: (1..=6)
                        .filter_map(|i| desc.table(&format!("extraAction{i}")))
                        .filter(|a| int(a, "action") == WAZA_ACTION)
                        .map(|a| int(a, "wazano") as u16)
                        .filter(|&m| m != 0)
                        .collect(),
                    penalty: int(desc, "powerChargePenaltyAction") as u16,
                }
            })
            .collect()
    }

    /// Vanilla and seeded bosses of one table.
    fn compare(game: &GameFiles, out: &RandoOutput, path: &str) -> Vec<(Boss, Boss)> {
        let before = bosses(&load_table(game, path).unwrap());
        let after = bosses(&output_table(game, out, path));
        assert_eq!(before.len(), after.len(), "{path}: row count");
        before.into_iter().zip(after).collect()
    }

    struct Learnsets {
        dex: Dex,
        personal: TableValue,
    }

    impl Learnsets {
        fn new(game: &GameFiles, out: &RandoOutput) -> Learnsets {
            Learnsets {
                dex: Dex::load(&mut TableStore::new(game)).unwrap(),
                personal: output_table(game, out, PERSONAL),
            }
        }

        fn at(&self, p: Pokemon, level: u16) -> BTreeSet<u16> {
            let row = self.dex.get(p).unwrap_or_else(|| panic!("{p:?} not in dex")).row;
            let entry = self.personal.tables("entry").nth(row).unwrap();
            entry
                .vector("levelup_moves")
                .unwrap_or(&[])
                .iter()
                .filter_map(|m| m.as_struct())
                .filter(|m| m.int("level").unwrap() <= i64::from(level))
                .map(|m| m.int("move").unwrap() as u16)
                .collect()
        }

        fn bst(&self, p: Pokemon) -> u32 {
            self.dex.get(p).unwrap().bst()
        }
    }

    fn assert_legal_boss(learn: &Learnsets, path: &str, row: usize, old: &Boss, new: &Boss) {
        let what = format!("{path} row {row} {:?}", new.species);
        assert!(learn.dex.is_playable(new.species), "{what}: not playable");
        assert_eq!(new.level, old.level, "{what}: level");
        let learnable = learn.at(new.species, new.level);
        assert_eq!(new.waza_type, MANUAL, "{what}: moves not manual");
        assert!(!new.moves.is_empty(), "{what}: no moves");
        for m in new.moves.iter().chain(&new.cued) {
            assert!(learnable.contains(m), "{what}: move {m} is not a level-up move at {}", new.level);
        }
        assert!(new.cued.len() <= old.cued.len(), "{what}: new cued moves appeared");
        assert_eq!(new.penalty == 0, old.penalty == 0, "{what}: penalty move appeared or vanished");
        if new.penalty != 0 {
            assert!(learnable.contains(&new.penalty), "{what}: penalty move {} not learnable", new.penalty);
        }
    }

    fn raid_files(out: &RandoOutput) -> BTreeMap<String, Vec<u8>> {
        let paths = all_raid_tables();
        out.files.iter().filter(|(p, _)| paths.contains(p)).map(|(p, b)| (p.clone(), b.clone())).collect()
    }

    /// Runs twice to show the seed alone decides the output.
    fn run_twice(game: &GameFiles, s: &Settings, seed: &str) -> RandoOutput {
        let out = run(game, s, seed);
        assert_eq!(raid_files(&out), raid_files(&run(game, s, seed)), "seed {seed} is not reproducible");
        out
    }

    fn table_rows<'a>(out: &'a RandoOutput, title: &str) -> &'a [Vec<String>] {
        out.log
            .sections
            .iter()
            .filter(|s| s.title == "Tera Raids")
            .flat_map(|s| &s.tables)
            .find(|t| t.title == title)
            .map_or(&[], |t| &t.rows)
    }

    #[test]
    fn raids_defaults_leave_every_raid_table_unchanged() {
        let Some(game) = scarlet("raids defaults") else { return };
        let out = run(&game, &settings(&[]), "raids-default");
        for path in all_raid_tables() {
            assert!(!out.files.contains_key(&path), "{path} changed with default settings");
        }
        assert!(out.log.sections.iter().all(|s| s.title != "Tera Raids"));
        println!("OK default settings leave all 20 raid tables alone");
    }

    #[test]
    fn raids_random_bosses_respect_regions_tiers_and_learnsets() {
        let Some(game) = scarlet("raids bosses") else { return };
        let s = settings(&[
            on("raids.paldea.bosses"),
            on("raids.kitakami.bosses"),
            ("raids.tier6", OptValue::Bool(false)),
        ]);
        let names = Names::load(&game, Language::English).unwrap();
        let mut first_paldea = None;
        for seed in ["raids-1", "raids-2"] {
            let out = run_twice(&game, &s, seed);
            let learn = Learnsets::new(&game, &out);
            for (region, prefix) in REGIONS {
                let mut changed = 0;
                let mut rows = 0;
                let mut pairs = Vec::new();
                let mut remaps = Vec::new();
                for tier in 1..=6 {
                    let path = boss_table(prefix, tier);
                    if region == "blueberry" || tier == 6 {
                        assert!(!out.files.contains_key(&path), "{path} changed but is off");
                        continue;
                    }
                    for (row, (old, new)) in compare(&game, &out, &path).iter().enumerate() {
                        rows += 1;
                        assert_eq!((new.gem, new.rare), (old.gem, old.rare), "{path} row {row}: Tera or shiny changed");
                        if new.species == old.species {
                            assert_eq!(new, old, "{path} row {row}: same species but other fields changed");
                            continue;
                        }
                        changed += 1;
                        assert_legal_boss(&learn, &path, row, old, new);
                        if pairs.len() < 4 && row % 9 == 0 {
                            pairs.push(format!(
                                "{tier}★ {} -> {}",
                                names.pokemon(old.species),
                                names.pokemon(new.species)
                            ));
                        }
                        let cued = old.cued.iter().zip(&new.cued).chain([(&old.penalty, &new.penalty)]);
                        for (a, b) in cued.filter(|(a, _)| **a != 0) {
                            if remaps.len() < 6 && (row + remaps.len()) % 5 == 0 {
                                remaps.push(format!("{tier}★ {} -> {}", names.move_name(*a), names.move_name(*b)));
                            }
                        }
                    }
                }
                if region != "blueberry" {
                    assert!(changed * 10 >= rows * 9, "{region}: only {changed} of {rows} bosses changed");
                    let logged = table_rows(&out, &format!("{}{} raid bosses", region[..1].to_uppercase(), &region[1..]));
                    assert_eq!(logged.len(), changed, "{region}: spoiler rows");
                    println!("OK {seed} {region}: {changed}/{rows} bosses changed, e.g. {pairs:?}");
                    println!("   cued moves: {remaps:?}");
                    if let Some(first) = logged.first() {
                        println!("   log: {first:?}");
                    }
                }
            }
            let paldea1 = out.files[&boss_table("", 1)].clone();
            match &first_paldea {
                None => first_paldea = Some(paldea1),
                Some(prev) => assert_ne!(prev, &paldea1, "different seeds gave the same Paldea 1★ bosses"),
            }
        }
    }

    #[test]
    fn raids_scaling_orders_tiers_by_strength() {
        let Some(game) = scarlet("raids scaling") else { return };
        let s = settings(&[
            on("raids.paldea.bosses"),
            on("raids.paldea.scale"),
            on("raids.paldea_for_all"),
            number("raids.kitakami.shiny", 100),
        ]);
        for seed in ["raids-scale-1", "raids-scale-2"] {
            let out = run_twice(&game, &s, seed);
            let learn = Learnsets::new(&game, &out);
            for (region, prefix) in REGIONS {
                let mut averages = Vec::new();
                for tier in 1..=6 {
                    let path = boss_table(prefix, tier);
                    let pairs = compare(&game, &out, &path);
                    for (row, (old, new)) in pairs.iter().enumerate() {
                        assert_ne!(new.rare, RARE, "{path} row {row}: Kitakami's own shiny setting leaked through");
                        if new.species != old.species {
                            assert_legal_boss(&learn, &path, row, old, new);
                        }
                    }
                    let total: u32 = pairs.iter().map(|(_, new)| learn.bst(new.species)).sum();
                    averages.push(total as f64 / pairs.len() as f64);
                }
                println!("OK {seed} {region}: average BST by tier {:?}", averages.iter().map(|a| a.round() as u32).collect::<Vec<_>>());
                for pair in averages.windows(2) {
                    assert!(pair[0] < pair[1], "{region}: tier averages not increasing: {averages:?}");
                }
            }
        }
    }

    #[test]
    fn raids_shiny_and_tera_without_new_species() {
        let Some(game) = scarlet("raids shiny tera") else { return };
        let s = settings(&[
            number("raids.paldea.shiny", 100),
            text("raids.paldea.tera", "fixed"),
            text("raids.kitakami.tera", "own"),
            number("raids.blueberry.shiny", 50),
        ]);
        let mut first_gems = None;
        for seed in ["raids-tera-1", "raids-tera-2"] {
            let out = run_twice(&game, &s, seed);
            let mut gems = Vec::new();
            let mut blueberry_shiny = 0;
            let mut blueberry_rows = 0;
            for (region, prefix) in REGIONS {
                for tier in 1..=6 {
                    let path = boss_table(prefix, tier);
                    for (row, (old, new)) in compare(&game, &out, &path).into_iter().enumerate() {
                        assert_eq!((new.species, &new.moves), (old.species, &old.moves), "{path} row {row}: boss changed");
                        match region {
                            "paldea" => {
                                assert_eq!(new.rare, RARE, "{path} row {row}: not forced shiny");
                                assert!((2..=19).contains(&new.gem), "{path} row {row}: gem {}", new.gem);
                                gems.push(new.gem);
                            }
                            "kitakami" => {
                                assert_eq!(new.gem, 0, "{path} row {row}: not the boss's own type");
                                assert_eq!(new.rare, old.rare, "{path} row {row}: shiny changed");
                            }
                            _ => {
                                assert_eq!(new.gem, old.gem, "{path} row {row}: Tera changed");
                                blueberry_rows += 1;
                                blueberry_shiny += usize::from(new.rare == RARE);
                            }
                        }
                    }
                }
            }
            let distinct: BTreeSet<i64> = gems.iter().copied().collect();
            assert_eq!(distinct.len(), 18, "fixed Tera types should cover all 18 types over 454 bosses");
            assert!(
                (blueberry_rows / 4..=blueberry_rows * 3 / 4).contains(&blueberry_shiny),
                "50% shiny gave {blueberry_shiny} of {blueberry_rows}"
            );
            println!(
                "OK {seed}: Paldea all shiny with {} Tera types, Kitakami own types, Blueberry {blueberry_shiny}/{blueberry_rows} shiny",
                distinct.len()
            );
            match &first_gems {
                None => first_gems = Some(gems),
                Some(prev) => assert_ne!(prev, &gems, "different seeds gave the same Tera types"),
            }
        }
    }

    #[test]
    fn raids_tera_locked_bosses_get_their_required_tera() {
        let Some(game) = scarlet("raids tera locked") else { return };
        let only_locked = PoolFilter {
            generations: [false; 9],
            allowed: vec![1017, 1024],
            ..PoolFilter::everything()
        };
        let s = settings(&[
            on("pool.allow_tera_locked"),
            ("raids.pool", OptValue::Pool(only_locked)),
            on("raids.paldea.bosses"),
            text("raids.paldea.tera", "fixed"),
        ]);
        for seed in ["raids-locked-1", "raids-locked-2"] {
            let out = run_twice(&game, &s, seed);
            let learn = Learnsets::new(&game, &out);
            let schema = load_table(&game, &boss_table("", 1)).unwrap().schema().clone();
            let gem = |name: &str| schema.enum_value("GemType", name).unwrap();
            let masks = [gem("KUSA"), gem("MIZU"), gem("HONOO"), gem("IWA")];
            let mut seen = BTreeMap::new();
            for tier in 1..=6 {
                let path = boss_table("", tier);
                for (row, (old, new)) in compare(&game, &out, &path).iter().enumerate() {
                    let info = learn.dex.get(new.species).unwrap();
                    let expected = match info.national {
                        1017 => masks[usize::from(new.species.form) % 4],
                        1024 => gem("NIJI"),
                        n => panic!("{path} row {row}: #{n} is outside the pool"),
                    };
                    assert_eq!(new.gem, expected, "{path} row {row}: {:?} has the wrong Tera type", new.species);
                    assert_legal_boss(&learn, &path, row, old, new);
                    *seen.entry((info.national, new.species.form)).or_insert(0) += 1;
                }
            }
            println!("OK {seed}: Ogerpon and Terapagos bosses by (dex, form): {seen:?}");
        }
    }

    #[test]
    fn raids_never_pick_bosses_that_flee_at_half_hp() {
        let Some(game) = scarlet("raids fleeing") else { return };
        let fleeing = [193, 194];
        let s = settings(&[on("pokemon.abilities"), on("raids.paldea.bosses"), on("raids.paldea_for_all")]);
        for seed in ["raids-flee-1", "raids-flee-2"] {
            let out = run(&game, &s, seed);
            let learn = Learnsets::new(&game, &out);
            let flees = |p: Pokemon| {
                let entry = learn.personal.tables("entry").nth(learn.dex.get(p).unwrap().row).unwrap();
                ["ability_1", "ability_2", "ability_hidden"].iter().any(|f| fleeing.contains(&int(entry, f)))
            };
            let at_risk = learn.dex.playable().filter(|f| flees(f.id)).count();
            assert!(at_risk > 0, "{seed}: no species rolled a fleeing ability, so this test proves nothing");
            let mut bosses_seen = 0;
            for (_, prefix) in REGIONS {
                for tier in 1..=6 {
                    for (row, (_, new)) in compare(&game, &out, &boss_table(prefix, tier)).iter().enumerate() {
                        assert!(!flees(new.species), "{prefix}{tier} row {row}: {:?} can flee", new.species);
                        bosses_seen += 1;
                    }
                }
            }
            println!("OK {seed}: {at_risk} species can flee, none of {bosses_seen} bosses can");
        }
    }

    struct Entry {
        category: i64,
        item: i64,
        rest: Vec<i64>,
    }

    fn reward_entries(t: &TableValue) -> Vec<(u64, String, Entry)> {
        let mut out = Vec::new();
        for row in t.tables("values") {
            let name = row.uint("table_name").unwrap();
            for def in &row.def().fields {
                let Some(e) = row.table(&def.name) else { continue };
                let rest = e
                    .def()
                    .fields
                    .iter()
                    .filter(|f| !["category", "itemID"].contains(&f.name.as_str()))
                    .map(|f| e.int(&f.name).unwrap_or_else(|_| i64::from(e.bool(&f.name).unwrap())))
                    .collect();
                out.push((
                    name,
                    def.name.clone(),
                    Entry {
                        category: int(e, "category"),
                        item: int(e, "itemID"),
                        rest,
                    },
                ));
            }
        }
        out
    }

    #[test]
    fn raids_rewards_keep_their_kind_and_leave_materials_alone() {
        let Some(game) = scarlet("raids rewards") else { return };
        let s = settings(&[on("raids.rewards.fixed"), on("raids.rewards.lottery")]);
        let items: BTreeMap<i64, TableValue> = load_table(&game, ITEMS)
            .unwrap()
            .tables("values")
            .map(|r| (int(r, "Id"), r.clone()))
            .collect();
        let kind = |id: i64| {
            let r = &items[&id];
            (int(r, "ItemType"), int(r, "FieldPocket"), int(r, "ItemGroup"), int(r, "WorkLevel") != 0)
        };
        let unlinked = 0xafc4_d6c0_5abe_f19e_u64;
        let names = Names::load(&game, Language::English).unwrap();
        let mut first = None;
        for seed in ["raids-rewards-1", "raids-rewards-2"] {
            let out = run_twice(&game, &s, seed);
            let mut examples = Vec::new();
            for path in [FIXED, LOTTERY] {
                let before = reward_entries(&load_table(&game, path).unwrap());
                let after = reward_entries(&output_table(&game, &out, path));
                assert_eq!(before.len(), after.len());
                let mut swaps = 0;
                for ((name, slot, old), (_, _, new)) in before.iter().zip(&after) {
                    let what = format!("{path} {name:016x} {slot}");
                    assert_eq!((new.category, &new.rest), (old.category, &old.rest), "{what}: more than the item changed");
                    if old.category != 0 || old.item == 0 || *name == unlinked {
                        assert_eq!(new.item, old.item, "{what}: material, shard, empty or unused slot changed");
                        continue;
                    }
                    assert!(items.contains_key(&new.item), "{what}: item {} does not exist", new.item);
                    assert_eq!(kind(new.item), kind(old.item), "{what}: {} -> {} changes kind", old.item, new.item);
                    if new.item != old.item {
                        swaps += 1;
                        if examples.len() < 6 && swaps % 9 == 1 {
                            examples.push(format!("{} -> {}", names.item(old.item as u16), names.item(new.item as u16)));
                        }
                    }
                }
                assert!(swaps > 20, "{path}: only {swaps} rewards changed");
                println!("OK {seed} {path}: {swaps} item rewards changed");
            }
            assert_eq!(table_rows(&out, "Raid rewards").len(), {
                let count = |p: &str| {
                    reward_entries(&load_table(&game, p).unwrap())
                        .iter()
                        .zip(reward_entries(&output_table(&game, &out, p)))
                        .filter(|((_, _, a), (_, _, b))| a.item != b.item)
                        .count()
                };
                count(FIXED) + count(LOTTERY)
            });
            println!("   e.g. {examples:?}");
            let bytes = (out.files[FIXED].clone(), out.files[LOTTERY].clone());
            match &first {
                None => first = Some(bytes),
                Some(prev) => assert_ne!(prev, &bytes, "different seeds gave the same rewards"),
            }
        }
    }
}
