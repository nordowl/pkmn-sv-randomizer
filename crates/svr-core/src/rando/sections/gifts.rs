//! Gift Pokémon, gift eggs and in-game trades. Koraidon and Miraidon gifts never change.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::{FormInfo, Pokemon};
use crate::fb::TableValue;
use crate::game::Edition;
use crate::rando::itempool;
use crate::rando::options::{Kind, Opt, OptionDef, Page, Source, Status};
use crate::rando::pick::Picker;
use crate::rando::{Ctx, RandoError, Rng, Section};

use super::encounter_common::{self as common, put, shape, GIFTS, PALDEA_WILD};

pub const SECTION: Section = Section {
    id: "gifts",
    options,
    run,
};

const RANDOM: &str = "gifts.random";
const EGGS: &str = "gifts.eggs";
const POOL: &str = "gifts.pool";
const SHINY: &str = "gifts.shiny";
const TERA: &str = "gifts.tera";
const TRADES: &str = "gifts.trades";
const GUESTS: &str = "gifts.guests";
const NICKNAMES: &str = "gifts.trade_nicknames";
const TRAINERS: &str = "gifts.trade_trainers";
const IVS: &str = "gifts.trade_ivs";
const ITEMS: &str = "gifts.trade_items";

const TRADE_LIST: &str = "world/data/event/eventTradeList/eventTradeList_array.bin";
const TRADE_POKEMON: &str = "world/data/event/eventTradePokemon/eventTradePokemon_array.bin";

const GIFT_LABELS: [&str; 5] = [
    "add_0195",
    "add_0193",
    "sub060_gnyasu",
    "s1_main_0160_oniballoon_gonbe",
    "s1_side02_0060_hisuiGardie",
];
const EGG_LABELS: [&str; 3] = ["s1_sub_001_egg_000", "s1_sub_001_egg_001", "s1_sub_001_egg_002"];

/// The Hisuian Growlithe quest shows the gift in these scenes, each in a `_0` and `_1` variant.
const GROWLITHE_LABEL: &str = "s1_side02_0060_hisuiGardie";
const SUB_SCENARIO: &str = "world/scene/parts/event/event_scenario/sub_scenario/";
const GROWLITHE_SCENES: [&str; 6] = [
    "s1_side02_0000_/s1_side02_0000_always",
    "s1_side02_0030_/s1_side02_0030_always",
    "s1_side02_0060_/s1_side02_0060_pre_start",
    "s2_side02_0000_/s2_side02_0000_pre_start",
    "s2_side02_0005_/s2_side02_0005_pre_start",
    "s2_side02_0030_/s2_side02_0030_always",
];

const GUEST_PREFIX: &str = "trade_gest_";
const MAX_IV: i64 = 31;
const MAX_TRAINER_ID: i64 = 999_999;
const IV_FIELDS: [&str; 6] = ["hp", "atk", "def", "spAtk", "spDef", "agi"];

fn options() -> Vec<OptionDef> {
    let page = Page::GiftsTrades;
    vec![
        Opt::toggle(RANDOM, page, "Gifts", "Random gift Pokémon", false)
            .help("Replaces the Quagsire, Yanma, Meowth, Munchlax and Hisuian Growlithe you are given. The Growlithe quest's cutscenes show the new Pokémon.")
            .sources(&[Source::Sv])
            .status(Status::Test)
            .build(),
        Opt::toggle(EGGS, page, "Gifts", "Random gift eggs", false)
            .help("The Turtwig, Chimchar and Piplup eggs hatch into other first-stage Pokémon.")
            .sources(&[Source::Sv])
            .status(Status::Test)
            .build(),
        Opt::new(POOL, page, "Gifts", "Gift and trade pool", Kind::Pool)
            .help("Which species gifts, eggs and trades can roll.")
            .sources(&[Source::Sv])
            .build(),
        Opt::number(SHINY, page, "Gifts", "Shiny chance", 0, 0, 100, "%")
            .step(5)
            .help("Chance for each gift and egg to be shiny. Gifts that are already shiny stay shiny.")
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(TERA, page, "Gifts", "Random Tera types", false)
            .help("Each gift and egg gets a random Tera type.")
            .sources(&[Source::Sv])
            .build(),
        Opt::choice(
            TRADES,
            page,
            "Story trades",
            "Story trades",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "The three story trades stay as they are."),
                ("given", "Given Pokémon only", "You still send the usual Pokémon but receive a random one."),
                (
                    "both",
                    "Given and requested",
                    "Both sides are random. The Pokémon you are asked for is always one you can catch in Paldea in your game by then.",
                ),
            ],
        )
        .help("The three trades with characters during the story.")
        .sources(&[Source::Upr])
        .build(),
        Opt::toggle(GUESTS, page, "League Club", "Random guest trades", false)
            .help("The 30 guests you can invite to the League Club Room offer random Pokémon in trade.")
            .sources(&[Source::New])
            .status(Status::Test)
            .build(),
        Opt::toggle(NICKNAMES, page, "Trade details", "Mix up nicknames", false)
            .help("Traded Pokémon swap nicknames with each other, so no trade keeps its own. Story trades and guest trades mix separately.")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
        Opt::toggle(TRAINERS, page, "Trade details", "Mix up original trainers", false)
            .help("Traded Pokémon swap original trainer names and get random trainer IDs.")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
        Opt::toggle(IVS, page, "Trade details", "Random IVs", false)
            .help("Every traded Pokémon gets random individual values from 0 to 31.")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
        Opt::toggle(ITEMS, page, "Trade details", "Random held items", false)
            .help("Every traded Pokémon holds a random useful item.")
            .sources(&[Source::Upr])
            .status(Status::Test)
            .build(),
    ]
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    gifts(ctx)?;
    trades(ctx)
}

fn row_by_label<'t>(table: &'t mut TableValue, label: &str, path: &str) -> Result<&'t mut TableValue, RandoError> {
    table
        .tables_mut("values")
        .find(|r| r.string("label") == Some(label))
        .ok_or_else(|| RandoError::Section {
            section: "gifts",
            detail: format!("{path} has no row {label}"),
        })
}

fn gifts(ctx: &mut Ctx) -> Result<(), RandoError> {
    let random = ctx.settings.on(RANDOM);
    let eggs = ctx.settings.on(EGGS);
    let shiny = ctx.settings.number(SHINY);
    let tera = ctx.settings.on(TERA);
    if !random && !eggs && shiny == 0 && !tera {
        return Ok(());
    }
    let pool = if random || eggs { ctx.pool(POOL, "gifts")? } else { Vec::new() };
    let egg_pool = if eggs { common::without_form_items(&ctx.dex, pool.clone(), "gift eggs")? } else { Vec::new() };
    let dex = &ctx.dex;
    let picker = Picker::new(dex, pool);
    let egg_picker = Picker::new(dex, egg_pool);
    let mut species_rng = ctx.rng("gifts.species");
    let mut egg_rng = ctx.rng("gifts.eggs");
    let mut shiny_rng = ctx.rng("gifts.shiny");
    let mut tera_rng = ctx.rng("gifts.tera");
    let basic = |f: &FormInfo| dex.pre_evolution(f.id).is_none();
    let mut gift_species = Vec::new();
    let mut egg_species = Vec::new();
    let mut lines = Vec::new();
    let mut growlithe = None;
    let table = ctx.store.get_mut(GIFTS)?;
    let rows = GIFT_LABELS.iter().map(|&l| (l, false)).chain(EGG_LABELS.iter().map(|&l| (l, true)));
    for (label, is_egg) in rows {
        let poke = row_by_label(table, label, GIFTS)?.table_mut("pokeData").map_err(shape)?;
        let original = common::species_of(poke)?;
        let p = match (is_egg, random, eggs) {
            (false, true, _) => {
                let p = common::choose(&picker, &mut species_rng, None, &[&common::other_species(&gift_species)]);
                gift_species.push(p);
                p
            }
            (true, _, true) => {
                let p = common::choose(&egg_picker, &mut egg_rng, None, &[&basic, &common::other_species(&egg_species)]);
                egg_species.push(p);
                p
            }
            _ => original,
        };
        if p != original {
            common::set_species(poke, p)?;
            if label == GROWLITHE_LABEL {
                growlithe = Some((original, p));
            }
        }
        common::roll_shiny(poke, &mut shiny_rng, shiny)?;
        common::set_tera(poke, dex, p, tera.then_some(&mut tera_rng))?;
        common::give_form_item(poke, dex, p)?;
        lines.push(vec![
            format!("{} {}", ctx.names.pokemon(original), if is_egg { "egg" } else { "gift" }),
            ctx.names.pokemon(p),
            poke.int("level").map_err(shape)?.to_string(),
            common::yes(common::is_shiny(poke)?),
            common::gem_label(&ctx.names, poke.int("gemType").map_err(shape)?),
            common::item_label(&ctx.names, poke.int("item").map_err(shape)?),
        ]);
    }
    if let Some((original, p)) = growlithe {
        let map = BTreeMap::from([(original, p)]);
        for stem in GROWLITHE_SCENES {
            for variant in 0..2 {
                common::patch_scene(ctx, format!("{SUB_SCENARIO}{stem}_{variant}.trsog"), &map, "gifts")?;
            }
        }
    }
    let log = ctx.log.section("Gifts & trades");
    let t = log.table("Gifts", &["Gift", "Now", "Level", "Shiny", "Tera type", "Held item"]);
    for line in lines {
        t.row(line);
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TradeKind {
    Story,
    Guest,
}

fn trades(ctx: &mut Ctx) -> Result<(), RandoError> {
    let mode = ctx.settings.choice(TRADES).to_owned();
    let guests = ctx.settings.on(GUESTS);
    let nicknames = ctx.settings.on(NICKNAMES);
    let trainers = ctx.settings.on(TRAINERS);
    let ivs = ctx.settings.on(IVS);
    let items = ctx.settings.on(ITEMS);
    let given = mode != "unchanged";
    if !given && !guests && !nicknames && !trainers && !ivs && !items {
        return Ok(());
    }
    let story: BTreeSet<String> = ctx
        .store
        .get(TRADE_LIST)?
        .tables("values")
        .filter_map(|r| r.string("receivePoke").map(str::to_owned))
        .collect();
    let pool = if given || guests { ctx.pool(POOL, "trades")? } else { Vec::new() };
    let held = if items { itempool::held_items(ctx, true)? } else { Vec::new() };
    let picker = Picker::new(&ctx.dex, pool);
    let mut story_rng = ctx.rng("trades.story");
    let mut guest_rng = ctx.rng("trades.guests");
    let mut iv_rng = ctx.rng("trades.ivs");
    let mut item_rng = ctx.rng("trades.items");
    let mut trainer_rng = ctx.rng("trades.trainers");
    let mut nickname_rng = ctx.rng("trades.nicknames");
    let mut trainer_name_rng = ctx.rng("trades.trainer_names");
    let mut story_species = Vec::new();
    let mut guest_species = Vec::new();
    let mut originals = BTreeMap::new();
    let table = ctx.store.get_mut(TRADE_POKEMON)?;
    for row in table.tables_mut("values") {
        let label = row.string("label").unwrap_or_default().to_owned();
        let Some(kind) = trade_kind(&story, &label) else { continue };
        let poke = row.table_mut("pokeData").map_err(shape)?;
        originals.insert(label, common::species_of(poke)?);
        let (on, rng, chosen) = match kind {
            TradeKind::Story => (given, &mut story_rng, &mut story_species),
            TradeKind::Guest => (guests, &mut guest_rng, &mut guest_species),
        };
        if on {
            let p = common::choose(&picker, rng, None, &[&common::other_species(chosen)]);
            common::set_species(poke, p)?;
            chosen.push(p);
        }
        if ivs {
            put(poke, "talentType", common::enum_value(poke, "TalentType", "VALUE")?)?;
            let values = poke.table_mut("talentValue").map_err(shape)?;
            for field in IV_FIELDS {
                put(values, field, iv_rng.range(0, MAX_IV))?;
            }
        }
        if let Some(&item) = item_rng.pick(&held) {
            put(poke, "item", i64::from(item))?;
        }
        let p = common::species_of(poke)?;
        common::give_form_item(poke, &ctx.dex, p)?;
        if trainers {
            put(poke, "trainerId", trainer_rng.range(0, MAX_TRAINER_ID))?;
        }
    }
    for kind in [TradeKind::Story, TradeKind::Guest] {
        if nicknames {
            rotate_fields(table, &story, kind, &["nicknameLabel"], &mut nickname_rng)?;
        }
        if trainers {
            rotate_fields(table, &story, kind, &["parentNameLabel", "parentSex"], &mut trainer_name_rng)?;
        }
    }

    let mut lines: Vec<(TradeKind, Vec<String>)> = Vec::new();
    for row in table.tables("values") {
        let label = row.string("label").unwrap_or_default();
        let (Some(kind), Some(&original)) = (trade_kind(&story, label), originals.get(label)) else { continue };
        let poke = row.table("pokeData").ok_or_else(|| RandoError::Section {
            section: "gifts",
            detail: format!("trade {label} has no Pokémon data"),
        })?;
        let ivs = poke.table("talentValue").map_or_else(String::new, |t| {
            IV_FIELDS.iter().map(|f| t.int(f).unwrap_or(0).to_string()).collect::<Vec<_>>().join("/")
        });
        lines.push((
            kind,
            vec![
                label.to_owned(),
                ctx.names.pokemon(original),
                ctx.names.pokemon(common::species_of(poke)?),
                common::item_label(&ctx.names, poke.int("item").map_err(shape)?),
                ivs,
                poke.int("trainerId").map_err(shape)?.to_string(),
            ],
        ));
    }
    let log = ctx.log.section("Gifts & trades");
    for (kind, title) in [(TradeKind::Story, "Story trades: you receive"), (TradeKind::Guest, "League Club guest trades")] {
        let t = log.table(title, &["Trade", "Was", "Now", "Held item", "IVs", "Trainer ID"]);
        for (_, line) in lines.iter().filter(|l| l.0 == kind) {
            t.row(line.clone());
        }
    }
    let mixed: Vec<&str> = [(nicknames, "nicknames"), (trainers, "original trainer names")]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, what)| *what)
        .collect();
    if !mixed.is_empty() {
        log.note(format!(
            "Traded Pokémon swapped {} with each other. The game stores those as text labels, so the log cannot show them.",
            mixed.join(" and ")
        ));
    }
    Ok(())
}

fn trade_kind(story: &BTreeSet<String>, label: &str) -> Option<TradeKind> {
    if story.contains(label) {
        Some(TradeKind::Story)
    } else if label.starts_with(GUEST_PREFIX) {
        Some(TradeKind::Guest)
    } else {
        None
    }
}

/// Hands each trade of `kind` the fields of the next one in a random cycle, so with two or more
/// trades none keeps its own values.
fn rotate_fields(
    table: &mut TableValue,
    story: &BTreeSet<String>,
    kind: TradeKind,
    fields: &[&str],
    rng: &mut Rng,
) -> Result<(), RandoError> {
    let mut members: Vec<&mut TableValue> = table
        .tables_mut("values")
        .filter(|row| trade_kind(story, row.string("label").unwrap_or_default()) == Some(kind))
        .filter_map(|row| row.table_mut("pokeData").ok())
        .collect();
    rng.shuffle(&mut members);
    let values: Vec<Vec<u64>> = members
        .iter()
        .map(|poke| fields.iter().map(|f| poke.uint(f).map_err(shape)).collect())
        .collect::<Result<_, _>>()?;
    let count = members.len();
    for (i, poke) in members.into_iter().enumerate() {
        for (field, &value) in fields.iter().zip(&values[(i + 1) % count]) {
            if poke.uint(field).map_err(shape)? != value {
                poke.set_uint(field, value).map_err(shape)?;
            }
        }
    }
    Ok(())
}

/// Called by the wild section once wild Pokémon are final, so a requested species is always one
/// the player can catch in Paldea in their version, no later than the trade's own level.
pub(super) fn randomize_trade_requests(ctx: &mut Ctx) -> Result<(), RandoError> {
    if ctx.settings.choice(TRADES) != "both" {
        return Ok(());
    }
    let received: BTreeMap<String, (i64, Pokemon)> = ctx
        .store
        .get(TRADE_POKEMON)?
        .tables("values")
        .filter_map(|r| {
            let poke = r.table("pokeData")?;
            Some((r.string("label")?.to_owned(), (poke.int("level").ok()?, common::species_of(poke).ok()?)))
        })
        .collect();
    let version = match ctx.game.edition() {
        Edition::Scarlet => "A",
        Edition::Violet => "B",
    };
    let mut catchable: Vec<(Pokemon, i64)> = Vec::new();
    for row in ctx.store.get(PALDEA_WILD)?.tables("values") {
        let p = Pokemon {
            species: row.int("devid").map_err(shape)? as u16,
            form: row.int("formno").map_err(shape)? as u16,
        };
        let in_version = row.table("versiontable").is_some_and(|v| v.bool(version).unwrap_or(false));
        let always = row.string("flagName").unwrap_or_default().is_empty();
        if in_version && always && ctx.dex.is_playable(p) {
            catchable.push((p, row.int("minlevel").map_err(shape)?));
        }
    }
    let mut rng = ctx.rng("trades.requests");
    let mut requested: Vec<Pokemon> = Vec::new();
    let mut lines = Vec::new();
    let table = ctx.store.get_mut(TRADE_LIST)?;
    for row in table.tables_mut("values") {
        let label = row.string("receivePoke").unwrap_or_default().to_owned();
        let (level, gives) = received.get(&label).copied().unwrap_or((100, Pokemon { species: 0, form: 0 }));
        let fits = |&&(p, min): &&(Pokemon, i64)| {
            min <= level && p.species != gives.species && requested.iter().all(|r| r.species != p.species)
        };
        let choices: Vec<&(Pokemon, i64)> = catchable.iter().filter(fits).collect();
        let pick = match rng.pick(&choices) {
            Some(&&(p, _)) => p,
            None => match rng.pick(&catchable) {
                Some(&(p, _)) => p,
                None => continue,
            },
        };
        let was = Pokemon {
            species: row.int("sendPoke_devId").map_err(shape)? as u16,
            form: row.int("sendPoke_formId").map_err(shape)? as u16,
        };
        put(row, "sendPoke_devId", i64::from(pick.species))?;
        put(row, "sendPoke_formId", i64::from(pick.form))?;
        requested.push(pick);
        lines.push(vec![label, ctx.names.species(was.species), ctx.names.pokemon(pick)]);
    }
    let t = ctx.log.section("Gifts & trades").table("Story trades: you send", &["Trade", "Was", "Now"]);
    for line in lines {
        t.row(line);
    }
    Ok(())
}
