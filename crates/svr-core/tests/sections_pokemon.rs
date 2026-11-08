//! Real-game tests for the stats, evolutions and moves sections. Each group runs `randomize` on
//! Scarlet with its options on, checks that the same seed repeats byte for byte and that other
//! seeds differ, then checks the invariants the options promise in the output tables.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use svr_core::container::Keys;
use svr_core::data::facts::Rarity;
use svr_core::data::store::{load_table, ABILITIES, ITEMS, MOVES, PERSONAL};
use svr_core::data::{Dex, Names, Pokemon, TableStore};
use svr_core::fb::{self, builtin, Schema, TableValue, Value};
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::rando::movepool::{GAME_BREAKING, POOR_ATTACKS};
use svr_core::rando::options::Kind;
use svr_core::rando::{self, OptValue, RandoOutput, Seed, Settings};
use svr_core::text::Language;

const TM_MACHINE: &str = "world/data/ui/shop/shop_wazamachine/shop_wazamachine_data/shop_wazamachine_data_array.bin";
const PLIB: &str = "world/data/battle/plib_item_conversion/plib_item_conversion_array.bin";
const METRONOME: u16 = 118;
const NEVER_MISSES: u8 = 101;
const ON_EVOLUTION: i8 = -3;

const FORM_CHANGING: &[&str] = &[
    "Multitype", "Zen Mode", "Stance Change", "Shields Down", "Schooling", "Disguise", "Battle Bond", "Power Construct",
    "RKS System", "Forecast", "Flower Gift", "Gulp Missile", "Ice Face", "Hunger Switch", "Zero to Hero", "Commander",
    "Tera Shift", "Tera Shell", "Teraform Zero", "Embody Aspect",
];
const BANNED_BY_OPTIONS: &[&str] = &[
    "Wonder Guard", "Wimp Out", "Emergency Exit", "Shadow Tag", "Magnet Pull", "Arena Trap", "Defeatist", "Slow Start",
    "Truant", "Klutz", "Stall", "Minus", "Plus", "Anticipation", "Forewarn", "Frisk", "Honey Gather", "Aura Break",
    "Receiver", "Power of Alchemy", "Friend Guard", "Healer", "Telepathy", "Symbiosis", "Battery",
];

struct Game {
    files: GameFiles,
    dex: Dex,
    names: Names,
}

fn scarlet(test: &str) -> Option<Game> {
    let (Some(keys), Some(games)) = (std::env::var_os("SVR_TEST_KEYS"), std::env::var_os("SVR_TEST_GAMES")) else {
        println!("SKIP {test}: set SVR_TEST_KEYS and SVR_TEST_GAMES");
        return None;
    };
    let games = PathBuf::from(games);
    let paths = GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES");
    let files = GameFiles::open(&paths, &Keys::load(Path::new(&keys)).unwrap()).unwrap();
    let dex = Dex::load(&mut TableStore::new(&files)).unwrap();
    let names = Names::load(&files, Language::English).unwrap();
    Some(Game { files, dex, names })
}

#[derive(Clone)]
enum V {
    On,
    Off,
    Is(&'static str),
    N(i64),
}

fn settings(values: &[(&str, V)]) -> Settings {
    let mut s = Settings::defaults(&rando::registry());
    for (id, v) in values {
        let value = match v {
            V::On => OptValue::Bool(true),
            V::Off => OptValue::Bool(false),
            V::Is(t) => OptValue::Text((*t).to_owned()),
            V::N(n) => OptValue::Int(*n),
        };
        s.set(id, value);
    }
    s
}

/// Runs every seed, reruns the first, and checks repeatability and, if `seeded`, that seeds differ.
fn runs(game: &Game, s: &Settings, seeds: &[&str], seeded: bool) -> Vec<RandoOutput> {
    let outs: Vec<RandoOutput> = seeds
        .iter()
        .map(|seed| rando::randomize(&game.files, s, Seed((*seed).into())).unwrap_or_else(|e| panic!("seed {seed}: {e}")))
        .collect();
    let again = rando::randomize(&game.files, s, Seed(seeds[0].into())).unwrap();
    assert_eq!(again.files, outs[0].files, "the same seed gives the same bytes");
    assert!(outs[0].warnings.is_empty(), "{:?}", outs[0].warnings);
    for (seed, out) in seeds.iter().zip(&outs).skip(1) {
        if seeded {
            assert_ne!(out.files, outs[0].files, "seed {seed} gives the same output as {}", seeds[0]);
        } else {
            assert_eq!(out.files, outs[0].files, "seed {seed} should not matter");
        }
    }
    println!("OK {} seeds validated, same seed repeats, files {:?}", seeds.len(), outs[0].files.keys().collect::<Vec<_>>());
    outs
}

fn excerpt(out: &RandoOutput, title: &str, lines: usize) {
    let text = out.log.to_text();
    let start = text.find(&format!("== {title} ==")).unwrap_or_else(|| panic!("no spoiler section {title}"));
    let body: Vec<&str> = text[start..].lines().take(lines).collect();
    println!("{}", body.join("\n"));
}

fn schema_for(game: &Game, path: &str) -> Arc<Schema> {
    match path {
        PERSONAL => builtin::personal(),
        MOVES => builtin::waza(),
        _ => Schema::from_bfbs(&game.files.read(&path.replace(".bin", ".bfbs")).unwrap()).unwrap(),
    }
}

fn table(game: &Game, out: Option<&RandoOutput>, path: &str) -> TableValue {
    match out.and_then(|o| o.files.get(path)) {
        Some(bytes) => fb::parse(&schema_for(game, path), bytes).unwrap(),
        None => load_table(&game.files, path).unwrap(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Evo {
    condition: u16,
    parameter: u16,
    level: u16,
    into: Pokemon,
}

#[derive(Clone, Debug, PartialEq)]
struct Mon {
    id: Pokemon,
    present: bool,
    type_locked: bool,
    stats: [u8; 6],
    types: [u8; 2],
    abilities: [u16; 3],
    xp: u8,
    catch: u8,
    hatch: u8,
    evos: Vec<Evo>,
    levelup: Vec<(u16, i8)>,
    egg: Vec<u16>,
    reminder: Vec<u16>,
    tm: Vec<u16>,
}

impl Mon {
    fn bst(&self) -> u32 {
        self.stats.iter().map(|&s| u32::from(s)).sum()
    }
}

fn ushorts(t: &TableValue, field: &str) -> Vec<u16> {
    t.vector(field).unwrap_or(&[]).iter().filter_map(Value::as_uint).map(|v| v as u16).collect()
}

fn mons(game: &Game, out: Option<&RandoOutput>) -> Vec<Mon> {
    let personal = table(game, out, PERSONAL);
    personal
        .tables("entry")
        .map(|e| {
            let sp = e.struct_value("species").unwrap();
            let st = e.struct_value("base_stats").unwrap();
            Mon {
                id: Pokemon {
                    species: sp.int("species").unwrap() as u16,
                    form: sp.int("form").unwrap() as u16,
                },
                present: e.bool("is_present").unwrap(),
                type_locked: e.bool("type_change_disallowed").unwrap_or(false),
                stats: ["hp", "atk", "def", "spa", "spd", "spe"].map(|f| st.int(f).unwrap() as u8),
                types: [e.int("type_1").unwrap() as u8, e.int("type_2").unwrap() as u8],
                abilities: ["ability_1", "ability_2", "ability_hidden"].map(|f| e.int(f).unwrap() as u16),
                xp: e.int("xp_growth").unwrap() as u8,
                catch: e.int("catch_rate").unwrap() as u8,
                hatch: e.int("egg_hatch_steps").unwrap() as u8,
                evos: e
                    .vector("evolutions")
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(Value::as_struct)
                    .map(|v| Evo {
                        condition: v.int("condition").unwrap() as u16,
                        parameter: v.int("parameter").unwrap() as u16,
                        level: v.int("level").unwrap() as u16,
                        into: Pokemon {
                            species: v.int("species").unwrap() as u16,
                            form: v.int("form").unwrap() as u16,
                        },
                    })
                    .collect(),
                levelup: e
                    .vector("levelup_moves")
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(Value::as_struct)
                    .map(|m| (m.int("move").unwrap() as u16, m.int("level").unwrap() as i8))
                    .collect(),
                egg: ushorts(e, "egg_moves"),
                reminder: ushorts(e, "reminder_moves"),
                tm: ushorts(e, "tm_moves"),
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
struct Move {
    usable: bool,
    type_id: u8,
    category: u8,
    power: u8,
    accuracy: u8,
    pp: u8,
    hits: (u8, u8),
}

impl Move {
    fn strength(&self) -> f64 {
        let hits = match self.hits {
            (_, 0 | 1) => 1.0,
            (2, 5) => 19.0 / 6.0,
            (a, b) => f64::from(a + b) / 2.0,
        };
        if self.category != 0 && self.power > 1 {
            f64::from(self.power) * hits
        } else {
            0.0
        }
    }

    fn good(&self) -> bool {
        let s = self.strength();
        s >= 100.0 || (s >= 50.0 && self.accuracy >= 90)
    }
}

fn moves(game: &Game, out: Option<&RandoOutput>) -> Vec<Move> {
    table(game, out, MOVES)
        .tables("Table")
        .map(|m| {
            let b = |f: &str| m.int(f).unwrap() as u8;
            Move {
                usable: m.bool("CanUseMove").unwrap(),
                type_id: b("Type"),
                category: b("Category"),
                power: b("Power"),
                accuracy: b("Accuracy"),
                pp: b("PP"),
                hits: (b("HitMin"), b("HitMax")),
            }
        })
        .collect()
}

/// (TM item, move, LP, [(material, count)]) per machine row.
type MachineRow = (u16, u16, i64, Vec<(u16, i64)>);

fn machine(game: &Game, out: Option<&RandoOutput>) -> Vec<MachineRow> {
    table(game, out, TM_MACHINE)
        .tables("values")
        .map(|r| {
            let mats = [("Item01", "ItemNum01"), ("Item02", "ItemNum02"), ("Item03", "ItemNum03")]
                .iter()
                .map(|(i, n)| (r.int(i).unwrap() as u16, r.int(n).unwrap()))
                .collect();
            (r.int("WazaItemID").unwrap() as u16, r.int("WazaNo").unwrap() as u16, r.int("LP").unwrap(), mats)
        })
        .collect()
}

fn item_moves(game: &Game, out: Option<&RandoOutput>) -> BTreeMap<u16, u16> {
    table(game, out, ITEMS)
        .tables("values")
        .map(|i| (i.int("Id").unwrap() as u16, i.int("MachineWaza").unwrap_or(0) as u16))
        .filter(|&(_, m)| m != 0)
        .collect()
}

fn ability_ids(game: &Game, list: &[&str]) -> BTreeSet<u16> {
    (1..400u16).filter(|&a| list.contains(&game.names.ability(a).as_str())).collect()
}

fn move_ids(game: &Game, list: &[&str]) -> BTreeSet<u16> {
    (1..1000u16).filter(|&m| list.contains(&game.names.move_name(m).as_str())).collect()
}

fn playable(game: &Game, i: usize) -> bool {
    game.dex.all()[i].playable()
}

fn index_of(all: &[Mon]) -> BTreeMap<Pokemon, usize> {
    all.iter().enumerate().map(|(i, m)| (m.id, i)).collect()
}

/// Cosmetic forms copy their base form: the lowest form of the species with the same vanilla key.
fn twin_of<K: PartialEq>(vanilla: &[Mon], key: impl Fn(&Mon) -> K) -> Vec<Option<usize>> {
    let mut groups: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
    let mut out = vec![None; vanilla.len()];
    for (i, m) in vanilla.iter().enumerate().filter(|(_, m)| m.present) {
        let g = groups.entry(m.id.species).or_default();
        match g.iter().find(|&&l| key(&vanilla[l]) == key(m)) {
            Some(&l) => out[i] = Some(l),
            None => g.push(i),
        }
    }
    out
}

/// Vanilla parent → child edges between present non-cosmetic rows, with whether the parent splits.
fn vanilla_edges(vanilla: &[Mon], twin: &[Option<usize>]) -> Vec<(usize, usize, bool)> {
    let index = index_of(vanilla);
    let mut first_parent: BTreeMap<usize, usize> = BTreeMap::new();
    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, m) in vanilla.iter().enumerate() {
        if !m.present || twin[i].is_some() {
            continue;
        }
        for e in &m.evos {
            let Some(&t) = index.get(&e.into) else { continue };
            if t == i || !vanilla[t].present || twin[t].is_some() {
                continue;
            }
            let list = children.entry(i).or_default();
            if !list.contains(&t) {
                list.push(t);
                first_parent.entry(t).or_insert(i);
            }
        }
    }
    first_parent.into_iter().map(|(c, p)| (p, c, children[&p].len() > 1)).collect()
}

#[test]
fn game_facts_behind_the_pokemon_sections() {
    let Some(game) = scarlet("facts") else { return };
    assert_eq!(game.names.ability(278), "Zero to Hero");
    assert_eq!(game.names.ability(307), "Tera Shift");
    for name in FORM_CHANGING.iter().chain(BANNED_BY_OPTIONS) {
        assert!(!ability_ids(&game, &[name]).is_empty(), "ability {name} is not in the game text");
    }
    for name in GAME_BREAKING.iter().chain(POOR_ATTACKS) {
        assert!(!move_ids(&game, &[name]).is_empty(), "move {name} is not in the game text");
    }
    let vanilla = mons(&game, None);
    let tms = machine(&game, None);
    let items = item_moves(&game, None);
    assert_eq!(tms.len(), 229);
    assert!(tms.iter().all(|t| items[&t.0] == t.1), "item and machine agree on every TM move");
    let pikachu = vanilla.iter().position(|m| m.id == Pokemon { species: 25, form: 0 }).unwrap();
    let thunderbolt = move_ids(&game, &["Thunderbolt"]).into_iter().next().unwrap();
    assert!(vanilla[pikachu].tm.contains(&thunderbolt), "tm_moves holds move IDs");
    let tm_set: BTreeSet<u16> = tms.iter().map(|t| t.1).collect();
    let extras = vanilla[pikachu].tm.iter().filter(|m| !tm_set.contains(m)).count();
    println!(
        "OK 278 Zero to Hero, 307 Tera Shift; tm_moves are move IDs; Pikachu lists {} moves, {extras} of them taught by no TM",
        vanilla[pikachu].tm.len()
    );
    let plib = table(&game, None, PLIB);
    let sun = plib.tables("values").find(|r| r.int("itemID").unwrap_or(0) == 80).map(|r| r.int("plibID").unwrap());
    assert_eq!(sun, Some(1), "evolution items use their own IDs: Sun Stone is 1");
    let usable: Vec<Move> = moves(&game, None);
    for (i, m) in vanilla.iter().enumerate().filter(|(i, _)| playable(&game, *i)) {
        for &(mv, _) in &m.levelup {
            assert!(usable[usize::from(mv)].usable, "vanilla {} learns unusable move {mv}", game.names.pokemon(m.id));
        }
        assert!(m.levelup.windows(2).all(|w| w[0].1 <= w[1].1), "row {i} learnset is sorted");
    }
    println!("OK every vanilla level-up move of a playable Pokémon is usable and learnsets are sorted");
}

#[test]
fn pokemon_options_default_to_unchanged() {
    let Some(game) = scarlet("defaults") else { return };
    let ours: Vec<_> = rando::registry()
        .into_iter()
        .filter(|d| ["pokemon.", "evolutions.", "moves."].iter().any(|p| d.id.starts_with(p)))
        .collect();
    for def in &ours {
        let neutral = match &def.kind {
            Kind::Toggle { default } => !default || def.id == "pokemon.stats_keep_bst",
            Kind::Choice { default, .. } => ["unchanged", "keep", "medium_fast"].contains(default),
            Kind::Number { default, .. } => *default == 0,
            Kind::Pool => true,
            _ => false,
        };
        assert!(neutral, "{} does not default to unchanged", def.id);
        assert!(!def.help.is_empty() || matches!(def.kind, Kind::Choice { .. }), "{} has no help", def.id);
        assert!(!def.sources.is_empty(), "{} has no sources", def.id);
    }
    let out = rando::randomize(&game.files, &settings(&[]), Seed("1".into())).unwrap();
    for path in [PERSONAL, MOVES, ITEMS, TM_MACHINE] {
        assert!(!out.files.contains_key(path), "defaults wrote {path}");
    }
    println!("OK {} options default to unchanged, and defaults write none of our tables", ours.len());
}

#[test]
fn random_stats_types_and_abilities_follow_evolutions() {
    let Some(game) = scarlet("stats") else { return };
    let s = settings(&[
        ("pokemon.stats", V::Is("random")),
        ("pokemon.stats_follow_evolutions", V::On),
        ("pokemon.types", V::Is("follow_evolutions")),
        ("pokemon.abilities", V::On),
        ("pokemon.abilities_follow_evolutions", V::On),
        ("pokemon.abilities_ban_exit", V::On),
        ("pokemon.abilities_ban_trapping", V::On),
        ("pokemon.abilities_ban_negative", V::On),
        ("pokemon.abilities_ban_bad", V::On),
        ("pokemon.abilities_combine_duplicates", V::On),
        ("pokemon.exp_curves", V::Is("legendaries")),
        ("pokemon.min_catch_rate", V::Is("easier")),
        ("pokemon.egg_hatching", V::Is("faster")),
    ]);
    let outs = runs(&game, &s, &["11", "12"], true);
    let vanilla = mons(&game, None);
    let twin = twin_of(&vanilla, |m| (m.stats, m.types, m.abilities));
    let edges = vanilla_edges(&vanilla, &twin);
    let banned = ability_ids(&game, BANNED_BY_OPTIONS);
    let form_changing = ability_ids(&game, FORM_CHANGING);
    let ability_count = load_table(&game.files, ABILITIES).unwrap().tables("Table").count() as u16;
    let dual = |m: &Mon| m.types[0] != m.types[1];
    let mut expect_dual: Vec<bool> = vanilla.iter().map(dual).collect();
    for _ in 0..4 {
        for &(p, c, split) in &edges {
            if !split {
                expect_dual[c] = expect_dual[p] || dual(&vanilla[c]);
            }
        }
    }
    let keeps_abilities = |m: &Mon| m.abilities.iter().any(|a| form_changing.contains(a) || game.names.ability(*a) == "Wonder Guard");
    let copies_abilities: BTreeSet<usize> = edges
        .iter()
        .filter(|&&(p, _, split)| !split && !keeps_abilities(&vanilla[p]))
        .map(|&(_, c, _)| c)
        .collect();
    for out in &outs {
        let new = mons(&game, Some(out));
        for (i, (n, v)) in new.iter().zip(&vanilla).enumerate().filter(|(_, (n, _))| n.present) {
            let name = game.names.pokemon(n.id);
            assert_eq!(n.bst(), v.bst(), "{name} keeps its total");
            assert!(n.stats.iter().all(|&x| x >= 1), "{name} stats {:?}", n.stats);
            assert!(n.types.iter().all(|&t| t < 18), "{name} types");
            let lead = twin[i].unwrap_or(i);
            if !v.type_locked {
                assert_eq!(dual(n), expect_dual[lead], "{name} has the expected number of types");
            }
            if keeps_abilities(v) {
                assert_eq!(n.abilities, v.abilities, "{name} keeps its form-changing ability");
            } else {
                for a in n.abilities {
                    assert!(a >= 1 && a < ability_count, "{name} ability {a}");
                    assert!(!banned.contains(&a) && !form_changing.contains(&a), "{name} got banned {}", game.names.ability(a));
                }
                if !copies_abilities.contains(&twin[i].unwrap_or(i)) {
                    assert_eq!(n.abilities[0] == n.abilities[1], v.abilities[0] == v.abilities[1], "{name} keeps its ability count");
                }
            }
            let legendary = game.dex.all()[i].rarity != Rarity::Standard;
            assert_eq!(n.xp, if legendary { 5 } else { 0 }, "{name} EXP curve");
            assert!(n.catch >= if legendary { 37 } else { 75 } && n.catch >= v.catch, "{name} catch rate");
            assert_eq!(n.hatch, v.hatch.div_ceil(2).max(v.hatch.min(5)), "{name} egg cycles");
            if let Some(l) = twin[i] {
                assert_eq!((n.stats, n.abilities), (new[l].stats, new[l].abilities), "{name} matches its base form");
            }
        }
        let mut checked = 0;
        for &(p, c, split) in &edges {
            let (pn, cn) = (&new[p], &new[c]);
            let name = game.names.pokemon(cn.id);
            if split {
                continue;
            }
            assert!(cn.types.contains(&pn.types[0]) && cn.types.contains(&pn.types[1]), "{name} keeps its line's types");
            let parent_keeps = vanilla[p].abilities.iter().any(|a| form_changing.contains(a));
            if !parent_keeps && !vanilla[c].abilities.iter().any(|a| form_changing.contains(a)) {
                assert_eq!(cn.abilities, pn.abilities, "{name} has its line's abilities");
            }
            for a in 0..6 {
                for b in 0..6 {
                    if pn.stats[a] >= pn.stats[b] + 3 {
                        assert!(cn.stats[a] >= cn.stats[b], "{name} keeps its line's stat shape {:?} vs {:?}", pn.stats, cn.stats);
                    }
                }
            }
            checked += 1;
        }
        println!("OK seed {}: totals kept, bans hold, {checked} evolutions follow their line", out.seed.0);
    }
    excerpt(&outs[0], "Stats, types & abilities", 12);
}

#[test]
fn shuffled_stats_dual_types_and_two_abilities() {
    let Some(game) = scarlet("shuffle") else { return };
    let s = settings(&[
        ("pokemon.stats", V::Is("shuffle")),
        ("pokemon.stats_follow_evolutions", V::On),
        ("pokemon.types", V::Is("random")),
        ("pokemon.types_count", V::Is("dual")),
        ("pokemon.abilities", V::On),
        ("pokemon.abilities_ensure_two", V::On),
        ("pokemon.abilities_allow_wonder_guard", V::On),
        ("pokemon.exp_curves", V::Is("all")),
        ("pokemon.exp_curve", V::Is("fast")),
        ("pokemon.min_catch_rate", V::Is("maximum")),
        ("pokemon.egg_hatching", V::Is("fastest")),
    ]);
    let outs = runs(&game, &s, &["21", "22"], true);
    let vanilla = mons(&game, None);
    let form_changing = ability_ids(&game, FORM_CHANGING);
    for out in &outs {
        let new = mons(&game, Some(out));
        for (n, v) in new.iter().zip(&vanilla).filter(|(n, _)| n.present) {
            let name = game.names.pokemon(n.id);
            let (mut a, mut b) = (n.stats, v.stats);
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "{name} has the same six stats in some order");
            if v.type_locked {
                assert_eq!(n.types, v.types, "{name} keeps its types");
            } else {
                assert_ne!(n.types[0], n.types[1], "{name} is dual-typed");
            }
            if !v.abilities.iter().any(|x| form_changing.contains(x) || game.names.ability(*x) == "Wonder Guard") {
                assert_ne!(n.abilities[0], n.abilities[1], "{name} has two abilities");
            }
            assert_eq!(n.xp, 4, "{name} uses Fast");
            assert_eq!(n.catch, 255, "{name} catch rate");
            assert_eq!(n.hatch, v.hatch.min(5), "{name} egg cycles");
        }
        println!("OK seed {}: stats shuffled within each Pokémon, everyone dual-typed", out.seed.0);
    }
    excerpt(&outs[0], "Stats, types & abilities", 8);
}

#[test]
fn random_stats_without_keeping_totals() {
    let Some(game) = scarlet("chaos") else { return };
    let s = settings(&[("pokemon.stats", V::Is("random")), ("pokemon.stats_keep_bst", V::Off)]);
    let outs = runs(&game, &s, &["31", "32"], true);
    let vanilla = mons(&game, None);
    let new = mons(&game, Some(&outs[0]));
    let changed = new.iter().zip(&vanilla).filter(|(n, v)| n.present && n.bst() != v.bst()).count();
    assert!(changed > 800, "totals change: {changed}");
    for n in new.iter().filter(|n| n.present) {
        assert!(n.stats.iter().all(|&x| x >= 15) || n.stats[0] == 1, "{:?}", n.stats);
    }
    println!("OK {changed} Pokémon got new totals, every stat 15 or more");
}

/// Longest chain of evolutions starting at `i`, counting Pokémon, over playable rows.
fn stages(new: &[Mon], index: &BTreeMap<Pokemon, usize>, i: usize, depth: usize) -> usize {
    assert!(depth < 20, "evolution cycle through {:?}", new[i].id);
    new[i]
        .evos
        .iter()
        .filter_map(|e| index.get(&e.into))
        .map(|&t| 1 + stages(new, index, t, depth + 1))
        .max()
        .unwrap_or(1)
}

#[test]
fn random_evolutions_stay_valid() {
    let Some(game) = scarlet("evolutions") else { return };
    let s = settings(&[
        ("evolutions.mode", V::Is("random")),
        ("evolutions.similar_strength", V::On),
        ("evolutions.same_typing", V::On),
        ("evolutions.limit_three_stages", V::On),
        ("evolutions.force_change", V::On),
        ("evolutions.allow_alt_forms", V::On),
        ("evolutions.replace_trade", V::On),
        ("evolutions.replace_time_and_region", V::On),
        ("evolutions.easier", V::On),
    ]);
    let outs = runs(&game, &s, &["41", "42"], true);
    let vanilla = mons(&game, None);
    let index = index_of(&vanilla);
    for out in &outs {
        let new = mons(&game, Some(out));
        let mut edges = 0;
        for (i, (n, v)) in new.iter().zip(&vanilla).enumerate().filter(|(i, _)| playable(&game, *i)) {
            let name = game.names.pokemon(n.id);
            assert_eq!(n.evos.is_empty(), v.evos.is_empty(), "{name} still evolves");
            for e in &n.evos {
                let t = index[&e.into];
                assert!(playable(&game, t), "{name} evolves into unplayable {:?}", e.into);
                assert_eq!(new[t].xp, n.xp, "{name} evolves into the same EXP curve");
                assert!(!v.evos.iter().any(|ve| ve.into == e.into), "{name} still evolves into {}", game.names.pokemon(e.into));
                assert!(![2, 3, 5, 6, 7, 32, 33, 40, 51, 61].contains(&e.condition), "{name} kept method {}", e.condition);
                assert!(e.level <= 40, "{name} evolves at level {}", e.level);
                edges += 1;
            }
            assert!(stages(&new, &index, i, 0) <= 3, "{name} starts a line longer than three");
        }
        println!("OK seed {}: {edges} evolutions, all playable, same EXP curve, changed, at most three stages", out.seed.0);
    }
    excerpt(&outs[0], "Evolutions", 10);
}

#[test]
fn evolution_methods_are_replaced() {
    let Some(game) = scarlet("methods") else { return };
    let find = |all: &[Mon], species: u16, form: u16| all.iter().find(|m| m.id == Pokemon { species, form }).unwrap().clone();
    let trade_only = settings(&[("evolutions.replace_trade", V::On)]);
    let out = &runs(&game, &trade_only, &["51", "52"], false)[0];
    let new = mons(&game, Some(out));
    let haunter = find(&new, 93, 0);
    assert_eq!((haunter.evos[0].condition, haunter.evos[0].level), (4, 37), "Haunter levels into Gengar at 37");
    let slowpoke = find(&new, 79, 0);
    assert_eq!(slowpoke.evos.iter().map(|e| e.condition).collect::<Vec<_>>(), vec![32, 33], "Slowpoke splits by day and night");
    let rhydon = find(&new, 112, 0);
    assert_eq!(rhydon.evos.iter().map(|e| e.condition).collect::<Vec<_>>(), vec![19, 20], "Rhydon levels holding Protector");
    let finizen = new.iter().find(|m| game.names.pokemon(m.id) == "Finizen").unwrap();
    assert_eq!((finizen.evos[0].condition, finizen.evos[0].level), (4, 38), "Finizen no longer needs a Union Circle");

    let both = settings(&[("evolutions.replace_trade", V::On), ("evolutions.replace_time_and_region", V::On)]);
    let out = &runs(&game, &both, &["53"], false)[0];
    let new = mons(&game, Some(out));
    let eevee = find(&new, 133, 0);
    let stones: Vec<(u16, u16)> = eevee.evos.iter().filter(|e| [196, 197].contains(&e.into.species)).map(|e| (e.condition, e.parameter)).collect();
    assert_eq!(stones, vec![(8, 1), (8, 2)], "Espeon takes a Sun Stone and Umbreon a Moon Stone");
    for (_, m) in new.iter().enumerate().filter(|(i, _)| playable(&game, *i)) {
        for e in &m.evos {
            assert!(![2, 3, 5, 6, 7, 32, 33, 40, 51, 61].contains(&e.condition), "{} kept method {}", game.names.pokemon(m.id), e.condition);
        }
    }
    println!("OK trade, Union Circle, day, night and Hisui evolutions are gone");
    excerpt(out, "Evolutions", 12);
}

#[test]
fn every_level_evolutions() {
    let Some(game) = scarlet("every level") else { return };
    let s = settings(&[("evolutions.mode", V::Is("every_level")), ("evolutions.same_typing", V::On), ("evolutions.force_change", V::On)]);
    let outs = runs(&game, &s, &["61", "62"], true);
    let vanilla = mons(&game, None);
    let index = index_of(&vanilla);
    let story = ["Koraidon", "Miraidon", "Ogerpon", "Terapagos"];
    for out in &outs {
        let new = mons(&game, Some(out));
        let mut count = 0;
        for (i, n) in new.iter().enumerate().filter(|(i, _)| playable(&game, *i)) {
            let name = game.names.pokemon(n.id);
            if story.contains(&game.names.species(n.id.species).as_str()) {
                assert_eq!(n.evos, vanilla[i].evos, "{name} is left alone");
                continue;
            }
            assert_eq!(n.evos.len(), 1, "{name} has one evolution");
            let e = n.evos[0];
            let t = index[&e.into];
            assert_eq!((e.condition, e.level), (4, 1), "{name} evolves on any level-up");
            assert!(playable(&game, t) && t != i, "{name} evolves into a valid other Pokémon");
            assert_eq!(new[t].xp, n.xp, "{name} keeps its EXP curve");
            count += 1;
        }
        println!("OK seed {}: {count} Pokémon evolve at every level", out.seed.0);
    }
    excerpt(&outs[0], "Evolutions", 6);
}

fn tm_moves_of(rows: &[MachineRow]) -> Vec<u16> {
    rows.iter().map(|t| t.1).collect()
}

/// Every move a playable Pokémon can learn by level-up, egg, reminder or TM is usable.
fn assert_learnable_moves_usable(game: &Game, new: &[Mon], mv: &[Move], tm_moves: &BTreeSet<u16>) {
    let vanilla = mons(game, None);
    for (i, (n, v)) in new.iter().zip(&vanilla).enumerate().filter(|(i, _)| playable(game, *i)) {
        let name = game.names.pokemon(n.id);
        let mut all: Vec<u16> = n.levelup.iter().map(|m| m.0).collect();
        if n.egg != v.egg {
            all.extend(&n.egg);
        }
        if n.reminder != v.reminder {
            all.extend(&n.reminder);
        }
        all.extend(n.tm.iter().filter(|m| tm_moves.contains(m)));
        for m in all {
            assert!(m != 0 && mv[usize::from(m)].usable, "{name} can learn unusable move {m}");
        }
        assert!(n.levelup.windows(2).all(|w| w[0].1 <= w[1].1), "row {i} learnset is sorted");
    }
}

/// Pokémon that evolve by knowing a move, or a move of a type, can still learn one.
fn assert_move_evolutions_possible(game: &Game, new: &[Mon], mv: &[Move], tm_moves: &BTreeSet<u16>) {
    let rage_fist = move_ids(game, &["Rage Fist"]).into_iter().next().unwrap();
    let mut checked = 0;
    for (i, n) in new.iter().enumerate().filter(|(i, _)| playable(game, *i)) {
        let learnable: Vec<u16> = n
            .levelup
            .iter()
            .map(|x| x.0)
            .chain(n.reminder.iter().copied())
            .chain(n.tm.iter().copied().filter(|m| tm_moves.contains(m)))
            .collect();
        for e in &n.evos {
            let ok = match e.condition {
                21 | 57 | 58 => learnable.contains(&e.parameter),
                56 => learnable.contains(&rage_fist),
                29 | 30 => learnable.iter().any(|&m| mv[usize::from(m)].type_id == e.parameter as u8),
                _ => continue,
            };
            assert!(ok, "{} (row {i}) can no longer meet evolution method {} ({})", game.names.pokemon(n.id), e.condition, e.parameter);
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} move-based evolutions checked");
}

#[test]
fn random_learnsets_and_move_data() {
    let Some(game) = scarlet("learnsets") else { return };
    let s = settings(&[
        ("moves.levelup", V::Is("prefer_type")),
        ("moves.guaranteed_level1", V::N(3)),
        ("moves.good_damaging_percent", V::N(30)),
        ("moves.evolution_moves", V::On),
        ("moves.min_stab", V::N(2)),
        ("moves.min_damaging", V::N(4)),
        ("moves.expand_to", V::N(20)),
        ("moves.spread_to", V::N(60)),
        ("moves.reorder_damaging", V::On),
        ("moves.ban_game_breaking", V::On),
        ("moves.egg_and_reminder", V::On),
        ("moves.data_power", V::On),
        ("moves.data_accuracy", V::On),
        ("moves.data_pp", V::On),
        ("moves.data_type", V::On),
        ("moves.data_category", V::On),
    ]);
    let outs = runs(&game, &s, &["71", "72"], true);
    let vanilla = mons(&game, None);
    let vanilla_moves = moves(&game, None);
    let ohko = move_ids(&game, GAME_BREAKING);
    let tm_moves: BTreeSet<u16> = tm_moves_of(&machine(&game, None)).into_iter().collect();
    for out in &outs {
        let new = mons(&game, Some(out));
        let mv = moves(&game, Some(out));
        assert_learnable_moves_usable(&game, &new, &mv, &tm_moves);
        assert_move_evolutions_possible(&game, &new, &mv, &tm_moves);
        for (old, m) in vanilla_moves.iter().zip(&mv) {
            assert_eq!(m.usable, old.usable);
            assert!(m.type_id < 18 && m.pp >= 5 && m.pp <= 40 || !m.usable || *m == *old, "move data in range: {m:?}");
            if old.accuracy == NEVER_MISSES {
                assert_eq!(m.accuracy, NEVER_MISSES, "moves that never miss still never miss");
            }
            assert_eq!(m.category == 0, old.category == 0, "status moves stay status moves");
        }
        let mut rows = 0;
        for (_, (n, v)) in new.iter().zip(&vanilla).enumerate().filter(|(i, (n, _))| playable(&game, *i) && !n.levelup.is_empty()) {
            let name = game.names.pokemon(n.id);
            let regular: Vec<&(u16, i8)> = n.levelup.iter().filter(|m| m.1 != ON_EVOLUTION).collect();
            assert!(regular.iter().filter(|m| m.1 == 1).count() >= 3, "{name} knows three moves at level 1");
            assert!(regular.len() >= 20 || regular.len() >= v.levelup.len(), "{name} has at least 20 moves");
            let required: BTreeSet<u16> = n.evos.iter().filter(|e| [21, 57, 58].contains(&e.condition)).map(|e| e.parameter).collect();
            assert!(regular.iter().all(|m| m.1 <= 60 || required.contains(&m.0)), "{name} learns everything by 60");
            let damaging = n.levelup.iter().filter(|m| mv[usize::from(m.0)].strength() > 0.0).count();
            assert!(damaging >= 4, "{name} has {damaging} damaging moves");
            let stab = n
                .levelup
                .iter()
                .filter(|m| mv[usize::from(m.0)].strength() > 0.0 && n.types.contains(&mv[usize::from(m.0)].type_id))
                .count();
            assert!(stab >= 2, "{name} has {stab} same-type attacks");
            for m in &n.levelup {
                assert!(!ohko.contains(&m.0) || v.levelup.contains(m), "{name} got a one-hit KO move");
            }
            let strengths: Vec<f64> = regular.iter().map(|m| mv[usize::from(m.0)].strength()).filter(|&s| s > 0.0).collect();
            assert!(strengths.windows(2).all(|w| w[0] <= w[1]), "{name} learns attacks weakest first: {strengths:?}");
            let has_parent = vanilla.iter().any(|p| p.present && p.evos.iter().any(|e| e.into == n.id && e.into != p.id));
            if has_parent {
                assert!(n.levelup.iter().any(|m| m.1 == ON_EVOLUTION), "{name} learns a move on evolution");
            }
            assert_eq!(n.egg.len(), v.egg.len(), "{name} keeps its egg move count");
            rows += 1;
        }
        println!("OK seed {}: {rows} learnsets checked", out.seed.0);
    }
    excerpt(&outs[0], "Moves & TMs", 8);
}

#[test]
fn random_tms_stay_consistent() {
    let Some(game) = scarlet("tms") else { return };
    let s = settings(&[
        ("moves.levelup", V::Is("random")),
        ("moves.tm_moves", V::On),
        ("moves.tm_good_damaging_percent", V::N(40)),
        ("moves.ban_game_breaking", V::On),
        ("moves.tm_compatibility", V::Is("prefer_type")),
        ("moves.tm_follow_evolutions", V::On),
        ("moves.tm_levelup_sanity", V::On),
        ("moves.tm_lp_cost", V::Is("reduced")),
        ("moves.tm_material_cost", V::Is("free")),
    ]);
    let outs = runs(&game, &s, &["81", "82"], true);
    let vanilla_machine = machine(&game, None);
    let vanilla = mons(&game, None);
    let ohko = move_ids(&game, GAME_BREAKING);
    for out in &outs {
        let new = mons(&game, Some(out));
        let mv = moves(&game, Some(out));
        let rows = machine(&game, Some(out));
        let items = item_moves(&game, Some(out));
        let tm_list = tm_moves_of(&rows);
        let tm_moves: BTreeSet<u16> = tm_list.iter().copied().collect();
        assert_eq!(tm_moves.len(), 229, "every TM teaches a different move");
        let good = tm_list.iter().filter(|&&m| mv[usize::from(m)].good()).count();
        assert!(good * 100 >= 40 * 229, "{good} good damaging TMs");
        for (t, old) in rows.iter().zip(&vanilla_machine) {
            assert_eq!(items[&t.0], t.1, "TM item and machine agree");
            assert!(mv[usize::from(t.1)].usable && !ohko.contains(&t.1), "TM move {} is usable and fair", t.1);
            assert_eq!(t.2, (old.2 + 1) / 2, "LP halved");
            assert!(t.3.iter().all(|m| *m == (0, 0)), "no materials");
        }
        assert_learnable_moves_usable(&game, &new, &mv, &tm_moves);
        assert_move_evolutions_possible(&game, &new, &mv, &tm_moves);
        let twin = twin_of(&vanilla, |m| (m.types, m.levelup.clone(), m.egg.clone(), m.reminder.clone(), m.tm.clone()));
        let mut gained = 0;
        for (n, _) in new.iter().zip(&vanilla).enumerate().filter(|(i, _)| playable(&game, *i)).map(|(_, p)| p) {
            for &(m, _) in &n.levelup {
                if tm_moves.contains(&m) {
                    assert!(n.tm.contains(&m), "{} learns {} by level-up, so it can use its TM", game.names.pokemon(n.id), m);
                }
            }
            gained += n.tm.iter().filter(|m| tm_moves.contains(m)).count();
        }
        let mut followed = 0;
        for (p, c, _) in vanilla_edges(&vanilla, &twin) {
            let parent_tms = new[p].tm.iter().filter(|m| tm_moves.contains(m));
            for m in parent_tms {
                assert!(new[c].tm.contains(m), "{} keeps {}'s TMs", game.names.pokemon(new[c].id), game.names.pokemon(new[p].id));
            }
            followed += 1;
        }
        println!("OK seed {}: 229 distinct TMs, {good} good attacks, tables agree, {gained} TM compatibilities, {followed} evolutions keep their TMs", out.seed.0);
    }
    excerpt(&outs[0], "Moves & TMs", 6);
    let text = outs[0].log.to_text();
    let tms_at = text.find("-- TMs --").unwrap();
    println!("{}", text[tms_at..].lines().take(5).collect::<Vec<_>>().join("\n"));
}

#[test]
fn unchanged_tm_compatibility_follows_the_tm_slot() {
    let Some(game) = scarlet("tm slots") else { return };
    let s = settings(&[("moves.tm_moves", V::On)]);
    let outs = runs(&game, &s, &["91", "92"], true);
    let vanilla = mons(&game, None);
    let old_moves = tm_moves_of(&machine(&game, None));
    for out in &outs {
        let new = mons(&game, Some(out));
        let new_moves = tm_moves_of(&machine(&game, Some(out)));
        for (n, v) in new.iter().zip(&vanilla).filter(|(n, _)| n.present) {
            for (old, now) in old_moves.iter().zip(&new_moves) {
                assert_eq!(v.tm.contains(old), n.tm.contains(now), "{} keeps TM slot compatibility", game.names.pokemon(n.id));
            }
            let extras: Vec<u16> = v.tm.iter().copied().filter(|m| !old_moves.contains(m) && !new_moves.contains(m)).collect();
            assert!(extras.iter().all(|m| n.tm.contains(m)), "moves no TM teaches stay listed");
        }
    }
    println!("OK every Pokémon can use the same TM numbers after the TMs changed");
}

#[test]
fn full_tm_compatibility() {
    let Some(game) = scarlet("full tms") else { return };
    let s = settings(&[("moves.tm_compatibility", V::Is("full")), ("moves.tm_lp_cost", V::Is("free"))]);
    let out = &runs(&game, &s, &["93", "94"], false)[0];
    let new = mons(&game, Some(out));
    let tm_moves = tm_moves_of(&machine(&game, Some(out)));
    for n in new.iter().filter(|n| n.present) {
        assert!(tm_moves.iter().all(|m| n.tm.contains(m)), "{} can use every TM", game.names.pokemon(n.id));
    }
    assert!(machine(&game, Some(out)).iter().all(|t| t.2 == 0), "crafting is free");
    println!("OK every Pokémon can use all 229 TMs and they cost no LP");
}

#[test]
fn metronome_only_mode() {
    let Some(game) = scarlet("metronome") else { return };
    let s = settings(&[("moves.metronome_only", V::On), ("moves.levelup", V::Is("random")), ("moves.tm_moves", V::On)]);
    let out = &runs(&game, &s, &["95", "96"], false)[0];
    excerpt(out, "Moves & TMs", 6);
    let new = mons(&game, Some(out));
    let mv = moves(&game, Some(out));
    let tm_moves: BTreeSet<u16> = tm_moves_of(&machine(&game, Some(out))).into_iter().collect();
    assert_eq!(tm_moves_of(&machine(&game, Some(out))), tm_moves_of(&machine(&game, None)), "TMs stay as they are");
    assert_eq!(mv[usize::from(METRONOME)].pp, 40);
    let mut extra = 0;
    for n in new.iter().filter(|n| n.present) {
        let name = game.names.pokemon(n.id);
        assert_eq!(n.levelup.first(), Some(&(METRONOME, 1)), "{name} learns Metronome");
        extra += n.levelup.len() - 1;
        assert!(n.egg.is_empty() && n.reminder.is_empty(), "{name} has no egg or reminder moves");
        assert!(!n.tm.iter().any(|m| tm_moves.contains(m)), "{name} can use no TM");
    }
    assert_move_evolutions_possible(&game, &new, &mv, &tm_moves);
    println!("OK every Pokémon knows only Metronome; {extra} moves kept so move-based evolutions still work");
}

#[test]
fn all_three_pages_together() {
    let Some(game) = scarlet("together") else { return };
    let mut values: Vec<(&str, V)> = rando::registry()
        .into_iter()
        .filter(|d| ["pokemon.", "evolutions.", "moves."].iter().any(|p| d.id.starts_with(p)))
        .filter(|d| matches!(d.kind, Kind::Toggle { .. }) && d.id != "moves.metronome_only")
        .map(|d| (d.id, V::On))
        .collect();
    values.extend([
        ("pokemon.stats", V::Is("random")),
        ("pokemon.types", V::Is("follow_evolutions")),
        ("pokemon.types_count", V::Is("extra")),
        ("pokemon.exp_curves", V::Is("strong_legendaries")),
        ("evolutions.mode", V::Is("random")),
        ("moves.levelup", V::Is("prefer_type")),
        ("moves.guaranteed_level1", V::N(2)),
        ("moves.good_damaging_percent", V::N(25)),
        ("moves.min_stab", V::N(1)),
        ("moves.min_damaging", V::N(2)),
        ("moves.tm_good_damaging_percent", V::N(30)),
        ("moves.tm_compatibility", V::Is("random")),
    ]);
    let outs = runs(&game, &settings(&values), &["101", "102", "103"], true);
    let vanilla = mons(&game, None);
    let index = index_of(&vanilla);
    for out in &outs {
        let new = mons(&game, Some(out));
        let mv = moves(&game, Some(out));
        let tm_moves: BTreeSet<u16> = tm_moves_of(&machine(&game, Some(out))).into_iter().collect();
        assert_learnable_moves_usable(&game, &new, &mv, &tm_moves);
        assert_move_evolutions_possible(&game, &new, &mv, &tm_moves);
        for (i, n) in new.iter().enumerate().filter(|(i, _)| playable(&game, *i)) {
            assert_eq!(n.bst(), vanilla[i].bst(), "{} keeps its total", game.names.pokemon(n.id));
            for e in &n.evos {
                let t = index[&e.into];
                assert!(playable(&game, t) && new[t].xp == n.xp, "{} evolves into a valid Pokémon", game.names.pokemon(n.id));
            }
            assert!(stages(&new, &index, i, 0) <= 3);
        }
        println!("OK seed {}: all three pages together validate and keep their invariants", out.seed.0);
    }
}
