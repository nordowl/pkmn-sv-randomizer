//! Base stats, types, abilities, EXP curves, catch rates and egg hatching.
//!
//! "Follow evolutions" uses the game's own evolution lines, because this section runs before
//! evolutions are randomized.

#[path = "pokemon_abilities.rs"]
mod abilities;
#[path = "pokemon_stats.rs"]
mod stats;
#[path = "pokemon_table.rs"]
pub(super) mod table;

use std::collections::BTreeSet;

use crate::data::facts::Rarity;
use crate::data::store::ABILITIES;
use crate::rando::movepool::TYPE_COUNT;
use crate::rando::options::{Opt, OptionDef, Page, Source};
use crate::rando::{Ctx, RandoError, Rng, Section};

use abilities::AbilityPool;
use table::{Families, Row, Species};

pub const SECTION: Section = Section {
    id: "pokemon",
    options,
    run,
};

const STATS: &str = "pokemon.stats";
const KEEP_BST: &str = "pokemon.stats_keep_bst";
const STATS_FOLLOW: &str = "pokemon.stats_follow_evolutions";
const RANDOM_GAINS: &str = "pokemon.stats_random_evolution_gains";
const EXP_MODE: &str = "pokemon.exp_curves";
const EXP_CURVE: &str = "pokemon.exp_curve";
const TYPES: &str = "pokemon.types";
const TYPE_COUNT_MODE: &str = "pokemon.types_count";
const ABILITIES_ON: &str = "pokemon.abilities";
const ABILITIES_FOLLOW: &str = "pokemon.abilities_follow_evolutions";
const ALLOW_WONDER_GUARD: &str = "pokemon.abilities_allow_wonder_guard";
const BAN_EXIT: &str = "pokemon.abilities_ban_exit";
const BAN_TRAPPING: &str = "pokemon.abilities_ban_trapping";
const BAN_NEGATIVE: &str = "pokemon.abilities_ban_negative";
const BAN_BAD: &str = "pokemon.abilities_ban_bad";
const ENSURE_TWO: &str = "pokemon.abilities_ensure_two";
const WEIGH_DUPLICATES: &str = "pokemon.abilities_combine_duplicates";
const CATCH_RATE: &str = "pokemon.min_catch_rate";
const EGG_HATCH: &str = "pokemon.egg_hatching";

const STAT_GROUP: &str = "Base stats";
const TYPE_GROUP: &str = "Types";
const ABILITY_GROUP: &str = "Abilities";
const OTHER_GROUP: &str = "EXP, catching and eggs";

/// The game's EXP growth values, by option value.
const CURVES: &[(&str, u8, &str)] = &[
    ("medium_fast", 0, "Medium Fast"),
    ("erratic", 1, "Erratic"),
    ("fluctuating", 2, "Fluctuating"),
    ("medium_slow", 3, "Medium Slow"),
    ("fast", 4, "Fast"),
    ("slow", 5, "Slow"),
];
const SLOW: u8 = 5;

/// Minimum catch rate for other Pokémon and for legendaries, by option value. UPR's levels.
const CATCH_LEVELS: &[(&str, u8, u8)] = &[("easier", 75, 37), ("much_easier", 128, 64), ("very_easy", 200, 100), ("maximum", 255, 255)];

/// The fewest egg cycles any species has in the game.
const FASTEST_HATCH: u8 = 5;

fn options() -> Vec<OptionDef> {
    vec![
        Opt::choice(
            STATS,
            Page::Pokemon,
            STAT_GROUP,
            "Base stats",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon keeps its base stats."),
                ("shuffle", "Shuffled", "Each Pokémon's six base stats trade places. Totals stay the same."),
                ("random", "Random", "Each Pokémon gets new base stats."),
            ],
        )
        .help("Changes how strong and fast each Pokémon is.")
        .sources(&[Source::Upr, Source::Sv])
        .build(),
        Opt::toggle(KEEP_BST, Page::Pokemon, STAT_GROUP, "Keep each Pokémon's base stat total", true)
            .help("On: random stats add up to the Pokémon's original total, so strong Pokémon stay strong. Off: stats are random numbers from 15 to 255, so totals change. Evolutions that follow their line grow from their pre-evolution instead.")
            .is(STATS, "random")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(STATS_FOLLOW, Page::Pokemon, STAT_GROUP, "Stats follow evolutions", false)
            .help("An evolution keeps the same stat shape as the Pokémon it evolves from, so a fast Pokémon evolves into a fast one. Uses the game's own evolution lines.")
            .not(STATS, "unchanged")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(RANDOM_GAINS, Page::Pokemon, STAT_GROUP, "Randomize the stats added on evolution", false)
            .help("Instead of keeping the shape, an evolution keeps its pre-evolution's stats and the points it gains are spread at random.")
            .is(STATS, "random")
            .on(STATS_FOLLOW)
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            EXP_MODE,
            Page::Pokemon,
            OTHER_GROUP,
            "Standardize EXP curves",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon keeps its EXP curve."),
                ("legendaries", "Slow legendaries", "Legendary and mythical Pokémon and Ultra Beasts level slowly. Everything else uses the curve below."),
                ("strong_legendaries", "Slow strong legendaries", "Only box legendaries such as Koraidon level slowly. Everything else uses the curve below."),
                ("all", "All Pokémon", "Every Pokémon uses the curve below."),
            ],
        )
        .help("Gives Pokémon the same leveling speed. This also lets random evolutions pick from more Pokémon, because an evolution must keep its EXP curve.")
        .sources(&[Source::Upr])
        .build(),
        Opt::choice(
            EXP_CURVE,
            Page::Pokemon,
            OTHER_GROUP,
            "EXP curve to use",
            "medium_fast",
            &[
                ("medium_fast", "Medium Fast", "1,000,000 EXP to level 100."),
                ("erratic", "Erratic", "600,000 EXP to level 100. Fast late, slow early."),
                ("fluctuating", "Fluctuating", "1,640,000 EXP to level 100. Fast early, slow late."),
                ("medium_slow", "Medium Slow", "1,059,860 EXP to level 100."),
                ("fast", "Fast", "800,000 EXP to level 100."),
                ("slow", "Slow", "1,250,000 EXP to level 100."),
            ],
        )
        .not(EXP_MODE, "unchanged")
        .sources(&[Source::Upr])
        .build(),
        Opt::choice(
            TYPES,
            Page::Pokemon,
            TYPE_GROUP,
            "Types",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon keeps its types."),
                ("follow_evolutions", "Random, following evolutions", "Each evolution line gets new types. Evolutions keep the types of the Pokémon they evolve from and may gain one. Pokémon with several evolutions, like Eevee, give each evolution its own types."),
                ("random", "Random", "Every Pokémon gets new types on its own."),
            ],
        )
        .help("Arceus keeps its types, because its forms follow the Plate it holds.")
        .sources(&[Source::Upr, Source::Sv])
        .build(),
        Opt::choice(
            TYPE_COUNT_MODE,
            Page::Pokemon,
            TYPE_GROUP,
            "Number of types",
            "keep",
            &[
                ("keep", "Same as before", "Single-type Pokémon stay single-type and dual-type Pokémon stay dual-type, except that an evolution keeps both types of a dual-type pre-evolution."),
                ("extra", "Second type at random", "Single-type Pokémon get a second type half of the time. Evolutions that follow their line gain one a quarter of the time."),
                ("dual", "Always two types", "Every Pokémon gets two different types."),
            ],
        )
        .not(TYPES, "unchanged")
        .sources(&[Source::Upr, Source::Sv])
        .build(),
        Opt::toggle(ABILITIES_ON, Page::Pokemon, ABILITY_GROUP, "Random abilities", false)
            .help("Every Pokémon gets new abilities, including a new hidden ability. Abilities that change a Pokémon's form, such as Zero to Hero, Tera Shift and Disguise, never move, and Pokémon that have one keep their abilities. Shedinja keeps Wonder Guard.")
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::toggle(ABILITIES_FOLLOW, Page::Pokemon, ABILITY_GROUP, "Abilities follow evolutions", false)
            .help("An evolution gets the same abilities as the Pokémon it evolves from. Pokémon with more than one evolution, like Eevee, still give each evolution its own abilities.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::toggle(ALLOW_WONDER_GUARD, Page::Pokemon, ABILITY_GROUP, "Allow Wonder Guard", false)
            .help("Wonder Guard blocks every hit that is not super effective, which can make a Pokémon unbeatable early on.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr, Source::Sv])
            .build(),
        Opt::toggle(BAN_EXIT, Page::Pokemon, ABILITY_GROUP, "Ban Wimp Out and Emergency Exit", false)
            .help("These abilities pull a Pokémon out of battle at half HP, which can end wild battles early.")
            .on(ABILITIES_ON)
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(BAN_TRAPPING, Page::Pokemon, ABILITY_GROUP, "Ban trapping abilities", false)
            .help("Bans Shadow Tag, Magnet Pull and Arena Trap, which stop you from switching out or running.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_NEGATIVE, Page::Pokemon, ABILITY_GROUP, "Ban negative abilities", false)
            .help("Bans Defeatist, Slow Start, Truant, Klutz and Stall, which hurt their holder.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_BAD, Page::Pokemon, ABILITY_GROUP, "Ban weak abilities", false)
            .help("Bans abilities that do little in single battles, such as Plus, Minus, Frisk, Honey Gather, Friend Guard and Telepathy.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(ENSURE_TWO, Page::Pokemon, ABILITY_GROUP, "Ensure two abilities", false)
            .help("Off: Pokémon that had one regular ability still have one. On: every Pokémon gets two different regular abilities.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(WEIGH_DUPLICATES, Page::Pokemon, ABILITY_GROUP, "Combine duplicate abilities", false)
            .help("Abilities with the same effect, such as Protean and Libero, count as one ability in the draw, so each effect is equally likely.")
            .on(ABILITIES_ON)
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            CATCH_RATE,
            Page::Pokemon,
            OTHER_GROUP,
            "Minimum catch rate",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Every Pokémon keeps its catch rate."),
                ("easier", "Easier", "At least 75, and 37 for legendaries."),
                ("much_easier", "Much easier", "At least 128, and 64 for legendaries."),
                ("very_easy", "Very easy", "At least 200, and 100 for legendaries."),
                ("maximum", "Maximum", "Every Pokémon has the highest catch rate, 255."),
            ],
        )
        .help("Raises low catch rates. Rates that are already higher stay as they are.")
        .sources(&[Source::Upr])
        .build(),
        Opt::choice(
            EGG_HATCH,
            Page::Pokemon,
            OTHER_GROUP,
            "Faster egg hatching",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Eggs take as long as usual."),
                ("faster", "Twice as fast", "Every egg needs half as many steps."),
                ("fastest", "Fastest", "Every egg needs as few steps as the quickest egg in the game."),
            ],
        )
        .sources(&[Source::New])
        .build(),
    ]
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let mut species = Species::load(ctx)?;
    standardize_exp(ctx, &mut species);
    randomize_types(ctx, &mut species);
    randomize_stats(ctx, &mut species);
    randomize_abilities(ctx, &mut species)?;
    raise_catch_rates(ctx, &mut species);
    speed_up_eggs(ctx, &mut species);
    log(ctx, &species);
    species.save(ctx)
}

/// Rows that copy a lower form because they looked the same: same stats, types and abilities.
fn twins(species: &Species) -> Vec<Option<usize>> {
    species.twins(|r| (r.stats, r.types, r.abilities))
}

fn families(species: &Species, twin: &[Option<usize>]) -> Families {
    species.families(|i| twin[i].is_none())
}

fn standardize_exp(ctx: &Ctx, species: &mut Species) {
    let mode = ctx.settings.choice(EXP_MODE);
    if mode == "unchanged" {
        return;
    }
    let chosen = CURVES
        .iter()
        .find(|c| c.0 == ctx.settings.choice(EXP_CURVE))
        .map_or(0, |c| c.1);
    for (row, info) in species.rows.iter_mut().zip(ctx.dex.all()) {
        if !row.present {
            continue;
        }
        let slow = match mode {
            "legendaries" => info.rarity != Rarity::Standard,
            "strong_legendaries" => info.rarity == Rarity::Legendary,
            _ => false,
        };
        row.xp_growth = if slow { SLOW } else { chosen };
    }
}

fn random_type(rng: &mut Rng, other_than: Option<u8>) -> u8 {
    loop {
        let t = rng.below(u64::from(TYPE_COUNT)) as u8;
        if Some(t) != other_than {
            return t;
        }
    }
}

fn is_dual(types: [u8; 2]) -> bool {
    types[0] != types[1]
}

fn randomize_types(ctx: &Ctx, species: &mut Species) {
    let mode = ctx.settings.choice(TYPES);
    if mode == "unchanged" {
        return;
    }
    let count = ctx.settings.choice(TYPE_COUNT_MODE);
    let mut rng = ctx.rng("pokemon.types");
    let twin = twins(species);
    let fam = families(species, &twin);
    for &i in fam.top_down() {
        if species.rows[i].type_locked {
            continue;
        }
        let was_dual = is_dual(species.before(i).types);
        let parent = fam.parent(i).filter(|_| mode == "follow_evolutions" && !fam.is_split(i));
        let types = match parent {
            Some(p) => {
                let inherited = species.rows[p].types;
                let gains = !is_dual(inherited)
                    && match count {
                        "keep" => was_dual && !is_dual(species.before(p).types),
                        "extra" => rng.percent(25),
                        _ => true,
                    };
                if gains {
                    [inherited[0], random_type(&mut rng, Some(inherited[0]))]
                } else {
                    inherited
                }
            }
            None => {
                let first = random_type(&mut rng, None);
                let dual = match count {
                    "keep" => was_dual,
                    "extra" => was_dual || rng.percent(50),
                    _ => true,
                };
                [first, if dual { random_type(&mut rng, Some(first)) } else { first }]
            }
        };
        species.rows[i].types = types;
    }
    species.copy_twins(&twin, |row, from| {
        if !row.type_locked {
            row.types = from.types;
        }
    });
}

fn randomize_stats(ctx: &Ctx, species: &mut Species) {
    let mode = ctx.settings.choice(STATS);
    if mode == "unchanged" {
        return;
    }
    let follow = ctx.settings.on(STATS_FOLLOW);
    let keep_bst = mode == "random" && ctx.settings.on(KEEP_BST);
    let random_gains = mode == "random" && follow && ctx.settings.on(RANDOM_GAINS);
    let mut rng = ctx.rng("pokemon.stats");
    let twin = twins(species);
    let fam = families(species, &twin);
    let mut perms: Vec<Option<[usize; 6]>> = vec![None; species.rows.len()];
    for &i in fam.top_down() {
        let before = species.before(i).stats;
        let hp_is_one = before[0] == 1;
        let parent = fam.parent(i).filter(|_| follow);
        let new = if mode == "shuffle" {
            let inherited = parent.filter(|_| !fam.is_split(i) && !hp_is_one).and_then(|p| perms[p]);
            let perm = inherited.unwrap_or_else(|| stats::random_permutation(&mut rng, hp_is_one));
            perms[i] = Some(perm);
            stats::permute(before, perm)
        } else {
            match parent {
                Some(p) => {
                    let parent_now = species.rows[p].stats;
                    let total = if keep_bst {
                        species.before(i).bst()
                    } else {
                        let ratio = f64::from(species.before(i).bst()) / f64::from(species.before(p).bst().max(1));
                        (f64::from(species.rows[p].bst()) * ratio).round().clamp(6.0, 1530.0) as u32
                    };
                    if random_gains || fam.is_split(i) {
                        stats::with_random_gains(&mut rng, parent_now, total, hp_is_one)
                    } else {
                        stats::scaled(parent_now, total, hp_is_one)
                    }
                }
                None if keep_bst => stats::random_with_total(&mut rng, species.before(i).bst(), hp_is_one),
                None => stats::random_any(&mut rng, hp_is_one),
            }
        };
        species.rows[i].stats = new;
    }
    species.copy_twins(&twin, |row, from| row.stats = from.stats);
}

fn randomize_abilities(ctx: &mut Ctx, species: &mut Species) -> Result<(), RandoError> {
    if !ctx.settings.on(ABILITIES_ON) {
        return Ok(());
    }
    let count = ctx.store.get(ABILITIES)?.tables("Table").count() as u16;
    let names = &ctx.names;
    let fixed: BTreeSet<u16> = abilities::ids(names, count, abilities::FORM_CHANGING)
        .union(&abilities::ids(names, count, abilities::WONDER_GUARD))
        .copied()
        .collect();
    let mut banned = abilities::ids(names, count, abilities::FORM_CHANGING);
    let mut ban = |on: bool, list: &[&str]| {
        if on {
            banned.extend(abilities::ids(names, count, list));
        }
    };
    ban(!ctx.settings.on(ALLOW_WONDER_GUARD), abilities::WONDER_GUARD);
    ban(ctx.settings.on(BAN_EXIT), abilities::EXIT);
    ban(ctx.settings.on(BAN_TRAPPING), abilities::TRAPPING);
    ban(ctx.settings.on(BAN_NEGATIVE), abilities::NEGATIVE);
    ban(ctx.settings.on(BAN_BAD), abilities::BAD);
    let pool = AbilityPool::new(names, count, &banned, ctx.settings.on(WEIGH_DUPLICATES));
    if pool.is_empty() {
        return Err(RandoError::Section {
            section: "pokemon",
            detail: "the ability bans leave too few abilities".into(),
        });
    }
    let follow = ctx.settings.on(ABILITIES_FOLLOW);
    let ensure_two = ctx.settings.on(ENSURE_TWO);
    let mut rng = ctx.rng("pokemon.abilities");
    let keeps = |row: &Row| row.abilities.iter().any(|a| fixed.contains(a));
    let twin = twins(species);
    let fam = families(species, &twin);
    for &i in fam.top_down() {
        if keeps(species.before(i)) {
            continue;
        }
        let parent = fam
            .parent(i)
            .filter(|&p| follow && !fam.is_split(i) && !keeps(species.before(p)));
        let new = match parent {
            Some(p) => species.rows[p].abilities,
            None => {
                let first = pool.pick(&mut rng, &[]);
                let old = species.before(i).abilities;
                let second = if old[0] == old[1] && !ensure_two { first } else { pool.pick(&mut rng, &[first]) };
                [first, second, pool.pick(&mut rng, &[first, second])]
            }
        };
        species.rows[i].abilities = new;
    }
    species.copy_twins(&twin, |row, from| {
        if !keeps(row) {
            row.abilities = from.abilities;
        }
    });
    Ok(())
}

fn raise_catch_rates(ctx: &Ctx, species: &mut Species) {
    let Some(&(_, normal, legendary)) = CATCH_LEVELS.iter().find(|l| l.0 == ctx.settings.choice(CATCH_RATE)) else {
        return;
    };
    for (row, info) in species.rows.iter_mut().zip(ctx.dex.all()) {
        if row.present {
            let min = if info.is_legendary_class() { legendary } else { normal };
            row.catch_rate = row.catch_rate.max(min);
        }
    }
}

fn speed_up_eggs(ctx: &Ctx, species: &mut Species) {
    let mode = ctx.settings.choice(EGG_HATCH);
    for row in species.rows.iter_mut().filter(|r| r.present) {
        row.hatch_cycles = match mode {
            "faster" => row.hatch_cycles.div_ceil(2).max(FASTEST_HATCH.min(row.hatch_cycles)),
            "fastest" => row.hatch_cycles.min(FASTEST_HATCH),
            _ => row.hatch_cycles,
        };
    }
}

fn type_text(ctx: &Ctx, types: [u8; 2]) -> String {
    if is_dual(types) {
        format!("{}/{}", ctx.names.type_name(types[0]), ctx.names.type_name(types[1]))
    } else {
        ctx.names.type_name(types[0])
    }
}

fn log(ctx: &mut Ctx, species: &Species) {
    let changed: Vec<usize> = species.changed().filter(|&i| species.rows[i].playable).collect();
    let mut stat_rows = Vec::new();
    let mut type_rows = Vec::new();
    let mut ability_rows = Vec::new();
    let (mut curves, mut catch, mut eggs) = (0, 0, 0);
    for &i in &changed {
        let (old, new) = (species.before(i), &species.rows[i]);
        let name = ctx.names.pokemon(new.id);
        if new.stats != old.stats {
            let mut cells = vec![name.clone()];
            cells.extend(new.stats.iter().map(u8::to_string));
            cells.push(format!("{} (was {})", new.bst(), old.bst()));
            stat_rows.push(cells);
        }
        if new.types != old.types {
            type_rows.push(vec![name.clone(), type_text(ctx, new.types), type_text(ctx, old.types)]);
        }
        if new.abilities != old.abilities {
            let a = new.abilities.map(|a| ctx.names.ability(a));
            let regular = if new.abilities[0] == new.abilities[1] { a[0].clone() } else { format!("{} / {}", a[0], a[1]) };
            ability_rows.push(vec![name.clone(), regular, a[2].clone()]);
        }
        curves += usize::from(new.xp_growth != old.xp_growth);
        catch += usize::from(new.catch_rate != old.catch_rate);
        eggs += usize::from(new.hatch_cycles != old.hatch_cycles);
    }
    if stat_rows.is_empty() && type_rows.is_empty() && ability_rows.is_empty() && curves + catch + eggs == 0 {
        return;
    }
    let section = ctx.log.section("Stats, types & abilities");
    if curves > 0 {
        section.note(format!("{curves} Pokémon got a new EXP curve."));
    }
    if catch > 0 {
        section.note(format!("{catch} Pokémon got a higher catch rate."));
    }
    if eggs > 0 {
        section.note(format!("{eggs} Pokémon hatch from eggs faster."));
    }
    for (title, headers, rows) in [
        ("Base stats", &["Pokémon", "HP", "Atk", "Def", "SpA", "SpD", "Spe", "Total"][..], stat_rows),
        ("Types", &["Pokémon", "Types", "Was"][..], type_rows),
        ("Abilities", &["Pokémon", "Abilities", "Hidden ability"][..], ability_rows),
    ] {
        if !rows.is_empty() {
            let table = section.table(title, headers);
            for r in rows {
                table.row(r);
            }
        }
    }
}
