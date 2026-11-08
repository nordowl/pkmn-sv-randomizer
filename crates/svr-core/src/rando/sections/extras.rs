//! Other content: the Pokémon guarding the Team Star bases.

use std::collections::BTreeMap;

use crate::data::{FormInfo, Pokemon};
use crate::fb::{FieldError, TableValue};
use crate::rando::options::{Kind, Opt, OptionDef, Page, Settings, Source, Status};
use crate::rando::itempool;
use crate::rando::pick::Picker;
use crate::rando::{Ctx, RandoError, Section};

pub const SECTION: Section = Section {
    id: "extras",
    options,
    run,
};

const TEAM_STAR: &str = "extras.team_star";
const TEAM_STAR_POOL: &str = "extras.team_star.pool";
const TEAM_STAR_SIMILAR: &str = "extras.team_star.similar";

const AJITO: &str = "world/data/ajito/AjitoPokemon/AjitoPokemon_array.bin";
/// Rows are numbered crew * 100 + member; this many crews exist.
const CREWS: usize = 5;
/// The boss of each crew, by crew number, for the spoiler log.
const CREW_BOSSES: [&str; CREWS] = ["Mela", "Giacomo", "Atticus", "Eri", "Ortega"];

fn options() -> Vec<OptionDef> {
    let page = Page::Extras;
    vec![
        Opt::choice(
            TEAM_STAR,
            page,
            "Team Star bases",
            "Team Star base Pokémon",
            "unchanged",
            &[
                ("unchanged", "Unchanged", "The bases keep the game's Pokémon."),
                ("random", "Random", "Every Pokémon at the bases is a random species."),
                ("crew_type", "Crew type", "Every Pokémon at a base is a random species of that crew's type, such as Fire for Mela's crew."),
            ],
        )
        .help("The Pokémon your team fights in the timed Star Barrage at each Team Star base.")
        .sources(&[Source::New])
        .status(Status::Test)
        .build(),
        Opt::new(TEAM_STAR_POOL, page, "Team Star bases", "Team Star base pool", Kind::Pool)
            .help("Which species the Team Star bases can roll.")
            .not(TEAM_STAR, "unchanged")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(TEAM_STAR_SIMILAR, page, "Team Star bases", "Similar strength", false)
            .help("Each replacement has a base stat total close to the Pokémon it replaces.")
            .not(TEAM_STAR, "unchanged")
            .sources(&[Source::New])
            .build(),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Random,
    CrewType,
}

struct Plan {
    mode: Mode,
    similar: bool,
}

impl Plan {
    fn read(s: &Settings) -> Option<Plan> {
        let mode = match s.choice(TEAM_STAR) {
            "random" => Mode::Random,
            "crew_type" => Mode::CrewType,
            _ => return None,
        };
        Some(Plan {
            mode,
            similar: s.on(TEAM_STAR_SIMILAR),
        })
    }
}

struct Member {
    row: usize,
    crew: usize,
    species: Pokemon,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "extras",
        detail: e.to_string(),
    }
}

fn read_member(row: usize, values: &TableValue) -> Result<Option<Member>, RandoError> {
    let Some(entry) = values.table("ajitoPokemonTable") else { return Ok(None) };
    let id = entry.int("pokemonId").map_err(shape)?;
    // Row 0 is a placeholder outside every crew.
    if id <= 0 {
        return Ok(None);
    }
    let field = |name: &str| -> Result<u16, RandoError> {
        let v = entry.int(name).map_err(shape)?;
        u16::try_from(v).map_err(|_| shape_detail(format!("{name} {v} is out of range")))
    };
    let crew = usize::try_from(id / 100).unwrap_or(usize::MAX);
    if crew >= CREWS {
        return Err(shape_detail(format!("Team Star row {row} names crew {crew}")));
    }
    Ok(Some(Member {
        row,
        crew,
        species: Pokemon {
            species: field("devId")?,
            form: field("formNo")?,
        },
    }))
}

fn shape_detail(detail: String) -> RandoError {
    RandoError::Section { section: "extras", detail }
}

/// The type most of a crew's vanilla Pokémon share; ties go to the lower type id.
fn crew_types(members: &[Member], dex: &crate::data::Dex) -> [Option<u8>; CREWS] {
    let mut counts: [BTreeMap<u8, usize>; CREWS] = Default::default();
    for m in members {
        let Some(info) = dex.get(m.species) else { continue };
        let [a, b] = info.types;
        for t in if a == b { vec![a] } else { vec![a, b] } {
            *counts[m.crew].entry(t).or_default() += 1;
        }
    }
    counts.map(|c| c.into_iter().max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0))).map(|(t, _)| t))
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let Some(plan) = Plan::read(&ctx.settings) else { return Ok(()) };
    let mut members = Vec::new();
    for (row, values) in ctx.store.get(AJITO)?.tables("values").enumerate() {
        members.extend(read_member(row, values)?);
    }
    let types = crew_types(&members, &ctx.dex);
    let pool = ctx.pool(TEAM_STAR_POOL, "Team Star base Pokémon")?;
    let picker = Picker::new(&ctx.dex, pool);
    let mut rng = ctx.rng(TEAM_STAR);
    let mut picks = Vec::new();
    for m in &members {
        let crew_type = match plan.mode {
            Mode::Random => None,
            Mode::CrewType => types[m.crew],
        };
        // Base Pokémon hold nothing, so forms that need an item (plates, masks, Origin forms) stay out.
        let keep = |f: &FormInfo| {
            crew_type.is_none_or(|t| f.types.contains(&t)) && itempool::form_item(f.national, f.id.form).is_none()
        };
        let target = ctx.dex.get(m.species).map(FormInfo::bst).filter(|_| plan.similar);
        let pick = match target {
            Some(bst) => picker.similar_strength(&mut rng, bst, keep),
            None => picker.matching(&mut rng, keep),
        };
        let what = match crew_type {
            Some(t) => format!("Team Star base Pokémon of the {} type", ctx.names.type_name(t)),
            None => "Team Star base Pokémon".to_owned(),
        };
        picks.push(pick.ok_or(RandoError::EmptyPool { what })?);
    }

    let changed: Vec<(&Member, Pokemon)> =
        members.iter().zip(picks).filter(|(m, p)| m.species != *p).collect();
    if !changed.is_empty() {
        let mut rows: Vec<&mut TableValue> = ctx.store.get_mut(AJITO)?.tables_mut("values").collect();
        for &(m, p) in &changed {
            let entry = rows[m.row].table_mut("ajitoPokemonTable").map_err(shape)?;
            entry.set_int("devId", i64::from(p.species)).map_err(shape)?;
            entry.set_int("formNo", i64::from(p.form)).map_err(shape)?;
        }
    }
    let log = ctx.log.section("Team Star bases");
    for (crew, boss) in CREW_BOSSES.iter().enumerate() {
        if let Some(t) = types[crew] {
            log.note(format!("{boss}'s crew is {}.", ctx.names.type_name(t)));
        }
    }
    let table = log.table("Team Star bases", &["Crew", "Was", "Now"]);
    for (m, p) in changed {
        table.row(vec![CREW_BOSSES[m.crew].to_owned(), ctx.names.pokemon(m.species), ctx.names.pokemon(p)]);
    }
    Ok(())
}
