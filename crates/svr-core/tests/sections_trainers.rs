use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::{load_table, ITEMS};
use svr_core::data::{Dex, Pokemon, TableStore};
use svr_core::fb::TableValue;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::rando::itempool::{ItemCatalog, ItemInfo, ItemKind, SHOP_OVERPOWERED};
use svr_core::rando::{self, Ctx, OptValue, RandoOutput, Seed, Settings};
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

fn pick<'a>(id: &'a str, value: &str) -> (&'a str, OptValue) {
    (id, OptValue::Text(value.into()))
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

/// Runs two seeds; a repeat of the first must be byte-identical and the second must differ.
fn two_seeds(game: &GameFiles, settings: &Settings) -> [RandoOutput; 2] {
    let a = run(game, settings, "economy-1");
    let again = run(game, settings, "economy-1");
    assert!(a.files == again.files, "the same seed wrote different files");
    let b = run(game, settings, "economy-2");
    assert!(a.files != b.files, "two seeds wrote the same files");
    [a, b]
}

fn only_writes(out: &RandoOutput, allowed: &[&str]) {
    for path in out.files.keys() {
        assert!(allowed.contains(&path.as_str()), "unexpected output file {path}");
    }
}

fn catalog(game: &GameFiles) -> ItemCatalog {
    let mut ctx = Ctx::new(game, Settings::default(), Seed("catalog".into())).unwrap();
    ItemCatalog::load(&mut ctx).unwrap()
}

fn int(t: &TableValue, field: &str) -> i64 {
    t.int(field).unwrap()
}

fn rows(t: &TableValue) -> Vec<&TableValue> {
    t.tables("values").collect()
}

// ---- items ----

const HIDDEN: [&str; 4] = [
    "world/data/item/hiddenItemDataTable/hiddenItemDataTable_array.bin",
    "world/data/item/hiddenItemDataTable_su1/hiddenItemDataTable_su1_array.bin",
    "world/data/item/hiddenItemDataTable_su2/hiddenItemDataTable_su2_array.bin",
    "world/data/item/hiddenItemDataTable_lc/hiddenItemDataTable_lc_array.bin",
];
/// The overworld Poké Ball scenes, Scarlet and Violet versions, which the field items option also writes.
const BALL_SCENES: [&str; 6] = [
    "world/scene/parts/field/streaming_event/world_item_/world_item_0.trscn",
    "world/scene/parts/field/streaming_event/world_item_/world_item_1.trscn",
    "world/scene/parts/field/streaming_event/su1_world_item_/su1_world_item_0.trscn",
    "world/scene/parts/field/streaming_event/su1_world_item_/su1_world_item_1.trscn",
    "world/scene/parts/field/streaming_event/su2_world_item_/su2_world_item_0.trscn",
    "world/scene/parts/field/streaming_event/su2_world_item_/su2_world_item_1.trscn",
];
const PICKUP: &str = "world/data/item/monohiroiItemData/monohiroiItemData_array.bin";
const LETS_GO: &str = "world/data/item/rummagingItemDataTable/rummagingItemDataTable_array.bin";
const DROPS: &str = "world/data/item/dropitemdata/dropitemdata_array.bin";
const TM_RECIPES: &str = "world/data/ui/shop/shop_wazamachine/shop_wazamachine_data/shop_wazamachine_data_array.bin";
const SHOPS: &str = "world/data/ui/shop/friendlyshop/friendlyshop_data/friendlyshop_data_array.bin";
const LINEUPS: &str = "world/data/ui/shop/friendlyshop/friendlyshop_lineup_data/friendlyshop_lineup_data_array.bin";
const AJITO: &str = "world/data/ajito/AjitoPokemon/AjitoPokemon_array.bin";

/// (itemId, emergePercent, dropCount) of every slot of every row, empty slots as zeros.
fn hidden_slots(t: &TableValue) -> Vec<(i64, i64, i64)> {
    let mut out = Vec::new();
    for row in rows(t) {
        for n in 1..=10 {
            out.push(row.table(&format!("item_{n}")).map_or((0, 0, 0), |s| {
                (int(s, "itemId"), int(s, "emergePercent"), int(s, "dropCount"))
            }));
        }
    }
    out
}

/// (itemId, rate) of every pickup slot.
fn pickup_slots(t: &TableValue) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for row in rows(t) {
        for n in 1..=30 {
            out.push(row.table(&format!("item_{n:02}")).map_or((0, 0), |s| (int(s, "itemId"), int(s, "rate"))));
        }
    }
    out
}

fn lets_go_slots(t: &TableValue) -> Vec<i64> {
    rows(t).iter().flat_map(|row| (0..5).map(|n| int(row, &format!("Item0{n}")))).collect()
}

fn is_material(catalog: &ItemCatalog, id: i64) -> bool {
    catalog.get(id as u16).is_some_and(|i| i.kind == ItemKind::Material)
}

fn sorted<T: Ord + Clone>(v: &[T]) -> Vec<T> {
    let mut v = v.to_vec();
    v.sort();
    v
}

#[test]
fn economy_defaults_write_none_of_these_tables() {
    let Some(game) = scarlet("economy defaults") else { return };
    let out = run(&game, &settings(&[]), "economy-defaults");
    let mine = [&HIDDEN[..], &BALL_SCENES[..], &[PICKUP, LETS_GO, DROPS, LINEUPS, ITEMS, AJITO]].concat();
    for path in &mine {
        assert!(!out.files.contains_key(*path), "defaults wrote {path}");
    }
    println!("OK defaults write none of the {} economy tables", mine.len());
}

#[test]
fn field_items_keep_odds_empty_slots_and_materials() {
    let Some(game) = scarlet("field items") else { return };
    let catalog = catalog(&game);
    let vanilla: Vec<Vec<(i64, i64, i64)>> = HIDDEN.iter().map(|p| hidden_slots(&load_table(&game, p).unwrap())).collect();
    let filled = |slots: &[Vec<(i64, i64, i64)>]| -> Vec<(i64, i64)> {
        slots
            .iter()
            .flatten()
            .filter(|s| s.0 != 0 && !is_material(&catalog, s.0))
            .map(|s| (s.0, s.2))
            .collect()
    };
    println!(
        "vanilla: {} slots, {} filled, {} material",
        vanilla.iter().flatten().count(),
        vanilla.iter().flatten().filter(|s| s.0 != 0).count(),
        vanilla.iter().flatten().filter(|s| is_material(&catalog, s.0)).count()
    );
    for (mode, bans) in [("shuffle", false), ("random", false), ("random", true), ("even", false)] {
        let mut overrides = vec![pick("items.field", mode)];
        if bans {
            overrides.extend([on("items.ban_bad"), on("items.ban_money")]);
        }
        let pool: BTreeSet<i64> = catalog.field(bans, bans).into_iter().map(i64::from).collect();
        for out in two_seeds(&game, &settings(&overrides)) {
            only_writes(&out, &[&HIDDEN[..], &BALL_SCENES[..]].concat());
            let output: Vec<Vec<(i64, i64, i64)>> =
                HIDDEN.iter().map(|p| hidden_slots(&output_table(&game, &out, p))).collect();
            let mut changed = 0;
            for (v, o) in vanilla.iter().flatten().zip(output.iter().flatten()) {
                assert_eq!(v.1, o.1, "emergePercent moved");
                if v.0 == 0 || is_material(&catalog, v.0) {
                    assert_eq!(v, o, "empty or material slot changed");
                    continue;
                }
                changed += usize::from(v != o);
                if mode != "shuffle" {
                    assert!(pool.contains(&o.0), "{} is not in the field pool", o.0);
                    let item = catalog.get(o.0 as u16).unwrap();
                    assert!(!bans || (!item.is_bad() && !item.is_big_money()), "banned {}", item.name);
                    if item.price > 1000 {
                        assert_eq!(o.2, 1, "{} stacked {}", item.name, o.2);
                    }
                }
            }
            if mode == "shuffle" {
                assert_eq!(sorted(&filled(&vanilla)), sorted(&filled(&output)), "shuffle changed the item multiset");
            }
            if mode == "even" {
                let mut counts: BTreeMap<i64, usize> = pool.iter().map(|&id| (id, 0)).collect();
                for s in filled(&output) {
                    *counts.get_mut(&s.0).unwrap() += 1;
                }
                let (min, max) = (counts.values().min().unwrap(), counts.values().max().unwrap());
                assert!(max - min <= 1, "even spread ranges {min}..{max}");
            }
            println!("OK field {mode} bans={bans}: {changed} slots changed, log rows {}", log_rows(&out, "Field items"));
            preview(&out, "Field items");
        }
    }
}

fn log_rows(out: &RandoOutput, section: &str) -> usize {
    out.log.sections.iter().filter(|s| s.title == section).flat_map(|s| &s.tables).map(|t| t.rows.len()).sum()
}

/// Prints a spoiler section's notes and the first rows of each of its tables.
fn preview(out: &RandoOutput, section: &str) {
    for s in out.log.sections.iter().filter(|s| s.title == section) {
        println!("  == {} == {}", s.title, s.notes.join(" | "));
        for t in &s.tables {
            for row in t.rows.iter().take(3) {
                println!("  {}: {}", t.title, row.join(" | "));
            }
        }
    }
}

#[test]
fn pickup_and_lets_go_keep_rates() {
    let Some(game) = scarlet("pickup") else { return };
    let catalog = catalog(&game);
    let vanilla_pickup = pickup_slots(&load_table(&game, PICKUP).unwrap());
    let vanilla_lets_go = lets_go_slots(&load_table(&game, LETS_GO).unwrap());
    let pool: BTreeSet<i64> = catalog.field(false, false).into_iter().map(i64::from).collect();
    for mode in ["shuffle", "random", "even"] {
        let s = settings(&[pick("items.pickup", mode), pick("items.lets_go", mode)]);
        for out in two_seeds(&game, &s) {
            only_writes(&out, &[PICKUP, LETS_GO]);
            let pickup = pickup_slots(&output_table(&game, &out, PICKUP));
            let lets_go = lets_go_slots(&output_table(&game, &out, LETS_GO));
            for (v, o) in vanilla_pickup.iter().zip(&pickup) {
                assert_eq!(v.1, o.1, "pickup rate moved");
                assert_eq!(v.0 == 0, o.0 == 0, "pickup emptiness changed");
            }
            for (v, o) in vanilla_lets_go.iter().zip(&lets_go) {
                assert_eq!(*v == 0, *o == 0, "Let's Go emptiness changed");
            }
            let items = |p: &[(i64, i64)], l: &[i64]| -> Vec<i64> {
                p.iter().map(|s| s.0).chain(l.iter().copied()).filter(|&i| i != 0).collect()
            };
            let (before, after) = (items(&vanilla_pickup, &vanilla_lets_go), items(&pickup, &lets_go));
            if mode == "shuffle" {
                assert_eq!(sorted(&before), sorted(&after));
            } else {
                assert!(after.iter().all(|i| pool.contains(i)), "item outside the field pool");
            }
            let changed = before.iter().zip(&after).filter(|(a, b)| a != b).count();
            println!("OK pickup + Let's Go {mode}: {changed} of {} slots changed", before.len());
            preview(&out, "Pickup items");
            preview(&out, "Let's Go items");
        }
    }
}

/// species -> material for each drop row.
fn drop_rows(t: &TableValue) -> Vec<(u16, i64, i64, i64)> {
    rows(t)
        .iter()
        .map(|row| {
            let item = row.table("item1").unwrap();
            (int(row, "devid") as u16, int(item, "itemid"), int(item, "rate"), int(item, "num"))
        })
        .collect()
}

#[test]
fn drops_keep_every_tm_craftable() {
    let Some(game) = scarlet("drops") else { return };
    let catalog = catalog(&game);
    let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
    let playable: BTreeSet<u16> = dex.playable().map(|f| f.id.species).collect();
    let farmable: BTreeSet<u16> = dex.playable().filter(|f| !f.is_legendary_class()).map(|f| f.id.species).collect();
    let vanilla = drop_rows(&load_table(&game, DROPS).unwrap());
    let materials: BTreeSet<i64> = vanilla.iter().map(|r| r.1).filter(|&i| is_material(&catalog, i)).collect();
    let recipes = load_table(&game, TM_RECIPES).unwrap();
    let needed: BTreeSet<i64> = rows(&recipes)
        .iter()
        .flat_map(|r| ["Item01", "Item02", "Item03"].map(|f| int(r, f)))
        .filter(|&i| i != 0)
        .collect();
    assert!(needed.is_subset(&materials), "a recipe needs a material no Pokémon drops");
    let covered = |drops: &[(u16, i64, i64, i64)], carriers: &BTreeSet<u16>| -> usize {
        let dropped: BTreeSet<i64> = drops.iter().filter(|r| carriers.contains(&r.0)).map(|r| r.1).collect();
        needed.iter().filter(|m| dropped.contains(m)).count()
    };
    println!(
        "vanilla: {} rows, {} materials, {} recipe materials; covered by playable {}, by non-legendary playable {}",
        vanilla.len(),
        materials.len(),
        needed.len(),
        covered(&vanilla, &playable),
        covered(&vanilla, &farmable)
    );
    for mode in ["shuffle", "random"] {
        for out in two_seeds(&game, &settings(&[pick("items.drops", mode)])) {
            only_writes(&out, &[DROPS]);
            let output = drop_rows(&output_table(&game, &out, DROPS));
            let mut changed = 0;
            for (v, o) in vanilla.iter().zip(&output) {
                assert_eq!((v.0, v.2, v.3), (o.0, o.2, o.3), "species, rate or count moved");
                if !playable.contains(&v.0) || !materials.contains(&v.1) {
                    assert_eq!(v, o, "an unplayable species' or non-material drop changed");
                }
                assert_eq!(materials.contains(&v.1), materials.contains(&o.1), "{} now drops {}", v.0, o.1);
                changed += usize::from(v.1 != o.1);
            }
            assert_eq!(covered(&output, &playable), needed.len(), "a recipe material lost every playable source");
            assert_eq!(covered(&output, &farmable), needed.len(), "a recipe material is only on legendaries");
            if mode == "shuffle" {
                let multiset = |d: &[(u16, i64, i64, i64)]| sorted(&d.iter().map(|r| r.1).collect::<Vec<_>>());
                println!("shuffle kept the drop multiset: {}", multiset(&vanilla) == multiset(&output));
            }
            println!("OK drops {mode}: {changed} rows changed, every TM craftable");
            preview(&out, "Pokémon drops");
        }
    }
}

// ---- shops ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Goods,
    Tm,
    Picnic,
}

fn family(catalog: &ItemCatalog, id: i64) -> Family {
    match catalog.get(id as u16).map(|i| i.kind) {
        Some(ItemKind::Tm) => Family::Tm,
        Some(ItemKind::Picnic) => Family::Picnic,
        _ => Family::Goods,
    }
}

struct LineupRow {
    lineup: String,
    sortnum: i64,
    item: i64,
    gate: (i64, String, i64),
}

fn lineup_rows(t: &TableValue) -> Vec<LineupRow> {
    rows(t)
        .iter()
        .map(|r| LineupRow {
            lineup: r.string("lineupid").unwrap_or_default().to_owned(),
            sortnum: int(r, "sortnum"),
            item: int(r, "item"),
            gate: (
                int(r, "item_condkind"),
                r.string("item_condvalue").unwrap_or_default().to_owned(),
                int(r, "gym_badge_num"),
            ),
        })
        .collect()
}

/// The price field each lineup charges in.
fn price_fields(game: &GameFiles) -> BTreeMap<String, Vec<&'static str>> {
    let mut out: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for shop in rows(&load_table(game, SHOPS).unwrap()) {
        let field = if int(shop, "payKind") == 2 { "BP" } else { "Price" };
        out.entry(shop.string("lineup").unwrap().to_owned()).or_default().push(field);
    }
    out
}

fn prices(t: &TableValue) -> BTreeMap<i64, (i64, i64)> {
    rows(t).iter().map(|r| (int(r, "Id"), (int(r, "Price"), int(r, "BP")))).collect()
}

fn guaranteed(catalog: &ItemCatalog, test: fn(&ItemInfo) -> bool) -> Vec<&ItemInfo> {
    catalog
        .all()
        .iter()
        .filter(|i| {
            i.named()
                && !matches!(i.kind, ItemKind::Key | ItemKind::Tm | ItemKind::Material | ItemKind::Picnic)
                && !i.is_form_or_species_item()
                && test(i)
        })
        .collect()
}

#[test]
fn shop_stock_respects_families_prices_and_guarantees() {
    let Some(game) = scarlet("shops") else { return };
    let catalog = catalog(&game);
    let vanilla = lineup_rows(&load_table(&game, LINEUPS).unwrap());
    let vanilla_prices = prices(&load_table(&game, ITEMS).unwrap());
    let fields = price_fields(&game);
    let regular: BTreeSet<i64> = vanilla.iter().filter(|r| r.lineup == "shop_00_lineup").map(|r| r.item).collect();
    let evolution = guaranteed(&catalog, ItemInfo::is_evolution);
    let x_items = guaranteed(&catalog, ItemInfo::is_x_item);
    println!("vanilla: {} slots, {} evolution items, {} X items", vanilla.len(), evolution.len(), x_items.len());

    let configs: Vec<(&str, Vec<(&str, OptValue)>)> = vec![
        ("shuffle", vec![pick("shops.stock", "shuffle")]),
        (
            "random with bans",
            vec![pick("shops.stock", "random"), on("shops.ban_op"), on("shops.ban_bad"), on("shops.ban_regular")],
        ),
        (
            "random balanced with guarantees",
            vec![
                pick("shops.stock", "random"),
                on("shops.balance_prices"),
                on("shops.guarantee_evolution"),
                on("shops.guarantee_x"),
            ],
        ),
        (
            "shuffle with guarantees, badges removed",
            vec![
                pick("shops.stock", "shuffle"),
                on("shops.guarantee_evolution"),
                on("shops.guarantee_x"),
                pick("shops.badge_gating", "remove"),
            ],
        ),
        ("random counter", vec![pick("shops.stock", "random"), pick("shops.pokemart", "random")]),
        ("shuffle, 16 counter extras", vec![pick("shops.stock", "shuffle"), ("shops.pokemart_extras", OptValue::Int(16))]),
        ("random, 4 counter extras per badge", vec![pick("shops.stock", "random"), ("shops.pokemart_extras_per_badge", OptValue::Int(4))]),
        ("random with repeats allowed", vec![pick("shops.stock", "random"), ("shops.no_repeats", OptValue::Bool(false))]),
        ("random with random food", vec![pick("shops.stock", "random"), pick("shops.food", "random")]),
    ];
    for (name, overrides) in configs {
        let has = |id: &str| overrides.iter().any(|(o, v)| *o == id && *v == OptValue::Bool(true));
        let balance = has("shops.balance_prices");
        let bans = has("shops.ban_op");
        let removed = overrides.iter().any(|(o, _)| *o == "shops.badge_gating");
        let keep = !overrides.iter().any(|(o, v)| *o == "shops.pokemart" && *v == OptValue::Text("random".into()));
        let number = |id: &str, default: i64| match overrides.iter().find(|(o, _)| *o == id) {
            Some((_, OptValue::Int(n))) => *n,
            _ => default,
        };
        let (start, per_badge) = (number("shops.pokemart_extras", 8), number("shops.pokemart_extras_per_badge", 2));
        let extras = if keep { (start + per_badge * 8) as usize } else { 0 };
        let random = overrides.iter().any(|(o, v)| *o == "shops.stock" && *v == OptValue::Text("random".into()));
        let no_repeats = random && !overrides.iter().any(|(o, v)| *o == "shops.no_repeats" && *v == OptValue::Bool(false));
        let food = overrides.iter().any(|(o, v)| *o == "shops.food" && *v == OptValue::Text("random".into()));
        let mut food_changed = 0;
        for out in two_seeds(&game, &settings(&overrides)) {
            only_writes(&out, if balance { &[LINEUPS, ITEMS] } else { &[LINEUPS] });
            let output = lineup_rows(&output_table(&game, &out, LINEUPS));
            let output_prices = prices(&output_table(&game, &out, ITEMS));
            assert_eq!(output.len(), vanilla.len() + extras, "{extras} extras");
            let by_key: BTreeMap<(&str, i64), &LineupRow> = output.iter().map(|o| ((o.lineup.as_str(), o.sortnum), o)).collect();
            let vanilla_keys: BTreeSet<(&str, i64)> = vanilla.iter().map(|v| (v.lineup.as_str(), v.sortnum)).collect();
            let mut per_lineup: BTreeMap<&str, BTreeSet<i64>> = BTreeMap::new();
            for o in &output {
                assert!(per_lineup.entry(&o.lineup).or_default().insert(o.item), "{} sells {} twice", o.lineup, o.item);
                for &field in fields.get(&o.lineup).map_or(&["Price"][..], Vec::as_slice) {
                    let price = |p: &BTreeMap<i64, (i64, i64)>| p.get(&o.item).map_or(0, |&(m, bp)| if field == "BP" { bp } else { m });
                    assert!(price(&output_prices) > 0, "{} sells {} for free ({field})", o.lineup, o.item);
                    assert!(balance || price(&vanilla_prices) > 0, "unbalanced run sells unpriced {}", o.item);
                }
            }
            let counter: Vec<usize> = (0..output.len()).filter(|&i| output[i].lineup == "shop_00_lineup").collect();
            assert_eq!(counter.last().unwrap() - counter[0] + 1, counter.len(), "the counter's rows stay together");
            let added: Vec<&LineupRow> =
                output.iter().filter(|o| !vanilla_keys.contains(&(o.lineup.as_str(), o.sortnum))).collect();
            assert_eq!(added.len(), extras);
            for o in &added {
                assert_eq!(o.lineup, "shop_00_lineup");
                assert_eq!(family(&catalog, o.item), Family::Goods);
                let k = o.sortnum - 26;
                let badges = if k < start { 0 } else { 1 + (k - start) / per_badge };
                let expected = if removed || badges == 0 { (0, String::new(), 0) } else { (3, format!("BADGE{badges}"), badges) };
                assert_eq!(o.gate, expected, "extra {} gate", o.sortnum);
            }
            let mut sellers: BTreeMap<i64, BTreeSet<&str>> = BTreeMap::new();
            for o in &output {
                let standard = keep && o.lineup == "shop_00_lineup" && vanilla_keys.contains(&(o.lineup.as_str(), o.sortnum));
                if !standard && family(&catalog, o.item) == Family::Goods {
                    sellers.entry(o.item).or_default().insert(&o.lineup);
                }
            }
            let shared = sellers.values().filter(|l| l.len() > 1).count();
            let noted = out.log.sections.iter().any(|s| s.notes.iter().any(|n| n.contains("repeat an item another kind of shop sells")));
            let slots: usize = sellers.values().map(|l| l.len()).sum();
            if no_repeats {
                assert!(sellers.len() * 4 >= slots * 3, "{name}: only {} distinct goods items over {slots} slots", sellers.len());
                assert!(shared == 0 || noted, "{name}: repeats without a spoiler note");
            }
            println!("   {name}: {} distinct goods items over {slots} slots, {shared} shared", sellers.len());
            let mut changed = 0;
            for v in &vanilla {
                let o = by_key[&(v.lineup.as_str(), v.sortnum)];
                let kept = keep && v.lineup == "shop_00_lineup";
                if kept {
                    assert_eq!(v.item, o.item, "the counter's standard item {} moved", v.sortnum);
                }
                if family(&catalog, v.item) == Family::Picnic {
                    if food {
                        food_changed += usize::from(v.item != o.item);
                    } else {
                        assert_eq!(v.item, o.item, "{} slot {}: food changed while it should stay", v.lineup, v.sortnum);
                    }
                }
                assert_eq!(family(&catalog, v.item), family(&catalog, o.item), "{} slot {} left its family", o.lineup, o.sortnum);
                if bans && !kept && family(&catalog, o.item) == Family::Goods {
                    let item = catalog.get(o.item as u16).unwrap();
                    assert!(!item.is_shop_overpowered() && !item.is_bad() && !regular.contains(&o.item), "banned {}", item.name);
                }
                if removed {
                    assert_ne!(o.gate.0, 3, "a badge gate survived");
                    if v.gate.0 != 3 {
                        assert_eq!(v.gate, o.gate);
                    }
                } else {
                    assert_eq!(v.gate, o.gate, "gating changed");
                }
                changed += usize::from(v.item != o.item);
            }
            let sold_goods: BTreeSet<i64> =
                output.iter().filter(|r| family(&catalog, r.item) == Family::Goods).map(|r| r.item).collect();
            if has("shops.guarantee_evolution") {
                for item in &evolution {
                    assert!(sold_goods.contains(&i64::from(item.id)), "{} is not sold", item.name);
                }
                for item in &x_items {
                    assert!(sold_goods.contains(&i64::from(item.id)), "{} is not sold", item.name);
                }
            }
            assert!(!food || food_changed > 0, "random food changed nothing");
            let repriced = output_prices.iter().filter(|(id, p)| vanilla_prices.get(id) != Some(p)).count();
            println!("OK shops {name}: {changed} slots changed, {repriced} items repriced");
            preview(&out, "Shops");
        }
    }
}

#[test]
fn badge_gating_alone_only_touches_conditions() {
    let Some(game) = scarlet("badge gating") else { return };
    let vanilla = lineup_rows(&load_table(&game, LINEUPS).unwrap());
    let out = run(&game, &settings(&[pick("shops.badge_gating", "remove")]), "economy-badges");
    only_writes(&out, &[LINEUPS]);
    let output = lineup_rows(&output_table(&game, &out, LINEUPS));
    let mut cleared = 0;
    for (v, o) in vanilla.iter().zip(&output) {
        assert_eq!((&v.lineup, v.sortnum, v.item), (&o.lineup, o.sortnum, o.item));
        if v.gate.0 == 3 {
            assert_eq!(o.gate, (0, String::new(), 0));
            cleared += 1;
        } else {
            assert_eq!(v.gate, o.gate);
        }
    }
    println!("OK {cleared} badge gates removed, nothing else changed");
    preview(&out, "Shops");
}

#[test]
fn curated_item_lists_match_the_game_text() {
    let Some(game) = scarlet("item lists") else { return };
    let names = GameText::load(&game, Language::English, "itemname").unwrap();
    let curated = svr_core::rando::itempool::curated_names();
    for &(id, name) in &curated {
        assert_eq!(names.line(usize::from(id)).unwrap_or_default(), name, "item {id}");
    }
    assert!(SHOP_OVERPOWERED.iter().all(|entry| curated.contains(entry)));
    println!("OK {} curated item ids match their English names", curated.len());
}

// ---- Team Star ----

fn ajito_rows(t: &TableValue) -> Vec<(i64, Pokemon)> {
    rows(t)
        .iter()
        .map(|r| {
            let e = r.table("ajitoPokemonTable").unwrap();
            (int(e, "pokemonId"), Pokemon { species: int(e, "devId") as u16, form: int(e, "formNo") as u16 })
        })
        .collect()
}

#[test]
fn team_star_bases_use_playable_crew_typed_species() {
    let Some(game) = scarlet("team star") else { return };
    let dex = Dex::load(&mut TableStore::new(&game)).unwrap();
    let vanilla = ajito_rows(&load_table(&game, AJITO).unwrap());
    let mut type_counts: BTreeMap<i64, BTreeMap<u8, usize>> = BTreeMap::new();
    for &(id, p) in vanilla.iter().filter(|r| r.0 > 0) {
        let types: BTreeSet<u8> = dex.get(p).unwrap().types.into_iter().collect();
        for t in types {
            *type_counts.entry(id / 100).or_default().entry(t).or_default() += 1;
        }
    }
    let crew_type: BTreeMap<i64, u8> = type_counts
        .iter()
        .map(|(&crew, counts)| (crew, *counts.iter().max_by_key(|(t, n)| (**n, std::cmp::Reverse(**t))).unwrap().0))
        .collect();
    println!("crew types {crew_type:?}");
    for overrides in [
        vec![pick("extras.team_star", "random")],
        vec![pick("extras.team_star", "crew_type")],
        vec![pick("extras.team_star", "crew_type"), on("extras.team_star.similar")],
    ] {
        let typed = overrides[0].1 == OptValue::Text("crew_type".into());
        for out in two_seeds(&game, &settings(&overrides)) {
            only_writes(&out, &[AJITO]);
            let output = ajito_rows(&output_table(&game, &out, AJITO));
            let mut changed = 0;
            for (v, o) in vanilla.iter().zip(&output) {
                assert_eq!(v.0, o.0, "pokemonId moved");
                if v.0 == 0 {
                    assert_eq!(v, o, "the placeholder changed");
                    continue;
                }
                let info = dex.get(o.1).unwrap_or_else(|| panic!("{:?} is not in the dex", o.1));
                assert!(info.playable(), "{:?} is not playable", o.1);
                if typed {
                    assert!(info.types.contains(&crew_type[&(o.0 / 100)]), "{:?} misses its crew's type", o.1);
                }
                changed += usize::from(v != o);
            }
            println!("OK team star {:?}: {changed} of {} changed", overrides.iter().map(|o| o.0).collect::<Vec<_>>(), output.len() - 1);
            preview(&out, "Team Star bases");
        }
    }
}

#[test]
fn every_economy_option_together_passes_validation() {
    let Some(game) = scarlet("economy together") else { return };
    let s = settings(&[
        pick("items.field", "random"),
        pick("items.pickup", "even"),
        pick("items.lets_go", "shuffle"),
        pick("items.drops", "random"),
        on("items.ban_bad"),
        on("items.ban_money"),
        pick("shops.stock", "random"),
        on("shops.ban_op"),
        on("shops.balance_prices"),
        on("shops.guarantee_evolution"),
        on("shops.guarantee_x"),
        pick("shops.badge_gating", "remove"),
        pick("extras.team_star", "crew_type"),
        on("extras.team_star.similar"),
    ]);
    let [out, _] = two_seeds(&game, &s);
    let mut expected = [&HIDDEN[..], &BALL_SCENES[..], &[PICKUP, LETS_GO, DROPS, LINEUPS, ITEMS, AJITO]].concat();
    expected.sort_unstable();
    let written: Vec<&str> = out.files.keys().map(String::as_str).collect();
    assert_eq!(written, expected);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    println!("OK all economy options: {} files, spoiler {} lines", written.len(), out.log.to_text().lines().count());
}
// ---- trainers ----

const TRDATA: &str = "world/data/trainer/trdata/trdata_array.bin";
const TRTYPE: &str = "world/data/trainer/trtype/trtype_array.bin";
const WONDER_GUARD: u16 = 25;

#[derive(Clone, Debug, PartialEq)]
struct TrMon {
    species: Pokemon,
    level: i64,
    item: i64,
    tokusei: i64,
    manual: bool,
    moves: Vec<i64>,
    talent: (i64, i64),
    rare: i64,
}

#[derive(Clone, Debug, PartialEq)]
struct TrRow {
    trid: String,
    label: String,
    class: String,
    doubles: bool,
    ai_double: bool,
    change_gem: bool,
    smart_ai: bool,
    team: Vec<TrMon>,
}

fn trainer_rows(t: &TableValue) -> Vec<TrRow> {
    rows(t)
        .into_iter()
        .map(|r| {
            let team = (1..=6)
                .filter_map(|k| r.table(&format!("poke{k}")))
                .filter(|p| int(p, "devId") != 0)
                .map(|p| TrMon {
                    species: Pokemon {
                        species: int(p, "devId") as u16,
                        form: int(p, "formId") as u16,
                    },
                    level: int(p, "level"),
                    item: int(p, "item"),
                    tokusei: int(p, "tokusei"),
                    manual: int(p, "wazaType") == 1,
                    moves: (1..=4).map(|m| p.table(&format!("waza{m}")).map_or(0, |w| int(w, "wazaId"))).collect(),
                    talent: (int(p, "talentType"), p.table("talentValue").map_or(0, |t| int(t, "spDef"))),
                    rare: int(p, "rareType"),
                })
                .collect();
            TrRow {
                trid: r.string("trid").unwrap_or("").to_owned(),
                label: r.string("trNameLabel").unwrap_or("").to_owned(),
                class: r.string("trainerType").unwrap_or("").to_owned(),
                doubles: int(r, "battleType") == 1,
                ai_double: r.bool("aiDouble").unwrap(),
                change_gem: r.bool("changeGem").unwrap(),
                smart_ai: ["aiBasic", "aiHigh", "aiExpert", "aiItem", "aiChange"].iter().all(|f| r.bool(f).unwrap()),
                team,
            }
        })
        .collect()
}

fn is_test_row(trid: &str) -> bool {
    trid.is_empty() || trid.starts_with("00_test") || trid.starts_with("test")
}

fn dex(game: &GameFiles) -> Dex {
    Dex::load(&mut TableStore::new(game)).unwrap()
}

/// Multi and partner battles: none of their trainers Terastallizes in the game.
fn multi(trid: &str) -> bool {
    const EXACT: &[&str] = &[
        "botan_multi",
        "pepper_multi",
        "pepper_02_01",
        "brother_kodaigame",
        "s2_side_brother",
        "sister_muruchi_01",
        "sister_muruchi_01_strong",
        "sister_onitaizi",
        "sister_onitaizi_strong",
        "sister_02_02",
        "s2_side_grandfather",
        "s2_side_grandmother",
        "s2_side_villager01",
        "s2_side_villager02",
    ];
    EXACT.contains(&trid) || ["pepper_nusi_", "rival_multi_"].iter().any(|p| trid.starts_with(p))
}

/// The reference's protective cases, written out independently of the section's own table.
fn fixed_battle(trid: &str) -> bool {
    const EXACT: &[&str] = &[
        "botan_multi",
        "pepper_multi",
        "pepper_02_01",
        "brother_kodaigame",
        "s2_side_brother",
        "sister_muruchi_01",
        "sister_muruchi_01_strong",
        "sister_onitaizi",
        "sister_onitaizi_strong",
        "sister_02_02",
        "s2_side_grandfather",
        "s2_side_grandmother",
        "s2_side_villager01",
        "s2_side_villager02",
        "sister_01_01",
        "sister_01_01_strong",
        "professor_A_02",
        "professor_B_02",
        "brother_02_02",
    ];
    EXACT.contains(&trid)
        || ["pepper_nusi_", "rival_multi_", "rival_01_", "brother_01_", "raid_assist_"].iter().any(|p| trid.starts_with(p))
        || (trid.starts_with("dan_") && trid.ends_with("_boss_01"))
}

fn max_team(trid: &str) -> usize {
    if ["rival_01_hono", "rival_01_kusa", "rival_01_mizu", "sister_02_02"].contains(&trid) {
        1
    } else if trid.starts_with("dan_") && trid.ends_with("_boss_01") {
        5
    } else {
        6
    }
}

fn fixed_team(trid: &str) -> bool {
    trid.starts_with("raid_assist_")
        || trid.starts_with("pepper_nusi_")
        || ["professor_A_02", "professor_B_02", "brother_02_02"].contains(&trid)
}

fn no_tera(trid: &str) -> bool {
    max_team(trid) == 1 || fixed_team(trid) || multi(trid)
}

fn abilities_of(dex: &Dex, mon: &TrMon) -> Vec<u16> {
    let f = dex.get(mon.species).unwrap();
    match mon.tokusei {
        0 => vec![f.abilities[0], f.abilities[1]],
        2..=4 => vec![f.abilities[(mon.tokusei - 2) as usize]],
        _ => f.abilities.to_vec(),
    }
}

/// Which vanilla starter line a per-starter row's opponent takes: Nemona takes the starter weak to
/// the player's, Clavell the one strong against it. Lines are 0 grass, 1 fire, 2 water.
fn expected_line(trid: &str) -> Option<usize> {
    let player = [("kusa", 0), ("honoo", 1), ("hono", 1), ("mizu", 2)]
        .iter()
        .find(|(s, _)| trid.ends_with(s))
        .map(|&(_, p)| p)?;
    if trid.starts_with("rival_") || trid.starts_with("su2_bukatu_nemo") {
        Some((player + 2) % 3)
    } else if trid.starts_with("clavel_") || trid.starts_with("su2_bukatu_claver") {
        Some((player + 1) % 3)
    } else {
        None
    }
}

fn starter_lines(dex: &Dex) -> [BTreeSet<Pokemon>; 3] {
    [906u16, 909, 912].map(|n| {
        let base = dex.all().iter().find(|f| f.national == n && f.id.form == 0).unwrap().id;
        dex.line_from(base)
    })
}

/// Slot index and line of each vanilla starter on a row.
fn starter_slots(lines: &[BTreeSet<Pokemon>; 3], row: &TrRow) -> Vec<(usize, usize)> {
    row.team
        .iter()
        .enumerate()
        .filter_map(|(k, m)| lines.iter().position(|l| l.contains(&m.species)).map(|line| (k, line)))
        .collect()
}

#[test]
fn trainer_vanilla_layout_matches_the_assumptions() {
    let Some(game) = scarlet("trainer layout") else { return };
    let dex = dex(&game);
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    assert_eq!(vanilla.len(), 756);
    let lines = starter_lines(&dex);
    let mut checked = 0;
    for row in vanilla.iter().filter(|r| !is_test_row(&r.trid)) {
        assert!(!row.team.is_empty(), "{} has no Pokémon", row.trid);
        if row.doubles {
            assert!(row.team.len() >= 2 && row.ai_double, "{} is a double battle without two Pokémon", row.trid);
        }
        if let Some(line) = expected_line(&row.trid) {
            let slots = starter_slots(&lines, row);
            assert_eq!(slots.len(), 1, "{} should carry one starter", row.trid);
            assert_eq!(slots[0].1, line, "{} carries the wrong starter line", row.trid);
            checked += 1;
        }
    }
    println!("OK vanilla trainers: {} rows, {checked} per-starter rows follow the counter-pick rule", vanilla.len());
    assert!(checked >= 40, "only {checked} per-starter rows");
}

#[test]
fn trainer_defaults_change_nothing() {
    let Some(game) = scarlet("trainer defaults") else { return };
    let out = run(&game, &settings(&[]), "defaults");
    for path in out.files.keys() {
        assert!(!path.contains("trainer/") && !path.contains("trname") && !path.contains("trtype"), "defaults wrote {path}");
    }
    let alone = run(&game, &settings(&[on("trainers.rival_starter")]), "rival-only");
    assert!(!alone.files.contains_key(TRDATA), "carrying the vanilla starters changed the vanilla rows");
    println!("OK defaults and rival-carry with vanilla starters write no trainer file");
}

fn chaos() -> Settings {
    settings(&[
        pick("trainers.mode", "random"),
        on("trainers.similar_strength"),
        on("trainers.alt_forms"),
        pick("trainers.type_theme", "all"),
        on("trainers.weight_types"),
        on("trainers.no_early_wonder_guard"),
        ("trainers.league_unique", OptValue::Int(3)),
        on("trainers.force_evolved"),
        ("trainers.force_evolved_level", OptValue::Int(45)),
        ("trainers.level_modifier", OptValue::Int(20)),
        on("trainers.rival_starter"),
        on("trainers.better_movesets"),
        on("trainers.random_abilities"),
        ("trainers.shiny_percent", OptValue::Int(30)),
        on("trainers.random_tera"),
        ("trainers.route.extra", OptValue::Int(2)),
        ("trainers.rivals.extra", OptValue::Int(5)),
        pick("trainers.doubles", "random"),
        on("trainers.items_sensible"),
        on("trainers.items_ban_bad"),
        on("trainers.same_as_rivals"),
        on("trainers.rivals.items"),
        on("trainers.rivals.tera"),
        on("trainers.rivals.perfect_ivs"),
        on("trainers.rivals.smart_ai"),
    ])
}

fn check_protections(game: &GameFiles, dex: &Dex, vanilla: &[TrRow], out: &RandoOutput) -> usize {
    let new = trainer_rows(&output_table(game, out, TRDATA));
    let lines = starter_lines(dex);
    let mut changed = 0;
    for (old, row) in vanilla.iter().zip(&new) {
        if is_test_row(&old.trid) {
            assert_eq!(old, row, "test row {} changed", old.trid);
            continue;
        }
        if old != row {
            changed += 1;
        }
        let id = &row.trid;
        assert!(row.team.len() >= old.team.len() && row.team.len() <= max_team(id).max(old.team.len()), "{id} team size {}", row.team.len());
        if row.doubles {
            assert!(row.team.len() >= 2 && row.ai_double, "{id} is a double battle with {} Pokémon", row.team.len());
        }
        if fixed_battle(id) {
            assert_eq!((row.doubles, row.ai_double), (old.doubles, old.ai_double), "{id} battle type changed");
        }
        if fixed_team(id) {
            assert_eq!(row.team.len(), old.team.len(), "{id} team size changed");
        }
        if no_tera(id) {
            assert_eq!(row.change_gem, old.change_gem, "{id} Tera changed");
        }
        if ["professor_A_02", "professor_B_02", "brother_02_02"].contains(&id.as_str()) {
            for mon in &row.team {
                assert!(!abilities_of(dex, mon).contains(&WONDER_GUARD), "{id} can have Wonder Guard");
            }
        }
        if id.starts_with("professor_") && id.ends_with("_02") {
            assert_eq!(
                row.team.iter().map(|m| m.species).collect::<Vec<_>>(),
                old.team.iter().map(|m| m.species).collect::<Vec<_>>(),
                "{id}: the Koraidon/Miraidon fight changed"
            );
        }
        for (k, mon) in row.team.iter().enumerate() {
            let form = dex.get(mon.species).unwrap_or_else(|| panic!("{id} has unknown {:?}", mon.species));
            let placed = old.team.get(k).is_none_or(|o| o.species != mon.species);
            assert!(form.playable() || !placed, "{id} got unplayable {:?}", mon.species);
            if let Some(item) = svr_core::rando::itempool::form_item(form.national, form.id.form) {
                assert_eq!(mon.item, i64::from(item), "{id} slot {k}: form without its item");
            }
            if mon.level < 20 && placed {
                assert!(!form.abilities.contains(&WONDER_GUARD), "{id} has early Wonder Guard");
            }
        }
        if let Some(line) = expected_line(id) {
            for (k, vanilla_line) in starter_slots(&lines, old) {
                assert_eq!(vanilla_line, line);
                assert!(lines[line].contains(&row.team[k].species), "{id} slot {k} no longer carries the starter");
            }
        }
    }
    changed
}

/// Moves each species row can know at a level: level-up up to it, TM, egg and reminder moves.
fn learnable(store: &mut TableStore, dex: &Dex, p: Pokemon, level: i64) -> BTreeSet<i64> {
    let personal = store.get(svr_core::data::store::PERSONAL).unwrap();
    let entry = personal.tables("entry").nth(dex.get(p).unwrap().row).unwrap();
    let mut out = BTreeSet::new();
    for field in ["tm_moves", "egg_moves", "reminder_moves"] {
        out.extend(entry.vector(field).unwrap_or(&[]).iter().filter_map(|v| v.as_int()));
    }
    for m in entry.vector("levelup_moves").unwrap_or(&[]).iter().filter_map(|v| v.as_struct()) {
        if m.int("level").unwrap() <= level {
            out.insert(m.int("move").unwrap());
        }
    }
    out
}

#[test]
fn random_trainers_keep_every_protection() {
    let Some(game) = scarlet("trainer chaos") else { return };
    let dex = dex(&game);
    let mut store = TableStore::new(&game);
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    let outs = two_seeds(&game, &chaos());
    for out in &outs {
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        let changed = check_protections(&game, &dex, &vanilla, out);
        let new = trainer_rows(&output_table(&game, out, TRDATA));
        let boosters = svr_core::rando::itempool::TYPE_BOOSTERS;
        let bad = svr_core::rando::itempool::BAD_ITEMS;
        let mut items = 0;
        for (old, row) in vanilla.iter().zip(&new) {
            if is_test_row(&old.trid) || (old.trid.starts_with("professor_") && old.trid.ends_with("_02")) {
                continue;
            }
            assert!(row.smart_ai, "{} lacks smart AI", row.trid);
            assert!(row.change_gem || no_tera(&row.trid), "{} cannot Terastallize", row.trid);
            for (k, mon) in row.team.iter().enumerate() {
                assert_eq!(mon.talent, (2, 31), "{} slot {k} lacks perfect IVs", row.trid);
                let types = dex.get(mon.species).unwrap().types;
                if let Some(&(_, name, ty)) = boosters.iter().find(|b| i64::from(b.0) == mon.item) {
                    assert!(types.contains(&ty), "{} gives {name} to the wrong type", row.trid);
                }
                assert!(!bad.iter().any(|b| i64::from(b.0) == mon.item), "{} holds a bad item", row.trid);
                items += usize::from(mon.item != 0);
                if mon.manual && old.team.get(k).is_none_or(|o| o.species != mon.species) {
                    let legal = learnable(&mut store, &dex, mon.species, mon.level);
                    for &m in mon.moves.iter().filter(|&&m| m != 0) {
                        assert!(legal.contains(&m), "{} slot {k}: move {m} is not learnable", row.trid);
                    }
                }
            }
        }
        let manual = new.iter().flat_map(|r| &r.team).filter(|m| m.manual).count();
        let doubles = new.iter().filter(|r| r.doubles).count();
        println!("OK chaos seed: {items} held items, all sensible");
        println!("OK chaos seed: {changed} trainers changed, {doubles} double battles, {manual} hand-picked movesets");
        assert!(changed > 600);
        let log = out.log.to_text();
        let start = log.find("-- Teams --").unwrap();
        println!("{}", log[start..].lines().take(6).collect::<Vec<_>>().join("
"));
    }
}

/// Arven's partner in each Titan's second battle, and the clash scene whose object the battle uses
/// as the partner's model.
const TITAN_PARTNERS: [(&str, &str); 5] = [
    ("pepper_nusi_01", "world/scene/parts/field/field_contents/nushi/common/RockClashEvent_/RockClashEvent"),
    ("pepper_nusi_02", "world/scene/parts/field/field_contents/nushi/hagane/HaganeRockClashEvent_/HaganeRockClashEvent"),
    ("pepper_nusi_03", "world/scene/parts/field/field_contents/nushi/hiko/HikoRockClashEvent_/HikoRockClashEvent"),
    ("pepper_nusi_04", "world/scene/parts/field/field_contents/nushi/jimen/JimenRockClashEvent_/JimenRockClashEvent"),
    ("pepper_nusi_05", "world/scene/parts/field/field_contents/nushi/dragon/DragonRockClashEvent_/DragonRockClashEvent"),
];

fn scene_shows(bytes: &[u8]) -> BTreeSet<Pokemon> {
    svr_core::data::scene::swap_species(bytes, &BTreeMap::new()).unwrap().found.into_keys().collect()
}

#[test]
fn titan_partner_models_follow_the_new_lead() {
    let Some(game) = scarlet("titan partners") else { return };
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    let mut patched = 0;
    for out in two_seeds(&game, &chaos()) {
        let new = trainer_rows(&output_table(&game, &out, TRDATA));
        for (trid, stem) in TITAN_PARTNERS {
            let i = vanilla.iter().position(|r| r.trid == trid).unwrap();
            let (old, lead) = (&vanilla[i].team[0], &new[i].team[0]);
            assert_eq!(new[i].team.len(), 1, "{trid} gained Pokémon");
            assert_eq!(lead.rare, old.rare, "{trid} changed shininess");
            assert!(!new[i].change_gem, "{trid} can Terastallize");
            for version in 0..2 {
                let path = format!("{stem}_{version}.trscn");
                assert!(scene_shows(&game.read(&path).unwrap()).contains(&old.species), "{path} lacks {trid}'s vanilla lead");
                match out.files.get(&path) {
                    Some(bytes) => {
                        let shows = scene_shows(bytes);
                        assert!(shows.contains(&lead.species) && !shows.contains(&old.species), "{path} shows {shows:?}, not {trid}'s {:?}", lead.species);
                        patched += 1;
                    }
                    None => assert_eq!(lead.species, old.species, "{path} still shows {trid}'s vanilla lead"),
                }
            }
        }
    }
    println!("OK {patched} clash scenes show Arven's new Titan partner");
    assert!(patched >= 10, "only {patched} clash scenes patched over two seeds");
}

#[test]
fn trainer_themes_levels_and_league_uniqueness_hold() {
    let Some(game) = scarlet("trainer rules") else { return };
    let dex = dex(&game);
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    let s = settings(&[
        pick("trainers.mode", "random"),
        pick("trainers.type_theme", "gym_league"),
        ("trainers.league_unique", OptValue::Int(2)),
        ("trainers.level_modifier", OptValue::Int(-30)),
    ]);
    for out in two_seeds(&game, &s) {
        let new = trainer_rows(&output_table(&game, &out, TRDATA));
        let mut gyms: BTreeMap<String, Vec<[u8; 2]>> = BTreeMap::new();
        for (old, row) in vanilla.iter().zip(&new) {
            if is_test_row(&old.trid) || old.trid.starts_with("professor_") && old.trid.ends_with("_02") {
                continue;
            }
            for (o, m) in old.team.iter().zip(&row.team) {
                assert_eq!(m.level, ((o.level * 70 + 50) / 100).clamp(1, 100), "{} level", row.trid);
            }
            if let Some(gym) = row.trid.strip_prefix("gym_").and_then(|g| g.split('_').next()) {
                gyms.entry(gym.to_owned()).or_default().extend(row.team.iter().map(|m| dex.get(m.species).unwrap().types));
            }
        }
        for (gym, types) in &gyms {
            let shared = (0u8..18).find(|t| types.iter().all(|ty| ty.contains(t)));
            assert!(shared.is_some(), "gym {gym} has no shared type: {types:?}");
        }
        let league = |r: &TrRow| (r.trid.starts_with("e4_") || r.trid.starts_with("chairperson_") || r.trid.contains("4_02_"));
        let mut unique = BTreeSet::new();
        for row in new.iter().filter(|r| league(r)) {
            let ace = row.team.iter().enumerate().max_by_key(|(k, m)| (m.level, *k)).unwrap().1;
            unique.insert(ace.species);
        }
        for row in new.iter().filter(|r| !league(r) && !is_test_row(&r.trid)) {
            for mon in &row.team {
                let vanilla_mon = vanilla.iter().find(|v| v.trid == row.trid).unwrap().team.iter().any(|v| v.species == mon.species);
                assert!(!unique.contains(&mon.species) || vanilla_mon, "{} uses League ace {:?}", row.trid, mon.species);
            }
        }
        println!("OK {} gyms share a type, {} League aces are unique, levels scaled by -30%", gyms.len(), unique.len());
    }
}

#[test]
fn even_distribution_spreads_species() {
    let Some(game) = scarlet("trainer even") else { return };
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    for out in two_seeds(&game, &settings(&[pick("trainers.mode", "even")])) {
        let new = trainer_rows(&output_table(&game, &out, TRDATA));
        let mut counts: BTreeMap<Pokemon, usize> = BTreeMap::new();
        for (old, row) in vanilla.iter().zip(&new) {
            if is_test_row(&old.trid) || old.trid.starts_with("professor_") {
                continue;
            }
            for mon in &row.team {
                *counts.entry(mon.species).or_default() += 1;
            }
        }
        let (min, max) = (counts.values().min().unwrap(), counts.values().max().unwrap());
        println!("OK even: {} species used, each {min} to {max} times", counts.len());
        assert!(max - min <= 2, "uneven spread {min}..{max}");
    }
}

#[test]
fn all_doubles_gives_every_free_trainer_two_pokemon() {
    let Some(game) = scarlet("trainer doubles") else { return };
    let dex = dex(&game);
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    let s = settings(&[pick("trainers.doubles", "all"), on("trainers.force_six"), ("trainers.shiny_percent", OptValue::Int(100))]);
    for out in two_seeds(&game, &s) {
        check_protections(&game, &dex, &vanilla, &out);
        let new = trainer_rows(&output_table(&game, &out, TRDATA));
        let free = vanilla
            .iter()
            .zip(&new)
            .filter(|(old, _)| !is_test_row(&old.trid) && !fixed_battle(&old.trid) && max_team(&old.trid) > 1)
            .inspect(|(old, row)| {
                assert!(row.doubles && row.ai_double, "{} stayed singles", old.trid);
                assert_eq!(row.team.len(), max_team(&old.trid), "{} not forced to a full team", old.trid);
            })
            .count();
        println!("OK {free} trainers fight full double battles");
    }
}

fn text_lines(bytes: &[u8]) -> svr_core::text::TextFile {
    let file = svr_core::text::TextFile::parse(bytes).unwrap();
    assert_eq!(file.to_bytes(), bytes, "text file does not round-trip");
    file
}

/// Checks every language's file is a valid text file whose only changes are whole lines, the same
/// new text for a label in every language, and that `accept` allows that text. Returns the labels.
fn check_text(game: &GameFiles, out: &RandoOutput, name: &str, accept: &dyn Fn(&str) -> bool) -> BTreeSet<String> {
    let mut new_text: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut changed = BTreeSet::new();
    for lang in Language::ALL {
        let vanilla = GameText::load(game, lang, name).unwrap();
        let bytes = out.files.get(&lang.dat_path(name)).unwrap_or_else(|| panic!("{lang:?} {name} not written"));
        assert!(!out.files.contains_key(&lang.tbl_path(name)), "{lang:?} label table rewritten");
        let new = text_lines(bytes);
        assert_eq!(new.len(), vanilla.file().len(), "{lang:?} line count");
        let labels = vanilla.labels().unwrap();
        for i in 0..new.len() {
            assert_eq!(new.flags(i), vanilla.file().flags(i), "{lang:?} line {i} flags");
            let label = labels.entries()[i].name.clone();
            let (before, after) = (vanilla.line(i).unwrap_or_default(), new.line(i).unwrap_or_default());
            if before != after {
                assert!(accept(&after), "{lang:?} {label} became {after:?}");
                changed.insert(label.clone());
            }
            new_text.entry(label).or_default().push(after);
        }
    }
    for label in &changed {
        let texts: BTreeSet<&String> = new_text[label].iter().collect();
        assert_eq!(texts.len(), 1, "{label} differs between languages: {texts:?}");
    }
    changed
}

#[test]
fn trainer_names_and_class_names_change_only_names() {
    let Some(game) = scarlet("trainer names") else { return };
    let s = settings(&[pick("trainers.names", "generic"), pick("trainers.class_names", "all")]);
    let fits = |max: usize| move |t: &str| !t.is_empty() && t.chars().count() <= max && t.is_ascii();
    for out in two_seeds(&game, &s) {
        let renamed = check_text(&game, &out, "trname", &fits(12));
        let reclassed = check_text(&game, &out, "trtype", &fits(20));
        assert!(!renamed.contains("TRNAME_NEMO") && !renamed.contains("TRNAME_NANJAMO"), "generic mode renamed a character");
        let english = text_lines(&out.files[&Language::English.dat_path("trname")]);
        let labels = GameText::load(&game, Language::English, "trname").unwrap();
        let nemo = labels.labels().unwrap().index_of("TRNAME_NEMO").unwrap();
        assert_eq!(english.line(nemo).unwrap(), "Nemona");
        println!("OK {} trainer names and {} class names changed in all 10 languages", renamed.len(), reclassed.len());
        assert!(renamed.len() > 200 && reclassed.len() > 50);
    }
    let custom = settings(&[pick("trainers.names", "all"), ("trainers.name_list", OptValue::Text("Alpha, Beta,  ,Gamma".into()))]);
    let out = run(&game, &custom, "custom");
    let renamed = check_text(&game, &out, "trname", &|t: &str| ["Alpha", "Beta", "Gamma"].contains(&t));
    assert!(renamed.contains("TRNAME_NEMO"), "everyone mode kept Nemona's name");
    println!("OK custom list renamed {} trainers", renamed.len());
}

#[test]
fn random_classes_stay_plain_and_keep_gender() {
    let Some(game) = scarlet("trainer classes") else { return };
    let types = load_table(&game, TRTYPE).unwrap();
    let info: BTreeMap<String, (bool, i64)> = rows(&types)
        .into_iter()
        .map(|t| {
            let empty = |f: &str| t.string(f).unwrap_or("").is_empty();
            let plain = empty("introTml") && empty("cameraObjectName") && int(t, "category") == 0;
            (t.string("nameLabel").unwrap().to_owned(), (plain, int(t, "sex")))
        })
        .collect();
    let vanilla = trainer_rows(&load_table(&game, TRDATA).unwrap());
    for out in two_seeds(&game, &settings(&[on("trainers.classes")])) {
        let new = trainer_rows(&output_table(&game, &out, TRDATA));
        let mut changed = 0;
        let mut by_label: BTreeMap<(String, String), String> = BTreeMap::new();
        for (old, row) in vanilla.iter().zip(&new) {
            assert_eq!(old.team, row.team, "{} team changed", old.trid);
            if old.class == row.class {
                continue;
            }
            changed += 1;
            let (old_plain, old_sex) = info[&old.class];
            let (new_plain, new_sex) = info[&row.class];
            assert!(old_plain && new_plain, "{}: {} -> {}", old.trid, old.class, row.class);
            assert_eq!(old_sex, new_sex, "{} changed gender", old.trid);
            let first = by_label.entry((old.label.clone(), old.class.clone())).or_insert(row.class.clone());
            assert_eq!(first, &row.class, "{} differs from another row with its name", old.trid);
        }
        println!("OK {changed} trainers got a new class");
        assert!(changed > 300);
    }
}
