use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::load_table;
use svr_core::data::{facts, Dex, TableStore};
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::rando::options::{Kind, Req};
use svr_core::rando::{self, Seed, Settings};
use svr_core::text::{GameText, Language};

fn scarlet(test: &str) -> Option<GameFiles> {
    let (Some(keys), Some(games)) = (std::env::var_os("SVR_TEST_KEYS"), std::env::var_os("SVR_TEST_GAMES")) else {
        println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES");
        return None;
    };
    let games = PathBuf::from(games);
    let paths = GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES");
    Some(GameFiles::open(&paths, &Keys::load(Path::new(&keys)).unwrap()).unwrap())
}

#[test]
fn dex_matches_the_measured_species_pool() {
    let Some(game) = scarlet("dex") else { return };
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).unwrap();
    let species: BTreeSet<u16> = dex.playable().map(|f| f.national).collect();
    println!("OK playable forms {}, distinct species {}", dex.playable().count(), species.len());
    assert_eq!(species.len(), 733);
    assert!(species.contains(&25) && species.contains(&249) && !species.contains(&494));
}

#[test]
fn classification_names_match_the_game_text() {
    let Some(game) = scarlet("facts") else { return };
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).unwrap();
    let names = GameText::load(&game, Language::English, "monsname").unwrap();
    let lists = [
        ("mythical", facts::MYTHICAL),
        ("legendary", facts::LEGENDARY),
        ("sub-legendary", facts::SUB_LEGENDARY),
        ("ultra beast", facts::ULTRA_BEAST),
        ("paradox", facts::PARADOX),
        ("never random", facts::NEVER_RANDOM),
        ("tera locked", facts::TERA_LOCKED),
        ("object only", facts::OBJECT_ONLY),
    ];
    let mut checked = 0;
    for (kind, list) in lists {
        for &(national, expected) in list {
            let form = dex
                .all()
                .iter()
                .find(|f| f.national == national && f.id.form == 0)
                .unwrap_or_else(|| panic!("{kind} {expected} ({national}) has no species row"));
            let name = names.line(usize::from(form.id.species)).unwrap_or_default();
            assert_eq!(name, expected, "{kind} #{national}");
            checked += 1;
        }
    }
    println!("OK {checked} classified species match the English names");

    let text = svr_core::data::Names::load(&game, Language::English).unwrap();
    let masks = facts::OGERPON_MASK_TYPES.map(|t| text.type_name(t));
    assert_eq!(masks, ["Grass", "Water", "Fire", "Rock"]);
}

#[test]
fn registry_is_consistent() {
    let registry = rando::registry();
    let ids: BTreeSet<&str> = registry.iter().map(|d| d.id).collect();
    assert_eq!(ids.len(), registry.len(), "duplicate option ids");
    for def in &registry {
        for req in &def.requires {
            let target = match req {
                Req::On { id } | Req::Off { id } | Req::Is { id, .. } | Req::Not { id, .. } => *id,
            };
            assert!(ids.contains(target), "{} requires unknown option {target}", def.id);
        }
        if let Kind::Choice { default, choices } = &def.kind {
            assert!(choices.iter().any(|c| c.value == *default), "{} default not among choices", def.id);
        }
    }
    println!("OK {} options, all requirements resolve", registry.len());
}

#[test]
fn defaults_randomize_end_to_end() {
    let Some(game) = scarlet("e2e") else { return };
    let settings = Settings::defaults(&rando::registry());
    let out = rando::randomize(&game, &settings, Seed("boot".into())).unwrap();
    println!("OK default run: {} files, warnings {:?}\n{}", out.files.len(), out.warnings, out.log.to_text());
    assert!(out.warnings.is_empty());
}

/// Every toggle on, and each choice and number rotated through its values across seeds, so one run
/// combines every section's work and the next tries the other choices.
#[test]
fn every_option_at_once_randomizes_and_validates() {
    use svr_core::rando::options::OptValue;

    let Some(game) = scarlet("everything") else { return };
    let registry = rando::registry();
    let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
    let object_only = |species: i64, form: i64| {
        let p = svr_core::data::Pokemon { species: species as u16, form: form as u16 };
        dex.get(p).is_some_and(|f| facts::OBJECT_ONLY.iter().any(|&(n, _)| n == f.national))
    };
    for round in 0..4usize {
        let mut settings = Settings::defaults(&registry);
        for (i, def) in registry.iter().enumerate() {
            let value = match &def.kind {
                Kind::Toggle { .. } => OptValue::Bool(true),
                Kind::Choice { choices, .. } => OptValue::Text(choices[(round + i) % choices.len()].value.to_owned()),
                Kind::Number { default, min, max, .. } => OptValue::Int([*max, *min, *default][round % 3]),
                _ => continue,
            };
            settings.set(def.id, value);
        }
        let seed = || Seed(format!("everything-{round}"));
        let out = rando::randomize(&game, &settings, seed()).unwrap_or_else(|e| panic!("round {round}: {e}"));
        if round == 0 {
            let again = rando::randomize(&game, &settings, seed()).unwrap();
            assert!(out.files == again.files, "the same seed gives the same files");
        }
        let eden = Path::new("C:/Users/Player/AppData/Roaming/eden/load/0100A3D008C5C000/SV Randomizer");
        svr_core::output::check_path_lengths(eden, out.files.keys().map(String::as_str)).unwrap();
        assert!(!out.files.keys().any(|p| p.contains("coin_symbol")), "Gimmighoul's own objects stay vanilla");
        for (path, bytes) in out.files.iter().filter(|(p, _)| p.contains("/encount/pokedata/")) {
            let table = svr_core::fb::parse(load_table(&game, path).unwrap().schema(), bytes).unwrap();
            for row in table.tables("values") {
                assert!(!object_only(row.int("devid").unwrap(), row.int("formno").unwrap()), "{path} spawns an object-only species");
            }
        }
        let fixed = "world/data/field/fixed_symbol/fixed_symbol_table/fixed_symbol_table_array.bin";
        if let Some(bytes) = out.files.get(fixed) {
            let table = svr_core::fb::parse(load_table(&game, fixed).unwrap().schema(), bytes).unwrap();
            let vanilla = load_table(&game, fixed).unwrap();
            for (row, before) in table.tables("values").zip(vanilla.tables("values")) {
                let poke = |r: &svr_core::fb::TableValue| r.table("pokeDataSymbol").map(|p| (p.int("devId").unwrap_or(0), p.int("formId").unwrap_or(0)));
                if poke(row) != poke(before) {
                    let (s, f) = poke(row).unwrap();
                    assert!(!object_only(s, f), "a static encounter became an object-only species");
                }
            }
        }
        println!("OK round {round}: {} files, warnings {:?}", out.files.len(), out.warnings);
    }
}

#[test]
fn validator_flags_unusable_references_and_passes_legal_ones() {
    use std::collections::BTreeMap;
    use svr_core::rando::validate;
    let Some(game) = scarlet("validator") else { return };
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).unwrap();
    let rules = validate::Rules::load(&mut store).unwrap();
    let missing = dex.all().iter().find(|f| f.national == 494).unwrap().id.species;
    let unusable_move = store
        .get(svr_core::data::store::MOVES)
        .unwrap()
        .tables("Table")
        .find(|m| !m.bool("CanUseMove").unwrap_or(true) && m.int("MoveID").unwrap_or(0) > 0)
        .map(|m| m.int("MoveID").unwrap())
        .unwrap();
    let path = "world/data/trainer/trdata/trdata_array.bin";
    let trainers = store.get_mut(path).unwrap();
    let row = trainers.tables_mut("values").nth(1).unwrap();
    let poke = row.table_mut("poke1").unwrap();
    poke.set_int("devId", i64::from(missing)).unwrap();
    poke.table_mut("waza1").unwrap().set_int("wazaId", unusable_move).unwrap();
    let problems = validate::check(&game, &dex, &rules, &store.changed(), &BTreeMap::new());
    println!("OK flagged: {problems:?}");
    assert_eq!(problems.len(), 2, "{problems:?}");

    let mut clean = TableStore::new(&game);
    let row = clean.get_mut(path).unwrap().tables_mut("values").nth(1).unwrap();
    row.table_mut("poke1").unwrap().set_int("devId", 25).unwrap();
    let problems = validate::check(&game, &dex, &rules, &clean.changed(), &BTreeMap::new());
    assert!(problems.is_empty(), "{problems:?}");
    println!("OK legal change passes");
}

#[test]
fn irregular_forms_exist_and_are_filtered() {
    let Some(game) = scarlet("irregular") else { return };
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).unwrap();
    let irregular: Vec<_> = dex
        .playable()
        .filter(|f| facts::is_irregular_form(f.national, f.id.form))
        .map(|f| (f.national, f.id.form))
        .collect();
    println!("OK playable irregular forms: {irregular:?}");
    assert!(!irregular.is_empty());
    let mut filter = svr_core::rando::PoolFilter::everything();
    let with = svr_core::rando::pool::build(&dex, &filter, true).len();
    filter.irregular_forms = true;
    let all = svr_core::rando::pool::build(&dex, &filter, true).len();
    assert_eq!(all - with, irregular.len());
    println!("OK pool without irregular forms {with}, with {all}");
}

#[test]
fn scene_pokemon_without_a_form_field_take_an_alternate_form() {
    use std::collections::BTreeMap;
    use svr_core::data::{scene, Pokemon};

    let Some(game) = scarlet("scene forms") else { return };
    let path = "world/scene/parts/event/event_scenario/main_scenario/common_0070_/common_0070_always_0.trsog";
    let raw = game.read(path).unwrap();
    let sprigatito = Pokemon { species: 906, form: 0 };
    let alolan_vulpix = Pokemon { species: 37, form: 1 };
    let before = scene::swap_species(&raw, &BTreeMap::new()).unwrap().found;
    let count = before[&sprigatito];

    let standard = scene::swap_species(&raw, &BTreeMap::from([(sprigatito, Pokemon { species: 37, form: 0 })])).unwrap();
    assert_eq!(standard.bytes.len(), raw.len(), "a standard form still patches in place");

    let patch = scene::swap_species(&raw, &BTreeMap::from([(sprigatito, alolan_vulpix)])).unwrap();
    assert_eq!(patch.changed, count);
    assert!(patch.bytes.len() > raw.len());
    let differing = raw.iter().zip(&patch.bytes).filter(|(a, b)| a != b).count();
    assert!(differing <= count * 6, "only species ushorts and payload offsets change in place, saw {differing}");

    let after = scene::swap_species(&patch.bytes, &BTreeMap::new()).unwrap().found;
    let mut expected = before.clone();
    expected.remove(&sprigatito);
    expected.insert(alolan_vulpix, count);
    assert_eq!(after, expected);

    let alolan_ninetales = Pokemon { species: 38, form: 1 };
    let again = scene::swap_species(&patch.bytes, &BTreeMap::from([(alolan_vulpix, alolan_ninetales)])).unwrap();
    assert_eq!(again.bytes.len(), patch.bytes.len(), "a copied payload now has a form field");
    assert_eq!(scene::swap_species(&again.bytes, &BTreeMap::new()).unwrap().found[&alolan_ninetales], count);
    println!("OK {count} Sprigatito objects became Alolan Vulpix, file grew by {}", patch.bytes.len() - raw.len());
}
