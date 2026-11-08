//! Learnsets, move data and TMs.

#[path = "pokemon_learnsets.rs"]
mod learnsets;
#[path = "pokemon_tms.rs"]
pub(super) mod tms;

use std::collections::BTreeSet;

use crate::rando::movepool::{self, Category, MoveTable, METRONOME, NEVER_MISSES, TYPE_COUNT};
use crate::rando::options::{Opt, OptionDef, Page, Req, Source, Status};
use crate::rando::{Ctx, RandoError, Rng, Section};

use super::evolutions::methods::{self, Needs};
use super::pokemon::table::{Families, LevelUpMove, Row, Species, ON_EVOLUTION};
use learnsets::{LevelUpRules, MovePool};
use tms::TmTable;

pub const SECTION: Section = Section {
    id: "moves",
    options,
    run,
};

const LEVELUP: &str = "moves.levelup";
const METRONOME_ONLY: &str = "moves.metronome_only";
const GUARANTEED: &str = "moves.guaranteed_level1";
const REORDER: &str = "moves.reorder_damaging";
const BAN_BROKEN: &str = "moves.ban_game_breaking";
const GOOD_PERCENT: &str = "moves.good_damaging_percent";
const EVOLUTION_MOVES: &str = "moves.evolution_moves";
const MIN_STAB: &str = "moves.min_stab";
const MIN_DAMAGING: &str = "moves.min_damaging";
const EXPAND: &str = "moves.expand_to";
const SPREAD: &str = "moves.spread_to";
const EGG_REMINDER: &str = "moves.egg_and_reminder";
const POWER: &str = "moves.data_power";
const ACCURACY: &str = "moves.data_accuracy";
const PP: &str = "moves.data_pp";
const MOVE_TYPE: &str = "moves.data_type";
const CATEGORY: &str = "moves.data_category";
const TM_MOVES: &str = "moves.tm_moves";
const TM_GOOD_PERCENT: &str = "moves.tm_good_damaging_percent";
const TM_COMPAT: &str = "moves.tm_compatibility";
const TM_FOLLOW: &str = "moves.tm_follow_evolutions";
const TM_SANITY: &str = "moves.tm_levelup_sanity";
const TM_LP: &str = "moves.tm_lp_cost";
const TM_MATERIALS: &str = "moves.tm_material_cost";

const LEARN_GROUP: &str = "Level-up moves";
const OTHER_LEARN_GROUP: &str = "Egg and reminder moves";
const DATA_GROUP: &str = "Move data";
const TM_GROUP: &str = "TMs";

/// UPR gives Metronome-only mode 40 PP, since it is the only move.
const METRONOME_PP: u8 = 40;

fn options() -> Vec<OptionDef> {
    let random_levelup = |o: Opt| o.not(LEVELUP, "unchanged").requires(Req::Off { id: METRONOME_ONLY });
    let not_metronome = |o: Opt| o.requires(Req::Off { id: METRONOME_ONLY });
    let costs = &[
        ("unchanged", "Unchanged", "Crafting costs what it always did."),
        ("reduced", "Halved", "Crafting costs half as much."),
        ("free", "Free", "Crafting costs nothing."),
    ];
    vec![
        not_metronome(Opt::choice(
            LEVELUP,
            Page::Moves,
            LEARN_GROUP,
            "Level-up moves",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon learns its usual moves."),
                ("prefer_type", "Random, preferring same type", "New moves, often matching the Pokémon's types."),
                ("random", "Random", "New moves of any type."),
            ],
        ))
        .help("Each Pokémon learns the same number of moves at the same levels, unless the options below change that. One of its level 1 moves is always a strong attack. Only moves the game can use are chosen.")
        .sources(&[Source::Upr, Source::Sv])
        .build(),
        Opt::toggle(METRONOME_ONLY, Page::Moves, LEARN_GROUP, "Metronome-only mode", false)
            .help("Every Pokémon knows only Metronome, which gets 40 PP. Egg and reminder moves are removed and no Pokémon can use TMs. Pokémon that evolve by knowing a move still learn that move.")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
        random_levelup(Opt::number(GUARANTEED, Page::Moves, LEARN_GROUP, "Guaranteed level 1 moves", 0, 0, 4, "moves"))
            .help("Every Pokémon knows at least this many moves at level 1. 0 keeps the usual number.")
            .sources(&[Source::Upr])
            .build(),
        not_metronome(Opt::toggle(REORDER, Page::Moves, LEARN_GROUP, "Reorder damaging moves by strength", false))
            .help("Damaging moves are learned from weakest to strongest. Works with unchanged moves too.")
            .sources(&[Source::Upr])
            .build(),
        not_metronome(Opt::toggle(BAN_BROKEN, Page::Moves, LEARN_GROUP, "Ban game-breaking moves", false))
            .help("Random level-up, egg, reminder and TM moves never include one-hit KO moves: Fissure, Sheer Cold, Horn Drill and Guillotine.")
            .sources(&[Source::Upr])
            .build(),
        random_levelup(Opt::number(GOOD_PERCENT, Page::Moves, LEARN_GROUP, "Good damaging moves", 0, 0, 100, "%"))
            .help("At least this share of each Pokémon's level-up moves are strong, accurate attacks.")
            .step(5)
            .sources(&[Source::Upr])
            .build(),
        random_levelup(Opt::toggle(EVOLUTION_MOVES, Page::Moves, LEARN_GROUP, "Evolution moves for all Pokémon", false))
            .help("Every Pokémon that evolves from another learns a random move the moment it evolves.")
            .sources(&[Source::Upr])
            .build(),
        random_levelup(Opt::number(MIN_STAB, Page::Moves, LEARN_GROUP, "Minimum same-type attacks", 0, 0, 10, "moves"))
            .help("Each level-up learnset has at least this many damaging moves that match the Pokémon's types.")
            .sources(&[Source::Pknx])
            .build(),
        random_levelup(Opt::number(MIN_DAMAGING, Page::Moves, LEARN_GROUP, "Minimum damaging moves", 0, 0, 10, "moves"))
            .help("Each level-up learnset has at least this many damaging moves.")
            .sources(&[Source::Pknx])
            .build(),
        random_levelup(Opt::number(EXPAND, Page::Moves, LEARN_GROUP, "Expand learnsets to", 0, 0, 40, "moves"))
            .help("Learnsets with fewer moves get extra moves at higher levels. 0 keeps their size.")
            .sources(&[Source::Pknx])
            .build(),
        random_levelup(Opt::number(SPREAD, Page::Moves, LEARN_GROUP, "Spread moves up to level", 0, 0, 100, ""))
            .help("Moves are learned at evenly spaced levels from 1 up to this level. 0 keeps the usual levels.")
            .step(5)
            .sources(&[Source::Pknx])
            .build(),
        not_metronome(Opt::toggle(EGG_REMINDER, Page::Moves, OTHER_LEARN_GROUP, "Random egg and reminder moves", false))
            .help("Egg moves and the extra moves the Move Reminder teaches are replaced. They prefer the Pokémon's types unless level-up moves are fully random.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(POWER, Page::Moves, DATA_GROUP, "Random move power", false)
            .help("Attacks with a fixed power get a new power, usually 50 to 100. Multi-hit moves are scaled down.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(ACCURACY, Page::Moves, DATA_GROUP, "Random move accuracy", false)
            .help("Moves that can miss get a new accuracy. Accurate moves tend to stay accurate. Moves that never miss stay that way.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(PP, Page::Moves, DATA_GROUP, "Random move PP", false)
            .help("Every move gets new PP from 5 to 40.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(MOVE_TYPE, Page::Moves, DATA_GROUP, "Random move types", false)
            .help("Every move gets a random type.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(CATEGORY, Page::Moves, DATA_GROUP, "Random move category", false)
            .help("Each attack has a 50% chance to swap between physical and special.")
            .sources(&[Source::Upr])
            .build(),
        not_metronome(Opt::toggle(TM_MOVES, Page::Moves, TM_GROUP, "Random TM moves", false))
            .help("Each TM teaches a different random move. No two TMs teach the same move.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::number(TM_GOOD_PERCENT, Page::Moves, TM_GROUP, "Good damaging TMs", 0, 0, 100, "%")
            .help("At least this share of TMs teach strong, accurate attacks.")
            .step(5)
            .on(TM_MOVES)
            .requires(Req::Off { id: METRONOME_ONLY })
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        not_metronome(Opt::choice(
            TM_COMPAT,
            Page::Moves,
            TM_GROUP,
            "TM compatibility",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Each Pokémon can use the same TMs as before, whatever they teach now."),
                ("prefer_type", "Random, preferring same type", "TMs of the Pokémon's types are compatible 90% of the time, Normal 50%, others 25%."),
                ("random", "Random", "Every TM has a 50% chance to be compatible."),
                ("full", "Full", "Every Pokémon can use every TM."),
            ],
        ))
        .help("Which Pokémon can use each TM.")
        .sources(&[Source::Upr])
        .build(),
        not_metronome(Opt::toggle(TM_FOLLOW, Page::Moves, TM_GROUP, "TM compatibility follows evolutions", false))
            .help("An evolution can use every TM its pre-evolution could, and sometimes more.")
            .not(TM_COMPAT, "unchanged")
            .not(TM_COMPAT, "full")
            .sources(&[Source::Upr])
            .build(),
        not_metronome(Opt::toggle(TM_SANITY, Page::Moves, TM_GROUP, "TM and level-up move sanity", false))
            .help("A Pokémon can use every TM whose move it learns by level-up. With the option above, evolutions also keep their pre-evolution's TMs.")
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(TM_LP, Page::Moves, TM_GROUP, "TM crafting LP cost", "unchanged", costs)
            .help("The League Points the TM Machine charges.")
            .sources(&[Source::New])
            .build(),
        Opt::choice(TM_MATERIALS, Page::Moves, TM_GROUP, "TM crafting materials", "unchanged", costs)
            .help("The Pokémon materials each TM needs. Halved rounds up, so a TM always needs at least one of each material it used.")
            .sources(&[Source::New])
            .build(),
    ]
}

struct Plan {
    levelup: String,
    metronome: bool,
    egg_reminder: bool,
    reorder: bool,
    tm_moves: bool,
    tm_compat: String,
    tm_sanity: bool,
}

impl Plan {
    fn read(ctx: &Ctx) -> Plan {
        let metronome = ctx.settings.on(METRONOME_ONLY);
        let s = &ctx.settings;
        Plan {
            levelup: if metronome { "unchanged".into() } else { s.choice(LEVELUP).to_owned() },
            metronome,
            egg_reminder: !metronome && s.on(EGG_REMINDER),
            reorder: !metronome && s.on(REORDER),
            tm_moves: !metronome && s.on(TM_MOVES),
            tm_compat: if metronome { "unchanged".into() } else { s.choice(TM_COMPAT).to_owned() },
            tm_sanity: !metronome && s.on(TM_SANITY),
        }
    }

    fn touches_species(&self) -> bool {
        self.levelup != "unchanged"
            || self.metronome
            || self.egg_reminder
            || self.reorder
            || self.tm_moves
            || self.tm_compat != "unchanged"
            || self.tm_sanity
    }
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let plan = Plan::read(ctx);
    let data_on = [POWER, ACCURACY, PP, MOVE_TYPE, CATEGORY].iter().any(|id| ctx.settings.on(id));
    let costs_on = ctx.settings.choice(TM_LP) != "unchanged" || ctx.settings.choice(TM_MATERIALS) != "unchanged";
    if !plan.touches_species() && !data_on && !costs_on {
        return Ok(());
    }
    let mut moves = MoveTable::load(ctx)?;
    randomize_move_data(ctx, &mut moves);
    if plan.metronome {
        if let Some(m) = moves.moves.get_mut(usize::from(METRONOME)) {
            m.pp = METRONOME_PP;
        }
    }
    let mut tms = TmTable::load(ctx)?;
    let types_changed = moves.changed().any(|id| moves.get(id).map(|m| m.type_id) != moves.before(id).map(|m| m.type_id));
    if plan.touches_species() || types_changed {
        let mut species = Species::load(ctx)?;
        let banned = if ctx.settings.on(BAN_BROKEN) && !plan.metronome {
            movepool::ids_named(ctx, moves.moves.len(), movepool::GAME_BREAKING)
        } else {
            BTreeSet::new()
        };
        let poor = movepool::ids_named(ctx, moves.moves.len(), movepool::POOR_ATTACKS);
        let pool = MovePool::new(&moves, &banned, &poor);
        let twin = species.twins(|r| (r.types, r.levelup.clone(), r.egg.clone(), r.reminder.clone(), r.tm.clone()));
        let fam = species.families(|i| twin[i].is_none());
        randomize_learnsets(ctx, &plan, &pool, &mut species, &twin, &fam);
        species.copy_twins(&twin, |row, from| {
            row.levelup = from.levelup.clone();
            row.egg = from.egg.clone();
            row.reminder = from.reminder.clone();
        });
        if plan.levelup != "unchanged" || plan.metronome || plan.egg_reminder || types_changed {
            keep_evolutions_possible(ctx, &pool, &mut species);
        }
        randomize_tms(ctx, &plan, &pool, &mut species, &mut tms, &twin, &fam);
        species.copy_twins(&twin, |row, from| row.tm = from.tm.clone());
        if plan.reorder {
            for row in species.rows.iter_mut().filter(|r| r.present) {
                learnsets::order_by_strength(&moves, &mut row.levelup);
            }
        }
        log_species(ctx, &species, &tms);
        species.save(ctx)?;
    }
    apply_costs(ctx, &mut tms);
    log_moves(ctx, &moves, &tms);
    moves.save(ctx)?;
    tms.save(ctx)
}

fn randomize_move_data(ctx: &Ctx, table: &mut MoveTable) {
    let on = |id: &str| ctx.settings.on(id);
    let (power, accuracy, pp, kind, category) = (on(POWER), on(ACCURACY), on(PP), on(MOVE_TYPE), on(CATEGORY));
    let mut streams = [
        ctx.rng("moves.data.power"),
        ctx.rng("moves.data.accuracy"),
        ctx.rng("moves.data.pp"),
        ctx.rng("moves.data.type"),
        ctx.rng("moves.data.category"),
    ];
    let assignable = table.assignable().clone();
    for m in table.moves.iter_mut().filter(|m| assignable.contains(&m.id)) {
        let [r_power, r_acc, r_pp, r_type, r_cat] = &mut streams;
        if power && m.damaging() && m.power >= 10 {
            let mut p = if r_power.below(3) != 2 { 50 + 5 * r_power.below(11) } else { 20 + 5 * r_power.below(27) };
            for _ in 0..2 {
                if r_power.below(100) == 0 {
                    p += 50;
                }
            }
            let hits = m.expected_hits();
            if hits > 1.0 {
                p = (((p as f64 / hits) / 5.0).round() * 5.0).max(5.0) as u64;
            }
            m.power = p.min(250) as u8;
        }
        if accuracy && m.accuracy >= 5 && m.accuracy != NEVER_MISSES {
            m.accuracy = new_accuracy(r_acc, m.accuracy);
        }
        if pp {
            m.pp = if r_pp.below(3) != 2 { 15 + 5 * r_pp.below(3) as u8 } else { 5 + 5 * r_pp.below(8) as u8 };
        }
        if kind {
            m.type_id = r_type.below(u64::from(TYPE_COUNT)) as u8;
        }
        if category && m.damaging() && r_cat.below(2) == 0 {
            m.category = if m.category == Category::Physical { Category::Special } else { Category::Physical };
        }
    }
}

/// UPR's accuracy tiers: inaccurate moves stay inaccurate, accurate ones mostly stay near 100.
fn new_accuracy(rng: &mut Rng, old: u8) -> u8 {
    if old <= 50 {
        let a = 20 + 5 * rng.below(7) as u8;
        return if rng.below(10) == 0 { (a * 3 / 2) / 5 * 5 } else { a };
    }
    let stop_chance = if old < 90 { 2 } else { 4 };
    let mut a = 100;
    while a > 20 && rng.below(10) >= stop_chance {
        a -= 5;
    }
    a
}

fn randomize_learnsets(ctx: &Ctx, plan: &Plan, pool: &MovePool, species: &mut Species, twin: &[Option<usize>], fam: &Families) {
    let leaders: Vec<usize> = (0..species.rows.len())
        .filter(|&i| species.rows[i].present && twin[i].is_none())
        .collect();
    if plan.metronome {
        for &i in &leaders {
            let row = &mut species.rows[i];
            row.levelup = vec![LevelUpMove { move_id: METRONOME, level: 1 }];
            row.egg.clear();
            row.reminder.clear();
        }
        return;
    }
    if plan.levelup != "unchanged" {
        let s = &ctx.settings;
        let mut rng = ctx.rng("moves.levelup");
        let base = LevelUpRules {
            prefer_type: plan.levelup == "prefer_type",
            guaranteed_level1: s.number(GUARANTEED) as usize,
            good_percent: s.number(GOOD_PERCENT) as u32,
            add_evolution_move: false,
            min_damaging: s.number(MIN_DAMAGING) as usize,
            min_stab: s.number(MIN_STAB) as usize,
            expand_to: s.number(EXPAND) as usize,
            spread_to: s.number(SPREAD) as u8,
        };
        let evolution_moves = s.on(EVOLUTION_MOVES);
        for &i in &leaders {
            let rules = LevelUpRules {
                add_evolution_move: evolution_moves && fam.parent(i).is_some(),
                ..base
            };
            species.rows[i].levelup = learnsets::randomize_levelup(&mut rng, pool, &species.rows[i], &rules);
        }
    }
    if plan.egg_reminder {
        let prefer = plan.levelup != "random";
        let mut rng = ctx.rng("moves.egg_reminder");
        for &i in &leaders {
            let row = &species.rows[i];
            let egg = learnsets::randomize_list(&mut rng, pool, row, row.egg.len(), prefer);
            let reminder = learnsets::randomize_list(&mut rng, pool, row, row.reminder.len(), prefer);
            species.rows[i].egg = egg;
            species.rows[i].reminder = reminder;
        }
    }
}

/// UPR's per-TM odds.
fn compat_chance(mode: &str, row: &Row, move_type: u8) -> u32 {
    match mode {
        "prefer_type" if row.has_type(move_type) => 90,
        "prefer_type" if move_type == movepool::NORMAL => 50,
        "prefer_type" => 25,
        _ => 50,
    }
}

fn randomize_tms(
    ctx: &Ctx,
    plan: &Plan,
    pool: &MovePool,
    species: &mut Species,
    tms: &mut TmTable,
    twin: &[Option<usize>],
    fam: &Families,
) {
    let old_moves = tms.moves();
    if plan.tm_moves {
        let mut rng = ctx.rng("moves.tm");
        let mut any: Vec<u16> = pool.all().to_vec();
        let mut good: Vec<u16> = pool.good().to_vec();
        let percent = ctx.settings.number(TM_GOOD_PERCENT) as f64;
        let mut good_left = (percent * tms.tms.len() as f64 / 100.0).round() as usize;
        let mut picks = Vec::new();
        for _ in 0..tms.tms.len() {
            let from = if good_left > 0 && !good.is_empty() { &good } else { &any };
            let Some(&m) = rng.pick(from) else { break };
            any.retain(|&x| x != m);
            good.retain(|&x| x != m);
            good_left = good_left.saturating_sub(1);
            picks.push(m);
        }
        rng.shuffle(&mut picks);
        for (tm, m) in tms.tms.iter_mut().zip(picks) {
            tm.move_id = m;
        }
    }
    let new_moves = tms.moves();
    let table = pool.table();
    let move_type = |m: u16| table.get(m).map_or(0, |i| i.type_id);
    let n = species.rows.len();
    let mut flags: Vec<Vec<bool>> = (0..n)
        .map(|i| tms::compatibility(&species.rows[i], &old_moves))
        .collect();
    let mode = plan.tm_compat.as_str();
    let follow = ctx.settings.on(TM_FOLLOW) && mode != "unchanged" && mode != "full";
    if plan.metronome {
        for f in &mut flags {
            f.fill(false);
        }
    } else if mode == "full" {
        for f in &mut flags {
            f.fill(true);
        }
    } else if mode != "unchanged" {
        let mut rng = ctx.rng("moves.tm_compatibility");
        for &i in fam.top_down() {
            let row = &species.rows[i];
            let parent = fam.parent(i).filter(|_| follow);
            flags[i] = new_moves
                .iter()
                .enumerate()
                .map(|(s, &m)| {
                    let t = move_type(m);
                    match parent {
                        Some(p) if flags[p][s] => true,
                        Some(p) => {
                            let new_type = row.has_type(t) && !species.rows[p].has_type(t);
                            let chance = match mode {
                                "prefer_type" if new_type => 90,
                                "prefer_type" => 10,
                                _ => 25,
                            };
                            rng.percent(chance)
                        }
                        None => rng.percent(compat_chance(mode, row, t)),
                    }
                })
                .collect();
        }
    }
    if plan.tm_sanity {
        for (i, f) in flags.iter_mut().enumerate() {
            for (s, &m) in new_moves.iter().enumerate() {
                f[s] |= species.rows[i].learns_by_level(m);
            }
        }
        if follow {
            for &i in fam.top_down() {
                if let Some(p) = fam.parent(i) {
                    let inherited = flags[p].clone();
                    for (f, from) in flags[i].iter_mut().zip(inherited) {
                        *f |= from;
                    }
                }
            }
        }
    }
    for (i, f) in flags.iter().enumerate() {
        if species.rows[i].present && twin[i].is_none() {
            tms::write_compatibility(&mut species.rows[i], f, &old_moves, &new_moves);
        }
    }
}

/// Inserts `m` into a learnset sorted by level.
fn learn_at(levelup: &mut Vec<LevelUpMove>, move_id: u16, level: i8) {
    let at = levelup.iter().position(|x| x.level > level).unwrap_or(levelup.len());
    levelup.insert(at, LevelUpMove { move_id, level });
}

/// Pokémon that evolve by knowing a move, or a move of some type, must still learn one by level-up
/// or from the Move Reminder. Otherwise they learn it here, at the level they used to, or at
/// level 1. Runs before TM compatibility so the TM sanity option sees these moves.
fn keep_evolutions_possible(ctx: &Ctx, pool: &MovePool, species: &mut Species) {
    let table = pool.table();
    for i in 0..species.rows.len() {
        if !species.rows[i].playable {
            continue;
        }
        let evos = species.rows[i].evolutions.clone();
        for evo in evos {
            let row = &species.rows[i];
            let knows = |m: u16| row.learns_by_level(m) || row.reminder.contains(&m);
            let needed = match methods::needs(evo.condition) {
                Needs::Nothing => None,
                Needs::ParamMove => Some(evo.parameter).filter(|&m| !knows(m)),
                Needs::Move(name) => movepool::ids_named(ctx, table.moves.len(), &[name]).into_iter().next().filter(|&m| !knows(m)),
                Needs::MoveOfParamType => {
                    let of_type = |m: u16| table.get(m).is_some_and(|x| x.type_id == evo.parameter as u8);
                    let mut known = row.levelup.iter().map(|x| x.move_id).chain(row.reminder.iter().copied());
                    if known.any(of_type) {
                        None
                    } else {
                        let used: Vec<u16> = species.before(i).levelup.iter().map(|x| x.move_id).collect();
                        used.iter()
                            .copied()
                            .filter(|m| pool.all().contains(m))
                            .find(|&m| of_type(m))
                            .or_else(|| pool.of_type(evo.parameter as u8).iter().copied().find(|&m| !knows(m)))
                    }
                }
            };
            if let Some(m) = needed {
                let level = species
                    .before(i)
                    .levelup
                    .iter()
                    .find(|x| x.move_id == m && x.level > 0)
                    .map_or(1, |x| x.level);
                learn_at(&mut species.rows[i].levelup, m, level);
            }
        }
    }
}

fn apply_costs(ctx: &Ctx, tms: &mut TmTable) {
    let lp = ctx.settings.choice(TM_LP);
    let materials = ctx.settings.choice(TM_MATERIALS);
    for tm in &mut tms.tms {
        match lp {
            "reduced" => tm.lp = (tm.lp + 1) / 2,
            "free" => tm.lp = 0,
            _ => {}
        }
        for m in &mut tm.materials {
            match materials {
                "reduced" if m.count > 0 => m.count = (m.count + 1) / 2,
                "free" => {
                    *m = tms::Material { item: 0, count: 0, source: 0 };
                }
                _ => {}
            }
        }
    }
}

fn learnset_text(ctx: &Ctx, levelup: &[LevelUpMove]) -> String {
    levelup
        .iter()
        .map(|m| {
            let at = if m.level == ON_EVOLUTION { "Evo".to_owned() } else { m.level.to_string() };
            format!("{at} {}", ctx.names.move_name(m.move_id))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn list_text(ctx: &Ctx, moves: &[u16]) -> String {
    moves.iter().map(|&m| ctx.names.move_name(m)).collect::<Vec<_>>().join(", ")
}

fn log_species(ctx: &mut Ctx, species: &Species, tms: &TmTable) {
    let changed: Vec<usize> = species.changed().filter(|&i| species.rows[i].playable).collect();
    let mut levelup = Vec::new();
    let mut other = Vec::new();
    let mut compat = Vec::new();
    let tm_moves = tms.moves();
    for &i in &changed {
        let (old, new) = (species.before(i), &species.rows[i]);
        let name = ctx.names.pokemon(new.id);
        if new.levelup != old.levelup {
            levelup.push(vec![name.clone(), learnset_text(ctx, &new.levelup)]);
        }
        if new.egg != old.egg || new.reminder != old.reminder {
            other.push(vec![name.clone(), list_text(ctx, &new.egg), list_text(ctx, &new.reminder)]);
        }
        if new.tm != old.tm {
            let list: Vec<String> = tms
                .tms
                .iter()
                .zip(&tm_moves)
                .filter(|(_, m)| new.tm.contains(m))
                .map(|(tm, _)| ctx.names.item(tm.item).trim_start_matches("TM").to_owned())
                .collect();
            compat.push(vec![name, list.len().to_string(), list.join(" ")]);
        }
    }
    if levelup.is_empty() && other.is_empty() && compat.is_empty() {
        return;
    }
    let section = ctx.log.section("Moves & TMs");
    for (title, headers, rows) in [
        ("Level-up moves", &["Pokémon", "Level-up moves"][..], levelup),
        ("Egg and reminder moves", &["Pokémon", "Egg moves", "Reminder moves"][..], other),
        ("TM compatibility", &["Pokémon", "Count", "TMs"][..], compat),
    ] {
        if !rows.is_empty() {
            let table = section.table(title, headers);
            for r in rows {
                table.row(r);
            }
        }
    }
}

fn log_moves(ctx: &mut Ctx, moves: &MoveTable, tms: &TmTable) {
    let mut data = Vec::new();
    for id in moves.changed() {
        let (Some(new), Some(old)) = (moves.get(id), moves.before(id)) else { continue };
        let pair = |a: String, b: String| if a == b { a } else { format!("{b} → {a}") };
        data.push(vec![
            ctx.names.move_name(id),
            pair(ctx.names.type_name(new.type_id), ctx.names.type_name(old.type_id)),
            pair(new.category.label().into(), old.category.label().into()),
            pair(new.power.to_string(), old.power.to_string()),
            pair(new.accuracy.to_string(), old.accuracy.to_string()),
            pair(new.pp.to_string(), old.pp.to_string()),
        ]);
    }
    let mut machine = Vec::new();
    for i in tms.changed() {
        let (new, old) = (&tms.tms[i], &tms.before()[i]);
        let materials: Vec<String> = new
            .materials
            .iter()
            .filter(|m| m.item != 0)
            .map(|m| format!("{} x{}", ctx.names.item(m.item), m.count))
            .collect();
        let move_text = if new.move_id == old.move_id {
            ctx.names.move_name(new.move_id)
        } else {
            format!("{} (was {})", ctx.names.move_name(new.move_id), ctx.names.move_name(old.move_id))
        };
        machine.push(vec![ctx.names.item(new.item), move_text, new.lp.to_string(), materials.join(", ")]);
    }
    if data.is_empty() && machine.is_empty() {
        return;
    }
    let section = ctx.log.section("Moves & TMs");
    if !data.is_empty() {
        let table = section.table("Move data", &["Move", "Type", "Category", "Power", "Accuracy", "PP"]);
        for r in data {
            table.row(r);
        }
    }
    if !machine.is_empty() {
        let table = section.table("TMs", &["TM", "Move", "LP", "Materials"]);
        for r in machine {
            table.row(r);
        }
    }
}
