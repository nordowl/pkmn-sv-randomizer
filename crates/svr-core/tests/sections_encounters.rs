//! Real-game tests for the starter, gift, trade, wild and static sections. They run only with
//! SVR_TEST_KEYS and SVR_TEST_GAMES set, and check the randomizer's output bytes, not its log.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use svr_core::container::Keys;
use svr_core::data::store::load_table;
use svr_core::data::{facts, scene, Dex, Pokemon, TableStore};
use svr_core::fb::{self, Schema, TableValue};
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::rando::options::{Page, PoolFilter, SpeciesPick};
use svr_core::rando::{self, itempool, pool, OptValue, RandoOutput, Seed, Settings};

const GIFTS: &str = "world/data/event/event_add_pokemon/eventAddPokemon/eventAddPokemon_array.bin";
const TRADE_LIST: &str = "world/data/event/eventTradeList/eventTradeList_array.bin";
const TRADE_POKEMON: &str = "world/data/event/eventTradePokemon/eventTradePokemon_array.bin";
const FIXED: &str = "world/data/field/fixed_symbol/fixed_symbol_table/fixed_symbol_table_array.bin";
const WILD: [&str; 3] = [
    "world/data/encount/pokedata/pokedata/pokedata_array.bin",
    "world/data/encount/pokedata/pokedata_su1/pokedata_su1_array.bin",
    "world/data/encount/pokedata/pokedata_su2/pokedata_su2_array.bin",
];
const REGION_KEYS: [&str; 3] = ["paldea", "kitakami", "blueberry"];
const MAIN: &str = "world/scene/parts/event/event_scenario/main_scenario/";
const STARTER_LABELS: [&str; 3] = ["common_0065_kusa", "common_0065_hono", "common_0065_mizu"];
const ORIGINAL_STARTERS: [u16; 3] = [906, 909, 912];
const MOVE_FIELDS: [&str; 5] = ["land", "up_water", "underwater", "air1", "air2"];

static GAME: OnceLock<Option<GameFiles>> = OnceLock::new();
static DEX: OnceLock<Dex> = OnceLock::new();

fn scarlet(test: &str) -> Option<&'static GameFiles> {
    let game = GAME.get_or_init(|| {
        let keys = std::env::var_os("SVR_TEST_KEYS")?;
        let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES")?);
        let paths = GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES");
        Some(GameFiles::open(&paths, &Keys::load(Path::new(&keys)).unwrap()).unwrap())
    });
    if game.is_none() {
        println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES");
    }
    game.as_ref()
}

fn dex(game: &GameFiles) -> &'static Dex {
    DEX.get_or_init(|| Dex::load(&mut TableStore::new(game)).unwrap())
}

fn settings(values: &[(&str, OptValue)]) -> Settings {
    let mut s = Settings::defaults(&rando::registry());
    for (id, value) in values {
        assert!(s.values.contains_key(*id), "unknown option {id}");
        s.set(id, value.clone());
    }
    s
}

fn on(id: &str) -> (&str, OptValue) {
    (id, OptValue::Bool(true))
}

fn text<'a>(id: &'a str, value: &str) -> (&'a str, OptValue) {
    (id, OptValue::Text(value.to_owned()))
}

fn number(id: &str, value: i64) -> (&str, OptValue) {
    (id, OptValue::Int(value))
}

fn run(game: &GameFiles, s: &Settings, seed: &str) -> RandoOutput {
    rando::randomize(game, s, Seed(seed.into())).unwrap_or_else(|e| panic!("seed {seed} failed: {e}"))
}

/// The same seed gives identical bytes and another seed gives different ones.
fn assert_repeatable(game: &GameFiles, s: &Settings) -> [RandoOutput; 2] {
    let a = run(game, s, "1");
    let again = run(game, s, "1");
    assert!(a.files == again.files, "the same seed gave different output");
    let b = run(game, s, "2");
    assert!(!a.files.is_empty(), "nothing was written");
    assert!(a.files != b.files, "two seeds gave identical output");
    [a, b]
}

fn table(game: &GameFiles, out: &RandoOutput, path: &str) -> TableValue {
    match out.files.get(path) {
        Some(bytes) => {
            let stem = path.strip_suffix(".bin").unwrap();
            let schema = Schema::from_bfbs(&game.read(&format!("{stem}.bfbs")).unwrap()).unwrap();
            fb::parse(&schema, bytes).unwrap()
        }
        None => load_table(game, path).unwrap(),
    }
}

fn vanilla(game: &GameFiles, path: &str) -> TableValue {
    load_table(game, path).unwrap()
}

fn rows(t: &TableValue) -> Vec<&TableValue> {
    t.tables("values").collect()
}

fn poke_species(poke: &TableValue) -> Pokemon {
    Pokemon {
        species: poke.int("devId").unwrap() as u16,
        form: poke.int("formId").unwrap() as u16,
    }
}

fn wild_species(row: &TableValue) -> Pokemon {
    Pokemon {
        species: row.int("devid").unwrap() as u16,
        form: row.int("formno").unwrap() as u16,
    }
}

fn gift<'t>(t: &'t TableValue, label: &str) -> &'t TableValue {
    t.tables("values")
        .find(|r| r.string("label") == Some(label))
        .unwrap_or_else(|| panic!("no gift {label}"))
        .table("pokeData")
        .unwrap()
}

fn enum_name(t: &TableValue, enum_name: &str, field: &str) -> String {
    t.schema().enum_name(enum_name, t.int(field).unwrap()).unwrap_or("?").to_owned()
}

fn internal(dex: &Dex, national: u16, form: u16) -> Pokemon {
    dex.playable().find(|f| f.national == national && f.id.form == form).unwrap().id
}

fn scene_species(bytes: &[u8]) -> BTreeSet<Pokemon> {
    scene::swap_species(bytes, &BTreeMap::new()).unwrap().found.into_keys().collect()
}

fn starter_scenes(reveal: &str) -> BTreeSet<String> {
    let mut stems = vec![
        format!("{MAIN}common_0088_/common_0088_always"),
        format!("{MAIN}common_0090_/common_0090_main"),
    ];
    if reveal != "after" {
        stems.push(format!("{MAIN}common_0070_/common_0070_always"));
    }
    let mut out: BTreeSet<String> = stems.iter().flat_map(|s| [format!("{s}_0.trsog"), format!("{s}_1.trsog")]).collect();
    if reveal == "clavell" {
        for stem in ["common_0060_/common_0060_always", "common_0060_/common_0060_main"] {
            out.insert(format!("{MAIN}{stem}_0.trsog"));
            out.insert(format!("{MAIN}{stem}_1.trsog"));
        }
        out.insert("world/scene/parts/demo/ev/d030_/d030_0.trscn".into());
        out.insert("world/scene/parts/demo/ev/d030_/d030_1.trscn".into());
    }
    out
}

fn scene_files(out: &RandoOutput, prefix: &str) -> BTreeSet<String> {
    out.files.keys().filter(|p| p.starts_with(prefix) && !p.ends_with(".bin")).cloned().collect()
}

fn starters_of(game: &GameFiles, out: &RandoOutput) -> [Pokemon; 3] {
    let t = table(game, out, GIFTS);
    STARTER_LABELS.map(|label| poke_species(gift(&t, label)))
}

fn assert_ride_legends_untouched(game: &GameFiles, out: &RandoOutput) {
    let before = vanilla(game, GIFTS);
    let after = table(game, out, GIFTS);
    for label in ["gameclear_AIGUANA", "gameclear_BIGUANA"] {
        assert!(gift(&before, label) == gift(&after, label), "{label} changed");
    }
}

#[test]
fn random_starters_are_distinct_and_set_the_cutscenes() {
    let Some(game) = scarlet("random starters") else { return };
    let dex = dex(game);
    for mode in ["random", "basics"] {
        let s = settings(&[text("starters.mode", mode)]);
        for out in assert_repeatable(game, &s) {
            let starters = starters_of(game, &out);
            let species: BTreeSet<u16> = starters.iter().map(|p| p.species).collect();
            assert_eq!(species.len(), 3, "{mode}: {starters:?} repeat a species");
            for p in starters {
                assert!(dex.is_playable(p), "{mode}: {p:?} is not playable");
                if mode == "basics" {
                    let info = dex.get(p).unwrap();
                    assert!(dex.pre_evolution(p).is_none(), "{p:?} is not a first stage");
                    let twice = info.evolutions.iter().any(|e| {
                        dex.is_playable(e.into) && dex.get(e.into).unwrap().evolutions.iter().any(|e2| dex.is_playable(e2.into))
                    });
                    assert!(twice, "{p:?} does not evolve twice");
                }
            }
            assert_eq!(scene_files(&out, "world/scene/"), starter_scenes("nemona"));
            let shown: BTreeSet<Pokemon> = starters.iter().copied().collect();
            for path in starter_scenes("nemona") {
                assert_eq!(scene_species(&out.files[&path]), shown, "{path}");
            }
            assert_ride_legends_untouched(game, &out);
            println!("OK {mode}: {starters:?}");
        }
    }
}

#[test]
fn each_reveal_patches_exactly_its_scenes() {
    let Some(game) = scarlet("reveal") else { return };
    for reveal in ["clavell", "nemona", "after"] {
        let s = settings(&[text("starters.mode", "random"), text("starters.reveal", reveal)]);
        let out = run(game, &s, "7");
        let expected = starter_scenes(reveal);
        assert_eq!(scene_files(&out, "world/scene/"), expected, "{reveal}");
        let starters = starters_of(game, &out);
        for path in &expected {
            let shown = scene_species(&out.files[path]);
            for p in starters {
                assert!(shown.contains(&p), "{path} lacks {p:?}");
            }
            for original in ORIGINAL_STARTERS {
                let picked = starters.iter().any(|p| p.species == original);
                assert!(picked || !shown.iter().any(|p| p.species == original), "{path} still shows {original}");
            }
        }
        println!("OK reveal {reveal}: {} scene files", expected.len());
    }
}

#[test]
fn custom_starters_land_in_their_slots() {
    let Some(game) = scarlet("custom starters") else { return };
    let dex = dex(game);
    let pick = |national, form| OptValue::Species(SpeciesPick { national: Some(national), form });
    let s = settings(&[
        text("starters.mode", "custom"),
        ("starters.grass", pick(4, 0)),
        ("starters.fire", pick(7, 0)),
        ("starters.water", pick(37, 1)),
        text("starters.grass.gender", "female"),
        text("starters.fire.ball", "master"),
        on("starters.water.shiny"),
        on("starters.tera"),
        on("starters.items"),
    ]);
    let [out, _] = assert_repeatable(game, &s);
    let expected = [internal(dex, 4, 0), internal(dex, 7, 0), internal(dex, 37, 1)];
    assert_eq!(starters_of(game, &out), expected);
    let t = table(game, &out, GIFTS);
    let [grass, fire, water] = STARTER_LABELS.map(|l| gift(&t, l));
    assert_eq!(enum_name(grass, "SexType", "sex"), "FEMALE");
    assert_eq!(enum_name(fire, "BallType", "ballId"), "MASUTAABOORU");
    assert_eq!(enum_name(water, "RareType", "rareType"), "RARE");
    for poke in [grass, fire, water] {
        assert!((2..=19).contains(&poke.int("gemType").unwrap()), "no random Tera type");
        assert_ne!(poke.int("item").unwrap(), 0, "no held item");
    }
    let shown: BTreeSet<Pokemon> = expected.into_iter().collect();
    for path in starter_scenes("nemona") {
        assert_eq!(scene_species(&out.files[&path]), shown, "{path} should show Alolan Vulpix");
    }
    println!("OK custom starters {expected:?}");
}

#[test]
fn starter_extras_apply_without_new_species() {
    let Some(game) = scarlet("starter extras") else { return };
    let s = settings(&[text("starters.shiny", "all"), on("starters.tera")]);
    let out = run(game, &s, "1");
    let t = table(game, &out, GIFTS);
    assert_eq!(starters_of(game, &out).map(|p| p.species), ORIGINAL_STARTERS);
    for label in STARTER_LABELS {
        assert_eq!(enum_name(gift(&t, label), "RareType", "rareType"), "RARE");
        assert!((2..=19).contains(&gift(&t, label).int("gemType").unwrap()), "{label} has no random Tera type");
    }
    assert!(scene_files(&out, "world/scene/").is_empty(), "no cutscene should change");
    println!("OK shiny vanilla starters, no scenes patched");
}

fn custom(picks: [(u16, u16); 3], extra: &[(&str, OptValue)]) -> Settings {
    let pick = |(national, form)| OptValue::Species(SpeciesPick { national: Some(national), form });
    let mut values = vec![
        text("starters.mode", "custom"),
        ("starters.grass", pick(picks[0])),
        ("starters.fire", pick(picks[1])),
        ("starters.water", pick(picks[2])),
    ];
    values.extend_from_slice(extra);
    settings(&values)
}

#[test]
fn custom_genders_follow_what_the_species_allows() {
    let Some(game) = scarlet("custom genders") else { return };
    let hatterene_male = (858, 0);
    let magnemite_female = (81, 0);
    let salazzle_female = (758, 0);
    let s = custom(
        [hatterene_male, magnemite_female, salazzle_female],
        &[
            text("starters.grass.gender", "male"),
            text("starters.fire.gender", "female"),
            text("starters.water.gender", "female"),
        ],
    );
    let out = run(game, &s, "1");
    let t = table(game, &out, GIFTS);
    let genders = STARTER_LABELS.map(|l| enum_name(gift(&t, l), "SexType", "sex"));
    assert_eq!(genders, ["DEFAULT", "DEFAULT", "FEMALE"]);
    println!("OK impossible genders fall back to the game's roll");
}

#[test]
fn custom_starters_refuse_bad_setups_without_panicking() {
    let Some(game) = scarlet("custom refusals") else { return };
    let mut two = PoolFilter::everything();
    two.generations = [false; 9];
    two.allowed = vec![1, 4];
    let bulbasaur = OptValue::Species(SpeciesPick { national: Some(1), form: 0 });
    let tight = settings(&[
        text("starters.mode", "custom"),
        ("starters.grass", bulbasaur),
        ("starters.pool", OptValue::Pool(two.clone())),
    ]);
    let err = rando::randomize(game, &tight, Seed("1".into())).err().expect("two random slots cannot share one species");
    assert!(err.to_string().contains("species left"), "{err}");
    let random_two = settings(&[text("starters.mode", "random"), ("starters.pool", OptValue::Pool(two.clone()))]);
    let err = rando::randomize(game, &random_two, Seed("1".into())).err().expect("three starters need three species");
    assert!(err.to_string().contains("species left"), "{err}");
    let mut empty = two;
    empty.allowed.clear();
    let fixed = custom([(25, 0), (4, 0), (7, 0)], &[("starters.pool", OptValue::Pool(empty))]);
    run(game, &fixed, "1");
    let koraidon = custom([(1007, 0), (4, 0), (7, 0)], &[]);
    let err = rando::randomize(game, &koraidon, Seed("1".into())).err().expect("Koraidon must be refused");
    assert!(err.to_string().contains("cannot be placed"), "{err}");
    println!("OK tight pools and Koraidon are refused with plain errors");
}

#[test]
fn gifts_and_eggs_change_but_ride_legends_stay() {
    let Some(game) = scarlet("gifts") else { return };
    let dex = dex(game);
    let s = settings(&[on("gifts.random"), on("gifts.eggs"), number("gifts.shiny", 100), on("gifts.tera")]);
    let before = vanilla(game, GIFTS);
    for out in assert_repeatable(game, &s) {
        let after = table(game, &out, GIFTS);
        let gifts = ["add_0195", "add_0193", "sub060_gnyasu", "s1_main_0160_oniballoon_gonbe", "s1_side02_0060_hisuiGardie"];
        let eggs = ["s1_sub_001_egg_000", "s1_sub_001_egg_001", "s1_sub_001_egg_002"];
        let mut seen = [BTreeSet::new(), BTreeSet::new()];
        for (label, group) in gifts.iter().map(|l| (l, 0)).chain(eggs.iter().map(|l| (l, 1))) {
            let (was, now) = (gift(&before, label), gift(&after, label));
            let p = poke_species(now);
            assert_ne!(poke_species(was), p, "{label} kept its species");
            assert!(dex.is_playable(p));
            assert!(seen[group].insert(p.species), "{label} repeats a species");
            assert_eq!(enum_name(now, "RareType", "rareType"), "RARE", "{label}");
            assert!((2..=19).contains(&now.int("gemType").unwrap()), "{label} has no random Tera type");
            assert_eq!(now.int("level").unwrap(), was.int("level").unwrap());
        }
        for label in eggs {
            assert!(dex.pre_evolution(poke_species(gift(&after, label))).is_none(), "{label} is not a first stage");
        }
        for label in STARTER_LABELS {
            assert!(gift(&before, label) == gift(&after, label), "{label} changed");
        }
        assert_ride_legends_untouched(game, &out);
        let growlithe = poke_species(gift(&after, "s1_side02_0060_hisuiGardie"));
        let scenes = scene_files(&out, "world/scene/parts/event/event_scenario/sub_scenario/");
        assert_eq!(scenes.len(), 12);
        for path in scenes {
            assert_eq!(scene_species(&out.files[&path]), BTreeSet::from([growlithe]), "{path}");
        }
        println!("OK gifts, Growlithe became {growlithe:?}");
    }
}

fn trade_rows(t: &TableValue) -> BTreeMap<String, &TableValue> {
    t.tables("values").map(|r| (r.string("label").unwrap().to_owned(), r.table("pokeData").unwrap())).collect()
}

/// Species the player can catch in Paldea in Scarlet without a story flag, with the lowest level.
fn catchable(paldea: &TableValue) -> BTreeMap<Pokemon, i64> {
    let mut out = BTreeMap::new();
    for row in rows(paldea) {
        let scarlet = row.table("versiontable").is_some_and(|v| v.bool("A").unwrap());
        if scarlet && row.string("flagName").unwrap_or_default().is_empty() {
            let min = row.int("minlevel").unwrap();
            let entry = out.entry(wild_species(row)).or_insert(min);
            *entry = (*entry).min(min);
        }
    }
    out
}

#[test]
fn trades_randomize_both_sides() {
    let Some(game) = scarlet("trades") else { return };
    let s = settings(&[
        text("gifts.trades", "both"),
        on("gifts.guests"),
        on("gifts.trade_nicknames"),
        on("gifts.trade_trainers"),
        on("gifts.trade_ivs"),
        on("gifts.trade_items"),
        text("wild.paldea.mode", "swap"),
    ]);
    let before_list = vanilla(game, TRADE_LIST);
    let before = vanilla(game, TRADE_POKEMON);
    let before_rows = trade_rows(&before);
    for out in assert_repeatable(game, &s) {
        let after = table(game, &out, TRADE_POKEMON);
        let after_rows = trade_rows(&after);
        let story: Vec<String> = rows(&before_list).iter().map(|r| r.string("receivePoke").unwrap().to_owned()).collect();
        let guests: Vec<String> = before_rows.keys().filter(|l| l.starts_with("trade_gest_")).cloned().collect();
        assert_eq!((story.len(), guests.len()), (3, 30));
        let ivs = |poke: &TableValue| -> Vec<i64> {
            let t = poke.table("talentValue").unwrap();
            ["hp", "atk", "def", "spAtk", "spDef", "agi"].iter().map(|f| t.int(f).unwrap()).collect()
        };
        let mut new_ivs = 0;
        for group in [&story, &guests] {
            for field in ["nicknameLabel", "parentNameLabel"] {
                let label_of = |rows: &BTreeMap<String, &TableValue>, l: &String| rows[l].uint(field).unwrap();
                let was: BTreeSet<u64> = group.iter().map(|l| label_of(&before_rows, l)).collect();
                let now: BTreeSet<u64> = group.iter().map(|l| label_of(&after_rows, l)).collect();
                assert_eq!(was, now, "{field} must stay within its group");
                for label in group {
                    assert_ne!(label_of(&before_rows, label), label_of(&after_rows, label), "{label} kept its {field}");
                }
            }
            for label in group {
                let (b, a) = (before_rows[label], after_rows[label]);
                assert_ne!(poke_species(b), poke_species(a), "{label} kept its species");
                assert_ne!(b.int("trainerId").unwrap(), a.int("trainerId").unwrap(), "{label} kept its trainer ID");
                assert!(ivs(a).iter().all(|v| (0..=31).contains(v)));
                new_ivs += usize::from(ivs(a) != ivs(b));
                assert_ne!(a.int("item").unwrap(), 0, "{label} holds nothing");
            }
        }
        assert!(new_ivs > 30, "only {new_ivs} trades got new IVs");
        let wild = catchable(&table(game, &out, WILD[0]));
        for row in rows(&table(game, &out, TRADE_LIST)) {
            let asked = Pokemon {
                species: row.int("sendPoke_devId").unwrap() as u16,
                form: row.int("sendPoke_formId").unwrap() as u16,
            };
            let level = after_rows[row.string("receivePoke").unwrap()].int("level").unwrap();
            let min = wild.get(&asked).unwrap_or_else(|| panic!("{asked:?} cannot be caught in Paldea"));
            assert!(*min <= level, "{asked:?} is only found from level {min}, after the trade at {level}");
        }
        println!("OK trades");
    }
}

/// Where the vanilla tables let each species spawn, as a bit set over MOVE_FIELDS.
fn vanilla_moves(game: &GameFiles) -> BTreeMap<Pokemon, u8> {
    let mut out: BTreeMap<Pokemon, u8> = BTreeMap::new();
    for path in WILD {
        for row in rows(&vanilla(game, path)) {
            *out.entry(wild_species(row)).or_default() |= moves(row);
        }
    }
    out
}

fn moves(row: &TableValue) -> u8 {
    let e = row.struct_value("enabletable").unwrap();
    MOVE_FIELDS.iter().enumerate().filter(|(_, f)| e.bool(f).unwrap()).map(|(i, _)| 1 << i).sum()
}

fn assert_movement(game: &GameFiles, out: &RandoOutput) {
    let can = vanilla_moves(game);
    for path in WILD {
        for row in rows(&table(game, out, path)) {
            let p = wild_species(row);
            let spawn = moves(row);
            let allowed = can.get(&p).copied().filter(|&m| m != 0).unwrap_or(1);
            assert_ne!(spawn, 0, "{path}: {p:?} spawns nowhere");
            assert_eq!(spawn & !allowed, 0, "{path}: {p:?} spawns in modes {spawn:#b}, but only {allowed:#b} work");
        }
    }
}

fn follower(r: &TableValue) -> Pokemon {
    Pokemon {
        species: r.int("bandpoke").unwrap() as u16,
        form: r.int("bandFormno").unwrap() as u16,
    }
}

/// Herds in every changed table follow their new leader: its previous stage, or the leader itself.
fn assert_herds_follow_leaders(game: &GameFiles, out: &RandoOutput) {
    let dex = dex(game);
    let mut herds = 0;
    for path in WILD.into_iter().filter(|p| out.files.contains_key(*p)) {
        for row in rows(&table(game, out, path)) {
            let f = follower(row);
            if f.species == 0 {
                continue;
            }
            let leader = wild_species(row);
            assert!(f == leader || Some(f) == dex.pre_evolution(leader), "{path}: {leader:?} leads {f:?}");
            assert_eq!(row.int("bandSex").unwrap(), 0, "{path}: {f:?} kept a fixed gender");
            herds += 1;
        }
    }
    assert!(herds > 30, "only {herds} herds were checked");
}

fn wild_settings(mode: &str, extra: &[(&str, OptValue)]) -> Settings {
    let mut values = vec![on("wild.same"), text("wild.paldea.mode", mode)];
    values.extend_from_slice(extra);
    settings(&values)
}

#[test]
fn global_mapping_is_consistent_across_regions() {
    let Some(game) = scarlet("global wild") else { return };
    let s = wild_settings("global", &[on("wild.paldea.similar_strength")]);
    for out in assert_repeatable(game, &s) {
        let mut map: BTreeMap<Pokemon, Pokemon> = BTreeMap::new();
        let mut check = |was: Pokemon, now: Pokemon| {
            let first = *map.entry(was).or_insert(now);
            assert_eq!(first, now, "{was:?} became both {first:?} and {now:?}");
        };
        for path in WILD {
            let (before, after) = (vanilla(game, path), table(game, &out, path));
            assert_eq!(rows(&before).len(), rows(&after).len());
            for (b, a) in rows(&before).into_iter().zip(rows(&after)) {
                check(wild_species(b), wild_species(a));
                if follower(b).species != 0 {
                    check(follower(b), follower(a));
                    if follower(b) != follower(a) {
                        assert_eq!(a.int("bandSex").unwrap(), 0, "{path}: a new follower kept a fixed gender");
                    }
                }
                assert_eq!(b.int("minlevel").unwrap(), a.int("minlevel").unwrap());
                assert_eq!(b.string("area"), a.string("area"));
            }
        }
        let replacements: BTreeSet<Pokemon> = map.values().copied().collect();
        assert_eq!(replacements.len(), map.len(), "the map is not one-to-one");
        assert_movement(game, &out);
        println!("OK global map of {} species", map.len());
    }
}

#[test]
fn row_swap_and_shuffle_keep_movement_rules() {
    let Some(game) = scarlet("swap and shuffle") else { return };
    let dex = dex(game);
    let s = wild_settings("swap", &[]);
    for out in assert_repeatable(game, &s) {
        for path in WILD {
            let (before, after) = (vanilla(game, path), table(game, &out, path));
            assert_eq!(rows(&before).len(), rows(&after).len());
            for (b, a) in rows(&before).into_iter().zip(rows(&after)) {
                assert!(dex.is_playable(wild_species(a)));
                assert_eq!(b.int("maxlevel").unwrap(), a.int("maxlevel").unwrap());
                assert_eq!(b.int("biome1").unwrap(), a.int("biome1").unwrap());
            }
        }
        assert_movement(game, &out);
        assert_herds_follow_leaders(game, &out);
    }
    let mut gen1 = PoolFilter::everything();
    gen1.generations = [true, false, false, false, false, false, false, false, false];
    let cases = [
        ("full pool", PoolFilter::everything(), vec![]),
        ("Gen 1 pool", gen1.clone(), vec![("wild.paldea.pool", OptValue::Pool(gen1))]),
        ("similar strength", PoolFilter::everything(), vec![on("wild.paldea.similar_strength")]),
    ];
    for (case, pool_filter, extra) in cases {
        let s = wild_settings("shuffle", &extra);
        let object_only = |p: &Pokemon| facts::OBJECT_ONLY.iter().any(|&(n, _)| dex.get(*p).is_some_and(|f| f.national == n));
        let pool: Vec<Pokemon> = pool::build(dex, &pool_filter, false).into_iter().filter(|p| !object_only(p)).collect();
        for out in assert_repeatable(game, &s) {
            for path in WILD {
                let before = vanilla(game, path);
                let after = table(game, &out, path);
                let placed: BTreeSet<Pokemon> = rows(&after).into_iter().map(wild_species).collect();
                let missing = pool.iter().filter(|p| !placed.contains(p)).count();
                assert_eq!(missing, 0, "{path}: {missing} pool species never spawn");
                assert_eq!(rows(&after).len(), rows(&before).len().max(pool.len()), "{path}");
                let places = |t: &TableValue| -> BTreeSet<(String, String, i64)> {
                    rows(t)
                        .into_iter()
                        .map(|r| {
                            let s = |f: &str| r.string(f).unwrap_or_default().to_owned();
                            (s("area"), s("locationName"), r.int("biome1").unwrap())
                        })
                        .collect()
                };
                assert_eq!(places(&before), places(&after), "{path}: some places lost their spawns");
            }
            assert_movement(game, &out);
            assert_herds_follow_leaders(game, &out);
        }
        println!("OK shuffle with {case}: {} species, every place keeps spawns", pool.len());
    }
}

fn split_places(row: &TableValue) -> Vec<(String, String)> {
    let area = row.string("area").unwrap_or_default().to_owned();
    let location = row.string("locationName").unwrap_or_default().to_owned();
    let tokens = |s: &str| s.split(',').filter(|t| !t.is_empty()).map(str::to_owned).collect::<Vec<_>>();
    match (tokens(&area), tokens(&location)) {
        (a, l) if a.len() > 1 && l.is_empty() => a.into_iter().map(|t| (t, String::new())).collect(),
        (a, l) if a.is_empty() && l.len() > 1 => l.into_iter().map(|t| (String::new(), t)).collect(),
        _ => vec![(area, location)],
    }
}

#[test]
fn area_mapping_is_consistent_per_area_and_themed() {
    let Some(game) = scarlet("area wild") else { return };
    let dex = dex(game);
    let s = wild_settings("area", &[on("wild.paldea.type_themes")]);
    let walks = |p: &Pokemon, can: &BTreeMap<Pokemon, u8>| can.get(p).is_none_or(|&m| m & 1 != 0);
    let can = vanilla_moves(game);
    for out in assert_repeatable(game, &s) {
        let mut themed = 0;
        for path in WILD {
            let before = vanilla(game, path);
            let after = table(game, &out, path);
            let expected: Vec<(Pokemon, (String, String))> = rows(&before)
                .into_iter()
                .flat_map(|r| split_places(r).into_iter().map(move |place| (wild_species(r), place)))
                .collect();
            let after = rows(&after);
            assert_eq!(expected.len(), after.len(), "{path}: rows were not split per area");
            let mut map: BTreeMap<(&(String, String), Pokemon), Pokemon> = BTreeMap::new();
            let mut types: BTreeMap<&(String, String), BTreeSet<u8>> = BTreeMap::new();
            for ((was, place), row) in expected.iter().zip(&after) {
                let now = wild_species(row);
                assert_eq!((row.string("area").unwrap_or_default(), row.string("locationName").unwrap_or_default()), (place.0.as_str(), place.1.as_str()));
                let first = *map.entry((place, *was)).or_insert(now);
                assert_eq!(first, now, "{path} {place:?}: {was:?} became both {first:?} and {now:?}");
                // Swimmers and fliers keep their movement first, so a theme only binds walkers.
                if walks(was, &can) {
                    let own: BTreeSet<u8> = dex.get(now).unwrap().types.into_iter().collect();
                    let shared = types.entry(place).or_insert_with(|| own.clone());
                    *shared = shared.intersection(&own).copied().collect();
                    assert!(!shared.is_empty(), "{path} {place:?}: replacements share no type");
                    themed += 1;
                }
            }
        }
        assert!(themed > 1000, "only {themed} themed replacements were checked");
        assert_movement(game, &out);
        println!("OK area map, {themed} themed replacements share their area's type");
    }
}

#[test]
fn catch_em_all_places_every_pool_species() {
    let Some(game) = scarlet("catch em all") else { return };
    let dex = dex(game);
    let mut gen1 = PoolFilter::everything();
    gen1.generations = [true, false, false, false, false, false, false, false, false];
    let pool: BTreeSet<Pokemon> = pool::build(dex, &gen1, false).into_iter().collect();
    for mode in ["swap", "global"] {
        let s = settings(&[
            text("wild.paldea.mode", mode),
            on("wild.paldea.catch_all"),
            ("wild.paldea.pool", OptValue::Pool(gen1.clone())),
        ]);
        for out in assert_repeatable(game, &s) {
            let placed: BTreeSet<Pokemon> = rows(&table(game, &out, WILD[0])).into_iter().map(wild_species).collect();
            assert!(placed.is_subset(&pool), "{mode}: something outside the pool was placed");
            let missing: Vec<&Pokemon> = pool.difference(&placed).collect();
            assert!(missing.is_empty(), "{mode}: {missing:?} never appear");
            for path in &WILD[1..] {
                assert!(!out.files.contains_key(*path), "{path} changed without being enabled");
            }
            assert_movement(game, &out);
            if mode == "swap" {
                assert_herds_follow_leaders(game, &out);
            }
            println!("OK {mode}: all {} Gen 1 forms appear in Paldea", pool.len());
        }
    }
}

#[test]
fn wild_tweaks_apply_without_new_species() {
    let Some(game) = scarlet("wild tweaks") else { return };
    let s = settings(&[
        number("wild.kitakami.level", 50),
        on("wild.kitakami.items"),
        on("wild.kitakami.ban_bad_items"),
        on("wild.kitakami.all_day"),
        on("wild.kitakami.both_versions"),
    ]);
    let [out, _] = assert_repeatable(game, &s);
    let (before, after) = (vanilla(game, WILD[1]), table(game, &out, WILD[1]));
    for (b, a) in rows(&before).into_iter().zip(rows(&after)) {
        assert_eq!(wild_species(b), wild_species(a));
        let scaled = |v: i64| if v > 0 { ((v * 150 + 50) / 100).clamp(1, 100) } else { v };
        assert_eq!(a.int("minlevel").unwrap(), scaled(b.int("minlevel").unwrap()));
        assert_eq!(a.int("maxlevel").unwrap(), scaled(b.int("maxlevel").unwrap()));
        let item = a.struct_value("bringItem").unwrap();
        assert!(item.int("itemID").unwrap() != 0 && item.int("bringRate").unwrap() > 0);
        let time = a.table("timetable").unwrap();
        assert!(["morning", "noon", "evening", "night"].iter().all(|f| time.bool(f).unwrap()));
        let version = a.table("versiontable").unwrap();
        assert!(version.bool("A").unwrap() && version.bool("B").unwrap());
    }
    assert!(!out.files.contains_key(WILD[0]) && !out.files.contains_key(WILD[2]));
    println!("OK Kitakami tweaks");
}

#[test]
fn statics_keep_their_class_and_extras() {
    let Some(game) = scarlet("statics") else { return };
    let dex = dex(game);
    let s = settings(&[
        on("statics.random"),
        on("statics.legendary_swap"),
        on("statics.strong_as_legendary"),
        on("statics.similar_strength"),
        number("statics.level", 10),
        text("statics.tera", "random"),
        number("statics.shiny", 100),
    ]);
    let before = vanilla(game, FIXED);
    let strong = |p: Pokemon| dex.get(p).is_some_and(|f| f.is_legendary_class() || f.bst() >= 600);
    let fliers: BTreeSet<Pokemon> =
        vanilla_moves(game).into_iter().filter(|(_, m)| m & (1 << 3 | 1 << 4) != 0).map(|(p, _)| p).collect();
    for out in assert_repeatable(game, &s) {
        let after = table(game, &out, FIXED);
        let mut strong_rows = 0;
        let mut flying_rows = 0;
        for (b, a) in rows(&before).into_iter().zip(rows(&after)) {
            let key = b.string("tableKey").unwrap();
            let (bp, ap) = (b.table("pokeDataSymbol").unwrap(), a.table("pokeDataSymbol").unwrap());
            if key == "sample" || key == "takara_test" {
                assert!(b == a, "test row {key} changed");
                continue;
            }
            let (was, now) = (poke_species(bp), poke_species(ap));
            assert!(dex.is_playable(now), "{key}: {now:?}");
            assert_eq!(strong(was), strong(now), "{key}: {was:?} became {now:?}");
            strong_rows += usize::from(strong(was));
            assert_eq!(enum_name(ap, "RareType", "rareType"), "RARE", "{key}");
            let gem = bp.int("gemType").unwrap();
            if gem == 101 {
                assert_eq!(ap.int("gemType").unwrap(), 101, "{key} lost its Stellar type");
            } else {
                assert!((2..=19).contains(&ap.int("gemType").unwrap()), "{key} has no random Tera type");
            }
            let level = bp.int("level").unwrap();
            if level > 0 {
                assert_eq!(ap.int("level").unwrap(), ((level * 110 + 50) / 100).clamp(1, 100), "{key}");
            }
            if bp.int("wazaType").unwrap() == 1 && was != now {
                assert_eq!(ap.int("wazaType").unwrap(), 0, "{key} kept moves of another species");
            }
            let action = |r: &TableValue| {
                let ai = r.table("pokeAI").unwrap();
                ai.schema().enum_name("PokemonActionID", ai.int("actionId").unwrap()).unwrap_or("").to_owned()
            };
            let (old, new) = (action(b), action(a));
            let named = ["PACHIRISU", "OBAKEINU", "KUMO", "DRAGONITE"].iter().any(|s| old.contains(s));
            if was != now && named {
                assert_eq!(new, "SIMPLE_POP", "{key}: {now:?} kept {old}");
            }
            if was != now && old.contains("FLIGHT") {
                let flies = fliers.contains(&now);
                flying_rows += usize::from(flies);
                assert!(flies || new == "SIMPLE_POP", "{key}: {now:?} cannot fly but kept {old}");
            }
        }
        assert!(flying_rows >= 8, "only {flying_rows} flight spots got fliers");
        assert!(strong_rows > 10, "too few strong encounters were checked");
        println!("OK statics, {strong_rows} strong encounters stayed strong");
    }
}

#[test]
fn forms_that_need_an_item_hold_it_or_stay_out() {
    let Some(game) = scarlet("form items") else { return };
    let dex = dex(game);
    let needs = |p: Pokemon| dex.get(p).and_then(|f| itempool::form_item(f.national, p.form));
    let mut arceus = PoolFilter::everything();
    arceus.generations = [false; 9];
    arceus.allowed = vec![493];
    let arceus_pool = || OptValue::Pool(arceus.clone());
    let mut s = custom(
        [(493, 9), (487, 1), (1017, 1)],
        &[
            on("pool.allow_tera_locked"),
            on("starters.items"),
            on("gifts.random"),
            on("gifts.eggs"),
            ("gifts.pool", arceus_pool()),
            text("gifts.trades", "given"),
            on("gifts.guests"),
            on("gifts.trade_items"),
            on("statics.random"),
            ("statics.pool", arceus_pool()),
            text("wild.paldea.mode", "swap"),
            ("wild.paldea.pool", arceus_pool()),
            on("wild.paldea.items"),
        ],
    );
    s.set("wild.kitakami.mode", OptValue::Text("global".into()));
    s.set("wild.kitakami.pool", arceus_pool());
    let [out, _] = assert_repeatable(game, &s);

    let gifts = table(game, &out, GIFTS);
    let starter_items = STARTER_LABELS.map(|l| gift(&gifts, l).int("item").unwrap());
    assert_eq!(starter_items, [298, 1779, 2407], "Flame Plate, Griseous Core, Wellspring Mask");
    let mut held = 0;
    for (label, poke) in rows(&gifts).into_iter().map(|r| (r.string("label").unwrap(), r.table("pokeData").unwrap())) {
        let p = poke_species(poke);
        if label.contains("_egg_") {
            assert_eq!(needs(p), None, "{label}: an egg cannot hold the item {p:?} needs");
        } else if let Some(item) = needs(p) {
            assert_eq!(poke.int("item").unwrap(), i64::from(item), "{label}: {p:?} lacks its item");
            held += 1;
        }
    }
    for poke in trade_rows(&table(game, &out, TRADE_POKEMON)).into_values() {
        if let Some(item) = needs(poke_species(poke)) {
            assert_eq!(poke.int("item").unwrap(), i64::from(item), "a traded {:?} lacks its item", poke_species(poke));
            held += 1;
        }
    }
    for p in species_in(game, &out, FIXED, "pokeDataSymbol") {
        assert_eq!(needs(p), None, "static {p:?} cannot hold the item it needs");
    }
    for path in &WILD[..2] {
        for row in rows(&table(game, &out, path)) {
            if let Some(item) = needs(wild_species(row)) {
                let bring = row.struct_value("bringItem").unwrap();
                assert_eq!((bring.int("itemID").unwrap(), bring.int("bringRate").unwrap()), (i64::from(item), 100));
                held += 1;
            }
            assert_eq!(needs(follower(row)), None, "{path}: a follower cannot hold the item it needs");
        }
    }
    assert!(held > 100, "only {held} placements needed an item");
    println!("OK {held} placed forms hold the item they need");
}

fn species_in(game: &GameFiles, out: &RandoOutput, path: &str, field: &str) -> Vec<Pokemon> {
    rows(&table(game, out, path)).into_iter().map(|r| poke_species(r.table(field).unwrap())).collect()
}

#[test]
fn features_draw_from_separate_streams() {
    let Some(game) = scarlet("streams") else { return };
    let story = |out: &RandoOutput| {
        let list = vanilla(game, TRADE_LIST);
        let labels: BTreeSet<String> = rows(&list).iter().map(|r| r.string("receivePoke").unwrap().to_owned()).collect();
        let after = table(game, out, TRADE_POKEMON);
        trade_rows(&after).into_iter().filter(|(l, _)| labels.contains(l)).map(|(_, p)| poke_species(p)).collect::<Vec<_>>()
    };
    let given = run(game, &settings(&[text("gifts.trades", "given")]), "5");
    let with_guests = run(game, &settings(&[text("gifts.trades", "given"), on("gifts.guests")]), "5");
    assert_eq!(story(&given), story(&with_guests), "guest trades changed the story trades");

    let statics = run(game, &settings(&[on("statics.random")]), "5");
    let with_tera = run(game, &settings(&[on("statics.random"), text("statics.tera", "random"), number("statics.shiny", 50)]), "5");
    assert_eq!(species_in(game, &statics, FIXED, "pokeDataSymbol"), species_in(game, &with_tera, FIXED, "pokeDataSymbol"));

    let starters = run(game, &settings(&[text("starters.mode", "random")]), "5");
    let with_extras = run(game, &settings(&[text("starters.mode", "random"), on("starters.tera"), on("starters.items")]), "5");
    assert_eq!(starters_of(game, &starters), starters_of(game, &with_extras));

    let wild = run(game, &settings(&[text("wild.paldea.mode", "swap")]), "5");
    let with_items = run(game, &settings(&[text("wild.paldea.mode", "swap"), on("wild.paldea.items"), number("wild.paldea.level", 20)]), "5");
    let paldea = |out: &RandoOutput| rows(&table(game, out, WILD[0])).into_iter().map(wild_species).collect::<Vec<_>>();
    assert_eq!(paldea(&wild), paldea(&with_items));
    println!("OK extras never reshuffle species");
}

#[test]
fn everything_on_at_once_passes_the_validator() {
    let Some(game) = scarlet("everything") else { return };
    let values = [
        text("starters.mode", "basics"),
        text("starters.shiny", "chance"),
        number("starters.shiny_chance", 50),
        on("starters.tera"),
        on("starters.items"),
        text("starters.reveal", "clavell"),
        text("starters.grass.ball", "random"),
        on("gifts.random"),
        on("gifts.eggs"),
        number("gifts.shiny", 20),
        on("gifts.tera"),
        text("gifts.trades", "both"),
        on("gifts.guests"),
        on("gifts.trade_nicknames"),
        on("gifts.trade_trainers"),
        on("gifts.trade_ivs"),
        on("gifts.trade_items"),
        on("statics.random"),
        on("statics.legendary_swap"),
        text("statics.tera", "random"),
        number("statics.level", -20),
    ];
    let mut s = settings(&values);
    for (key, mode) in REGION_KEYS.iter().zip(["area", "global", "shuffle"]) {
        s.set(&format!("wild.{key}.mode"), OptValue::Text(mode.into()));
        s.set(&format!("wild.{key}.similar_strength"), OptValue::Bool(true));
        s.set(&format!("wild.{key}.no_legendaries"), OptValue::Bool(true));
        s.set(&format!("wild.{key}.items"), OptValue::Bool(true));
        s.set(&format!("wild.{key}.level"), OptValue::Int(10));
    }
    let [a, b] = assert_repeatable(game, &s);
    assert_ride_legends_untouched(game, &a);
    assert_ride_legends_untouched(game, &b);
    assert_movement(game, &a);
    for path in WILD {
        for row in rows(&table(game, &a, path)) {
            assert!(!dex(game).get(wild_species(row)).unwrap().is_legendary_class(), "{path}: a legendary spawns");
        }
    }
    println!("OK {} files written\n{}", a.files.len(), a.log.to_text().lines().take(60).collect::<Vec<_>>().join("\n"));
}

#[test]
fn encounter_options_are_described() {
    let registry = rando::registry();
    let prefixes = [
        (Page::Starters, "starters."),
        (Page::GiftsTrades, "gifts."),
        (Page::Wild, "wild."),
        (Page::Static, "statics."),
    ];
    let mut count = 0;
    for def in &registry {
        let Some((_, prefix)) = prefixes.iter().find(|(page, _)| *page == def.page) else { continue };
        assert!(def.id.starts_with(prefix), "{} is on the wrong page", def.id);
        assert!(!def.help.is_empty(), "{} has no help text", def.id);
        assert!(!def.sources.is_empty(), "{} has no source tag", def.id);
        count += 1;
    }
    for pool in ["starters.pool", "gifts.pool", "wild.paldea.pool", "wild.kitakami.pool", "wild.blueberry.pool", "statics.pool"] {
        assert!(registry.iter().any(|d| d.id == pool), "{pool} is missing");
    }
    let defaults = Settings::defaults(&registry);
    for def in registry.iter().filter(|d| prefixes.iter().any(|(p, _)| *p == d.page)) {
        let value = &defaults.values[def.id];
        let neutral = match value {
            OptValue::Bool(b) => !b || def.id.ends_with(".alt_forms") || def.id.ends_with(".movement"),
            OptValue::Int(v) => *v == 0,
            OptValue::Text(t) => ["unchanged", "random", "keep", "nemona"].contains(&t.as_str()),
            _ => true,
        };
        assert!(neutral, "{} does not default to off or unchanged: {value:?}", def.id);
    }
    println!("OK {count} encounter options");
}
