//! Raid item rewards. The boss tables of all three regions link into the same two reward tables,
//! one row per reward set of a star tier, so rewards change once for every region.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::store::ITEMS;
use crate::data::DataError;
use crate::fb::TableValue;
use crate::rando::options::{Opt, OptionDef, Page, Source};
use crate::rando::{Ctx, RandoError, Rng};

use super::{boss_tables, child, put, shape};

struct RewardTable {
    option: &'static str,
    path: &'static str,
    label: &'static str,
}

const TABLES: [RewardTable; 2] = [
    RewardTable {
        option: "raids.rewards.fixed",
        path: "world/data/raid/raid_fixed_reward_item/raid_fixed_reward_item_array.bin",
        label: "Guaranteed",
    },
    RewardTable {
        option: "raids.rewards.lottery",
        path: "world/data/raid/raid_lottery_reward_item/raid_lottery_reward_item_array.bin",
        label: "Lottery",
    },
];

pub(super) fn options() -> Vec<OptionDef> {
    vec![
        Opt::toggle(TABLES[0].option, Page::Raids, "Rewards", "Random guaranteed rewards", false)
            .help("Swaps each item a raid always gives for another item of the same kind, such as one Exp. Candy size for another. Tera Shards and species materials stay. All regions share these rewards.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(TABLES[1].option, Page::Raids, "Rewards", "Random lottery rewards", false)
            .help("The same for the extra items a raid draws at random, such as mints, berries and Herba Mystica. Each item keeps its drop chance and amount.")
            .sources(&[Source::New])
            .build(),
    ]
}

/// What an item does, so a replacement does the same job: Exp. Candies and Rare Candy level up,
/// vitamins and feathers train stats, berries stay berries. Items without a price are special,
/// like Herba Mystica, and only swap with special items the raids already give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Class {
    item_type: i64,
    pocket: i64,
    group: i64,
    effect: Effect,
    priced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Effect {
    Level,
    Stats,
    Pp,
    Other,
}

const STAT_WORK: [&str; 6] = [
    "WorkStatusHp",
    "WorkStatusAtk",
    "WorkStatusDef",
    "WorkStatusSpd",
    "WorkStatusSAtk",
    "WorkStatusSDef",
];

fn classify(item: &TableValue) -> Result<Class, RandoError> {
    let field = |name: &str| item.int(name).map_err(shape);
    let mut stats = false;
    for name in STAT_WORK {
        stats |= field(name)? != 0;
    }
    let effect = if field("WorkLevel")? != 0 {
        Effect::Level
    } else if stats {
        Effect::Stats
    } else if field("WorkPpUp")? != 0 {
        Effect::Pp
    } else {
        Effect::Other
    };
    Ok(Class {
        item_type: field("ItemType")?,
        pocket: field("FieldPocket")?,
        group: field("ItemGroup")?,
        effect,
        priced: field("Price")? > 0,
    })
}

struct Catalog {
    class: BTreeMap<u16, Class>,
    members: BTreeMap<Class, Vec<u16>>,
}

impl Catalog {
    /// Every item may be replaced; only priced items and items the raids already give may be the
    /// replacement, which keeps out unused and event-only entries.
    fn load(ctx: &mut Ctx, in_rewards: &BTreeSet<u16>) -> Result<Catalog, RandoError> {
        let mut class = BTreeMap::new();
        let mut members: BTreeMap<Class, Vec<u16>> = BTreeMap::new();
        for row in ctx.store.get(ITEMS)?.tables("values") {
            let id = row.int("Id").map_err(shape)? as u16;
            if id == 0 {
                continue;
            }
            let c = classify(row)?;
            if c.priced || in_rewards.contains(&id) {
                members.entry(c).or_default().push(id);
            }
            class.insert(id, c);
        }
        Ok(Catalog { class, members })
    }

    /// A random item of `item`'s class, preferring one not in `used`.
    fn replacement(&self, rng: &mut Rng, item: u16, used: &BTreeSet<u16>) -> u16 {
        let Some(members) = self.class.get(&item).and_then(|c| self.members.get(c)) else {
            return item;
        };
        let fresh: Vec<u16> = members.iter().copied().filter(|m| !used.contains(m)).collect();
        let from = if fresh.is_empty() { members } else { &fresh };
        rng.pick(from).copied().unwrap_or(item)
    }
}

fn item_category(table: &TableValue) -> Result<i64, RandoError> {
    table
        .schema()
        .enum_value("RaidRewardItemCategoryType", "ITEM")
        .ok_or_else(|| RandoError::Data(DataError::Shape("raid reward schema has no ITEM category".into())))
}

/// The slot field and item id of each item reward in a row. Species materials and Tera Shards are
/// other categories and never listed.
fn item_entries(row: &TableValue, item: i64) -> Result<Vec<(String, u16)>, RandoError> {
    let mut out = Vec::new();
    for def in &row.def().fields {
        if !def.name.starts_with("reward_item_") {
            continue;
        }
        let Some(entry) = row.table(&def.name) else { continue };
        let id = entry.int("itemID").map_err(shape)?;
        if entry.int("category").map_err(shape)? == item && id != 0 {
            out.push((def.name.clone(), id as u16));
        }
    }
    Ok(out)
}

/// The lowest star tier that links to each reward row, by reward table name. Rows no boss links
/// to are never handed out.
fn linked_tiers(ctx: &mut Ctx) -> Result<BTreeMap<u64, usize>, RandoError> {
    let mut out = BTreeMap::new();
    for (tier, path) in boss_tables() {
        for row in ctx.store.get(&path)?.tables("values") {
            let info = child(row, "raidEnemyInfo")?;
            for field in ["dropTableFix", "dropTableRandom"] {
                let lowest = out.entry(info.uint(field).map_err(shape)?).or_insert(tier);
                *lowest = (*lowest).min(tier);
            }
        }
    }
    Ok(out)
}

struct Swap {
    row: usize,
    tier: usize,
    slot: String,
    old: u16,
    new: u16,
}

pub(super) fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let chosen: Vec<&RewardTable> = TABLES.iter().filter(|t| ctx.settings.on(t.option)).collect();
    if chosen.is_empty() {
        return Ok(());
    }
    let tiers = linked_tiers(ctx)?;
    let mut in_rewards = BTreeSet::new();
    for t in &TABLES {
        let table = ctx.store.get(t.path)?;
        let item = item_category(table)?;
        for row in table.tables("values") {
            in_rewards.extend(item_entries(row, item)?.into_iter().map(|(_, id)| id));
        }
    }
    let catalog = Catalog::load(ctx, &in_rewards)?;

    let mut log_rows = Vec::new();
    for t in chosen {
        let mut rng = ctx.rng(t.option);
        let table = ctx.store.get(t.path)?;
        let item = item_category(table)?;
        let mut swaps = Vec::new();
        for (row, values) in table.tables("values").enumerate() {
            let Some(&tier) = tiers.get(&values.uint("table_name").map_err(shape)?) else { continue };
            let mut used = BTreeSet::new();
            for (slot, old) in item_entries(values, item)? {
                let new = catalog.replacement(&mut rng, old, &used);
                used.insert(new);
                if new != old {
                    swaps.push(Swap { row, tier, slot, old, new });
                }
            }
        }
        if swaps.is_empty() {
            continue;
        }
        let mut rows: Vec<&mut TableValue> = ctx.store.get_mut(t.path)?.tables_mut("values").collect();
        for s in &swaps {
            put(rows[s.row].table_mut(&s.slot).map_err(shape)?, "itemID", i64::from(s.new))?;
        }
        for s in swaps {
            log_rows.push(vec![
                t.label.to_owned(),
                format!("{}★", s.tier),
                s.row.to_string(),
                s.slot.trim_start_matches("reward_item_").to_owned(),
                ctx.names.item(s.old),
                ctx.names.item(s.new),
            ]);
        }
    }
    if !log_rows.is_empty() {
        let section = ctx.log.section("Tera Raids");
        section.note(format!("{} raid reward items changed, shared by all regions.", log_rows.len()));
        let table = section.table("Raid rewards", &["Rewards", "Tier", "Row", "Slot", "Old", "New"]);
        for row in log_rows {
            table.row(row);
        }
    }
    Ok(())
}
