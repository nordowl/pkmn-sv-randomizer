//! Evolutions: random targets, an evolution at every level, and replacements for methods that are
//! impossible or awkward on an emulator.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::{Evolution, Pokemon};
use crate::rando::options::{Kind, Opt, OptionDef, Page, Source};
use crate::rando::pick::Picker;
use crate::rando::pool::always_excluded;
use crate::rando::{Ctx, RandoError, Rng, Section};

use super::pokemon::table::{Row, Species};
use methods::{code, Arg};

#[path = "pokemon_methods.rs"]
pub(super) mod methods;

pub const SECTION: Section = Section {
    id: "evolutions",
    options,
    run,
};

const MODE: &str = "evolutions.mode";
const POOL: &str = "evolutions.pool";
const SIMILAR: &str = "evolutions.similar_strength";
const SAME_TYPE: &str = "evolutions.same_typing";
const THREE_STAGES: &str = "evolutions.limit_three_stages";
const FORCE_CHANGE: &str = "evolutions.force_change";
const ALT_FORMS: &str = "evolutions.allow_alt_forms";
const TRADE: &str = "evolutions.replace_trade";
const TIME: &str = "evolutions.replace_time_and_region";
const EASIER: &str = "evolutions.easier";

const GROUP: &str = "Evolutions";
const FIX_GROUP: &str = "Evolution methods";

/// The game's item IDs for evolution items are separate from its item IDs; this table maps them.
pub const PLIB_ITEMS: &str = "world/data/battle/plib_item_conversion/plib_item_conversion_array.bin";

/// UPR's level for trade evolutions turned into level-ups.
const TRADE_LEVEL: u16 = 37;
/// UPR's caps for "make evolutions easier": middle stages by 30, final stages by 40.
const MIDDLE_CAP: u16 = 30;
const FINAL_CAP: u16 = 40;

fn options() -> Vec<OptionDef> {
    let changes = |o: Opt| o.not(MODE, "unchanged");
    let keeps_methods = |o: Opt| o.not(MODE, "every_level");
    vec![
        Opt::choice(
            MODE,
            Page::Evolutions,
            GROUP,
            "Evolutions",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon evolves as usual."),
                ("random", "Random", "Each evolution leads to a random Pokémon. The method, such as a level or a stone, stays the same."),
                ("every_level", "Random at every level", "Every Pokémon evolves into a random Pokémon each time it levels up."),
            ],
        )
        .help("An evolution always keeps its EXP curve, so levels stay correct. Standardize EXP curves on the stats page to widen the choice.")
        .sources(&[Source::Upr, Source::Sv])
        .build(),
        changes(Opt::new(POOL, Page::Evolutions, GROUP, "Pokémon evolutions can lead to", Kind::Pool))
            .help("Overrides the global species pool for evolution targets.")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(SIMILAR, Page::Evolutions, GROUP, "Similar strength", false)
            .help("Each new evolution has a base stat total close to the evolution it replaces.")
            .is(MODE, "random")
            .sources(&[Source::Upr])
            .build(),
        changes(Opt::toggle(SAME_TYPE, Page::Evolutions, GROUP, "Same typing", false))
            .help("Each new evolution shares a type with the Pokémon that evolves. For Pokémon with many evolutions, like Eevee, it shares a type with the evolution it replaces.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(THREE_STAGES, Page::Evolutions, GROUP, "Limit to three stages", false)
            .help("No evolution line gets longer than three Pokémon.")
            .is(MODE, "random")
            .sources(&[Source::Upr])
            .build(),
        changes(Opt::toggle(FORCE_CHANGE, Page::Evolutions, GROUP, "Force a change", false))
            .help("No Pokémon evolves into the same Pokémon as before.")
            .sources(&[Source::Upr])
            .build(),
        changes(Opt::toggle(ALT_FORMS, Page::Evolutions, GROUP, "Allow alternate forms", false))
            .help("New evolutions can be regional and other alternate forms, such as Paldean Tauros. Off allows only standard forms.")
            .sources(&[Source::Upr])
            .build(),
        keeps_methods(Opt::toggle(TRADE, Page::Evolutions, FIX_GROUP, "Replace trade evolutions with level-ups", false))
            .help("Trading and Union Circle evolutions need a second player. Trades become a level-up at level 37, or at the Pokémon's other evolution level. A trade holding an item becomes a level-up while holding it where the game allows, otherwise a plain level-up. If two evolutions would then happen the same way, one happens during the day and the other at night.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        keeps_methods(Opt::toggle(TIME, Page::Evolutions, FIX_GROUP, "Replace time-of-day and regional evolutions", false))
            .help("Day, night and dusk evolutions work at any time, and evolutions that need the Hisui region become level-ups. If two evolutions would then happen the same way, like Eevee's Espeon and Umbreon, one needs a Sun Stone and the other a Moon Stone.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        keeps_methods(Opt::toggle(EASIER, Page::Evolutions, FIX_GROUP, "Make evolutions easier", false))
            .help("Level evolutions happen by level 40, or level 30 for Pokémon that evolve again. Inkay and Sliggoo no longer need an upside-down console or rain. Step, coin, win and Rage Fist counts drop to a quarter.")
            .sources(&[Source::Upr])
            .build(),
    ]
}

struct EvoItems {
    to_evo: BTreeMap<u16, u16>,
    to_item: BTreeMap<u16, u16>,
}

impl EvoItems {
    fn load(ctx: &mut Ctx) -> Result<EvoItems, RandoError> {
        let mut to_evo = BTreeMap::new();
        let mut to_item = BTreeMap::new();
        for row in ctx.store.get(PLIB_ITEMS)?.tables("values") {
            let (Ok(item), Ok(plib)) = (row.int("itemID"), row.int("plibID")) else { continue };
            if item > 0 && plib > 0 {
                to_evo.insert(item as u16, plib as u16);
                to_item.insert(plib as u16, item as u16);
            }
        }
        Ok(EvoItems { to_evo, to_item })
    }

    fn named(&self, ctx: &Ctx, name: &str) -> Option<u16> {
        self.to_evo
            .iter()
            .find(|(&item, _)| ctx.names.item(item) == name)
            .map(|(_, &evo)| evo)
    }
}

/// Describes an evolution method for the spoiler log.
fn describe(ctx: &Ctx, evo: &Evolution, items: &BTreeMap<u16, u16>) -> String {
    let Some(method) = methods::method(evo.condition) else {
        return format!("Method {} ({})", evo.condition, evo.parameter);
    };
    let (text, p) = (method.text, evo.parameter);
    let mut out = match method.arg {
        Arg::None => text.to_owned(),
        Arg::Item => format!("{text} {}", ctx.names.item(p)),
        Arg::EvoItem => format!("{text} {}", items.get(&p).map_or(format!("evolution item {p}"), |&i| ctx.names.item(i))),
        Arg::Move => format!("{text} {}", ctx.names.move_name(p)),
        Arg::Type => format!("{text} {}", ctx.names.type_name(p as u8)),
        Arg::Species => format!("{text} {}", ctx.names.species(p)),
        Arg::Count => format!("{text} {p}"),
    };
    if evo.level > 0 {
        out.push_str(&format!(" at level {}", evo.level));
    }
    out
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let mode = ctx.settings.choice(MODE).to_owned();
    let fixes = mode != "every_level" && [TRADE, TIME, EASIER].iter().any(|id| ctx.settings.on(id));
    if mode == "unchanged" && !fixes {
        return Ok(());
    }
    let mut species = Species::load(ctx)?;
    let items = EvoItems::load(ctx)?;
    let dropped = match mode.as_str() {
        "random" => randomize_targets(ctx, &mut species)?,
        "every_level" => {
            evolve_every_level(ctx, &mut species)?;
            Vec::new()
        }
        _ => Vec::new(),
    };
    if fixes {
        fix_methods(ctx, &mut species, &items);
    }
    log(ctx, &species, &items.to_item, &dropped);
    species.save(ctx)
}

fn fix_methods(ctx: &Ctx, species: &mut Species, items: &EvoItems) {
    let trade = ctx.settings.on(TRADE);
    let time = ctx.settings.on(TIME);
    let easier = ctx.settings.on(EASIER);
    let stones: Vec<u16> = ["Sun Stone", "Moon Stone", "Dusk Stone"]
        .iter()
        .filter_map(|name| items.named(ctx, name))
        .collect();
    let evolves_again: Vec<bool> = species.rows.iter().map(|r| !r.evolutions.is_empty()).collect();
    for i in 0..species.rows.len() {
        if !species.rows[i].playable || species.rows[i].evolutions.is_empty() {
            continue;
        }
        let mut evos = species.rows[i].evolutions.clone();
        if trade {
            replace_trades(&mut evos, &items.to_evo);
        }
        if time {
            replace_time_and_region(&mut evos);
        }
        if easier {
            let target_evolves = |p: Pokemon| species.row_of(p).is_some_and(|t| evolves_again[t]);
            make_easier(&mut evos, target_evolves);
        }
        if evos != species.rows[i].evolutions {
            resolve_clashes(&mut evos, time.then_some(stones.as_slice()));
            species.rows[i].evolutions = evos;
        }
    }
}

fn level_up_level(evos: &[Evolution]) -> u16 {
    evos.iter()
        .find(|e| e.condition == code::LEVEL && e.level > 0)
        .map_or(TRADE_LEVEL, |e| e.level)
}

fn level_up(e: &mut Evolution, level: u16) {
    e.condition = code::LEVEL;
    e.parameter = 0;
    e.level = level;
}

fn replace_trades(evos: &mut Vec<Evolution>, to_evo: &BTreeMap<u16, u16>) {
    let level = level_up_level(evos);
    let mut out = Vec::new();
    for mut e in evos.drain(..) {
        match e.condition {
            code::TRADE | code::TRADE_SPECIES => level_up(&mut e, level),
            code::UNION_CIRCLE => {
                let at = e.level;
                level_up(&mut e, at);
            }
            code::TRADE_ITEM => match to_evo.get(&e.parameter) {
                Some(&evo_item) => {
                    let day = Evolution {
                        condition: code::HELD_ITEM_DAY,
                        parameter: evo_item,
                        level: 0,
                        into: e.into,
                    };
                    out.push(day);
                    e = Evolution {
                        condition: code::HELD_ITEM_NIGHT,
                        ..day
                    };
                }
                None => level_up(&mut e, level),
            },
            _ => {}
        }
        out.push(e);
    }
    *evos = out;
}

fn replace_time_and_region(evos: &mut Vec<Evolution>) {
    let level = level_up_level(evos);
    let mut extra = Vec::new();
    for e in evos.iter_mut() {
        match e.condition {
            code::HISUI => {
                let at = if e.level > 0 { e.level } else { level };
                level_up(e, at);
            }
            code::FRIENDSHIP_DAY | code::FRIENDSHIP_NIGHT => e.condition = code::FRIENDSHIP,
            code::LEVEL_DAY | code::LEVEL_NIGHT | code::LEVEL_DUSK => {
                let at = e.level;
                level_up(e, at);
            }
            code::HELD_ITEM_DAY | code::HELD_ITEM_NIGHT => {
                let other = if e.condition == code::HELD_ITEM_DAY { code::HELD_ITEM_NIGHT } else { code::HELD_ITEM_DAY };
                extra.push(Evolution { condition: other, ..*e });
            }
            _ => {}
        }
    }
    for e in extra {
        let exists = evos
            .iter()
            .any(|x| x.condition == e.condition && x.parameter == e.parameter && x.into == e.into);
        if !exists {
            evos.push(e);
        }
    }
}

fn make_easier(evos: &mut [Evolution], target_evolves: impl Fn(Pokemon) -> bool) {
    for e in evos.iter_mut() {
        if matches!(e.condition, code::UPSIDE_DOWN | code::WEATHER) {
            let at = e.level;
            level_up(e, at);
        }
        if e.level > 0 {
            let cap = if target_evolves(e.into) { MIDDLE_CAP } else { FINAL_CAP };
            e.level = e.level.min(cap);
        }
        if methods::method(e.condition).is_some_and(|m| m.arg == Arg::Count) && e.parameter > 0 {
            e.parameter = (e.parameter / 4).max(1);
        }
    }
}

/// When a replacement leaves two evolutions with the same trigger, the game would only ever take
/// the first. Splits them by time of day, or by stone when time-of-day evolutions are removed.
fn resolve_clashes(evos: &mut [Evolution], stones: Option<&[u16]>) {
    for trigger in [code::LEVEL, code::FRIENDSHIP] {
        let clashing: Vec<usize> = (0..evos.len())
            .filter(|&i| evos[i].condition == trigger && evos[i].parameter == 0)
            .collect();
        if clashing.len() < 2 || clashing.iter().all(|&i| evos[i].into == evos[clashing[0]].into) {
            continue;
        }
        let shadowed = clashing.windows(2).any(|w| evos[w[1]].level >= evos[w[0]].level);
        if !shadowed {
            continue;
        }
        for (k, &i) in clashing.iter().enumerate() {
            match stones {
                Some(stones) => {
                    if let Some(&stone) = stones.get(k) {
                        evos[i] = Evolution {
                            condition: code::USE_ITEM,
                            parameter: stone,
                            level: 0,
                            into: evos[i].into,
                        };
                    }
                }
                None => {
                    let (day, night) = if trigger == code::LEVEL {
                        (code::LEVEL_DAY, code::LEVEL_NIGHT)
                    } else {
                        (code::FRIENDSHIP_DAY, code::FRIENDSHIP_NIGHT)
                    };
                    match k {
                        0 => evos[i].condition = day,
                        1 => evos[i].condition = night,
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Who may evolve and what they may evolve into.
struct Targets {
    rows: Vec<usize>,
}

impl Targets {
    fn new(ctx: &Ctx, species: &Species) -> Result<Targets, RandoError> {
        let alt_forms = ctx.settings.on(ALT_FORMS);
        let rows = ctx
            .pool(POOL, "evolution targets")?
            .into_iter()
            .filter(|p| alt_forms || p.form == 0)
            .filter_map(|p| species.row_of(p))
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return Err(RandoError::EmptyPool {
                what: "evolution targets".into(),
            });
        }
        Ok(Targets { rows })
    }
}

fn shares_type(a: &Row, b: &Row) -> bool {
    a.types.iter().any(|&t| b.has_type(t))
}

/// Keeps the candidates that pass `keep`, unless none would.
fn prefer(candidates: Vec<usize>, keep: impl Fn(usize) -> bool) -> Vec<usize> {
    let kept: Vec<usize> = candidates.iter().copied().filter(|&c| keep(c)).collect();
    if kept.is_empty() {
        candidates
    } else {
        kept
    }
}

/// The new evolution graph, built one edge at a time so it never gains a cycle.
struct Graph {
    children: Vec<Vec<usize>>,
    parents: Vec<Vec<usize>>,
}

impl Graph {
    fn reaches(&self, from: usize, to: usize) -> bool {
        let mut stack = vec![from];
        let mut seen = BTreeSet::new();
        while let Some(n) = stack.pop() {
            if n == to {
                return true;
            }
            if seen.insert(n) {
                stack.extend(&self.children[n]);
            }
        }
        false
    }

    fn depth_above(&self, n: usize) -> usize {
        self.parents[n].iter().map(|&p| 1 + self.depth_above(p)).max().unwrap_or(0)
    }

    fn depth_below(&self, n: usize, pending: &dyn Fn(usize) -> usize) -> usize {
        let now = self.children[n].iter().map(|&c| 1 + self.depth_below(c, pending)).max().unwrap_or(0);
        now.max(pending(n))
    }

    fn add(&mut self, from: usize, to: usize) {
        self.children[from].push(to);
        self.parents[to].push(from);
    }
}

/// How many stages follow `i` in the game's own evolution lines.
fn vanilla_depth(species: &Species, i: usize, guard: usize) -> usize {
    if guard > 8 {
        return 0;
    }
    species
        .before(i)
        .evolutions
        .iter()
        .filter_map(|e| species.row_of(e.into))
        .filter(|&t| t != i && species.rows[t].playable)
        .map(|t| 1 + vanilla_depth(species, t, guard + 1))
        .max()
        .unwrap_or(0)
}

fn distinct_targets(evos: &[Evolution]) -> Vec<Pokemon> {
    let mut out = Vec::new();
    for e in evos {
        if !out.contains(&e.into) {
            out.push(e.into);
        }
    }
    out
}

/// Returns the evolutions it had to drop because no Pokémon fit their rules.
fn randomize_targets(ctx: &Ctx, species: &mut Species) -> Result<Vec<String>, RandoError> {
    let targets = Targets::new(ctx, species)?;
    let similar = ctx.settings.on(SIMILAR);
    let same_type = ctx.settings.on(SAME_TYPE);
    let force = ctx.settings.on(FORCE_CHANGE);
    let max_edges = if ctx.settings.on(THREE_STAGES) { 2 } else { 9 };
    let mut rng = ctx.rng("evolutions.random");
    let n = species.rows.len();
    let mut sources: Vec<usize> = (0..n)
        .filter(|&i| species.rows[i].playable && !species.rows[i].evolutions.is_empty())
        .collect();
    rng.shuffle(&mut sources);
    let pending_depth: Vec<usize> = (0..n).map(|i| vanilla_depth(species, i, 0)).collect();
    let mut done = vec![false; n];
    let mut graph = Graph {
        children: vec![Vec::new(); n],
        parents: vec![Vec::new(); n],
    };
    let mut picked = vec![0usize; n];
    let mut dropped = Vec::new();
    for &src in &sources {
        let old_targets = distinct_targets(&species.rows[src].evolutions);
        let mut chosen: BTreeMap<Pokemon, Pokemon> = BTreeMap::new();
        for &old in &old_targets {
            let old_row = species.row_of(old);
            let pending = |i: usize| if done[i] { 0 } else { pending_depth[i] };
            let above = graph.depth_above(src);
            let used: BTreeSet<Pokemon> = chosen.values().copied().collect();
            let eligible = |force: bool| -> Vec<usize> {
                targets
                    .rows
                    .iter()
                    .copied()
                    .filter(|&t| {
                        let row = &species.rows[t];
                        t != src
                            && row.xp_growth == species.rows[src].xp_growth
                            && !(force && old_targets.contains(&row.id))
                            && !used.contains(&row.id)
                            && !graph.reaches(t, src)
                            && above + 1 + graph.depth_below(t, &pending) <= max_edges
                    })
                    .collect()
            };
            let mut candidates = eligible(force);
            if candidates.is_empty() && force {
                candidates = eligible(false);
            }
            let reference = if old_targets.len() >= 3 { old_row.unwrap_or(src) } else { src };
            let candidates = if same_type {
                prefer(candidates, |c| shares_type(&species.rows[c], &species.rows[reference]))
            } else {
                candidates
            };
            let candidates = prefer(candidates, |c| picked[c] == 0);
            let pick = if candidates.is_empty() {
                None
            } else if similar {
                let target_bst = old_row.map_or(species.rows[src].bst(), |o| species.rows[o].bst());
                let pool: Vec<Pokemon> = candidates.iter().map(|&c| species.rows[c].id).collect();
                Picker::new(&ctx.dex, pool)
                    .similar_strength(&mut rng, target_bst, |_| true)
                    .and_then(|p| species.row_of(p))
            } else {
                rng.pick(&candidates).copied()
            };
            match pick {
                Some(t) => {
                    graph.add(src, t);
                    picked[t] += 1;
                    chosen.insert(old, species.rows[t].id);
                }
                None => dropped.push(format!("{} into {}", ctx.names.pokemon(species.rows[src].id), ctx.names.pokemon(old))),
            }
        }
        done[src] = true;
        let row = &mut species.rows[src];
        row.evolutions.retain(|e| chosen.contains_key(&e.into));
        for e in &mut row.evolutions {
            e.into = chosen[&e.into];
        }
    }
    Ok(dropped)
}

fn evolve_every_level(ctx: &Ctx, species: &mut Species) -> Result<(), RandoError> {
    let targets = Targets::new(ctx, species)?;
    let same_type = ctx.settings.on(SAME_TYPE);
    let force = ctx.settings.on(FORCE_CHANGE);
    let mut rng: Rng = ctx.rng("evolutions.every_level");
    for src in 0..species.rows.len() {
        let row = &species.rows[src];
        let info = &ctx.dex.all()[src];
        if !row.playable || always_excluded(info, false) {
            continue;
        }
        let old_targets = distinct_targets(&row.evolutions);
        let candidates: Vec<usize> = targets
            .rows
            .iter()
            .copied()
            .filter(|&t| {
                let target = &species.rows[t];
                t != src && target.xp_growth == row.xp_growth && !(force && old_targets.contains(&target.id))
            })
            .collect();
        let candidates = if same_type {
            prefer(candidates, |c| shares_type(&species.rows[c], row))
        } else {
            candidates
        };
        let Some(&t) = rng.pick(&candidates) else { continue };
        let into = species.rows[t].id;
        species.rows[src].evolutions = vec![Evolution {
            condition: code::LEVEL,
            parameter: 0,
            level: 1,
            into,
        }];
    }
    Ok(())
}

fn log(ctx: &mut Ctx, species: &Species, items: &BTreeMap<u16, u16>, dropped: &[String]) {
    let mut rows = Vec::new();
    for i in species.changed().filter(|&i| species.rows[i].playable) {
        let from = ctx.names.pokemon(species.rows[i].id);
        if species.rows[i].evolutions.is_empty() {
            rows.push(vec![from.clone(), "(no evolution)".into(), String::new()]);
        }
        for e in &species.rows[i].evolutions {
            rows.push(vec![from.clone(), ctx.names.pokemon(e.into), describe(ctx, e, items)]);
        }
    }
    if rows.is_empty() {
        return;
    }
    let section = ctx.log.section("Evolutions");
    section.note(format!(
        "{} Pokémon evolve differently.",
        species.changed().filter(|&i| species.rows[i].playable).count()
    ));
    if !dropped.is_empty() {
        section.note(format!("No Pokémon fit the rules for these evolutions, so they were removed: {}.", dropped.join(", ")));
    }
    let table = section.table("", &["Pokémon", "Evolves into", "How"]);
    for r in rows {
        table.row(r);
    }
}
