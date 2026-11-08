//! Wild Pokémon: the spawn tables of Paldea, Kitakami and Blueberry Academy.
//!
//! Each row is a spawn rule: one species in some areas and biomes. Rows keep their places and
//! levels and only the species changes, except in full shuffle, which builds rows from the
//! vanilla ones.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::{Dex, FormInfo, Pokemon};
use crate::fb::{TableValue, Value};
use crate::rando::itempool;
use crate::rando::options::{Kind, Opt, OptionDef, Page, Req, Settings, Source, Status};
use crate::rando::pick::Picker;
use crate::rando::{Ctx, RandoError, Rng, Section};

use super::encounter_common::{
    self as common, enum_value, put, shape, Habits, Moves, Pref, BLUEBERRY_WILD, KITAKAMI_WILD, PALDEA_WILD, TYPE_COUNT,
};

pub const SECTION: Section = Section {
    id: "wild",
    options,
    run,
};

const SAME: &str = "wild.same";

struct Ids {
    mode: &'static str,
    pool: &'static str,
    similar: &'static str,
    catch_all: &'static str,
    themes: &'static str,
    no_legendaries: &'static str,
    alt_forms: &'static str,
    movement: &'static str,
    items: &'static str,
    ban_bad_items: &'static str,
    level: &'static str,
    all_day: &'static str,
    both_versions: &'static str,
}

macro_rules! ids {
    ($region:literal) => {
        Ids {
            mode: concat!("wild.", $region, ".mode"),
            pool: concat!("wild.", $region, ".pool"),
            similar: concat!("wild.", $region, ".similar_strength"),
            catch_all: concat!("wild.", $region, ".catch_all"),
            themes: concat!("wild.", $region, ".type_themes"),
            no_legendaries: concat!("wild.", $region, ".no_legendaries"),
            alt_forms: concat!("wild.", $region, ".alt_forms"),
            movement: concat!("wild.", $region, ".movement"),
            items: concat!("wild.", $region, ".items"),
            ban_bad_items: concat!("wild.", $region, ".ban_bad_items"),
            level: concat!("wild.", $region, ".level"),
            all_day: concat!("wild.", $region, ".all_day"),
            both_versions: concat!("wild.", $region, ".both_versions"),
        }
    };
}

struct Region {
    key: &'static str,
    name: &'static str,
    table: &'static str,
    ids: Ids,
}

static REGIONS: [Region; 3] = [
    Region {
        key: "paldea",
        name: "Paldea",
        table: PALDEA_WILD,
        ids: ids!("paldea"),
    },
    Region {
        key: "kitakami",
        name: "Kitakami",
        table: KITAKAMI_WILD,
        ids: ids!("kitakami"),
    },
    Region {
        key: "blueberry",
        name: "Blueberry Academy",
        table: BLUEBERRY_WILD,
        ids: ids!("blueberry"),
    },
];

/// Rows that give a held item but have no rate get this one, the game's most common rate.
const DEFAULT_ITEM_RATE: i64 = 5;
const ALWAYS: i64 = 100;

fn options() -> Vec<OptionDef> {
    let page = Page::Wild;
    let mut out = vec![Opt::toggle(SAME, page, "All regions", "Use Paldea settings for all regions", false)
        .help("Kitakami and Blueberry Academy follow the Paldea settings. In global 1-to-1 mode one species map then covers all three, and catch-'em-all counts all three together.")
        .sources(&[Source::Sv])
        .build()];
    for region in &REGIONS {
        let ids = &region.ids;
        let group = region.name;
        let follows_paldea = region.key != "paldea";
        let opt = |o: Opt| if follows_paldea { o.requires(Req::Off { id: SAME }) } else { o };
        let randomized = |o: Opt| opt(o).not(ids.mode, "unchanged");
        out.extend([
            opt(Opt::choice(
                ids.mode,
                page,
                group,
                "Wild Pokémon",
                "unchanged",
                &[
                    ("unchanged", "Unchanged", "Wild Pokémon stay as they are."),
                    ("swap", "Row swap", "Every spawn rule gets a random species but keeps its areas, biomes and levels."),
                    ("global", "Global 1-to-1", "Each species always becomes the same replacement, everywhere."),
                    ("area", "Area 1-to-1", "Each species becomes one replacement per area. Spawn rules that cover several areas are split up first."),
                    ("shuffle", "Full shuffle", "Every species in the pool spawns somewhere, in places borrowed from the original spawns."),
                ],
            ))
            .help("How wild Pokémon are replaced. Herds follow their new leader: its previous evolution stage when the pool has it, otherwise the leader's own species.")
            .sources(&[Source::Upr, Source::Sv, Source::New])
            .status(Status::Test)
            .build(),
            randomized(Opt::new(ids.pool, page, group, "Wild pool", Kind::Pool))
                .help("Which species can appear in the wild here.")
                .sources(&[Source::Sv])
                .build(),
            randomized(Opt::toggle(ids.similar, page, group, "Similar strength", false))
                .help("Each replacement has a base stat total close to the original's, so early areas stay gentle.")
                .sources(&[Source::Upr])
                .build(),
            randomized(Opt::toggle(ids.catch_all, page, group, "Catch 'em all", false))
                .help("Every species in the pool appears before any repeats, as far as there are spawn rules for them.")
                .not(ids.mode, "shuffle")
                .sources(&[Source::Upr])
                .build(),
            opt(Opt::toggle(ids.themes, page, group, "Type-themed areas", false))
                .help("Every area gets one random type, and its replacements share that type when the pool has one that can spawn there.")
                .is(ids.mode, "area")
                .sources(&[Source::Upr])
                .status(Status::Test)
                .build(),
            randomized(Opt::toggle(ids.no_legendaries, page, group, "No legendaries", false))
                .help("Keeps legendary, mythical and Ultra Beast Pokémon out of the wild, whatever the pool says.")
                .sources(&[Source::Upr, Source::Sv])
                .build(),
            randomized(Opt::toggle(ids.alt_forms, page, group, "Allow alternate forms", true))
                .help("Off rolls only each species' standard form, even if the pool allows others.")
                .sources(&[Source::Upr])
                .build(),
            randomized(Opt::toggle(ids.movement, page, group, "Fliers and swimmers only where they can move", true))
                .help("A replacement spawns only the ways the game itself spawns that species somewhere: on the ground, on or under water, or in the air. Species the game never spawns in the wild count as walkers.")
                .sources(&[Source::Sv])
                .status(Status::Test)
                .build(),
            opt(Opt::toggle(ids.items, page, group, "Random held items", false))
                .help("Wild Pokémon here can hold a random item.")
                .sources(&[Source::Upr])
                .build(),
            opt(Opt::toggle(ids.ban_bad_items, page, group, "No useless held items", false))
                .help("Leaves out items that only hurt the holder or do nothing.")
                .on(ids.items)
                .sources(&[Source::Upr])
                .build(),
            opt(Opt::number(ids.level, page, group, "Level change", 0, -50, 100, "%"))
                .step(5)
                .help("Raises or lowers every wild level range by this percentage.")
                .sources(&[Source::Upr])
                .build(),
            opt(Opt::toggle(ids.all_day, page, group, "Remove time-of-day restrictions", false))
                .help("Pokémon that only appear at certain times appear all day.")
                .sources(&[Source::Upr])
                .build(),
            opt(Opt::toggle(ids.both_versions, page, group, "Remove version exclusives", false))
                .help("Scarlet and Violet exclusives appear in both games.")
                .sources(&[Source::New])
                .build(),
        ]);
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unchanged,
    Swap,
    Global,
    Area,
    Shuffle,
}

struct Rules {
    mode: Mode,
    similar: bool,
    catch_all: bool,
    themes: bool,
    legendaries: bool,
    alt_forms: bool,
    movement: bool,
    items: Option<bool>,
    level: i64,
    all_day: bool,
    both_versions: bool,
}

impl Rules {
    fn read(settings: &Settings, ids: &Ids) -> Rules {
        let mode = match settings.choice(ids.mode) {
            "swap" => Mode::Swap,
            "global" => Mode::Global,
            "area" => Mode::Area,
            "shuffle" => Mode::Shuffle,
            _ => Mode::Unchanged,
        };
        Rules {
            mode,
            similar: settings.on(ids.similar),
            catch_all: settings.on(ids.catch_all) && mode != Mode::Shuffle,
            themes: settings.on(ids.themes) && mode == Mode::Area,
            legendaries: !settings.on(ids.no_legendaries),
            alt_forms: settings.on(ids.alt_forms),
            movement: settings.on(ids.movement),
            items: settings.on(ids.items).then(|| settings.on(ids.ban_bad_items)),
            level: settings.number(ids.level),
            all_day: settings.on(ids.all_day),
            both_versions: settings.on(ids.both_versions),
        }
    }

    fn tweaks(&self) -> bool {
        self.items.is_some() || self.level != 0 || self.all_day || self.both_versions
    }

    fn changes_anything(&self) -> bool {
        self.mode != Mode::Unchanged || self.tweaks()
    }
}

fn species(row: &TableValue) -> Result<Pokemon, RandoError> {
    Ok(Pokemon {
        species: row.int("devid").map_err(shape)? as u16,
        form: row.int("formno").map_err(shape)? as u16,
    })
}

/// The species that follows this row's leader in a herd, if any.
fn follower(row: &TableValue) -> Result<Option<Pokemon>, RandoError> {
    let species = row.int("bandpoke").map_err(shape)? as u16;
    let form = row.int("bandFormno").map_err(shape)? as u16;
    Ok((species != 0).then_some(Pokemon { species, form }))
}

/// Followers carry no held item, so a herd whose follower would need one spawns alone.
fn set_follower(row: &mut TableValue, dex: &Dex, p: Pokemon) -> Result<(), RandoError> {
    let alone = common::form_item(dex, p).is_some();
    if alone {
        put(row, "bandtype", enum_value(row, "bandtype", "NONE")?)?;
        put(row, "bandrate", 0)?;
    }
    let p = if alone { Pokemon { species: 0, form: 0 } } else { p };
    put(row, "bandpoke", i64::from(p.species))?;
    put(row, "bandFormno", i64::from(p.form))?;
    put(row, "bandSex", enum_value(row, "SexType", "DEFAULT")?)
}

/// A spawn rule whose species needs a held item to keep its form always holds it.
fn hold_form_item(row: &mut TableValue, dex: &Dex) -> Result<Option<u16>, RandoError> {
    let Some(item) = common::form_item(dex, species(row)?) else {
        return Ok(None);
    };
    let bring = row.struct_mut("bringItem").map_err(shape)?;
    bring.set_int("itemID", i64::from(item)).map_err(shape)?;
    bring.set_int("bringRate", ALWAYS).map_err(shape)?;
    Ok(Some(item))
}

/// Which replacements a pick must avoid to keep a mapping one-to-one.
#[derive(Clone, Copy)]
enum Avoid<'k> {
    Nothing,
    UsedAnywhere,
    UsedIn(&'k str),
}

/// Chooses replacements for one set of rules. Regions that share settings share a placer, so a
/// global map and the catch-'em-all count span all of them.
struct Placer<'a> {
    dex: &'a Dex,
    picker: Picker<'a>,
    in_pool: BTreeSet<Pokemon>,
    habits: &'a Habits,
    similar: bool,
    catch_all: bool,
    movement: bool,
    rng: Rng,
    unplaced: BTreeSet<Pokemon>,
    used: BTreeSet<Pokemon>,
    global: BTreeMap<Pokemon, Pokemon>,
    areas: BTreeMap<String, BTreeMap<Pokemon, Pokemon>>,
    themes: BTreeMap<String, u8>,
}

impl<'a> Placer<'a> {
    fn new(dex: &'a Dex, pool: Vec<Pokemon>, habits: &'a Habits, rules: &Rules, rng: Rng) -> Placer<'a> {
        let in_pool: BTreeSet<Pokemon> = pool.iter().copied().collect();
        Placer {
            dex,
            unplaced: in_pool.clone(),
            in_pool,
            picker: Picker::new(dex, pool),
            habits,
            similar: rules.similar,
            catch_all: rules.catch_all,
            movement: rules.movement,
            rng,
            used: BTreeSet::new(),
            global: BTreeMap::new(),
            areas: BTreeMap::new(),
            themes: BTreeMap::new(),
        }
    }

    /// A replacement for `original` that can move in some of the `wanted` spawn modes. A mapped
    /// species that also follows herds avoids forms that need a held item.
    fn pick(&mut self, original: Pokemon, wanted: Moves, theme: Option<u8>, avoid: Avoid) -> Pokemon {
        let target = if self.similar { self.dex.get(original).map(FormInfo::bst) } else { None };
        let mapped_follower = !matches!(avoid, Avoid::Nothing) && self.habits.follows_a_herd(original);
        let itemless = |f: &FormInfo| !mapped_follower || common::form_item(self.dex, f.id).is_none();
        let can_move = |f: &FormInfo| !self.movement || wanted.overlaps(self.habits.moves(f.id));
        let themed = |f: &FormInfo| theme.is_none_or(|t| f.types.contains(&t));
        let new = |f: &FormInfo| match avoid {
            Avoid::Nothing => true,
            Avoid::UsedAnywhere => !self.used.contains(&f.id),
            Avoid::UsedIn(area) => self.areas.get(area).is_none_or(|m| m.values().all(|&v| v != f.id)),
        };
        let unplaced = |f: &FormInfo| !self.catch_all || self.unplaced.contains(&f.id);
        let prefs: [Pref; 5] = [&itemless, &can_move, &themed, &new, &unplaced];
        let p = common::choose(&self.picker, &mut self.rng, target, &prefs);
        self.mark_placed(p);
        p
    }

    fn mark_placed(&mut self, p: Pokemon) {
        self.used.insert(p);
        self.unplaced.remove(&p);
        if self.unplaced.is_empty() {
            self.unplaced = self.in_pool.clone();
        }
    }

    fn global(&mut self, original: Pokemon) -> Pokemon {
        if let Some(&p) = self.global.get(&original) {
            return p;
        }
        let p = self.pick(original, self.habits.moves(original), None, Avoid::UsedAnywhere);
        self.global.insert(original, p);
        p
    }

    fn in_area(&mut self, area: &str, original: Pokemon, themes: bool) -> Pokemon {
        if let Some(&p) = self.areas.get(area).and_then(|m| m.get(&original)) {
            return p;
        }
        let theme = themes.then(|| {
            let rng = &mut self.rng;
            *self.themes.entry(area.to_owned()).or_insert_with(|| rng.below(u64::from(TYPE_COUNT)) as u8)
        });
        let p = self.pick(original, self.habits.moves(original), theme, Avoid::UsedIn(area));
        self.areas.entry(area.to_owned()).or_default().insert(original, p);
        p
    }

    /// A new herd leader is followed by its previous evolution stage, as in the vanilla herds,
    /// when the pool has it.
    fn herd_of(&self, leader: Pokemon) -> Pokemon {
        self.dex.pre_evolution(leader).filter(|p| self.in_pool.contains(p)).unwrap_or(leader)
    }
}

/// A spawn rule's place in plain words. In area mode it also names the unit a species map covers.
fn place_label(region: &Region, row: &TableValue) -> String {
    let area = row.string("area").unwrap_or_default();
    let location = row.string("locationName").unwrap_or_default();
    let place = match (area.is_empty(), location.is_empty()) {
        (false, true) => format!("area {area}"),
        (true, false) => location.to_owned(),
        (false, false) => format!("area {area} at {location}"),
        (true, true) => "anywhere".to_owned(),
    };
    format!("{}: {place}", region.name)
}

/// One row per listed area or location, so each area can get its own replacements. Spawn rates
/// stay the same, because each area keeps the row's weight.
fn split_by_area(rows: Vec<TableValue>) -> Result<Vec<TableValue>, RandoError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let area = row.string("area").unwrap_or_default().to_owned();
        let location = row.string("locationName").unwrap_or_default().to_owned();
        let (field, list) = match (area.is_empty(), location.is_empty()) {
            (false, true) => ("area", area),
            (true, false) => ("locationName", location),
            _ => {
                out.push(row);
                continue;
            }
        };
        let parts: Vec<&str> = list.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() <= 1 {
            out.push(row);
            continue;
        }
        for part in parts {
            let mut copy = row.clone();
            copy.set_string(field, part).map_err(shape)?;
            out.push(copy);
        }
    }
    Ok(out)
}

/// Writes a new species into a spawn rule, with that species' voice and, under the movement
/// rule, only the spawn modes it can use. Returns whether the spawn modes changed.
fn place(row: &mut TableValue, p: Pokemon, habits: &Habits, movement: bool) -> Result<bool, RandoError> {
    put(row, "devid", i64::from(p.species))?;
    put(row, "formno", i64::from(p.form))?;
    put(row, "sex", enum_value(row, "SexType", "DEFAULT")?)?;
    if let Some(voice) = habits.voice(p) {
        if row.string("pokeVoiceClassification") != Some(voice) {
            row.set_string("pokeVoiceClassification", voice).map_err(shape)?;
        }
    }
    if !movement {
        return Ok(false);
    }
    let before = Moves::read(row)?;
    let after = before.fit(habits.moves(p));
    after.write(row)?;
    Ok(after != before)
}

/// A row-level change worth its own log line, by its index in the region's final rows.
struct Change {
    index: usize,
    place: String,
    was: Pokemon,
}

struct Placed {
    rows: Vec<TableValue>,
    changes: Vec<Change>,
    moved: usize,
}

fn place_region(region: &Region, rules: &Rules, placer: &mut Placer, rows: Vec<TableValue>) -> Result<Placed, RandoError> {
    let rows = match rules.mode {
        Mode::Area => split_by_area(rows)?,
        Mode::Shuffle => return shuffle(region, rules, placer, &rows),
        _ => rows,
    };
    let mut out = Placed {
        rows: Vec::with_capacity(rows.len()),
        changes: Vec::new(),
        moved: 0,
    };
    for (index, mut row) in rows.into_iter().enumerate() {
        let was = species(&row)?;
        let area = place_label(region, &row);
        let now = match rules.mode {
            Mode::Swap => placer.pick(was, Moves::read(&row)?, None, Avoid::Nothing),
            Mode::Global => placer.global(was),
            Mode::Area => placer.in_area(&area, was, rules.themes),
            Mode::Shuffle | Mode::Unchanged => was,
        };
        if let Some(old) = follower(&row)? {
            let new = match rules.mode {
                Mode::Global => placer.global(old),
                Mode::Area => placer.in_area(&area, old, rules.themes),
                _ => placer.herd_of(now),
            };
            set_follower(&mut row, placer.dex, new)?;
        }
        out.moved += usize::from(place(&mut row, now, placer.habits, rules.movement)?);
        if rules.mode == Mode::Swap {
            out.changes.push(Change { index, place: area, was });
        }
        out.rows.push(row);
    }
    Ok(out)
}

/// Gives every pool species a spawn rule copied from a vanilla rule of this region, and uses
/// every vanilla rule at least once, so no place loses its spawns. Pairs prefer rules the species
/// can move in and, with similar strength, originals of similar strength.
fn shuffle(region: &Region, rules: &Rules, placer: &mut Placer, templates: &[TableValue]) -> Result<Placed, RandoError> {
    let dex = placer.dex;
    let habits = placer.habits;
    let bst = |p: Pokemon| dex.get(p).map_or(0, FormInfo::bst);
    let originals: Vec<Pokemon> = templates.iter().map(species).collect::<Result<_, _>>()?;
    let template_moves: Vec<Moves> = templates.iter().map(Moves::read).collect::<Result<_, _>>()?;
    let cost = |t: usize, p: Pokemon| {
        let stuck = rules.movement && !template_moves[t].overlaps(habits.moves(p));
        let gap = if rules.similar { bst(originals[t]).abs_diff(bst(p)) } else { 0 };
        (stuck, gap)
    };
    let mut species_order = placer.picker.pool().to_vec();
    placer.rng.shuffle(&mut species_order);
    let mut template_order: Vec<usize> = (0..templates.len()).collect();
    placer.rng.shuffle(&mut template_order);
    let pairs = pair_up(&mut placer.rng, template_order, species_order, cost);

    let mut out = Placed {
        rows: Vec::with_capacity(pairs.len()),
        changes: Vec::with_capacity(pairs.len()),
        moved: 0,
    };
    for (index, (t, p)) in pairs.into_iter().enumerate() {
        let mut row = templates[t].clone();
        if follower(&row)?.is_some() {
            set_follower(&mut row, dex, placer.herd_of(p))?;
        }
        out.moved += usize::from(place(&mut row, p, habits, rules.movement)?);
        placer.mark_placed(p);
        out.changes.push(Change {
            index,
            place: place_label(region, &row),
            was: originals[t],
        });
        out.rows.push(row);
    }
    Ok(out)
}

/// Pairs templates with species so every template and every species appears at least once: the
/// longer list is walked once, and the shorter one is dealt from a reshuffled deck. Each pick
/// takes the cheapest remaining card, earliest first.
fn pair_up(
    rng: &mut Rng,
    templates: Vec<usize>,
    species: Vec<Pokemon>,
    cost: impl Fn(usize, Pokemon) -> (bool, u32),
) -> Vec<(usize, Pokemon)> {
    let mut out = Vec::with_capacity(templates.len().max(species.len()));
    if species.is_empty() || templates.is_empty() {
        return out;
    }
    if templates.len() >= species.len() {
        let mut deck: Vec<Pokemon> = Vec::new();
        for t in templates {
            if deck.is_empty() {
                deck = species.clone();
                rng.shuffle(&mut deck);
            }
            let best = (0..deck.len()).min_by_key(|&i| cost(t, deck[i])).expect("deck is refilled");
            out.push((t, deck.remove(best)));
        }
    } else {
        let mut deck: Vec<usize> = Vec::new();
        for p in species {
            if deck.is_empty() {
                deck = templates.clone();
                rng.shuffle(&mut deck);
            }
            let best = (0..deck.len()).min_by_key(|&i| cost(deck[i], p)).expect("deck is refilled");
            out.push((deck.remove(best), p));
        }
    }
    out
}

fn tweak(row: &mut TableValue, rules: &Rules, held: &[u16], rng: &mut Rng) -> Result<(), RandoError> {
    if rules.level != 0 {
        for field in ["minlevel", "maxlevel"] {
            let level = row.int(field).map_err(shape)?;
            if level > 0 {
                put(row, field, common::scaled_level(level, rules.level))?;
            }
        }
    }
    if let Some(&item) = rng.pick(held) {
        let bring = row.struct_mut("bringItem").map_err(shape)?;
        bring.set_int("itemID", i64::from(item)).map_err(shape)?;
        if bring.int("bringRate").map_err(shape)? == 0 {
            bring.set_int("bringRate", DEFAULT_ITEM_RATE).map_err(shape)?;
        }
    }
    let mut all_true = |table: &str, fields: &[&str]| -> Result<(), RandoError> {
        let t = row.table_mut(table).map_err(shape)?;
        for field in fields {
            if !t.bool(field).map_err(shape)? {
                t.set_bool(field, true).map_err(shape)?;
            }
        }
        Ok(())
    };
    if rules.all_day {
        all_true("timetable", &["morning", "noon", "evening", "night"])?;
    }
    if rules.both_versions {
        all_true("versiontable", &["A", "B"])?;
    }
    Ok(())
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let same = ctx.settings.on(SAME);
    let rules: Vec<Rules> = REGIONS
        .iter()
        .map(|r| Rules::read(&ctx.settings, if same { &REGIONS[0].ids } else { &r.ids }))
        .collect();
    if rules.iter().any(Rules::changes_anything) {
        randomize(ctx, same, rules)?;
    }
    // Trade requests must name a species that can still be caught, so they wait for this section.
    super::gifts::randomize_trade_requests(ctx)
}

/// One region's settings and what its run produced for the log.
struct RegionRun {
    region: &'static Region,
    rules: Rules,
    held: Vec<u16>,
    item_rng: Rng,
    notes: Vec<String>,
    spawn_lines: Vec<Vec<String>>,
    item_lines: Vec<Vec<String>>,
}

fn randomize(ctx: &mut Ctx, same: bool, rules: Vec<Rules>) -> Result<(), RandoError> {
    let owner = |i: usize| if same { 0 } else { i };
    let habits = Habits::load(ctx)?;
    let mut runs = Vec::with_capacity(REGIONS.len());
    for (region, rules) in REGIONS.iter().zip(rules) {
        let held = match rules.items {
            Some(ban_bad) => itempool::held_items(ctx, ban_bad)?,
            None => Vec::new(),
        };
        runs.push(RegionRun {
            region,
            rules,
            held,
            item_rng: ctx.rng(&format!("wild.items.{}", region.key)),
            notes: Vec::new(),
            spawn_lines: Vec::new(),
            item_lines: Vec::new(),
        });
    }
    let mut placers: Vec<Option<Placer>> = Vec::with_capacity(runs.len());
    for (i, run) in runs.iter().enumerate() {
        let placer = if owner(i) == i && run.rules.mode != Mode::Unchanged {
            let what = if same { "wild Pokémon".to_owned() } else { format!("wild Pokémon in {}", run.region.name) };
            let pool = common::section_pool(ctx, run.region.ids.pool, &what, run.rules.alt_forms, run.rules.legendaries)?;
            let pool = common::overworld(&ctx.dex, pool, &what)?;
            let rng = ctx.rng(&format!("wild.species.{}", if same { "all" } else { run.region.key }));
            Some(Placer::new(&ctx.dex, pool, &habits, &run.rules, rng))
        } else {
            None
        };
        placers.push(placer);
    }

    for (i, run) in runs.iter_mut().enumerate() {
        if !run.rules.changes_anything() {
            continue;
        }
        let rows: Vec<TableValue> = ctx.store.get(run.region.table)?.tables("values").cloned().collect();
        let before = rows.len();
        let mut placed = match placers[owner(i)].as_mut() {
            Some(placer) if run.rules.mode != Mode::Unchanged => place_region(run.region, &run.rules, placer, rows)?,
            _ => Placed {
                rows,
                changes: Vec::new(),
                moved: 0,
            },
        };
        let mut form_items = 0;
        for row in &mut placed.rows {
            tweak(row, &run.rules, &run.held, &mut run.item_rng)?;
            if run.rules.mode != Mode::Unchanged {
                form_items += usize::from(hold_form_item(row, &ctx.dex)?.is_some());
            }
        }
        if form_items > 0 {
            run.notes.push(format!("{form_items} spawn rules always hold the item their form needs."));
        }
        let names = &ctx.names;
        for change in &placed.changes {
            let row = &placed.rows[change.index];
            let levels = format!("{}-{}", row.int("minlevel").map_err(shape)?, row.int("maxlevel").map_err(shape)?);
            run.spawn_lines.push(vec![change.place.clone(), levels, names.pokemon(change.was), names.pokemon(species(row)?)]);
        }
        if run.rules.items.is_some() {
            for row in &placed.rows {
                let bring = row.struct_value("bringItem");
                let item = bring.map_or(0, |b| b.int("itemID").unwrap_or(0));
                let rate = bring.map_or(0, |b| b.int("bringRate").unwrap_or(0));
                run.item_lines.push(vec![
                    place_label(run.region, row),
                    names.pokemon(species(row)?),
                    common::item_label(names, item),
                    format!("{rate}%"),
                ]);
            }
        }
        if placed.rows.len() != before {
            run.notes.push(format!("{before} spawn rules became {}.", placed.rows.len()));
        }
        if placed.moved > 0 {
            run.notes.push(format!("{} spawn rules now spawn only where their new species can move.", placed.moved));
        }
        run.notes.extend(tweak_notes(&run.rules));
        *ctx.store.get_mut(run.region.table)?.vector_mut("values").map_err(shape)? =
            placed.rows.into_iter().map(Value::Table).collect();
    }

    let names = &ctx.names;
    let log = ctx.log.section("Wild Pokémon");
    for (i, run) in runs.iter().enumerate() {
        let name = run.region.name;
        for note in &run.notes {
            log.note(format!("{name}: {note}"));
        }
        if !run.spawn_lines.is_empty() {
            let t = log.table(&format!("{name}: spawn rules"), &["Where", "Levels", "Was", "Now"]);
            for line in &run.spawn_lines {
                t.row(line.clone());
            }
        }
        if let Some(placer) = placers[owner(i)].as_ref() {
            let prefix = format!("{name}: ");
            let areas: Vec<_> = placer.areas.iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
            if !areas.is_empty() {
                let t = log.table(&format!("{name}: by area"), &["Area", "Type", "Was", "Now"]);
                for (area, map) in areas {
                    let theme = placer.themes.get(area).map(|&t| names.type_name(t)).unwrap_or_default();
                    for (was, now) in map {
                        t.row(vec![area.clone(), theme.clone(), names.pokemon(*was), names.pokemon(*now)]);
                    }
                }
            }
        }
        if !run.item_lines.is_empty() {
            let t = log.table(&format!("{name}: held items"), &["Where", "Pokémon", "Item", "Chance"]);
            for line in &run.item_lines {
                t.row(line.clone());
            }
        }
    }
    for (o, placer) in placers.iter().enumerate() {
        let Some(placer) = placer.as_ref().filter(|p| !p.global.is_empty()) else { continue };
        let title = if same { "All regions" } else { REGIONS[o].name };
        let t = log.table(&format!("{title}: species map"), &["Was", "Now"]);
        let mut lines: Vec<(String, String)> =
            placer.global.iter().map(|(was, now)| (names.pokemon(*was), names.pokemon(*now))).collect();
        lines.sort();
        for (was, now) in lines {
            t.row(vec![was, now]);
        }
    }
    Ok(())
}

fn tweak_notes(rules: &Rules) -> Vec<String> {
    let mut out = Vec::new();
    if rules.level != 0 {
        out.push(format!("levels changed by {}%.", rules.level));
    }
    if rules.items.is_some() {
        out.push("wild Pokémon can hold random items.".to_owned());
    }
    if rules.all_day {
        out.push("every Pokémon appears at any time of day.".to_owned());
    }
    if rules.both_versions {
        out.push("version exclusives appear in both games.".to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_uses_every_template_and_species() {
        let mut rng = Rng::new(3);
        let species = |n: u16| (1..=n).map(|s| Pokemon { species: s, form: 0 }).collect::<Vec<_>>();
        for (templates, pool) in [(10, 4), (4, 10), (5, 5)] {
            let pairs = pair_up(&mut rng, (0..templates).collect(), species(pool), |_, _| (false, 0));
            assert_eq!(pairs.len(), templates.max(usize::from(pool)));
            let used: BTreeSet<usize> = pairs.iter().map(|p| p.0).collect();
            let placed: BTreeSet<u16> = pairs.iter().map(|p| p.1.species).collect();
            assert_eq!((used.len(), placed.len()), (templates, usize::from(pool)));
        }
    }

    #[test]
    fn pairing_prefers_cheap_pairs() {
        let mut rng = Rng::new(9);
        let pool = vec![Pokemon { species: 1, form: 0 }, Pokemon { species: 2, form: 0 }];
        let pairs = pair_up(&mut rng, vec![0, 1], pool, |t, p| (t as u16 + 1 != p.species, 0));
        assert!(pairs.iter().all(|&(t, p)| t as u16 + 1 == p.species), "{pairs:?}");
    }
}
