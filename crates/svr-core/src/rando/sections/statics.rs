//! Static overworld encounters: the Pokémon placed at fixed spots around the world.

use crate::data::{FormInfo, Pokemon};
use crate::fb::TableValue;
use crate::rando::options::{Kind, Opt, OptionDef, Page, Source};
use crate::rando::pick::Picker;
use crate::rando::{Ctx, RandoError, Section};

use super::encounter_common::{self as common, enum_value, put, shape, Habits, Moves};

pub const SECTION: Section = Section {
    id: "statics",
    options,
    run,
};

const RANDOM: &str = "statics.random";
const POOL: &str = "statics.pool";
const LEGENDARY_SWAP: &str = "statics.legendary_swap";
const STRONG_AS_LEGENDARY: &str = "statics.strong_as_legendary";
const SIMILAR: &str = "statics.similar_strength";
const ALT_FORMS: &str = "statics.alt_forms";
const LEVEL: &str = "statics.level";
const TERA: &str = "statics.tera";
const SHINY: &str = "statics.shiny";

const FIXED: &str = "world/data/field/fixed_symbol/fixed_symbol_table/fixed_symbol_table_array.bin";
/// Developer test rows that never appear in the game.
const TEST_ROWS: [&str; 2] = ["sample", "takara_test"];
const STRONG_BST: u32 = 600;
/// AI actions named after the species whose animations they play.
const SPECIES_ACTIONS: [&str; 4] = ["PACHIRISU", "OBAKEINU", "KUMO", "DRAGONITE"];
/// The behavior most static encounters use.
const PLAIN_SPAWN: &str = "SIMPLE_POP";

fn options() -> Vec<OptionDef> {
    let page = Page::Static;
    vec![
        Opt::toggle(RANDOM, page, "Species", "Random static encounters", false)
            .help("Replaces the Pokémon that stand at fixed spots in the overworld, such as the Dragonite and Garchomp on the mountains. Flying spots go to Pokémon that fly in the wild when the pool has them.")
            .sources(&[Source::Sv])
            .build(),
        Opt::new(POOL, page, "Species", "Static encounter pool", Kind::Pool)
            .help("Which species static encounters can roll.")
            .on(RANDOM)
            .sources(&[Source::Sv])
            .build(),
        Opt::toggle(LEGENDARY_SWAP, page, "Species", "Legendaries stay legendary", false)
            .help("A legendary is replaced by a legendary and any other Pokémon by a non-legendary, when the pool has both.")
            .on(RANDOM)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(STRONG_AS_LEGENDARY, page, "Species", "Count 600+ stat totals as legendary", false)
            .help("Pokémon such as Dragonite and Garchomp, whose base stats add up to 600 or more, count as legendary for the rule above.")
            .on(RANDOM)
            .on(LEGENDARY_SWAP)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(SIMILAR, page, "Species", "Similar strength", false)
            .help("Each replacement has a base stat total close to the original's.")
            .on(RANDOM)
            .sources(&[Source::Upr])
            .build(),
        Opt::toggle(ALT_FORMS, page, "Species", "Allow alternate forms", true)
            .help("Off rolls only each species' standard form, even if the pool allows others.")
            .on(RANDOM)
            .sources(&[Source::Upr])
            .build(),
        Opt::number(LEVEL, page, "Details", "Level change", 0, -50, 100, "%")
            .step(5)
            .help("Raises or lowers every static encounter's level by this percentage.")
            .sources(&[Source::Upr])
            .build(),
        Opt::choice(
            TERA,
            page,
            "Details",
            "Tera types",
            "keep",
            &[
                ("keep", "Keep", "Each encounter keeps its Tera type, including Terastallized ones."),
                ("random", "Random", "Each encounter gets a random Tera type. Stellar ones stay Stellar."),
            ],
        )
        .help("The Tera type of static encounters, which shows when they are already Terastallized.")
        .sources(&[Source::Sv])
        .build(),
        Opt::number(SHINY, page, "Details", "Shiny chance", 0, 0, 100, "%")
            .step(5)
            .help("Chance for each static encounter to be shiny.")
            .sources(&[Source::Sv])
            .build(),
    ]
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let random = ctx.settings.on(RANDOM);
    let level_change = ctx.settings.number(LEVEL);
    let random_tera = ctx.settings.choice(TERA) == "random";
    let shiny = ctx.settings.number(SHINY);
    if !random && level_change == 0 && !random_tera && shiny == 0 {
        return Ok(());
    }
    let legendary_swap = ctx.settings.on(LEGENDARY_SWAP);
    let strong = ctx.settings.on(STRONG_AS_LEGENDARY);
    let similar = ctx.settings.on(SIMILAR);
    let pool = if random {
        let what = "static encounters";
        let pool = common::section_pool(ctx, POOL, what, ctx.settings.on(ALT_FORMS), true)?;
        let pool = common::overworld(&ctx.dex, pool, what)?;
        common::without_form_items(&ctx.dex, pool, what)?
    } else {
        Vec::new()
    };
    let habits = Habits::load(ctx)?;
    let dex = &ctx.dex;
    let picker = Picker::new(dex, pool);
    let mut species_rng = ctx.rng("statics.species");
    let mut tera_rng = ctx.rng("statics.tera");
    let mut shiny_rng = ctx.rng("statics.shiny");
    let counts_as_legendary = |f: &FormInfo| f.is_legendary_class() || (strong && f.bst() >= STRONG_BST);
    let flies = |p: Pokemon| habits.moves(p).overlaps(Moves::AIR);
    let mut lines = Vec::new();
    let table = ctx.store.get_mut(FIXED)?;
    for row in table.tables_mut("values") {
        let key = row.string("tableKey").unwrap_or_default().to_owned();
        if TEST_ROWS.contains(&key.as_str()) {
            continue;
        }
        let action = action_name(row);
        let flight = action.contains("FLIGHT");
        let poke = row.table_mut("pokeDataSymbol").map_err(shape)?;
        let before = poke.clone();
        let original = common::species_of(poke)?;
        let info = dex.get(original);
        let p = if random {
            let legendary = info.is_some_and(counts_as_legendary);
            let same_class = |f: &FormInfo| !legendary_swap || counts_as_legendary(f) == legendary;
            let can_fly = |f: &FormInfo| !flight || flies(f.id);
            let target = if similar { info.map(FormInfo::bst) } else { None };
            common::choose(&picker, &mut species_rng, target, &[&same_class, &can_fly])
        } else {
            original
        };
        if p != original {
            common::set_species(poke, p)?;
        }
        let level = poke.int("level").map_err(shape)?;
        if level_change != 0 && level > 0 {
            put(poke, "level", common::scaled_level(level, level_change))?;
        }
        let rolls_tera = random_tera && !common::is_stellar(poke)?;
        if rolls_tera || p != original {
            common::set_tera(poke, dex, p, rolls_tera.then_some(&mut tera_rng))?;
        }
        common::roll_shiny(poke, &mut shiny_rng, shiny)?;
        if *poke == before {
            continue;
        }
        let mut line = vec![
            key,
            poke.int("level").map_err(shape)?.to_string(),
            ctx.names.pokemon(original),
            ctx.names.pokemon(p),
            common::yes(common::is_shiny(poke)?),
            common::gem_label(&ctx.names, poke.int("gemType").map_err(shape)?),
            String::new(),
        ];
        let species_action = SPECIES_ACTIONS.iter().any(|s| action.contains(s));
        if p != original && (species_action || (flight && !flies(p))) {
            let ai = row.table_mut("pokeAI").map_err(shape)?;
            put(ai, "actionId", enum_value(ai, "PokemonActionID", PLAIN_SPAWN)?)?;
            line[6] = format!("{action} became a plain spawn");
        }
        lines.push(line);
    }
    let log = ctx.log.section("Static encounters");
    log.note(format!("{} static encounters changed.", lines.len()));
    let t = log.table("Static encounters", &["Encounter", "Level", "Was", "Now", "Shiny", "Tera type", "Behavior"]);
    for line in lines {
        t.row(line);
    }
    Ok(())
}

fn action_name(row: &TableValue) -> String {
    row.table("pokeAI")
        .and_then(|ai| ai.schema().enum_name("PokemonActionID", ai.int("actionId").ok()?).map(str::to_owned))
        .unwrap_or_default()
}

