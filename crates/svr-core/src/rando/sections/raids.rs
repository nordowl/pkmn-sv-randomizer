//! Tera Raids: random bosses per region and star tier, their Tera types and shininess, and the
//! item rewards the boss tables link to.

#[path = "raid_rewards.rs"]
mod rewards;

use std::collections::BTreeMap;

use crate::data::facts;
use crate::data::store::{MOVES, PERSONAL};
use crate::data::{DataError, Dex, FormInfo, Pokemon};
use crate::fb::{FieldError, TableValue};
use crate::rando::options::{Kind, Opt, OptionDef, Page, Req, Source, Status};
use crate::rando::{movepool, Ctx, RandoError, Rng, Section};

pub const SECTION: Section = Section {
    id: "raids",
    options,
    run,
};

const POOL: &str = "raids.pool";
const PALDEA_FOR_ALL: &str = "raids.paldea_for_all";
const TIERS: [&str; 6] = ["raids.tier1", "raids.tier2", "raids.tier3", "raids.tier4", "raids.tier5", "raids.tier6"];
const TIER_LABELS: [&str; 6] = ["1★ raids", "2★ raids", "3★ raids", "4★ raids", "5★ raids", "6★ raids"];
/// Wimp Out and Emergency Exit. A boss that leaves the battle at half HP could break the raid, and
/// randomized abilities can hand these to any species.
const FLEEING_ABILITIES: [u16; 2] = [193, 194];

struct Region {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    file_prefix: &'static str,
    bosses: &'static str,
    pool: &'static str,
    scale: &'static str,
    shiny: &'static str,
    tera: &'static str,
}

impl Region {
    fn table(&self, tier: usize) -> String {
        let p = self.file_prefix;
        format!("world/data/raid/{p}raid_enemy_0{tier}/{p}raid_enemy_0{tier}_array.bin")
    }
}

const PALDEA: Region = Region {
    id: "paldea",
    name: "Paldea",
    group: "Paldea raids",
    file_prefix: "",
    bosses: "raids.paldea.bosses",
    pool: "raids.paldea.pool",
    scale: "raids.paldea.scale",
    shiny: "raids.paldea.shiny",
    tera: "raids.paldea.tera",
};

const KITAKAMI: Region = Region {
    id: "kitakami",
    name: "Kitakami",
    group: "Kitakami raids",
    file_prefix: "su1_",
    bosses: "raids.kitakami.bosses",
    pool: "raids.kitakami.pool",
    scale: "raids.kitakami.scale",
    shiny: "raids.kitakami.shiny",
    tera: "raids.kitakami.tera",
};

const BLUEBERRY: Region = Region {
    id: "blueberry",
    name: "Blueberry",
    group: "Blueberry raids",
    file_prefix: "su2_",
    bosses: "raids.blueberry.bosses",
    pool: "raids.blueberry.pool",
    scale: "raids.blueberry.scale",
    shiny: "raids.blueberry.shiny",
    tera: "raids.blueberry.tera",
};

const REGIONS: [&Region; 3] = [&PALDEA, &KITAKAMI, &BLUEBERRY];

/// Every boss table, which is also every table that links to the reward tables.
fn boss_tables() -> impl Iterator<Item = (usize, String)> {
    REGIONS
        .into_iter()
        .flat_map(|r| (1..=6).map(move |tier| (tier, r.table(tier))))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tera {
    Unchanged,
    Fixed,
    Own,
}

impl Tera {
    const CHOICES: &'static [(&'static str, &'static str, &'static str)] = &[
        ("unchanged", "Unchanged", "Keeps the game's setting, which gives most raids any Tera type each time they appear."),
        ("fixed", "Random, fixed per raid", "Each boss gets one random Tera type, listed in the spoiler log."),
        ("own", "The boss's own type", "Each boss Terastallizes into one of its own types."),
    ];

    fn parse(value: &str) -> Tera {
        match value {
            "fixed" => Tera::Fixed,
            "own" => Tera::Own,
            _ => Tera::Unchanged,
        }
    }
}

fn options() -> Vec<OptionDef> {
    let mut out = vec![
        Opt::new(POOL, Page::Raids, "All raids", "Species pool for raid bosses", Kind::Pool)
            .help("Species any region's raid bosses can become. Each region can override this. Inherit uses the global pool.")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(PALDEA_FOR_ALL, Page::Raids, "All raids", "Use Paldea settings for all regions", false)
            .help("Kitakami and Blueberry raids follow the Paldea settings below, including the Paldea pool, and ignore their own.")
            .sources(&[Source::Sv])
            .build(),
    ];
    for (id, label) in TIERS.into_iter().zip(TIER_LABELS) {
        out.push(
            Opt::toggle(id, Page::Raids, "Star tiers", label, true)
                .help("Off keeps raids of this star tier as they are in the game, in every region.")
                .sources(&[Source::New])
                .build(),
        );
    }
    for region in REGIONS {
        out.extend(region_options(region));
    }
    out.extend(rewards::options());
    out
}

fn region_options(r: &'static Region) -> Vec<OptionDef> {
    let own_settings = |opt: Opt| {
        if r.id == PALDEA.id {
            opt
        } else {
            opt.requires(Req::Off { id: PALDEA_FOR_ALL })
        }
    };
    [
        Opt::toggle(r.bosses, Page::Raids, r.group, "Random bosses", false)
            .help("Replaces each raid boss with a random species. A new boss knows the level-up moves it would have at its level, and any move it uses on cue mid-battle becomes one it can learn.")
            .sources(&[Source::Sv]),
        Opt::new(r.pool, Page::Raids, r.group, "Species pool", Kind::Pool)
            .help("Species this region's bosses can become. Inherit uses the raid pool.")
            .on(r.bosses)
            .sources(&[Source::Sv]),
        Opt::toggle(r.scale, Page::Raids, r.group, "Stronger bosses in higher tiers", false)
            .help("Splits the pool into six bands by base stat total, so 1★ bosses come from the weakest band and 6★ bosses from the strongest.")
            .on(r.bosses)
            .sources(&[Source::New]),
        Opt::number(r.shiny, Page::Raids, r.group, "Shiny chance", 0, 0, 100, "%")
            .help("Chance that a boss is always shiny. 100% makes every boss shiny. 0% keeps the game's normal odds.")
            .sources(&[Source::Sv])
            .status(Status::Test),
        Opt::choice(r.tera, Page::Raids, r.group, "Tera types", "unchanged", Tera::CHOICES)
            .help("The type each boss Terastallizes into. Ogerpon always keeps its mask's type and Terapagos is always Stellar.")
            .sources(&[Source::New]),
    ]
    .into_iter()
    .map(|opt| own_settings(opt).build())
    .collect()
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let moves = MoveFacts::load(ctx)?;
    for region in REGIONS {
        if let Some(plan) = Plan::resolve(ctx, region)? {
            randomize_region(ctx, region, &plan, &moves)?;
        }
    }
    rewards::run(ctx)
}

/// What changes in one region's boss tables, read from the region whose settings apply.
struct Plan {
    /// The species each star tier draws from, weakest tier first, when bosses change.
    bands: Option<Vec<Vec<Pokemon>>>,
    shiny: u32,
    tera: Tera,
}

impl Plan {
    fn resolve(ctx: &Ctx, region: &Region) -> Result<Option<Plan>, RandoError> {
        let from = if region.id != PALDEA.id && ctx.settings.on(PALDEA_FOR_ALL) {
            &PALDEA
        } else {
            region
        };
        let s = &ctx.settings;
        let bands = if s.on(from.bosses) {
            let pool_id = if s.pool(from.pool).inherit { POOL } else { from.pool };
            let what = format!("{} raid bosses", from.name);
            let mut pool = ctx.pool(pool_id, &what)?;
            pool.retain(|&p| ctx.dex.get(p).is_some_and(|f| !f.abilities.iter().any(|a| FLEEING_ABILITIES.contains(a))));
            if pool.is_empty() {
                return Err(RandoError::EmptyPool { what });
            }
            Some(if s.on(from.scale) {
                strength_bands(&ctx.dex, pool)
            } else {
                vec![pool; 6]
            })
        } else {
            None
        };
        let plan = Plan {
            bands,
            shiny: s.number(from.shiny).clamp(0, 100) as u32,
            tera: Tera::parse(s.choice(from.tera)),
        };
        let changes_anything = plan.bands.is_some() || plan.shiny > 0 || plan.tera != Tera::Unchanged;
        Ok(changes_anything.then_some(plan))
    }
}

/// The pool sorted by base stat total and cut into six equal bands, so every boss of a higher tier
/// is at least as strong as every boss of a lower one.
fn strength_bands(dex: &Dex, mut pool: Vec<Pokemon>) -> Vec<Vec<Pokemon>> {
    pool.sort_by_key(|&p| (dex.get(p).map_or(0, FormInfo::bst), p));
    let n = pool.len();
    (0..6)
        .map(|i| {
            let band = &pool[i * n / 6..(i + 1) * n / 6];
            if band.is_empty() {
                pool.clone()
            } else {
                band.to_vec()
            }
        })
        .collect()
}

/// `count` picks from `band`, using every member once before any repeats.
fn deal(rng: &mut Rng, band: &[Pokemon], count: usize) -> Vec<Pokemon> {
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let mut deck = band.to_vec();
        rng.shuffle(&mut deck);
        out.extend(deck.into_iter().take(count - out.len()));
    }
    out
}

/// Enum values the boss tables use, read from their schema.
struct Consts {
    gem_default: i64,
    gem_first_type: i64,
    gem_stellar: i64,
    rare: i64,
    sex_default: i64,
    manual_moves: i64,
    action_none: i64,
    action_waza: i64,
    timing_none: i64,
}

const TYPE_COUNT: u64 = 18;

impl Consts {
    fn of(table: &TableValue) -> Result<Consts, RandoError> {
        let schema = table.schema();
        let get = |e: &str, v: &str| {
            schema
                .enum_value(e, v)
                .ok_or_else(|| RandoError::Data(DataError::Shape(format!("raid boss schema has no {e}.{v}"))))
        };
        Ok(Consts {
            gem_default: get("GemType", "DEFAULT")?,
            gem_first_type: get("GemType", "NORMAL")?,
            gem_stellar: get("GemType", "NIJI")?,
            rare: get("RareType", "RARE")?,
            sex_default: get("SexType", "DEFAULT")?,
            manual_moves: get("WazaType", "MANUAL")?,
            action_none: get("RaidBossExtraActType", "NONE")?,
            action_waza: get("RaidBossExtraActType", "WAZA")?,
            timing_none: get("RaidBossExtraTimingType", "NONE")?,
        })
    }

    /// Ogerpon's Tera type must match its mask and Terapagos can only be Stellar.
    fn required_gem(&self, info: &FormInfo) -> Option<i64> {
        facts::required_tera(info.national, info.id.form).map(|tera| match tera {
            facts::Tera::Type(t) => self.gem_first_type + i64::from(t),
            facts::Tera::Stellar => self.gem_stellar,
        })
    }
}

/// One boss as the game ships it.
struct Boss {
    species: Pokemon,
    level: u16,
    gem: i64,
    rare: i64,
    /// The move of each extra action that makes the boss use a move, by action slot.
    cued: [Option<u16>; 6],
    /// The move used when players fail to stop a power charge, or 0.
    penalty: u16,
}

/// A new species for a boss, with moves it can have.
struct NewBoss {
    species: Pokemon,
    moves: Vec<u16>,
    /// Replacement per cued move slot; 0 drops the action because nothing learnable fits.
    cued: [Option<u16>; 6],
    penalty: u16,
}

struct Change {
    boss: Option<NewBoss>,
    gem: Option<i64>,
    shiny: bool,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Data(DataError::Shape(e.to_string()))
}

fn child<'a>(t: &'a TableValue, field: &str) -> Result<&'a TableValue, RandoError> {
    t.table(field)
        .ok_or_else(|| RandoError::Data(DataError::Shape(format!("{} has no {field}", t.def().name))))
}

/// Writes only when the value differs, so untouched fields keep their original encoding.
fn put(t: &mut TableValue, field: &str, value: i64) -> Result<(), RandoError> {
    if t.int(field).map_err(shape)? != value {
        t.set_int(field, value).map_err(shape)?;
    }
    Ok(())
}

fn read_bosses(table: &TableValue, c: &Consts) -> Result<Vec<Boss>, RandoError> {
    table
        .tables("values")
        .map(|row| {
            let info = child(row, "raidEnemyInfo")?;
            let poke = child(info, "bossPokePara")?;
            let mut cued = [None; 6];
            let mut penalty = 0;
            if let Some(desc) = info.table("bossDesc") {
                penalty = desc.int("powerChargePenaltyAction").map_err(shape)? as u16;
                for (slot, cue) in cued.iter_mut().enumerate() {
                    let Some(action) = desc.table(&format!("extraAction{}", slot + 1)) else { continue };
                    let wazano = action.int("wazano").map_err(shape)? as u16;
                    if action.int("action").map_err(shape)? == c.action_waza && wazano != 0 {
                        *cue = Some(wazano);
                    }
                }
            }
            Ok(Boss {
                species: Pokemon {
                    species: poke.int("devId").map_err(shape)? as u16,
                    form: poke.int("formId").map_err(shape)? as u16,
                },
                level: poke.int("level").map_err(shape)? as u16,
                gem: poke.int("gemType").map_err(shape)?,
                rare: poke.int("rareType").map_err(shape)?,
                cued,
                penalty,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
struct MoveFact {
    usable: bool,
    status: bool,
    power: u8,
}

struct MoveFacts(BTreeMap<u16, MoveFact>);

impl MoveFacts {
    fn load(ctx: &mut Ctx) -> Result<MoveFacts, RandoError> {
        let mut facts = BTreeMap::new();
        for (i, row) in ctx.store.get(MOVES)?.tables("Table").enumerate() {
            let id = u16::try_from(row.int("MoveID").unwrap_or(i as i64)).unwrap_or(0);
            facts.insert(
                id,
                MoveFact {
                    usable: row.bool("CanUseMove").unwrap_or(false),
                    status: row.int("Category").unwrap_or(0) == 0,
                    power: row.int("Power").unwrap_or(0) as u8,
                },
            );
        }
        Ok(MoveFacts(facts))
    }

    fn get(&self, id: u16) -> MoveFact {
        self.0.get(&id).copied().unwrap_or(MoveFact {
            usable: false,
            status: true,
            power: 0,
        })
    }

    /// The learnable move closest to `old`: a status move for a status move, else the attack
    /// nearest in power, so a cued Rain Dance stays a support move and a cued Thunder stays a nuke.
    /// Returns 0 when nothing is learnable.
    fn closest(&self, rng: &mut Rng, old: u16, learnable: &[u16]) -> u16 {
        let old = self.get(old);
        let same_kind: Vec<u16> = learnable.iter().copied().filter(|&m| self.get(m).status == old.status).collect();
        let candidates = if same_kind.is_empty() { learnable } else { &same_kind };
        let gap = |m: u16| self.get(m).power.abs_diff(old.power);
        let Some(best) = candidates.iter().map(|&m| gap(m)).min() else { return 0 };
        let nearest: Vec<u16> = candidates.iter().copied().filter(|&m| gap(m) == best).collect();
        rng.pick(&nearest).copied().unwrap_or(0)
    }
}

/// Every usable move `p` learns by level-up at or below `level`.
fn learnable_at(ctx: &mut Ctx, facts: &MoveFacts, p: Pokemon, level: u16) -> Result<Vec<u16>, RandoError> {
    let Some(row) = ctx.dex.get(p).map(|f| f.row) else {
        return Ok(Vec::new());
    };
    let personal = ctx.store.get(PERSONAL)?;
    let Some(entry) = personal.tables("entry").nth(row) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for m in entry.vector("levelup_moves").unwrap_or(&[]) {
        let Some(m) = m.as_struct() else { continue };
        let id = m.int("move").unwrap_or(0) as u16;
        if m.int("level").unwrap_or(0) <= i64::from(level) && facts.get(id).usable && !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn new_boss(ctx: &mut Ctx, facts: &MoveFacts, rng: &mut Rng, old: &Boss, species: Pokemon) -> Result<NewBoss, RandoError> {
    let moves = movepool::level_up_moves_at(ctx, species, old.level)?;
    let learnable = learnable_at(ctx, facts, species, old.level)?;
    let cued = old.cued.map(|cue| cue.map(|m| facts.closest(rng, m, &learnable)));
    let penalty = if old.penalty == 0 {
        0
    } else {
        facts.closest(rng, old.penalty, &learnable)
    };
    Ok(NewBoss {
        species,
        moves,
        cued,
        penalty,
    })
}

fn randomize_region(ctx: &mut Ctx, region: &Region, plan: &Plan, facts: &MoveFacts) -> Result<(), RandoError> {
    let mut log_rows = Vec::new();
    for tier in 1..=6 {
        if !ctx.settings.on(TIERS[tier - 1]) {
            continue;
        }
        let path = region.table(tier);
        let stream = format!("raids.{}.{tier}", region.id);
        let table = ctx.store.get(&path)?;
        let c = Consts::of(table)?;
        let bosses = read_bosses(table, &c)?;

        let picks = plan.bands.as_ref().map(|bands| deal(&mut ctx.rng(&stream), &bands[tier - 1], bosses.len()));
        let mut move_rng = ctx.rng(&format!("{stream}.moves"));
        let mut tera_rng = ctx.rng(&format!("{stream}.tera"));
        let mut shiny_rng = ctx.rng(&format!("{stream}.shiny"));
        let mut changes = Vec::with_capacity(bosses.len());
        for (i, old) in bosses.iter().enumerate() {
            let boss = match &picks {
                Some(picks) if picks[i] != old.species => Some(new_boss(ctx, facts, &mut move_rng, old, picks[i])?),
                _ => None,
            };
            let species = boss.as_ref().map_or(old.species, |b| b.species);
            let rolled = (plan.tera == Tera::Fixed).then(|| c.gem_first_type + tera_rng.below(TYPE_COUNT) as i64);
            let gem = match ctx.dex.get(species).and_then(|f| c.required_gem(f)) {
                Some(required) => Some(required),
                None => match plan.tera {
                    Tera::Unchanged => None,
                    Tera::Fixed => rolled,
                    Tera::Own => Some(c.gem_default),
                },
            };
            let shiny = plan.shiny > 0 && shiny_rng.percent(plan.shiny);
            changes.push(Change { boss, gem, shiny });
        }

        let table = ctx.store.get_mut(&path)?;
        for (row, change) in table.tables_mut("values").zip(&changes) {
            write_change(row, change, &c)?;
        }

        for (i, (old, change)) in bosses.iter().zip(&changes).enumerate() {
            let species = change.boss.as_ref().map_or(old.species, |b| b.species);
            let gem = change.gem.unwrap_or(old.gem);
            let shiny = change.shiny || old.rare == c.rare;
            if species == old.species && gem == old.gem && !change.shiny {
                continue;
            }
            log_rows.push(vec![
                format!("{tier}★"),
                i.to_string(),
                ctx.names.pokemon(old.species),
                ctx.names.pokemon(species),
                gem_label(ctx, &c, gem),
                if shiny { "yes" } else { "" }.to_owned(),
            ]);
        }
    }
    if !log_rows.is_empty() {
        let section = ctx.log.section("Tera Raids");
        section.note(format!("{}: {} raid bosses changed.", region.name, log_rows.len()));
        let table = section.table(&format!("{} raid bosses", region.name), &["Tier", "Row", "Old", "New", "Tera", "Shiny"]);
        for row in log_rows {
            table.row(row);
        }
    }
    Ok(())
}

fn write_change(row: &mut TableValue, change: &Change, c: &Consts) -> Result<(), RandoError> {
    let info = row.table_mut("raidEnemyInfo").map_err(shape)?;
    let poke = info.table_mut("bossPokePara").map_err(shape)?;
    if let Some(boss) = &change.boss {
        put(poke, "devId", i64::from(boss.species.species))?;
        put(poke, "formId", i64::from(boss.species.form))?;
        put(poke, "sex", c.sex_default)?;
        put(poke, "wazaType", c.manual_moves)?;
        for (slot, field) in ["waza1", "waza2", "waza3", "waza4"].into_iter().enumerate() {
            let id = boss.moves.get(slot).copied().unwrap_or(0);
            put(poke.table_mut(field).map_err(shape)?, "wazaId", i64::from(id))?;
        }
    }
    if let Some(gem) = change.gem {
        put(poke, "gemType", gem)?;
    }
    if change.shiny {
        put(poke, "rareType", c.rare)?;
    }
    let Some(boss) = &change.boss else { return Ok(()) };
    if info.table("bossDesc").is_none() {
        return Ok(());
    }
    let desc = info.table_mut("bossDesc").map_err(shape)?;
    put(desc, "powerChargePenaltyAction", i64::from(boss.penalty))?;
    for (slot, cue) in boss.cued.iter().enumerate() {
        let Some(id) = *cue else { continue };
        let action = desc.table_mut(&format!("extraAction{}", slot + 1)).map_err(shape)?;
        if id == 0 {
            put(action, "timming", c.timing_none)?;
            put(action, "action", c.action_none)?;
        }
        put(action, "wazano", i64::from(id))?;
    }
    Ok(())
}

fn gem_label(ctx: &Ctx, c: &Consts, gem: i64) -> String {
    let types = c.gem_first_type..c.gem_first_type + TYPE_COUNT as i64;
    if gem == c.gem_default {
        "Own type".into()
    } else if gem == c.gem_stellar {
        "Stellar".into()
    } else if types.contains(&gem) {
        ctx.names.type_name((gem - c.gem_first_type) as u8)
    } else {
        "Any".into()
    }
}
