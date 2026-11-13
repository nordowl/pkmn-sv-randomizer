//! Trainers: species, levels, held items, battle style and AI per group, plus names and classes.

use std::collections::{BTreeMap, BTreeSet};

#[path = "trainer_groups.rs"]
mod groups;
#[path = "trainer_moves.rs"]
mod moves;
#[path = "trainer_rows.rs"]
mod rows;
#[path = "trainer_text.rs"]
mod text;

use groups::{Group, Placement, Region};
use moves::MoveBook;
use rows::{Mon, Trainer, TRDATA, TRTYPE};
use text::Deck;

use crate::data::{facts, Dex, FormInfo, Pokemon};
use crate::rando::itempool::{self, ItemCatalog};
use crate::rando::options::{Kind, Opt, OptionDef, Page, Req, Settings, Source, Status};
use crate::rando::{pool, Ctx, RandoError, Rng, Section, Seed, ALLOW_TERA_LOCKED, GLOBAL_POOL};
use crate::text::{GameText, Language};

pub const SECTION: Section = Section {
    id: "trainers",
    options,
    run,
};

const MODE: &str = "trainers.mode";
const POOL: &str = "trainers.pool";
const POOL_KITAKAMI: &str = "trainers.pool.kitakami";
const POOL_BLUEBERRY: &str = "trainers.pool.blueberry";
const SIMILAR: &str = "trainers.similar_strength";
const NO_LEGENDARIES: &str = "trainers.no_legendaries";
const ALT_FORMS: &str = "trainers.alt_forms";
const THEME: &str = "trainers.type_theme";
const WEIGHT_TYPES: &str = "trainers.weight_types";
const NO_EARLY_WG: &str = "trainers.no_early_wonder_guard";
const LEAGUE_UNIQUE: &str = "trainers.league_unique";
const FORCE_EVOLVED: &str = "trainers.force_evolved";
const FORCE_EVOLVED_LEVEL: &str = "trainers.force_evolved_level";
const LEVEL_MODIFIER: &str = "trainers.level_modifier";
const RIVAL_STARTER: &str = "trainers.rival_starter";
const BETTER_MOVES: &str = "trainers.better_movesets";
const RANDOM_ABILITIES: &str = "trainers.random_abilities";
const SHINY: &str = "trainers.shiny_percent";
const RANDOM_TERA: &str = "trainers.random_tera";
const FORCE_SIX: &str = "trainers.force_six";
const DOUBLES: &str = "trainers.doubles";
const ITEMS_CONSUMABLE: &str = "trainers.items_consumable";
const ITEMS_SENSIBLE: &str = "trainers.items_sensible";
const ITEMS_ACE: &str = "trainers.items_ace_only";
const ITEMS_BAN_BAD: &str = "trainers.items_ban_bad";
const SAME_AS_RIVALS: &str = "trainers.same_as_rivals";
const NAMES: &str = "trainers.names";
const NAME_LIST: &str = "trainers.name_list";
const CLASS_NAMES: &str = "trainers.class_names";
const CLASS_NAME_LIST: &str = "trainers.class_name_list";
const CLASSES: &str = "trainers.classes";

const REGIONS: [(Region, &str, &str); 3] = [
    (Region::Paldea, "trainers.region.paldea", "Change Paldea trainers"),
    (Region::Kitakami, "trainers.region.kitakami", "Change Kitakami trainers"),
    (Region::Blueberry, "trainers.region.blueberry", "Change Blueberry trainers"),
];

struct GroupIds {
    group: Group,
    enabled: &'static str,
    extra: Option<&'static str>,
    items: &'static str,
    tera: Option<&'static str>,
    ivs: &'static str,
    ai: &'static str,
}

const GROUP_IDS: [GroupIds; 8] = [
    GroupIds {
        group: Group::Rivals,
        enabled: "trainers.rivals.enabled",
        extra: Some("trainers.rivals.extra"),
        items: "trainers.rivals.items",
        tera: Some("trainers.rivals.tera"),
        ivs: "trainers.rivals.perfect_ivs",
        ai: "trainers.rivals.smart_ai",
    },
    GroupIds {
        group: Group::Gyms,
        enabled: "trainers.gyms.enabled",
        extra: Some("trainers.gyms.extra"),
        items: "trainers.gyms.items",
        tera: Some("trainers.gyms.tera"),
        ivs: "trainers.gyms.perfect_ivs",
        ai: "trainers.gyms.smart_ai",
    },
    GroupIds {
        group: Group::League,
        enabled: "trainers.league.enabled",
        extra: Some("trainers.league.extra"),
        items: "trainers.league.items",
        tera: Some("trainers.league.tera"),
        ivs: "trainers.league.perfect_ivs",
        ai: "trainers.league.smart_ai",
    },
    GroupIds {
        group: Group::TeamStar,
        enabled: "trainers.team_star.enabled",
        extra: Some("trainers.team_star.extra"),
        items: "trainers.team_star.items",
        tera: Some("trainers.team_star.tera"),
        ivs: "trainers.team_star.perfect_ivs",
        ai: "trainers.team_star.smart_ai",
    },
    GroupIds {
        group: Group::Club,
        enabled: "trainers.club.enabled",
        extra: Some("trainers.club.extra"),
        items: "trainers.club.items",
        tera: Some("trainers.club.tera"),
        ivs: "trainers.club.perfect_ivs",
        ai: "trainers.club.smart_ai",
    },
    GroupIds {
        group: Group::Story,
        enabled: "trainers.story.enabled",
        extra: Some("trainers.story.extra"),
        items: "trainers.story.items",
        tera: Some("trainers.story.tera"),
        ivs: "trainers.story.perfect_ivs",
        ai: "trainers.story.smart_ai",
    },
    GroupIds {
        group: Group::Route,
        enabled: "trainers.route.enabled",
        extra: Some("trainers.route.extra"),
        items: "trainers.route.items",
        tera: Some("trainers.route.tera"),
        ivs: "trainers.route.perfect_ivs",
        ai: "trainers.route.smart_ai",
    },
    GroupIds {
        group: Group::Raid,
        enabled: "trainers.raid.enabled",
        extra: None,
        items: "trainers.raid.items",
        tera: None,
        ivs: "trainers.raid.perfect_ivs",
        ai: "trainers.raid.smart_ai",
    },
];

const WONDER_GUARD: u16 = 25;
const EARLY_LEVEL: i64 = 20;
const PERFECT_IV: i64 = 31;
const TYPE_COUNT: u8 = 18;
const VANILLA_STARTERS: [u16; 3] = [906, 909, 912];

fn options() -> Vec<OptionDef> {
    use Source::{Pknx, Sv, Upr};
    let species = "Trainer Pokémon";
    let mut out = vec![
        Opt::choice(
            MODE,
            Page::Trainers,
            species,
            "Trainer Pokémon",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Trainers keep their own Pokémon."),
                ("random", "Random", "Every Pokémon is replaced by a random one from the pool."),
                ("even", "Even distribution", "Random, but each species appears about equally often across all trainers."),
                (
                    "even_main",
                    "Even in the main story",
                    "Paldea story trainers spread species evenly. Rematches, the Academy Ace Tournament and the DLC trainers pick freely.",
                ),
            ],
        )
        .help("How the Pokémon of the trainers chosen below are replaced.")
        .sources(&[Upr, Sv])
        .build(),
        Opt::new(POOL, Page::Trainers, species, "Species trainers may use", Kind::Pool)
            .help("Overrides the global pool for trainer Pokémon in Paldea, and in the other regions unless they set their own.")
            .not(MODE, "unchanged")
            .sources(&[Sv])
            .build(),
        Opt::new(POOL_KITAKAMI, Page::Trainers, species, "Species Kitakami trainers may use", Kind::Pool)
            .help("Leave on inherit to use the Paldea trainer pool for Kitakami too.")
            .not(MODE, "unchanged")
            .sources(&[Sv])
            .build(),
        Opt::new(POOL_BLUEBERRY, Page::Trainers, species, "Species Blueberry trainers may use", Kind::Pool)
            .help("Leave on inherit to use the Paldea trainer pool for Blueberry Academy too.")
            .not(MODE, "unchanged")
            .sources(&[Sv])
            .build(),
        Opt::toggle(SIMILAR, Page::Trainers, species, "Similar strength", false)
            .help("Each replacement has a base stat total close to the Pokémon it replaces.")
            .not(MODE, "unchanged")
            .sources(&[Upr])
            .build(),
        Opt::toggle(NO_LEGENDARIES, Page::Trainers, species, "No legendaries", false)
            .help("Legendary, mythical and Ultra Beast species never appear on trainer teams.")
            .not(MODE, "unchanged")
            .sources(&[Upr])
            .build(),
        Opt::toggle(ALT_FORMS, Page::Trainers, species, "Allow alternate forms", false)
            .help("Regional and other alternate forms may appear. Off uses only each species' standard form.")
            .not(MODE, "unchanged")
            .sources(&[Upr])
            .build(),
        Opt::choice(
            THEME,
            Page::Trainers,
            species,
            "Type-themed teams",
            "off",
            &[
                ("off", "Off", "Teams mix any types."),
                ("all", "All trainers", "Every trainer, gym and Team Star crew gets one random type its whole team shares."),
                ("gym_league", "Gyms and League only", "Each gym and each League member gets one random type."),
            ],
        )
        .help("A gym's trainers share their leader's type, and a person keeps the same type in every rematch.")
        .not(MODE, "unchanged")
        .sources(&[Upr, Pknx])
        .build(),
        Opt::toggle(WEIGHT_TYPES, Page::Trainers, species, "Weight types by species count", false)
            .help("Types with more species in the pool are picked as themes more often.")
            .not(THEME, "off")
            .not(MODE, "unchanged")
            .sources(&[Upr])
            .build(),
        Opt::toggle(NO_EARLY_WG, Page::Trainers, species, "No early Wonder Guard", false)
            .help("Pokémon below level 20 never get a species that can have Wonder Guard. The Paradise Protocol and Terapagos fights never allow it at any level.")
            .not(MODE, "unchanged")
            .sources(&[Upr, Sv])
            .build(),
        Opt::number(LEAGUE_UNIQUE, Page::Trainers, species, "League Pokémon are unique", 0, 0, 6, "Pokémon")
            .help("The strongest this-many Pokémon of each Elite Four member and champion appear on no other trainer's team. 0 turns it off.")
            .not(MODE, "unchanged")
            .sources(&[Upr])
            .build(),
        Opt::toggle(RIVAL_STARTER, Page::Trainers, species, "Rivals carry the starters", false)
            .help("Nemona and Clavell keep the starter they take from the three you choose from, evolved to fit each battle's level. Works with random starters even if trainer Pokémon are unchanged.")
            .sources(&[Upr])
            .status(Status::Test)
            .build(),
        Opt::toggle(FORCE_EVOLVED, Page::Trainers, species, "Force fully evolved", false)
            .help("Trainer Pokémon at or above the level below are always fully evolved.")
            .sources(&[Upr, Pknx])
            .build(),
        Opt::number(FORCE_EVOLVED_LEVEL, Page::Trainers, species, "Fully evolved from level", 40, 1, 100, "")
            .on(FORCE_EVOLVED)
            .sources(&[Upr, Pknx])
            .build(),
        Opt::number(LEVEL_MODIFIER, Page::Trainers, species, "Level modifier", 0, -50, 100, "%")
            .help("Raises or lowers every trainer Pokémon's level by this percentage, between 1 and 100.")
            .step(5)
            .sources(&[Upr, Pknx])
            .build(),
        Opt::toggle(BETTER_MOVES, Page::Trainers, species, "Better movesets", false)
            .help("Trainer Pokémon whose moves the game would pick get four moves they can really learn: strong same-type attacks, a coverage attack and a status move. Hand-picked movesets on unchanged Pokémon stay.")
            .sources(&[Upr])
            .build(),
        Opt::toggle(RANDOM_ABILITIES, Page::Trainers, species, "Random ability slot", false)
            .help("Each trainer Pokémon uses a random one of its species' abilities, hidden ability included.")
            .sources(&[Pknx])
            .build(),
        Opt::number(SHINY, Page::Trainers, species, "Shiny chance", 0, 0, 100, "%")
            .help("Chance for each trainer Pokémon to be shiny.")
            .sources(&[Upr, Sv, Pknx])
            .build(),
        Opt::toggle(RANDOM_TERA, Page::Trainers, species, "Random Tera types", false)
            .help("Each trainer Pokémon gets a random Tera type. A trainer only Terastallizes where the game or the group setting allows it.")
            .sources(&[Sv])
            .status(Status::Test)
            .build(),
        Opt::toggle(FORCE_SIX, Page::Trainers, species, "Force six Pokémon", false)
            .help("Every trainer fields six Pokémon, except fights the story needs smaller.")
            .sources(&[Upr, Sv, Pknx])
            .status(Status::Test)
            .build(),
        Opt::choice(
            DOUBLES,
            Page::Trainers,
            species,
            "Double battles",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "Battles keep their style."),
                ("all", "All possible", "Every trainer whose battle the story does not fix fights a double battle."),
                ("random", "Random", "Each such trainer fights a single or double battle at random."),
            ],
        )
        .help("A trainer in a double battle gets at least two Pokémon. Multi battles, raid helpers and fights the story needs as singles never change.")
        .sources(&[Upr, Sv, Pknx])
        .status(Status::Test)
        .build(),
        Opt::toggle(ITEMS_CONSUMABLE, Page::Trainers, "Held items", "Consumable items only", false)
            .help("Random held items are berries, herbs, gems and other single-use items.")
            .sources(&[Upr, Pknx])
            .build(),
        Opt::toggle(ITEMS_SENSIBLE, Page::Trainers, "Held items", "Sensible items only", false)
            .help("Type-boosting items only go to Pokémon of that type, Black Sludge only to Poison types and Eviolite only to Pokémon that can still evolve.")
            .sources(&[Upr, Pknx])
            .build(),
        Opt::toggle(ITEMS_ACE, Page::Trainers, "Held items", "Highest-level Pokémon only", false)
            .help("Only each trainer's highest-level Pokémon gets a random held item.")
            .sources(&[Upr, Pknx])
            .build(),
        Opt::toggle(ITEMS_BAN_BAD, Page::Trainers, "Held items", "Ban bad items", false)
            .help("Random held items never include items that do nothing in battle or only hurt the holder.")
            .sources(&[Upr])
            .build(),
    ];
    out.push(
        Opt::toggle(SAME_AS_RIVALS, Page::Trainers, "All groups", "Use rival settings for all groups", false)
            .help("Every group uses the extra Pokémon, held item, Tera, IV and AI settings of the Rivals group.")
            .sources(&[Sv])
            .build(),
    );
    for (_, id, label) in REGIONS {
        out.push(
            Opt::toggle(id, Page::Trainers, "Regions", label, true)
                .help("Off leaves every trainer of this region as it is.")
                .sources(&[Sv])
                .build(),
        );
    }
    for ids in &GROUP_IDS {
        out.extend(group_options(ids));
    }
    out.extend(text_options());
    out
}

fn group_options(ids: &GroupIds) -> Vec<OptionDef> {
    use Source::{Pknx, Sv, Upr};
    let group = ids.group.title();
    let shared = |opt: Opt| {
        if ids.group == Group::Rivals {
            opt
        } else {
            opt.requires(Req::Off { id: SAME_AS_RIVALS })
        }
    };
    let mut out = vec![Opt::toggle(ids.enabled, Page::Trainers, group, "Change these trainers", true)
        .help("Off leaves this group out of every trainer option on this page.")
        .sources(&[Sv])
        .build()];
    if let Some(extra) = ids.extra {
        out.push(
            shared(Opt::number(extra, Page::Trainers, group, "Extra Pokémon", 0, 0, 5, "Pokémon"))
                .help("Adds this many random Pokémon to each team, up to six.")
                .sources(&[Upr, Sv, Pknx])
                .status(Status::Test)
                .build(),
        );
    }
    out.push(
        shared(Opt::toggle(ids.items, Page::Trainers, group, "Random held items", false))
            .help("Gives these trainers' Pokémon random held items, following the held item rules.")
            .sources(&[Upr, Pknx])
            .build(),
    );
    if let Some(tera) = ids.tera {
        out.push(
            shared(Opt::toggle(tera, Page::Trainers, group, "Allow Terastallizing", false))
                .help("These trainers may Terastallize one Pokémon, except in fights the story keeps without it.")
                .sources(&[Sv])
                .status(Status::Test)
                .build(),
        );
    }
    out.push(
        shared(Opt::toggle(ids.ivs, Page::Trainers, group, "Perfect IVs", false))
            .help("These trainers' Pokémon have 31 IVs in every stat.")
            .sources(&[Sv, Pknx])
            .build(),
    );
    out.push(
        shared(Opt::toggle(ids.ai, Page::Trainers, group, "Smart AI", false))
            .help("These trainers use the game's strongest battle AI, switch Pokémon and use items.")
            .sources(&[Sv, Pknx])
            .build(),
    );
    out
}

fn text_options() -> Vec<OptionDef> {
    use Source::{Pknx, Upr};
    let scope = |id, label, help| {
        Opt::choice(
            id,
            Page::Trainers,
            "Names and classes",
            label,
            "off",
            &[
                ("off", "Unchanged", ""),
                ("generic", "Generic trainers", "Route trainers, gym trainers, grunts and raid helpers."),
                ("all", "Everyone", "Also Nemona, gym leaders, the League and other characters. Dialogue still uses their real names."),
            ],
        )
        .help(help)
        .sources(&[Upr])
        .status(Status::Test)
    };
    vec![
        scope(NAMES, "Random trainer names", "Replaces the names shown in battle with names from the list below, in every language.").build(),
        Opt::new(NAME_LIST, Page::Trainers, "Names and classes", "Custom names", Kind::Text { default: "", max_len: 4000 })
            .help("Names separated by commas, up to 12 characters each. Empty uses a built-in list of male and female names that match each trainer.")
            .not(NAMES, "off")
            .sources(&[Upr])
            .status(Status::Test)
            .build(),
        scope(CLASS_NAMES, "Random trainer class names", "Replaces class titles such as Hiker or Gym Leader with titles from the list below, in every language.").build(),
        Opt::new(CLASS_NAME_LIST, Page::Trainers, "Names and classes", "Custom class names", Kind::Text { default: "", max_len: 4000 })
            .help("Class names separated by commas, up to 20 characters each. Empty uses a built-in list.")
            .not(CLASS_NAMES, "off")
            .sources(&[Upr])
            .status(Status::Test)
            .build(),
        Opt::toggle(CLASSES, Page::Trainers, "Names and classes", "Random trainer classes", false)
            .help("Route-style trainers get another route-style class of the same gender, which changes their class title and may change their battle intro. Trainers with their own intro scene keep their class.")
            .sources(&[Pknx])
            .status(Status::Test)
            .build(),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unchanged,
    Random,
    Even,
    EvenMain,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    Off,
    All,
    GymLeague,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Battle {
    Unchanged,
    AllDoubles,
    Mixed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rename {
    Off,
    Generic,
    All,
}

impl Rename {
    fn read(value: &str) -> Rename {
        match value {
            "generic" => Rename::Generic,
            "all" => Rename::All,
            _ => Rename::Off,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct GroupRules {
    enabled: bool,
    extra: usize,
    items: bool,
    tera: bool,
    ivs: bool,
    ai: bool,
}

struct ItemRules {
    consumable: bool,
    sensible: bool,
    ace_only: bool,
    ban_bad: bool,
}

/// Settings parsed once, so the planner never touches option ids.
struct Config {
    mode: Mode,
    similar: bool,
    no_legendaries: bool,
    alt_forms: bool,
    theme: Theme,
    weight_types: bool,
    no_early_wg: bool,
    league_unique: usize,
    force_evolved: Option<i64>,
    level_modifier: i64,
    rival_starter: bool,
    better_moves: bool,
    random_abilities: bool,
    shiny: u32,
    random_tera: bool,
    force_six: bool,
    battle: Battle,
    items: ItemRules,
    groups: BTreeMap<Group, GroupRules>,
    regions: BTreeSet<Region>,
    names: Rename,
    name_list: Vec<String>,
    class_names: Rename,
    class_list: Vec<String>,
    classes: bool,
}

impl Config {
    fn read(s: &Settings) -> Config {
        let same = s.on(SAME_AS_RIVALS);
        let rivals = &GROUP_IDS[0];
        let groups = GROUP_IDS
            .iter()
            .map(|ids| {
                let src = if same { rivals } else { ids };
                let rules = GroupRules {
                    enabled: s.on(ids.enabled),
                    extra: ids.extra.and(src.extra).map_or(0, |id| s.number(id).clamp(0, 5) as usize),
                    items: s.on(src.items),
                    tera: ids.tera.and(src.tera).is_some_and(|id| s.on(id)),
                    ivs: s.on(src.ivs),
                    ai: s.on(src.ai),
                };
                (ids.group, rules)
            })
            .collect();
        Config {
            mode: match s.choice(MODE) {
                "random" => Mode::Random,
                "even" => Mode::Even,
                "even_main" => Mode::EvenMain,
                _ => Mode::Unchanged,
            },
            similar: s.on(SIMILAR),
            no_legendaries: s.on(NO_LEGENDARIES),
            alt_forms: s.on(ALT_FORMS),
            theme: match s.choice(THEME) {
                "all" => Theme::All,
                "gym_league" => Theme::GymLeague,
                _ => Theme::Off,
            },
            weight_types: s.on(WEIGHT_TYPES),
            no_early_wg: s.on(NO_EARLY_WG),
            league_unique: s.number(LEAGUE_UNIQUE).clamp(0, 6) as usize,
            force_evolved: s.on(FORCE_EVOLVED).then(|| s.number(FORCE_EVOLVED_LEVEL)),
            level_modifier: s.number(LEVEL_MODIFIER),
            rival_starter: s.on(RIVAL_STARTER),
            better_moves: s.on(BETTER_MOVES),
            random_abilities: s.on(RANDOM_ABILITIES),
            shiny: s.number(SHINY).clamp(0, 100) as u32,
            random_tera: s.on(RANDOM_TERA),
            force_six: s.on(FORCE_SIX),
            battle: match s.choice(DOUBLES) {
                "all" => Battle::AllDoubles,
                "random" => Battle::Mixed,
                _ => Battle::Unchanged,
            },
            items: ItemRules {
                consumable: s.on(ITEMS_CONSUMABLE),
                sensible: s.on(ITEMS_SENSIBLE),
                ace_only: s.on(ITEMS_ACE),
                ban_bad: s.on(ITEMS_BAN_BAD),
            },
            groups,
            regions: REGIONS.iter().filter(|(_, id, _)| s.on(id)).map(|&(r, _, _)| r).collect(),
            names: Rename::read(s.choice(NAMES)),
            name_list: text::parse_list(s.text(NAME_LIST), text::MAX_NAME),
            class_names: Rename::read(s.choice(CLASS_NAMES)),
            class_list: text::parse_list(s.text(CLASS_NAME_LIST), text::MAX_CLASS),
            classes: s.on(CLASSES),
        }
    }

    fn in_scope(&self, place: &Placement) -> bool {
        self.groups[&place.group].enabled && self.regions.contains(&place.region)
    }
}

fn has_wonder_guard(form: &FormInfo) -> bool {
    form.abilities.contains(&WONDER_GUARD)
}

/// The abilities a `tokusei` setting can resolve to.
fn possible_abilities(form: &FormInfo, tokusei: i64) -> Vec<u16> {
    match tokusei {
        rows::TOKUSEI_RANDOM_12 => vec![form.abilities[0], form.abilities[1]],
        rows::TOKUSEI_RANDOM_123 => form.abilities.to_vec(),
        t => rows::TOKUSEI_SET
            .iter()
            .position(|&s| s == t)
            .map_or_else(|| form.abilities.to_vec(), |slot| vec![form.abilities[slot]]),
    }
}

fn modified_level(level: i64, percent: i64) -> i64 {
    ((level * (100 + percent) + 50) / 100).clamp(1, 100)
}

/// A starter's evolution path with the level each stage is reached.
struct StarterLine {
    stages: Vec<(Pokemon, i64)>,
}

impl StarterLine {
    fn new(dex: &Dex, start: Pokemon, rng: &mut Rng) -> StarterLine {
        let mut path = vec![(start, 0i64)];
        let mut current = start;
        for step in 0..2 {
            let Some(info) = dex.get(current) else { break };
            let options: Vec<_> = info.evolutions.iter().filter(|e| dex.is_playable(e.into)).collect();
            let prefer: Vec<_> = options
                .iter()
                .copied()
                .filter(|e| step > 0 || !dex.is_fully_evolved(e.into))
                .collect();
            let Some(evo) = rng.pick(if prefer.is_empty() { &options } else { &prefer }) else { break };
            path.push((evo.into, i64::from(evo.level)));
            current = evo.into;
        }
        let defaults: &[i64] = if path.len() == 3 { &[0, 16, 36] } else { &[0, 30] };
        for (k, stage) in path.iter_mut().enumerate().skip(1) {
            if stage.1 == 0 {
                stage.1 = defaults[k];
            }
        }
        StarterLine { stages: path }
    }

    fn at(&self, level: i64) -> Pokemon {
        self.stages
            .iter()
            .take_while(|(_, from)| *from <= level)
            .last()
            .map_or(self.stages[0].0, |&(p, _)| p)
    }
}

struct Planner<'a> {
    dex: &'a Dex,
    settings: &'a Settings,
    seed: &'a Seed,
    cfg: &'a Config,
    pools: BTreeMap<Region, Vec<Pokemon>>,
    usage: BTreeMap<Pokemon, u32>,
    reserved: BTreeSet<Pokemon>,
    themes: BTreeMap<String, u8>,
    theme_labels: BTreeMap<String, String>,
    catalog: &'a ItemCatalog,
    held: Vec<u16>,
    moves: &'a MoveBook,
    vanilla_lines: [BTreeSet<Pokemon>; 3],
    starter_lines: [StarterLine; 3],
}

impl<'a> Planner<'a> {
    fn rng(&self, stream: &str, key: impl std::fmt::Display) -> Rng {
        Rng::stream(self.seed, &format!("trainers.{stream}/{key}"))
    }

    fn form(&self, p: Pokemon) -> Option<&'a FormInfo> {
        self.dex.get(p)
    }

    fn pool(&mut self, region: Region) -> Result<&[Pokemon], RandoError> {
        if !self.pools.contains_key(&region) {
            let global = self.settings.pool(GLOBAL_POOL);
            let paldea = pool::effective(global, self.settings.pool(POOL));
            let (filter, what) = match region {
                Region::Paldea => (paldea, "Paldea trainers"),
                Region::Kitakami => (pool::effective(paldea, self.settings.pool(POOL_KITAKAMI)), "Kitakami trainers"),
                Region::Blueberry => (pool::effective(paldea, self.settings.pool(POOL_BLUEBERRY)), "Blueberry trainers"),
            };
            let species: Vec<Pokemon> = pool::build(self.dex, filter, self.settings.on(ALLOW_TERA_LOCKED))
                .into_iter()
                .filter(|p| self.cfg.alt_forms || p.form == 0)
                .filter(|p| !self.cfg.no_legendaries || self.dex.get(*p).is_some_and(|f| !f.is_legendary_class()))
                .collect();
            if species.is_empty() {
                return Err(RandoError::EmptyPool { what: what.to_owned() });
            }
            self.pools.insert(region, species);
        }
        Ok(&self.pools[&region])
    }

    /// A pool member passing `keep`, near `target_bst` when similar strength is on, and among the
    /// least used so far when `even` is set.
    fn choose(
        &mut self,
        rng: &mut Rng,
        region: Region,
        target_bst: u32,
        even: bool,
        keep: &dyn Fn(&FormInfo) -> bool,
    ) -> Result<Option<Pokemon>, RandoError> {
        let dex = self.dex;
        let candidates: Vec<&FormInfo> = self
            .pool(region)?
            .iter()
            .filter_map(|&p| dex.get(p))
            .filter(|f| keep(f))
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }
        let mut band: Vec<&FormInfo> = candidates.clone();
        if self.cfg.similar {
            let mut width = target_bst / 10;
            for _ in 0..40 {
                let (low, high) = (target_bst.saturating_sub(width), target_bst + width);
                band = candidates.iter().copied().filter(|f| (low..=high).contains(&f.bst())).collect();
                if !band.is_empty() {
                    break;
                }
                width += (target_bst / 20).max(5);
            }
            if band.is_empty() {
                band = candidates;
            }
        }
        if even {
            let least = band.iter().map(|f| self.usage.get(&f.id).copied().unwrap_or(0)).min().unwrap_or(0);
            band.retain(|f| self.usage.get(&f.id).copied().unwrap_or(0) == least);
        }
        let picked = rng.pick(&band).map(|f| f.id);
        if let (Some(p), true) = (picked, even) {
            *self.usage.entry(p).or_default() += 1;
        }
        Ok(picked)
    }

    fn theme_key(&self, t: &Trainer, place: &Placement) -> Option<String> {
        let league = place.group == Group::League && place.named_character;
        let key = |fallback: String| self.theme_labels.get(&t.label).cloned().unwrap_or(fallback);
        let gym = groups::gym_key(&t.trid).map(|g| format!("gym:{g}"));
        match self.cfg.theme {
            Theme::Off => None,
            Theme::GymLeague if gym.is_some() => gym,
            Theme::GymLeague if league => Some(key(format!("name:{}", t.label))),
            Theme::GymLeague => None,
            Theme::All => gym
                .or_else(|| groups::crew_key(&t.trid).map(|c| format!("crew:{c}")))
                .or_else(|| (!t.label.is_empty()).then(|| key(format!("name:{}", t.label))))
                .or_else(|| Some(format!("row:{}", t.index))),
        }
    }

    fn theme(&mut self, key: &str) -> Result<u8, RandoError> {
        if let Some(&t) = self.themes.get(key) {
            return Ok(t);
        }
        let mut rng = self.rng("theme", key);
        let ty = if self.cfg.weight_types {
            let dex = self.dex;
            let pool = self.pool(Region::Paldea)?;
            let weights: Vec<u64> = (0..TYPE_COUNT)
                .map(|t| pool.iter().filter(|&&p| dex.get(p).is_some_and(|f| f.types.contains(&t))).count() as u64)
                .collect();
            rng.weighted(&weights).unwrap_or(0) as u8
        } else {
            rng.below(u64::from(TYPE_COUNT)) as u8
        };
        self.themes.insert(key.to_owned(), ty);
        Ok(ty)
    }

    fn fully_evolve(&self, p: Pokemon, theme: Option<u8>, rng: &mut Rng) -> Pokemon {
        let mut current = p;
        for _ in 0..4 {
            let Some(info) = self.form(current) else { break };
            let options: Vec<Pokemon> = info
                .evolutions
                .iter()
                .map(|e| e.into)
                .filter(|&e| self.dex.get(e).is_some_and(|f| f.playable() && (!self.cfg.no_legendaries || !f.is_legendary_class())))
                .collect();
            let themed: Vec<Pokemon> = options
                .iter()
                .copied()
                .filter(|&e| theme.is_none_or(|t| self.dex.get(e).is_some_and(|f| f.types.contains(&t))))
                .collect();
            match rng.pick(if themed.is_empty() { &options } else { &themed }) {
                Some(&next) => current = next,
                None => break,
            }
        }
        current
    }

    fn plan(&mut self, t: &Trainer, place: &Placement) -> Result<Trainer, RandoError> {
        let cfg = self.cfg;
        let rules = cfg.groups[&place.group];
        let guard = place.protection;
        let mut new = t.clone();

        for mon in &mut new.team {
            mon.level = modified_level(mon.level, cfg.level_modifier);
        }

        let cap = guard.max_team.unwrap_or(rows::SLOTS).min(rows::SLOTS);
        let mut size = t.team.len();
        if !guard.fixed_team {
            size = if cfg.force_six { rows::SLOTS } else { size + rules.extra };
        }
        let mut doubles = t.doubles;
        if !guard.fixed_battle {
            doubles = match cfg.battle {
                Battle::Unchanged => t.doubles,
                Battle::AllDoubles => true,
                Battle::Mixed => self.rng("battle", t.index).chance(1, 2),
            };
        }
        if doubles && size < 2 && !guard.fixed_team {
            size = 2;
        }
        let size = size.min(cap).max(t.team.len());
        if doubles && size < 2 {
            doubles = t.doubles;
        }
        if doubles != t.doubles {
            new.doubles = doubles;
            new.ai.double = doubles;
        }

        let top_level = new.team.iter().map(|m| m.level).max().unwrap_or(1);
        let ace_bst = t
            .team
            .iter()
            .max_by_key(|m| m.level)
            .and_then(|m| self.form(m.species))
            .map_or(400, FormInfo::bst);
        while new.team.len() < size {
            let mut mon = t.team.last().expect("rows with Pokémon only").clone();
            mon.level = top_level;
            mon.item = 0;
            mon.gem = rows::GEM_DEFAULT;
            mon.sex = rows::SEX_DEFAULT;
            mon.manual_moves = false;
            mon.moves = [0; 4];
            new.team.push(mon);
        }

        let mut locked = vec![false; new.team.len()];
        if cfg.rival_starter && groups::player_starter(&t.trid).is_some() {
            for (k, mon) in t.team.iter().enumerate() {
                if let Some(line) = self.vanilla_lines.iter().position(|l| l.contains(&mon.species)) {
                    new.team[k].species = self.starter_lines[line].at(new.team[k].level);
                    locked[k] = true;
                }
            }
        }

        let theme = match self.theme_key(t, place) {
            Some(key) => Some(self.theme(&key)?),
            None => None,
        };
        self.pick_species(t, place, &mut new, &locked, theme, ace_bst)?;

        if let Some(at) = cfg.force_evolved {
            let mut rng = self.rng("evolve", t.index);
            for (k, mon) in new.team.iter_mut().enumerate() {
                if !locked[k] && mon.level >= at {
                    mon.species = self.fully_evolve(mon.species, theme, &mut rng);
                }
            }
        }

        let items = self.item_plan(&new, &rules);
        let mut ability_rng = self.rng("abilities", t.index);
        let mut tera_rng = self.rng("tera", t.index);
        let mut shiny_rng = self.rng("shiny", t.index);
        let mut move_rng = self.rng("moves", t.index);
        for (k, mon) in new.team.iter_mut().enumerate() {
            let old = t.team.get(k).map(|m| m.species);
            let changed = old != Some(mon.species);
            let Some(form) = self.dex.get(mon.species) else { continue };
            if changed {
                mon.sex = rows::SEX_DEFAULT;
                mon.manual_moves = false;
                mon.moves = [0; 4];
                let old_form_item = old.and_then(|p| self.form(p)).and_then(|f| itempool::form_item(f.national, f.id.form));
                if old_form_item.is_some_and(|i| i64::from(i) == mon.item) {
                    mon.item = 0;
                }
                if mon.gem >= rows::GEM_FIRST_TYPE {
                    let ty = if locked[k] { Some(form.types[0]) } else { theme };
                    if let Some(ty) = ty {
                        mon.gem = rows::GEM_FIRST_TYPE + i64::from(ty);
                    }
                }
                if let Some(item) = itempool::form_item(form.national, form.id.form) {
                    mon.item = i64::from(item);
                }
            }
            if let Some(item) = items[k] {
                if itempool::form_item(form.national, form.id.form).is_none() {
                    mon.item = i64::from(item);
                }
            }
            let no_wg = guard.no_wonder_guard || (cfg.no_early_wg && mon.level < EARLY_LEVEL);
            if cfg.random_abilities {
                let slots: Vec<i64> = (0..3)
                    .filter(|&s| !(no_wg && form.abilities[s] == WONDER_GUARD))
                    .map(|s| rows::TOKUSEI_SET[s])
                    .collect();
                if let Some(&slot) = ability_rng.pick(&slots) {
                    mon.tokusei = slot;
                }
            }
            if no_wg {
                avoid_wonder_guard(mon, form);
            }
            if cfg.random_tera {
                mon.gem = rows::GEM_FIRST_TYPE + tera_rng.below(u64::from(TYPE_COUNT)) as i64;
            }
            if rules.ivs {
                mon.talent_type = rows::TALENT_VALUE;
                mon.talent = [PERFECT_IV; 6];
            }
            if cfg.shiny > 0 && shiny_rng.percent(cfg.shiny) {
                mon.rare = rows::RARE;
            }
            if cfg.better_moves && (changed || !mon.manual_moves) {
                if let Some(moves) = self.moves.better_moveset(form, mon.level, &mut move_rng) {
                    mon.manual_moves = true;
                    mon.moves = moves;
                }
            }
        }
        if rules.tera && !guard.no_tera {
            new.change_gem = true;
        }
        if rules.ai {
            new.ai.basic = true;
            new.ai.high = true;
            new.ai.expert = true;
            new.ai.weak = false;
            new.ai.item = true;
            new.ai.change = true;
        }
        Ok(new)
    }

    /// Replaces the species of every slot the mode allows to change. New slots always get one.
    fn pick_species(
        &mut self,
        t: &Trainer,
        place: &Placement,
        new: &mut Trainer,
        locked: &[bool],
        theme: Option<u8>,
        ace_bst: u32,
    ) -> Result<(), RandoError> {
        let cfg = self.cfg;
        let guard = place.protection;
        let even = match cfg.mode {
            Mode::Even => true,
            Mode::EvenMain => place.main_story,
            Mode::Random | Mode::Unchanged => false,
        };
        let league = place.group == Group::League && place.named_character;
        let mut unique_slots: Vec<usize> = (0..new.team.len()).collect();
        unique_slots.sort_by_key(|&k| std::cmp::Reverse((new.team[k].level, k)));
        unique_slots.truncate(if league { cfg.league_unique } else { 0 });
        let reserved = self.reserved.clone();
        let mut rng = self.rng("species", t.index);
        for (k, &is_locked) in locked.iter().enumerate() {
            let fresh = k >= t.team.len();
            if is_locked || (!fresh && cfg.mode == Mode::Unchanged) {
                continue;
            }
            let no_wg = guard.no_wonder_guard || (cfg.no_early_wg && new.team[k].level < EARLY_LEVEL);
            let target = match t.team.get(k) {
                Some(old) => self.form(old.species).map_or(ace_bst, FormInfo::bst),
                None => ace_bst,
            };
            let unique = unique_slots.contains(&k);
            let avoid_reserved = !league || unique;
            let taken: BTreeSet<Pokemon> = new.team.iter().take(k).map(|m| m.species).collect();
            let blocked = |f: &FormInfo| (no_wg && has_wonder_guard(f)) || (avoid_reserved && reserved.contains(&f.id));
            let allowed = |f: &FormInfo| !blocked(f);
            let themed = |f: &FormInfo| theme.is_none_or(|ty| f.types.contains(&ty));
            let fresh_themed = |f: &FormInfo| allowed(f) && themed(f) && !taken.contains(&f.id);
            let any_themed = |f: &FormInfo| allowed(f) && themed(f);
            let mut picked = None;
            for keep in [&fresh_themed as &dyn Fn(&FormInfo) -> bool, &any_themed, &allowed] {
                picked = self.choose(&mut rng, place.region, target, even, keep)?;
                if picked.is_some() {
                    break;
                }
            }
            if let Some(p) = picked {
                new.team[k].species = p;
                if unique {
                    self.reserved.insert(p);
                }
            }
        }
        Ok(())
    }

    /// The random held item per slot, or None to leave the slot's item alone.
    fn item_plan(&self, new: &Trainer, rules: &GroupRules) -> Vec<Option<u16>> {
        let mut out = vec![None; new.team.len()];
        if !rules.items {
            return out;
        }
        let mut rng = self.rng("items", new.index);
        let ace = (0..new.team.len()).max_by_key(|&k| (new.team[k].level, k));
        for (k, mon) in new.team.iter().enumerate() {
            if self.cfg.items.ace_only && Some(k) != ace {
                continue;
            }
            let Some(form) = self.form(mon.species) else { continue };
            let mut pool: Vec<u16> = self
                .held
                .iter()
                .copied()
                .filter(|&id| !self.cfg.items.consumable || self.catalog.get(id).is_some_and(|i| i.is_consumable()))
                .collect();
            if self.cfg.items.sensible {
                pool = itempool::sensible_for(self.catalog, &pool, form.types, self.dex.is_fully_evolved(form.id));
            }
            out[k] = rng.pick(&pool).copied();
        }
        out
    }
}

/// Moves a fixed ability slot off Wonder Guard, keeping the slot if it is safe.
fn avoid_wonder_guard(mon: &mut Mon, form: &FormInfo) {
    if !possible_abilities(form, mon.tokusei).contains(&WONDER_GUARD) {
        return;
    }
    if let Some(slot) = (0..3).find(|&s| form.abilities[s] != WONDER_GUARD) {
        mon.tokusei = rows::TOKUSEI_SET[slot];
    }
}

fn starter_bases(ctx: &Ctx) -> [Pokemon; 3] {
    let vanilla = VANILLA_STARTERS.map(|national| {
        ctx.dex
            .all()
            .iter()
            .find(|f| f.national == national && f.id.form == 0)
            .map_or(Pokemon { species: national, form: 0 }, |f| f.id)
    });
    ctx.starters.unwrap_or(vanilla)
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let cfg = Config::read(&ctx.settings);
    let vanilla: Vec<Trainer> = ctx
        .store
        .get(TRDATA)?
        .tables("values")
        .enumerate()
        .map(|(i, row)| rows::read(i, row))
        .collect();
    let catalog = ItemCatalog::load(ctx)?;
    let moves = MoveBook::load(&mut ctx.store)?;
    let types = text::read_types(ctx.store.get(TRTYPE)?);

    let planned = plan_all(ctx, &cfg, &vanilla, &catalog, &moves)?;
    let planned = random_classes(ctx, &cfg, &vanilla, planned, &types);

    let changed: Vec<usize> = (0..vanilla.len()).filter(|&i| planned[i] != vanilla[i]).collect();
    if !changed.is_empty() {
        let table = ctx.store.get_mut(TRDATA)?;
        for (i, row) in table.tables_mut("values").enumerate() {
            if planned[i] != vanilla[i] {
                rows::write(row, &vanilla[i], &planned[i]).map_err(|e| RandoError::Section {
                    section: "trainers",
                    detail: format!("{}: {e}", vanilla[i].trid),
                })?;
            }
        }
    }
    log_teams(ctx, &vanilla, &planned, &changed, &types)?;
    rename(ctx, &cfg, &vanilla, &types)?;
    Ok(())
}

fn plan_all(
    ctx: &Ctx,
    cfg: &Config,
    vanilla: &[Trainer],
    catalog: &ItemCatalog,
    moves: &MoveBook,
) -> Result<Vec<Trainer>, RandoError> {
    let bases = starter_bases(ctx);
    let vanilla_lines = VANILLA_STARTERS.map(|national| {
        ctx.dex
            .all()
            .iter()
            .find(|f| f.national == national && f.id.form == 0)
            .map(|f| ctx.dex.line_from(f.id))
            .unwrap_or_default()
    });
    let starter_lines = [0, 1, 2].map(|i| StarterLine::new(&ctx.dex, bases[i], &mut ctx.rng(&format!("trainers.rival_starter/{i}"))));
    let mut theme_labels = BTreeMap::new();
    for t in vanilla {
        if let (true, Some(gym)) = (t.trid.contains("_leader_"), groups::gym_key(&t.trid)) {
            theme_labels.insert(t.label.clone(), format!("gym:{gym}"));
        }
    }
    let mut planner = Planner {
        dex: &ctx.dex,
        settings: &ctx.settings,
        seed: &ctx.seed,
        cfg,
        pools: BTreeMap::new(),
        usage: BTreeMap::new(),
        reserved: BTreeSet::new(),
        themes: BTreeMap::new(),
        theme_labels,
        catalog,
        held: catalog.held(cfg.items.ban_bad),
        moves,
        vanilla_lines,
        starter_lines,
    };
    let ride_legend = |t: &Trainer| {
        t.team.iter().any(|m| {
            ctx.dex
                .get(m.species)
                .is_some_and(|f| facts::NEVER_RANDOM.iter().any(|&(n, _)| n == f.national))
        })
    };
    let mut planned: Vec<Trainer> = vanilla.to_vec();
    let mut order: Vec<(bool, usize)> = Vec::new();
    for t in vanilla {
        let Some(place) = groups::place(&t.trid) else { continue };
        if t.team.is_empty() || !t.contiguous {
            continue;
        }
        if ride_legend(t) || !cfg.in_scope(&place) {
            if place.protection.no_wonder_guard {
                for mon in &mut planned[t.index].team {
                    if let Some(form) = ctx.dex.get(mon.species) {
                        avoid_wonder_guard(mon, form);
                    }
                }
            }
            continue;
        }
        let league = place.group == Group::League && place.named_character;
        order.push((!league, t.index));
    }
    order.sort();
    for (_, index) in order {
        let t = &vanilla[index];
        let place = groups::place(&t.trid).expect("only placed rows are ordered");
        planned[index] = planner.plan(t, &place)?;
    }
    Ok(planned)
}

/// Gives route-style trainers another route-style class of the same gender, the same one for
/// every row sharing a name.
fn random_classes(
    ctx: &Ctx,
    cfg: &Config,
    vanilla: &[Trainer],
    mut planned: Vec<Trainer>,
    types: &BTreeMap<String, text::TypeInfo>,
) -> Vec<Trainer> {
    if !cfg.classes {
        return planned;
    }
    let used: BTreeSet<&str> = vanilla
        .iter()
        .filter(|t| groups::place(&t.trid).is_some())
        .map(|t| t.class.as_str())
        .collect();
    let candidates = |female: bool| -> Vec<String> {
        used.iter()
            .filter(|c| types.get(**c).is_some_and(|i| i.plain && i.female == female))
            .map(|c| (*c).to_owned())
            .collect()
    };
    let (male, female) = (candidates(false), candidates(true));
    let mut chosen: BTreeMap<(String, String), String> = BTreeMap::new();
    for t in vanilla {
        let Some(place) = groups::place(&t.trid) else { continue };
        let Some(info) = types.get(&t.class) else { continue };
        if !cfg.in_scope(&place) || !info.plain {
            continue;
        }
        let key = (t.label.clone(), t.class.clone());
        let class = chosen
            .entry(key)
            .or_insert_with(|| {
                let mut rng = ctx.rng(&format!("trainers.classes/{}/{}", t.label, t.class));
                let pool = if info.female { &female } else { &male };
                rng.pick(pool).cloned().unwrap_or_else(|| t.class.clone())
            })
            .clone();
        planned[t.index].class = class;
    }
    planned
}

fn display_name(names: &GameText, t: &Trainer) -> String {
    match names.by_label(&t.label) {
        Some(n) if !n.is_empty() && !n.contains('[') => format!("{n} ({})", t.trid),
        _ => t.trid.clone(),
    }
}

fn describe(ctx: &Ctx, mon: &Mon) -> String {
    let mut s = format!("{} Lv{}", ctx.names.pokemon(mon.species), mon.level);
    if mon.item != 0 {
        s.push_str(&format!(" @{}", ctx.names.item(mon.item as u16)));
    }
    if mon.rare == rows::RARE {
        s.push_str(" (shiny)");
    }
    if mon.manual_moves && mon.moves.iter().any(|&m| m != 0) {
        let moves: Vec<String> = mon.moves.iter().filter(|&&m| m != 0).map(|&m| ctx.names.move_name(m)).collect();
        s.push_str(&format!(" [{}]", moves.join(", ")));
    }
    s
}

fn log_teams(
    ctx: &mut Ctx,
    vanilla: &[Trainer],
    planned: &[Trainer],
    changed: &[usize],
    types: &BTreeMap<String, text::TypeInfo>,
) -> Result<(), RandoError> {
    if changed.is_empty() {
        return Ok(());
    }
    let names = GameText::load(ctx.game, Language::English, text::NAME_TEXT).map_err(|e| RandoError::Text(e.to_string()))?;
    let classes = GameText::load(ctx.game, Language::English, text::CLASS_TEXT).map_err(|e| RandoError::Text(e.to_string()))?;
    let class_name = |class: &str| {
        types
            .get(class)
            .and_then(|i| classes.by_label(&i.msg_label))
            .map_or_else(|| class.to_owned(), |n| format!("{n} ({class})"))
    };
    let rows: Vec<Vec<String>> = changed
        .iter()
        .map(|&i| {
            let (old, new) = (&vanilla[i], &planned[i]);
            let team: Vec<String> = new.team.iter().map(|m| describe(ctx, m)).collect();
            let mut battle = if new.doubles { "Double".to_owned() } else { "Single".to_owned() };
            if new.change_gem {
                battle.push_str(", Tera");
            }
            let class = if old.class == new.class {
                class_name(&new.class)
            } else {
                format!("{} → {}", class_name(&old.class), class_name(&new.class))
            };
            vec![display_name(&names, old), class, battle, team.join(", ")]
        })
        .collect();
    let section = ctx.log.section("Trainers");
    section.note(format!("{} trainers changed.", changed.len()));
    let table = section.table("Teams", &["Trainer", "Class", "Battle", "Pokémon"]);
    for row in rows {
        table.row(row);
    }
    Ok(())
}

fn rename(ctx: &mut Ctx, cfg: &Config, vanilla: &[Trainer], types: &BTreeMap<String, text::TypeInfo>) -> Result<(), RandoError> {
    if cfg.names != Rename::Off {
        let mut named: BTreeMap<String, (bool, bool)> = BTreeMap::new();
        for t in vanilla {
            let Some(place) = groups::place(&t.trid) else { continue };
            if t.label.is_empty() {
                continue;
            }
            let female = types.get(&t.class).is_some_and(|i| i.female);
            let entry = named.entry(t.label.clone()).or_insert((false, female));
            entry.0 |= place.named_character;
        }
        let wanted: BTreeSet<String> = named
            .iter()
            .filter(|(_, (character, _))| cfg.names == Rename::All || !character)
            .map(|(label, _)| label.clone())
            .collect();
        let labels = text::plain_labels(ctx, text::NAME_TEXT, &wanted)?;
        let mut rng = ctx.rng("trainers.names");
        let to_strings = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let custom = !cfg.name_list.is_empty();
        let mut male = Deck::new(if custom { cfg.name_list.clone() } else { to_strings(text::MALE_NAMES) }, &mut rng);
        let mut female = Deck::new(if custom { cfg.name_list.clone() } else { to_strings(text::FEMALE_NAMES) }, &mut rng);
        let changes: BTreeMap<String, String> = labels
            .into_iter()
            .map(|label| {
                let deck = if !custom && named[&label].1 { &mut female } else { &mut male };
                let name = deck.draw(&mut rng);
                (label, name)
            })
            .collect();
        let log = text::rewrite(ctx, text::NAME_TEXT, &changes)?;
        let table = ctx.log.section("Trainer names").table("Names", &["Label", "Before", "After"]);
        for (label, before, after) in log {
            table.row(vec![label, before, after]);
        }
    }
    if cfg.class_names != Rename::Off {
        let used: BTreeSet<&str> = vanilla
            .iter()
            .filter(|t| groups::place(&t.trid).is_some())
            .map(|t| t.class.as_str())
            .collect();
        let wanted: BTreeSet<String> = used
            .iter()
            .filter_map(|c| types.get(*c))
            .filter(|i| cfg.class_names == Rename::All || i.plain)
            .map(|i| i.msg_label.clone())
            .collect();
        let labels = text::plain_labels(ctx, text::CLASS_TEXT, &wanted)?;
        let mut rng = ctx.rng("trainers.class_names");
        let list = if cfg.class_list.is_empty() {
            text::CLASS_NAMES.iter().map(|s| (*s).to_owned()).collect()
        } else {
            cfg.class_list.clone()
        };
        let mut deck = Deck::new(list, &mut rng);
        let changes: BTreeMap<String, String> = labels.into_iter().map(|l| (l, deck.draw(&mut rng))).collect();
        let log = text::rewrite(ctx, text::CLASS_TEXT, &changes)?;
        let table = ctx.log.section("Trainer names").table("Classes", &["Label", "Before", "After"]);
        for (label, before, after) in log {
            table.row(vec![label, before, after]);
        }
    }
    Ok(())
}
