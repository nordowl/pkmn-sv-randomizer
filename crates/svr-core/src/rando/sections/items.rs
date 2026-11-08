//! Field items, pickup, Let's Go items and drops.

use crate::data::Names;
use crate::fb::{FieldError, TableValue};
use crate::rando::itempool::{ItemCatalog, ItemKind};
use crate::rando::options::{Opt, OptionDef, Page, Settings, Source};
use crate::rando::{Ctx, RandoError, Rng, Section};

#[path = "economy_drops.rs"]
mod economy_drops;

use economy_drops::DropMode;

pub const SECTION: Section = Section {
    id: "items",
    options,
    run,
};

const FIELD: &str = "items.field";
const PICKUP: &str = "items.pickup";
const LETS_GO: &str = "items.lets_go";
const DROPS: &str = "items.drops";
const BAN_BAD: &str = "items.ban_bad";
const BAN_MONEY: &str = "items.ban_money";

/// A random item keeps its spot's stack size only up to this price. The game's stacks of ten are
/// cheap berries; ten Rare Candies in one spot would not be.
const CHEAP: i64 = 1000;

const SLOT_MODES: &[(&str, &str, &str)] = &[
    ("unchanged", "Unchanged", "Every spot keeps the game's item."),
    ("shuffle", "Shuffle", "The game's own items trade places with each other."),
    ("random", "Random", "Every spot gets a random item."),
    ("even", "Even spread", "Every spot gets a random item, and each item turns up about equally often."),
];

fn options() -> Vec<OptionDef> {
    let page = Page::Items;
    vec![
        Opt::choice(FIELD, page, "Field items", "Hidden and sparkling items", "unchanged", SLOT_MODES)
            .help("The items that sparkle on the ground around the map. How likely each spot is to give its item stays the same, and crafting materials found this way are left alone.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::choice(PICKUP, page, "Field items", "Pickup ability items", "unchanged", SLOT_MODES)
            .help("What Pokémon with the Pickup ability find after battles. The odds of each slot stay the same.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::choice(LETS_GO, page, "Field items", "Let's Go auto-battle items", "unchanged", SLOT_MODES)
            .help("The items your Pokémon digs up from bushes, rocks, water and the ground while it walks ahead of you in Let's Go mode.")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(BAN_BAD, page, "Field items", "Ban bad items", false)
            .help("Random and even-spread spots never give items that only hurt their holder or do nothing in this game, such as the Lagging Tail.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_MONEY, page, "Field items", "Ban Lucky Egg and big-money items", false)
            .help("Random and even-spread spots never give the Lucky Egg, the Amulet Coin or treasures worth 10,000 or more, such as Big Nuggets.")
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            DROPS,
            page,
            "Pokémon drops",
            "Pokémon material drops",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon drops the game's material."),
                ("shuffle", "Shuffle", "Pokémon trade their materials with each other."),
                ("random", "Random", "Every Pokémon drops a random material."),
            ],
        )
        .help("The crafting materials Pokémon leave behind when you defeat them. Drops stay materials, and every material a TM recipe needs stays on at least one Pokémon that is not legendary, so you can still craft every TM. The SV reference randomizer can put other items here, which breaks TM crafting.")
        .sources(&[Source::Sv])
        .build(),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Unchanged,
    Shuffle,
    Random,
    Even,
}

impl Mode {
    fn read(settings: &Settings, id: &str) -> Mode {
        match settings.choice(id) {
            "shuffle" => Mode::Shuffle,
            "random" => Mode::Random,
            "even" => Mode::Even,
            _ => Mode::Unchanged,
        }
    }
}

struct Plan {
    field: Mode,
    pickup: Mode,
    lets_go: Mode,
    drops: Option<DropMode>,
    ban_bad: bool,
    ban_money: bool,
}

impl Plan {
    fn read(settings: &Settings) -> Plan {
        Plan {
            field: Mode::read(settings, FIELD),
            pickup: Mode::read(settings, PICKUP),
            lets_go: Mode::read(settings, LETS_GO),
            drops: match settings.choice(DROPS) {
                "shuffle" => Some(DropMode::Shuffle),
                "random" => Some(DropMode::Random),
                _ => None,
            },
            ban_bad: settings.on(BAN_BAD),
            ban_money: settings.on(BAN_MONEY),
        }
    }
}

/// What one slot hands out. Tables without a stack size read as a stack of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stack {
    item: u16,
    count: i64,
}

/// Where a family of tables keeps its item slots.
#[derive(Clone, Copy)]
enum Layout {
    /// One sub-table per slot with an `itemId` and, when `count` names a field, a stack size.
    SubTables {
        slots: &'static [&'static str],
        count: Option<&'static str>,
    },
    /// Item ids stored straight in the row.
    Fields(&'static [&'static str]),
}

fn item_id(value: i64) -> Option<u16> {
    u16::try_from(value).ok().filter(|&id| id != 0)
}

impl Layout {
    /// The filled slots of one row. Empty slots hold item 0 and are never touched.
    fn read(self, row: &TableValue) -> Vec<(&'static str, Stack)> {
        match self {
            Layout::SubTables { slots, count } => slots
                .iter()
                .filter_map(|&slot| {
                    let entry = row.table(slot)?;
                    let item = item_id(entry.int("itemId").ok()?)?;
                    let count = count.map_or(1, |field| entry.int(field).unwrap_or(1));
                    Some((slot, Stack { item, count }))
                })
                .collect(),
            Layout::Fields(slots) => slots
                .iter()
                .filter_map(|&slot| Some((slot, Stack { item: item_id(row.int(slot).ok()?)?, count: 1 })))
                .collect(),
        }
    }

    fn write(self, row: &mut TableValue, slot: &str, stack: Stack) -> Result<(), FieldError> {
        match self {
            Layout::SubTables { count, .. } => {
                let entry = row.table_mut(slot)?;
                entry.set_int("itemId", i64::from(stack.item))?;
                if let Some(field) = count {
                    entry.set_int(field, stack.count)?;
                }
                Ok(())
            }
            Layout::Fields(_) => row.set_int(slot, i64::from(stack.item)),
        }
    }

    fn has_count(self) -> bool {
        matches!(self, Layout::SubTables { count: Some(_), .. })
    }
}

/// Item tables that share a layout and one option.
struct SlotTables {
    option: &'static str,
    title: &'static str,
    paths: &'static [&'static str],
    /// A string field that names each row in the spoiler log.
    row_name: Option<&'static str>,
    layout: Layout,
}

const HIDDEN: SlotTables = SlotTables {
    option: FIELD,
    title: "Field items",
    paths: &[
        "world/data/item/hiddenItemDataTable/hiddenItemDataTable_array.bin",
        "world/data/item/hiddenItemDataTable_su1/hiddenItemDataTable_su1_array.bin",
        "world/data/item/hiddenItemDataTable_su2/hiddenItemDataTable_su2_array.bin",
        "world/data/item/hiddenItemDataTable_lc/hiddenItemDataTable_lc_array.bin",
    ],
    row_name: Some("table_Id"),
    layout: Layout::SubTables {
        slots: &[
            "item_1", "item_2", "item_3", "item_4", "item_5", "item_6", "item_7", "item_8", "item_9", "item_10",
        ],
        count: Some("dropCount"),
    },
};

const PICKUP_TABLES: SlotTables = SlotTables {
    option: PICKUP,
    title: "Pickup items",
    paths: &["world/data/item/monohiroiItemData/monohiroiItemData_array.bin"],
    row_name: None,
    layout: Layout::SubTables {
        slots: &[
            "item_01", "item_02", "item_03", "item_04", "item_05", "item_06", "item_07", "item_08", "item_09", "item_10",
            "item_11", "item_12", "item_13", "item_14", "item_15", "item_16", "item_17", "item_18", "item_19", "item_20",
            "item_21", "item_22", "item_23", "item_24", "item_25", "item_26", "item_27", "item_28", "item_29", "item_30",
        ],
        count: None,
    },
};

const LETS_GO_TABLES: SlotTables = SlotTables {
    option: LETS_GO,
    title: "Let's Go items",
    paths: &["world/data/item/rummagingItemDataTable/rummagingItemDataTable_array.bin"],
    row_name: None,
    layout: Layout::Fields(&["Item00", "Item01", "Item02", "Item03", "Item04"]),
};

/// New contents for every slot. `vanilla` is in slot order; random modes draw from `pool`, which
/// must not be empty, and keep a slot's stack size only for items costing at most [`CHEAP`].
fn reassign(
    mode: Mode,
    vanilla: &[Stack],
    pool: &[u16],
    price: impl Fn(u16) -> i64,
    rng: &mut Rng,
) -> Vec<Stack> {
    let restack = |old: Stack, item: u16| Stack {
        item,
        count: if price(item) <= CHEAP { old.count } else { 1 },
    };
    match mode {
        Mode::Unchanged => vanilla.to_vec(),
        Mode::Shuffle => {
            let mut stacks = vanilla.to_vec();
            rng.shuffle(&mut stacks);
            stacks
        }
        Mode::Random => vanilla.iter().map(|&old| restack(old, pool[rng.index(pool.len())])).collect(),
        Mode::Even => {
            let mut deck = Vec::new();
            vanilla
                .iter()
                .map(|&old| {
                    if deck.is_empty() {
                        deck = pool.to_vec();
                        rng.shuffle(&mut deck);
                    }
                    restack(old, deck.pop().expect("deck refilled above"))
                })
                .collect()
        }
    }
}

struct Slot {
    path: &'static str,
    row: usize,
    row_name: String,
    field: &'static str,
    stack: Stack,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "items",
        detail: e.to_string(),
    }
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let plan = Plan::read(&ctx.settings);
    let families = [(&HIDDEN, plan.field), (&PICKUP_TABLES, plan.pickup), (&LETS_GO_TABLES, plan.lets_go)];
    let changes_slots = families.iter().any(|&(_, mode)| mode != Mode::Unchanged);
    if !changes_slots && plan.drops.is_none() {
        return Ok(());
    }
    let catalog = ItemCatalog::load(ctx)?;
    if changes_slots {
        let pool = catalog.field(plan.ban_bad, plan.ban_money);
        if pool.is_empty() {
            return Err(RandoError::Section {
                section: "items",
                detail: "no item is left for field items. Turn off a ban.".into(),
            });
        }
        for (tables, mode) in families {
            if mode != Mode::Unchanged {
                reassign_tables(ctx, &catalog, &pool, tables, mode)?;
            }
        }
    }
    if let Some(mode) = plan.drops {
        economy_drops::run(ctx, &catalog, mode)?;
    }
    Ok(())
}

fn reassign_tables(
    ctx: &mut Ctx,
    catalog: &ItemCatalog,
    pool: &[u16],
    tables: &SlotTables,
    mode: Mode,
) -> Result<(), RandoError> {
    let mut slots = Vec::new();
    for &path in tables.paths {
        for (row, values) in ctx.store.get(path)?.tables("values").enumerate() {
            let row_name = tables
                .row_name
                .and_then(|field| values.string(field))
                .map_or_else(|| row.to_string(), str::to_owned);
            for (field, stack) in tables.layout.read(values) {
                // Material spots stay put, so the drops repair is the only thing that moves sources.
                if catalog.get(stack.item).is_some_and(|i| i.kind == ItemKind::Material) {
                    continue;
                }
                slots.push(Slot {
                    path,
                    row,
                    row_name: row_name.clone(),
                    field,
                    stack,
                });
            }
        }
    }
    let vanilla: Vec<Stack> = slots.iter().map(|s| s.stack).collect();
    let price = |id: u16| catalog.get(id).map_or(0, |i| i.price);
    let mut rng = ctx.rng(tables.option);
    let assigned = reassign(mode, &vanilla, pool, price, &mut rng);

    let mut lines = Vec::new();
    for &path in tables.paths {
        let changes: Vec<(&Slot, Stack)> = slots
            .iter()
            .zip(&assigned)
            .filter(|(slot, new)| slot.path == path && slot.stack != **new)
            .map(|(slot, new)| (slot, *new))
            .collect();
        if changes.is_empty() {
            continue;
        }
        let mut rows: Vec<&mut TableValue> = ctx.store.get_mut(path)?.tables_mut("values").collect();
        for (slot, new) in changes {
            tables.layout.write(rows[slot.row], slot.field, new).map_err(shape)?;
            lines.push(vec![
                table_name(path).to_owned(),
                slot.row_name.clone(),
                slot.field.to_owned(),
                stack_label(&ctx.names, slot.stack, tables.layout),
                stack_label(&ctx.names, new, tables.layout),
            ]);
        }
    }
    let log = ctx.log.section(tables.title);
    log.note(format!("{} of {} item spots changed.", lines.len(), slots.len()));
    let table = log.table(tables.title, &["Table", "Row", "Slot", "Was", "Now"]);
    for line in lines {
        table.row(line);
    }
    Ok(())
}

fn table_name(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.strip_suffix("_array.bin").unwrap_or(file)
}

fn stack_label(names: &Names, stack: Stack, layout: Layout) -> String {
    let name = names.item(stack.item);
    if layout.has_count() && stack.count != 1 {
        format!("{name} x{}", stack.count)
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn stacks(items: &[(u16, i64)]) -> Vec<Stack> {
        items.iter().map(|&(item, count)| Stack { item, count }).collect()
    }

    fn sorted(mut v: Vec<Stack>) -> Vec<(u16, i64)> {
        v.sort_by_key(|s| (s.item, s.count));
        v.into_iter().map(|s| (s.item, s.count)).collect()
    }

    #[test]
    fn shuffle_keeps_items_with_their_stacks() {
        let vanilla = stacks(&[(1, 1), (2, 10), (3, 2), (4, 1), (5, 1)]);
        let out = reassign(Mode::Shuffle, &vanilla, &[], |_| 0, &mut Rng::new(3));
        assert_eq!(sorted(out), sorted(vanilla));
    }

    #[test]
    fn random_stacks_only_cheap_items() {
        let vanilla = stacks(&[(9, 10); 50]);
        let price = |id: u16| if id == 1 { 100 } else { 5000 };
        let out = reassign(Mode::Random, &vanilla, &[1, 2], price, &mut Rng::new(5));
        assert!(out.iter().any(|s| s.item == 1) && out.iter().any(|s| s.item == 2));
        for s in out {
            assert_eq!(s.count, if s.item == 1 { 10 } else { 1 }, "{s:?}");
        }
    }

    #[test]
    fn even_spread_balances_counts() {
        let vanilla = stacks(&[(9, 1); 25]);
        let pool = [1, 2, 3, 4, 5, 6, 7];
        let out = reassign(Mode::Even, &vanilla, &pool, |_| 0, &mut Rng::new(11));
        let mut counts = BTreeMap::new();
        for s in out {
            *counts.entry(s.item).or_insert(0) += 1;
        }
        assert_eq!(counts.len(), pool.len());
        let (min, max) = (counts.values().min().unwrap(), counts.values().max().unwrap());
        assert!(max - min <= 1, "{counts:?}");
    }
}
