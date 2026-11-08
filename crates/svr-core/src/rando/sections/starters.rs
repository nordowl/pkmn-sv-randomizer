//! Starters and their cutscenes.
//!
//! The species the player receives always comes from the gift table. Cutscenes only show models,
//! so they are patched separately, and the reveal choice decides from which scene on they show the
//! new starters.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::Pokemon;
use crate::rando::itempool;
use crate::rando::options::{Kind, Opt, OptionDef, Page, Source, Status};
use crate::rando::pick::Picker;
use crate::rando::{Ctx, RandoError, Section};

use super::encounter_common::{self as common, enum_value, put, shape, GIFTS};

pub const SECTION: Section = Section {
    id: "starters",
    options,
    run,
};

const MODE: &str = "starters.mode";
const POOL: &str = "starters.pool";
const ALT_FORMS: &str = "starters.alt_forms";
const SHINY: &str = "starters.shiny";
const SHINY_CHANCE: &str = "starters.shiny_chance";
const TERA: &str = "starters.tera";
const ITEMS: &str = "starters.items";
const BAN_BAD_ITEMS: &str = "starters.ban_bad_items";
const REVEAL: &str = "starters.reveal";

struct Slot {
    label: &'static str,
    original: Pokemon,
    group: &'static str,
    species: &'static str,
    gender: &'static str,
    ball: &'static str,
    shiny: &'static str,
}

/// In `Ctx::starters` order: grass, fire, water.
const SLOTS: [Slot; 3] = [
    Slot {
        label: "common_0065_kusa",
        original: Pokemon { species: 906, form: 0 },
        group: "Sprigatito's slot",
        species: "starters.grass",
        gender: "starters.grass.gender",
        ball: "starters.grass.ball",
        shiny: "starters.grass.shiny",
    },
    Slot {
        label: "common_0065_hono",
        original: Pokemon { species: 909, form: 0 },
        group: "Fuecoco's slot",
        species: "starters.fire",
        gender: "starters.fire.gender",
        ball: "starters.fire.ball",
        shiny: "starters.fire.shiny",
    },
    Slot {
        label: "common_0065_mizu",
        original: Pokemon { species: 912, form: 0 },
        group: "Quaxly's slot",
        species: "starters.water",
        gender: "starters.water.gender",
        ball: "starters.water.ball",
        shiny: "starters.water.shiny",
    },
];

/// (option value, label, BallType constant)
const BALLS: [(&str, &str, &str); 26] = [
    ("poke", "Poké Ball", "MONSUTAABOORU"),
    ("great", "Great Ball", "SUUPAABOORU"),
    ("ultra", "Ultra Ball", "HAIPAABOORU"),
    ("master", "Master Ball", "MASUTAABOORU"),
    ("premier", "Premier Ball", "PUREMIABOORU"),
    ("luxury", "Luxury Ball", "GOOZYASUBOORU"),
    ("cherish", "Cherish Ball", "PURESYASUBOORU"),
    ("heal", "Heal Ball", "HIIRUBOORU"),
    ("net", "Net Ball", "NETTOBOORU"),
    ("dive", "Dive Ball", "DAIBUBOORU"),
    ("nest", "Nest Ball", "NESUTOBOORU"),
    ("repeat", "Repeat Ball", "RIPIITOBOORU"),
    ("timer", "Timer Ball", "TAIMAABOORU"),
    ("dusk", "Dusk Ball", "DAAKUBOORU"),
    ("quick", "Quick Ball", "KUIKKUBOORU"),
    ("safari", "Safari Ball", "SAFARIBOORU"),
    ("fast", "Fast Ball", "SUPIIDOBOORU"),
    ("level", "Level Ball", "REBERUBOORU"),
    ("lure", "Lure Ball", "RUAABOORU"),
    ("heavy", "Heavy Ball", "HEBIIBOORU"),
    ("love", "Love Ball", "RABURABUBOORU"),
    ("friend", "Friend Ball", "HURENDOBOORU"),
    ("moon", "Moon Ball", "MUUNBOORU"),
    ("sport", "Sport Ball", "KONPEBOORU"),
    ("dream", "Dream Ball", "DORIIMUBOORU"),
    ("beast", "Beast Ball", "URUTORABOORU"),
];

const MAIN_SCENARIO: &str = "world/scene/parts/event/event_scenario/main_scenario/";

/// Scenes that show the three starters, each as a path stem plus extension. Every scene exists in
/// a `_0` and a `_1` variant.
const AFTER_CHOOSING_SCENES: [(&str, &str); 2] =
    [("common_0088_/common_0088_always", "trsog"), ("common_0090_/common_0090_main", "trsog")];
const NEMONA_SCENES: [(&str, &str); 1] = [("common_0070_/common_0070_always", "trsog")];
const CLAVELL_SCENES: [(&str, &str); 2] =
    [("common_0060_/common_0060_always", "trsog"), ("common_0060_/common_0060_main", "trsog")];
const CLAVELL_DEMO: &str = "world/scene/parts/demo/ev/d030_/d030";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reveal {
    Clavell,
    Nemona,
    AfterChoosing,
}

impl Reveal {
    fn from_setting(value: &str) -> Reveal {
        match value {
            "clavell" => Reveal::Clavell,
            "after" => Reveal::AfterChoosing,
            _ => Reveal::Nemona,
        }
    }

    /// Every scene this choice patches. Earlier reveals include the later scenes.
    fn scenes(self) -> Vec<String> {
        let mut stems: Vec<(String, &str)> = AFTER_CHOOSING_SCENES
            .iter()
            .map(|&(stem, ext)| (format!("{MAIN_SCENARIO}{stem}"), ext))
            .collect();
        if self != Reveal::AfterChoosing {
            stems.extend(NEMONA_SCENES.iter().map(|&(stem, ext)| (format!("{MAIN_SCENARIO}{stem}"), ext)));
        }
        if self == Reveal::Clavell {
            stems.extend(CLAVELL_SCENES.iter().map(|&(stem, ext)| (format!("{MAIN_SCENARIO}{stem}"), ext)));
            stems.push((CLAVELL_DEMO.to_owned(), "trscn"));
        }
        stems
            .iter()
            .flat_map(|(stem, ext)| (0..2).map(move |variant| format!("{stem}_{variant}.{ext}")))
            .collect()
    }

    fn describe(self) -> &'static str {
        match self {
            Reveal::Clavell => "Cutscenes show the new starters from Clavell's visit on.",
            Reveal::Nemona => "Cutscenes show the new starters from Nemona's house on.",
            Reveal::AfterChoosing => "Cutscenes show the new starters only after you choose one.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unchanged,
    Custom,
    Random,
    Basics,
}

impl Mode {
    fn from_setting(value: &str) -> Mode {
        match value {
            "custom" => Mode::Custom,
            "random" => Mode::Random,
            "basics" => Mode::Basics,
            _ => Mode::Unchanged,
        }
    }
}

fn options() -> Vec<OptionDef> {
    let page = Page::Starters;
    let mut out = vec![
        Opt::choice(
            MODE,
            page,
            "Starters",
            "Starter Pokémon",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Sprigatito, Fuecoco and Quaxly, as usual."),
                ("custom", "Custom", "You choose all three below. A slot left on random rolls from the starter pool."),
                ("random", "Random", "Three different species from the starter pool."),
                ("basics", "Random basics", "Three different first-stage Pokémon that evolve twice, like the usual starters."),
            ],
        )
        .help("Which three Pokémon Nemona offers you. You receive exactly the one you pick.")
        .sources(&[Source::Upr, Source::Sv])
        .status(Status::Test)
        .build(),
        Opt::new(POOL, page, "Starters", "Starter pool", Kind::Pool)
            .help("Which species can be rolled as a starter.")
            .not(MODE, "unchanged")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(ALT_FORMS, page, "Starters", "Allow alternate forms", true)
            .help("Off rolls only each species' standard form, even if the pool allows others.")
            .not(MODE, "unchanged")
            .not(MODE, "custom")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
    ];
    let balls: Vec<(&'static str, &'static str, &'static str)> = [
        ("unchanged", "Unchanged", "A Poké Ball, as usual."),
        ("random", "Random", "Any Poké Ball type."),
    ]
    .into_iter()
    .chain(BALLS.iter().map(|&(value, label, _)| (value, label, "")))
    .collect();
    for slot in &SLOTS {
        out.push(
            Opt::new(slot.species, page, slot.group, "Species and form", Kind::Species)
                .help("The Pokémon in this slot. Leave it on random to roll one from the starter pool.")
                .is(MODE, "custom")
                .sources(&[Source::Sv])
                .status(Status::Test)
                .build(),
        );
        out.push(
            Opt::choice(
                slot.gender,
                page,
                slot.group,
                "Gender",
                "random",
                &[
                    ("random", "Random", "The game rolls the gender as usual."),
                    ("male", "Male", "Ignored for female-only and genderless species."),
                    ("female", "Female", "Ignored for male-only and genderless species."),
                ],
            )
            .help("The gender of this starter.")
            .is(MODE, "custom")
            .sources(&[Source::Sv])
            .status(Status::Test)
            .build(),
        );
        out.push(
            Opt::choice(slot.ball, page, slot.group, "Poké Ball", "unchanged", &balls)
                .help("The ball this starter comes in.")
                .sources(&[Source::Sv])
                .status(Status::Test)
                .build(),
        );
        out.push(
            Opt::toggle(slot.shiny, page, slot.group, "Always shiny", false)
                .help("This starter is always shiny.")
                .sources(&[Source::Sv])
                .status(Status::Test)
                .build(),
        );
    }
    out.extend([
        Opt::choice(
            SHINY,
            page,
            "Extras",
            "Shiny starters",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Only the slots set to always shiny are shiny."),
                ("all", "All three", "Every starter is shiny."),
                ("one", "One at random", "One random starter is shiny."),
                ("chance", "By chance", "Each starter is shiny with the chance below."),
            ],
        )
        .help("Makes starters shiny on top of the per-slot switches.")
        .sources(&[Source::Sv])
        .status(Status::Test)
        .build(),
        Opt::number(SHINY_CHANCE, page, "Extras", "Shiny chance", 0, 0, 100, "%")
            .step(5)
            .help("Chance for each starter to be shiny.")
            .is(SHINY, "chance")
            .sources(&[Source::Sv])
            .status(Status::Test)
            .build(),
        Opt::toggle(TERA, page, "Extras", "Random Tera types", false)
            .help("Each starter gets a random Tera type instead of one of its own types.")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(ITEMS, page, "Extras", "Random held items", false)
            .help("Each starter holds a random item.")
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(BAN_BAD_ITEMS, page, "Extras", "No useless held items", false)
            .help("Leaves out items that only hurt the holder or do nothing.")
            .on(ITEMS)
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            REVEAL,
            page,
            "Extras",
            "Show the new starters",
            "nemona",
            &[
                ("clavell", "At Clavell's visit", "The cutscene at your house already shows them."),
                ("nemona", "At Nemona's house", "Before that, cutscenes show the original three. Tested in game."),
                ("after", "Only after choosing", "You find out only once your starter is yours."),
            ],
        )
        .help("From which cutscene on the new starters are shown. Earlier cutscenes keep the original three. You always receive the new one.")
        .not(MODE, "unchanged")
        .sources(&[Source::Sv])
        .status(Status::Test)
        .build(),
    ]);
    out
}

fn pick_starters(ctx: &Ctx, mode: Mode) -> Result<[Pokemon; 3], RandoError> {
    let originals = SLOTS.each_ref().map(|s| s.original);
    if mode == Mode::Unchanged {
        return Ok(originals);
    }
    let mut rng = ctx.rng("starters.species");
    let mut fixed = [None; 3];
    if mode == Mode::Custom {
        for (out, slot) in fixed.iter_mut().zip(&SLOTS) {
            *out = common::resolve_pick(ctx, ctx.settings.species(slot.species), "starters", slot.group)?;
        }
    }
    let needed = fixed.iter().filter(|f| f.is_none()).count();
    if needed == 0 {
        return Ok(fixed.map(|f| f.expect("every slot is fixed")));
    }
    let alt_forms = mode == Mode::Custom || ctx.settings.on(ALT_FORMS);
    let what = if mode == Mode::Basics { "starters (random basics)" } else { "starters" };
    let mut pool = common::section_pool(ctx, POOL, what, alt_forms, true)?;
    if mode == Mode::Basics {
        pool.retain(|&p| ctx.dex.pre_evolution(p).is_none() && evolves_twice(ctx, p));
    }
    let mut chosen: Vec<Pokemon> = fixed.iter().flatten().copied().collect();
    let free: BTreeSet<u16> =
        pool.iter().map(|p| p.species).filter(|&s| chosen.iter().all(|c| c.species != s)).collect();
    if free.len() < needed {
        return Err(RandoError::Section {
            section: "starters",
            detail: format!(
                "the {what} pool has {} species left for {needed} different starters. Loosen its filters.",
                free.len()
            ),
        });
    }
    let picker = Picker::new(&ctx.dex, pool);
    let mut out = originals;
    for (slot, fixed) in out.iter_mut().zip(fixed) {
        *slot = match fixed {
            Some(p) => p,
            None => {
                let p = picker
                    .matching(&mut rng, common::other_species(&chosen))
                    .expect("enough free species were counted above");
                chosen.push(p);
                p
            }
        };
    }
    Ok(out)
}

fn evolves_twice(ctx: &Ctx, p: Pokemon) -> bool {
    let playable_evolutions = |p: Pokemon| -> Vec<Pokemon> {
        ctx.dex.get(p).map_or_else(Vec::new, |info| {
            info.evolutions.iter().map(|e| e.into).filter(|&e| ctx.dex.is_playable(e)).collect()
        })
    };
    playable_evolutions(p).into_iter().any(|middle| !playable_evolutions(middle).is_empty())
}

fn shiny_slots(ctx: &Ctx) -> [bool; 3] {
    let mut rng = ctx.rng("starters.shiny");
    let mut shiny = SLOTS.each_ref().map(|s| ctx.settings.on(s.shiny));
    match ctx.settings.choice(SHINY) {
        "all" => shiny = [true; 3],
        "one" => shiny[rng.index(3)] = true,
        "chance" => {
            let chance = ctx.settings.number(SHINY_CHANCE) as u32;
            for s in &mut shiny {
                *s |= rng.percent(chance);
            }
        }
        _ => {}
    }
    shiny
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let mode = Mode::from_setting(ctx.settings.choice(MODE));
    let starters = pick_starters(ctx, mode)?;
    if mode != Mode::Unchanged {
        ctx.starters = Some(starters);
    }
    let shiny = shiny_slots(ctx);
    let custom_gender = mode == Mode::Custom && SLOTS.iter().any(|s| ctx.settings.choice(s.gender) != "random");
    let custom_ball = SLOTS.iter().any(|s| ctx.settings.choice(s.ball) != "unchanged");
    let tera = ctx.settings.on(TERA);
    let items = ctx.settings.on(ITEMS);
    let species_changed = starters != SLOTS.each_ref().map(|s| s.original);
    if !species_changed && !custom_gender && !shiny.contains(&true) && !custom_ball && !tera && !items {
        return Ok(());
    }

    let held = if items { itempool::held_items(ctx, ctx.settings.on(BAN_BAD_ITEMS))? } else { Vec::new() };
    let mut item_rng = ctx.rng("starters.items");
    let mut ball_rng = ctx.rng("starters.ball");
    let mut tera_rng = ctx.rng("starters.tera");
    let mut notes = Vec::new();
    let mut lines = Vec::new();
    let table = ctx.store.get_mut(GIFTS)?;
    for (i, (slot, &p)) in SLOTS.iter().zip(&starters).enumerate() {
        let row = table
            .tables_mut("values")
            .find(|r| r.string("label") == Some(slot.label))
            .ok_or_else(|| RandoError::Section {
                section: "starters",
                detail: format!("the gift table has no row {}", slot.label),
            })?;
        let poke = row.table_mut("pokeData").map_err(shape)?;
        if p != slot.original {
            common::set_species(poke, p)?;
        }
        let wanted = ctx.settings.choice(slot.gender);
        if mode == Mode::Custom && wanted != "random" {
            if ctx.dex.get(p).is_some_and(|info| common::can_be(info, wanted)) {
                put(poke, "sex", enum_value(poke, "SexType", &wanted.to_uppercase())?)?;
            } else {
                notes.push(format!(
                    "{}: {} cannot be {wanted}, so the game rolls its gender.",
                    slot.group,
                    ctx.names.pokemon(p)
                ));
            }
        }
        let ball = match ctx.settings.choice(slot.ball) {
            "unchanged" => None,
            "random" => Some(BALLS[ball_rng.index(BALLS.len())]),
            value => BALLS.iter().find(|b| b.0 == value).copied(),
        };
        if let Some((_, _, constant)) = ball {
            put(poke, "ballId", enum_value(poke, "BallType", constant)?)?;
        }
        if shiny[i] {
            common::make_shiny(poke)?;
        }
        common::set_tera(poke, &ctx.dex, p, tera.then_some(&mut tera_rng))?;
        if let Some(&item) = item_rng.pick(&held) {
            put(poke, "item", i64::from(item))?;
        }
        common::give_form_item(poke, &ctx.dex, p)?;
        let gender = match poke.schema().enum_name("SexType", poke.int("sex").map_err(shape)?) {
            Some("MALE") => "Male",
            Some("FEMALE") => "Female",
            _ => "Random",
        };
        let ball = poke.schema().enum_name("BallType", poke.int("ballId").map_err(shape)?).unwrap_or_default();
        lines.push(vec![
            slot.group.to_owned(),
            ctx.names.pokemon(p),
            gender.to_owned(),
            BALLS.iter().find(|b| b.2 == ball).map_or("Poké Ball", |b| b.1).to_owned(),
            common::yes(common::is_shiny(poke)?),
            common::gem_label(&ctx.names, poke.int("gemType").map_err(shape)?),
            common::item_label(&ctx.names, poke.int("item").map_err(shape)?),
        ]);
    }

    let reveal = Reveal::from_setting(ctx.settings.choice(REVEAL));
    if species_changed {
        patch_scenes(ctx, &starters, reveal)?;
    }

    let log = ctx.log.section("Starters");
    if species_changed {
        log.note(reveal.describe());
    }
    for note in notes {
        log.note(note);
    }
    let table = log.table("Starters", &["Slot", "Pokémon", "Gender", "Ball", "Shiny", "Tera type", "Held item"]);
    for line in lines {
        table.row(line);
    }
    Ok(())
}

/// Shows the new starters in the reveal's cutscenes.
fn patch_scenes(ctx: &mut Ctx, starters: &[Pokemon; 3], reveal: Reveal) -> Result<(), RandoError> {
    let mut map = BTreeMap::new();
    for (slot, &p) in SLOTS.iter().zip(starters) {
        if p != slot.original {
            map.insert(slot.original, p);
        }
    }
    if map.is_empty() {
        return Ok(());
    }
    for path in reveal.scenes() {
        common::patch_scene(ctx, path, &map, "starters")?;
    }
    Ok(())
}
