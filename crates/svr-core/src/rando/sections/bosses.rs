//! Titans, legendaries and scripted battles.
//!
//! Every switch randomizes one group of battles from `boss_groups`. Within a group, battles that
//! share an original species share its replacement, and every scene file of the group swaps that
//! species to match, so the cutscene model and the battle always agree.

#[path = "boss_groups.rs"]
mod groups;

use std::collections::{BTreeMap, BTreeSet};

use crate::data::facts::{self, Tera};
use crate::data::scene;
use crate::data::store::{load_table, PERSONAL};
use crate::data::Pokemon;
use crate::fb::{FieldError, TableValue};
use crate::rando::itempool::{self, ItemCatalog};
use crate::rando::movepool::{self, MoveTable};
use crate::rando::options::{Kind, Opt, OptionDef, Page, Source, Status};
use crate::rando::{Ctx, RandoError, Rng, Section};

use groups::{Group, BATTLES, GROUPS};

pub const SECTION: Section = Section {
    id: "bosses",
    options,
    run,
};

const PAGE: Page = Page::Bosses;
const POOL: &str = "bosses.pool";
const LEVEL: &str = "bosses.level_percent";
const SAFE: &str = "bosses.no_softlock";
const RANDOM_MOVES: &str = "bosses.random_moves";
const RANDOM_ITEMS: &str = "bosses.random_items";
const RANDOM_TERA: &str = "bosses.random_tera";
const LOG: &str = "Titans & legendaries";

const WONDER_GUARD: u16 = 25;
/// Wimp Out and Emergency Exit make a wild Pokémon leave the battle.
const ESCAPE_ABILITIES: [u16; 2] = [193, 194];
/// Whirlwind, Roar, Teleport, Circle Throw and Dragon Tail end a battle against a wild Pokémon.
/// Self-Destruct, Explosion, Perish Song, Memento, Healing Wish, Lunar Dance, Final Gambit and
/// Misty Explosion knock out the user. Either loses a fight the player must win or a Pokémon they
/// came to catch.
const RISKY_MOVES: [u16; 13] = [18, 46, 100, 509, 525, 120, 153, 195, 262, 361, 461, 515, 802];
/// Red Card, Eject Button and Eject Pack end a battle against a wild Pokémon the way Roar does.
const RISKY_ITEMS: [u16; 3] = [542, 547, 1119];

/// GemType values: a type's Tera is its type id plus 2, Stellar is 101.
const GEM_NORMAL: i64 = 2;
const GEM_FAIRY: i64 = 19;
const GEM_STELLAR: i64 = 101;

const WAZA_MANUAL: i64 = 1;
const SEX_DEFAULT: i64 = 0;
/// TokuseiType SET_1..SET_3 pick ability slot 1, 2 or the hidden one.
const TOKUSEI_SET_1: i64 = 2;

fn options() -> Vec<OptionDef> {
    let mut out = vec![Opt::new(POOL, PAGE, "Species", "Species for scripted battles", Kind::Pool)
        .help("Which species may replace Titans, legendaries and other scripted battles. Each group below can narrow it further.")
        .sources(&[Source::Sv])
        .build()];
    for g in GROUPS {
        out.push(
            Opt::toggle(g.switch, PAGE, "Species", g.label, false)
                .help(g.help)
                .sources(&[Source::Sv])
                .status(Status::Test)
                .build(),
        );
        out.push(
            Opt::new(g.pool, PAGE, "Species", g.pool_label, Kind::Pool)
                .help("Species for this group only. Leave it on \"use the scripted battle pool\" to share the pool above.")
                .on(g.switch)
                .sources(&[Source::Sv])
                .build(),
        );
    }
    out.extend([
        Opt::number(LEVEL, PAGE, "Battle details", "Level change", 0, -50, 100, "%")
            .step(5)
            .help("Raises or lowers the level of every scripted battle Pokémon except Koraidon and Miraidon. 0 keeps the game's levels.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(RANDOM_MOVES, PAGE, "Battle details", "Random moves", false)
            .help("Gives every scripted battle Pokémon except Koraidon and Miraidon four random moves, at least one of them an attack. Off keeps their moves, and a Pokémon whose species changed uses the level-up moves it knows at its level.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(RANDOM_ITEMS, PAGE, "Battle details", "Random held items", false)
            .help("Gives every scripted battle Pokémon except Koraidon and Miraidon a random held item that suits it in battle, such as a berry, Leftovers or a booster for its own type. A form that needs an item, like an Arceus plate, always holds it.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(RANDOM_TERA, PAGE, "Battle details", "Random Tera types", false)
            .help("Gives every scripted battle Pokémon except Koraidon and Miraidon a random Tera type. Stellar fights stay Stellar, and Ogerpon and Terapagos keep the only Tera types they can use.")
            .sources(&[Source::New])
            .build(),
        Opt::toggle(SAFE, PAGE, "Safety", "Keep must-win fights winnable", true)
            .help("Fights you have to win, including both Terapagos fights, never use Wonder Guard, and no scripted Pokémon uses Wimp Out or Emergency Exit, even with random abilities. No scripted Pokémon gets a move or held item that ends the battle or knocks itself out, like Roar, Teleport, Explosion or Red Card, even at a changed level.")
            .sources(&[Source::Sv])
            .build(),
    ]);
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Moves {
    /// The game picks the level-up moves.
    Default,
    Manual([u16; 4]),
}

/// The parts of a battle Pokémon this section reads and writes.
#[derive(Clone, Debug, PartialEq)]
struct Mon {
    species: Pokemon,
    level: u16,
    moves: Moves,
    item: u16,
    gem: i64,
    tokusei: i64,
}

struct Battle {
    label: String,
    group: Option<&'static Group>,
    /// Not a catch encounter, so the player has to knock it out to go on.
    must_win: bool,
    vanilla: Mon,
}

impl Battle {
    fn banned_abilities(&self, safe: bool) -> &'static [u16] {
        match (safe, self.must_win) {
            (false, _) => &[],
            (true, true) => &[WONDER_GUARD, ESCAPE_ABILITIES[0], ESCAPE_ABILITIES[1]],
            (true, false) => &ESCAPE_ABILITIES,
        }
    }
}

fn risky_moves(safe: bool) -> &'static [u16] {
    if safe {
        &RISKY_MOVES
    } else {
        &[]
    }
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "bosses",
        detail: e.to_string(),
    }
}

fn read_battles(ctx: &mut Ctx) -> Result<Vec<Battle>, RandoError> {
    let table = ctx.store.get(BATTLES)?;
    let mut out = Vec::new();
    for row in table.tables("values") {
        let label = row.string("label").unwrap_or_default().to_owned();
        let p = row.table("pokeData").ok_or_else(|| RandoError::Section {
            section: "bosses",
            detail: format!("battle {label} has no pokeData"),
        })?;
        let int = |field: &str| p.int(field).map_err(shape);
        let moves = if int("wazaType")? == WAZA_MANUAL {
            let mut ids = [0u16; 4];
            for (k, id) in ids.iter_mut().enumerate() {
                *id = p.table(&format!("waza{}", k + 1)).map_or(Ok(0), |w| w.int("wazaId").map_err(shape))? as u16;
            }
            Moves::Manual(ids)
        } else {
            Moves::Default
        };
        out.push(Battle {
            group: groups::group_of(&label),
            must_win: !row.bool("eventEncount").map_err(shape)?,
            vanilla: Mon {
                species: Pokemon {
                    species: int("devId")? as u16,
                    form: int("formId")? as u16,
                },
                level: int("level")? as u16,
                moves,
                item: int("item")? as u16,
                gem: int("gemType")?,
                tokusei: int("tokusei")?,
            },
            label,
        });
    }
    Ok(out)
}

/// Current abilities of every form, after any ability randomization.
fn ability_table(ctx: &mut Ctx) -> Result<BTreeMap<Pokemon, [u16; 3]>, RandoError> {
    let personal = ctx.store.get(PERSONAL)?;
    let mut out = BTreeMap::new();
    for entry in personal.tables("entry") {
        let Some(id) = entry.struct_value("species") else { continue };
        let p = Pokemon {
            species: id.int("species").map_err(shape)? as u16,
            form: id.int("form").map_err(shape)? as u16,
        };
        let ability = |field: &str| entry.int(field).map(|a| a as u16).map_err(shape);
        out.insert(p, [ability("ability_1")?, ability("ability_2")?, ability("ability_hidden")?]);
    }
    Ok(out)
}

/// The ability slots a TokuseiType may use.
fn slots(tokusei: i64) -> &'static [usize] {
    match tokusei {
        0 => &[0, 1],
        2 => &[0],
        3 => &[1],
        4 => &[2],
        _ => &[0, 1, 2],
    }
}

/// `tokusei` if every slot it may use is allowed, else a fixed slot that is, else None.
fn safe_tokusei(abilities: [u16; 3], tokusei: i64, banned: &[u16]) -> Option<i64> {
    let ok = |slot: usize| !banned.contains(&abilities[slot]);
    if slots(tokusei).iter().all(|&s| ok(s)) {
        return Some(tokusei);
    }
    (0..3).find(|&s| ok(s)).map(|s| TOKUSEI_SET_1 + s as i64)
}

/// The last four level-up moves of personal row `row` at `level`, as the game picks them, but with
/// the banned moves taken out of the learnset first so a banned move never costs a slot.
fn last_four(personal: &TableValue, row: usize, level: u16, banned: &[u16]) -> Result<Vec<u16>, RandoError> {
    let Some(entry) = personal.tables("entry").nth(row) else {
        return Ok(Vec::new());
    };
    let mut learned: Vec<u16> = Vec::new();
    for m in entry.vector("levelup_moves").unwrap_or(&[]) {
        let Some(m) = m.as_struct() else { continue };
        let id = m.int("move").map_err(shape)? as u16;
        if m.int("level").map_err(shape)? <= i64::from(level) && id != 0 && !banned.contains(&id) && !learned.contains(&id) {
            learned.push(id);
        }
    }
    let start = learned.len().saturating_sub(4);
    Ok(learned[start..].to_vec())
}

fn legal_moves(ctx: &mut Ctx, p: Pokemon, level: u16, banned: &[u16]) -> Result<Vec<u16>, RandoError> {
    let Some(row) = ctx.dex.get(p).map(|f| f.row) else {
        return Ok(Vec::new());
    };
    last_four(ctx.store.get(PERSONAL)?, row, level, banned)
}

fn manual(moves: &[u16]) -> Moves {
    let mut ids = [0u16; 4];
    for (slot, &m) in ids.iter_mut().zip(moves) {
        *slot = m;
    }
    Moves::Manual(ids)
}

fn locked_gem(ctx: &Ctx, p: Pokemon) -> Option<i64> {
    facts::required_tera(ctx.dex.get(p)?.national, p.form).map(|tera| match tera {
        Tera::Type(t) => GEM_NORMAL + i64::from(t),
        Tera::Stellar => GEM_STELLAR,
    })
}

fn form_item(ctx: &Ctx, p: Pokemon) -> Option<u16> {
    ctx.dex.get(p).and_then(|f| itempool::form_item(f.national, p.form))
}

/// The item a battle holds after its species changes: the new form's own item if it needs one,
/// else nothing in place of the old form's item, else what it held before.
fn item_after_change(ctx: &Ctx, old: &Mon, new: Pokemon) -> u16 {
    match form_item(ctx, new) {
        Some(item) => item,
        None if form_item(ctx, old.species) == Some(old.item) => 0,
        None => old.item,
    }
}

fn scaled_level(level: u16, percent: i64) -> u16 {
    let scaled = (i64::from(level) * (100 + percent) + 50) / 100;
    scaled.clamp(1, 100) as u16
}

/// The group's own pool override, else the scripted battle pool, else the global pool.
fn group_pool(ctx: &Ctx, g: &Group) -> Result<Vec<Pokemon>, RandoError> {
    if ctx.settings.pool(g.pool).inherit {
        ctx.pool(POOL, "scripted battles")
    } else {
        ctx.pool(g.pool, g.label)
    }
}

struct SceneFile {
    path: String,
    bytes: Vec<u8>,
    found: BTreeMap<Pokemon, usize>,
}

/// Starts from another section's patch of the same file when there is one.
fn load_scenes(ctx: &Ctx, g: &Group) -> Result<Vec<SceneFile>, RandoError> {
    let mut out = Vec::new();
    for path in g.scene_files() {
        let bytes = match ctx.files.get(&path) {
            Some(bytes) => bytes.clone(),
            None => ctx.game.read(&path).map_err(|e| RandoError::Section {
                section: "bosses",
                detail: format!("{} scene {path}: {e}", g.label),
            })?,
        };
        let found = scene::swap_species(&bytes, &BTreeMap::new())?.found;
        out.push(SceneFile { path, bytes, found });
    }
    Ok(out)
}

/// Battles of one group that share an original species, in table order.
fn fights(battles: &[Battle], g: &Group) -> Vec<(u16, Vec<usize>)> {
    let mut out: Vec<(u16, Vec<usize>)> = Vec::new();
    for (i, b) in battles.iter().enumerate() {
        if b.group.is_none_or(|bg| bg.key != g.key) {
            continue;
        }
        let species = b.vanilla.species.species;
        match out.iter_mut().find(|(s, _)| *s == species) {
            Some((_, rows)) => rows.push(i),
            None => out.push((species, vec![i])),
        }
    }
    out
}

struct Choice<'a> {
    battles: &'a [Battle],
    targets: &'a [Mon],
    rows: &'a [usize],
    abilities: &'a BTreeMap<Pokemon, [u16; 3]>,
    safe: bool,
    random_moves: bool,
}

impl Choice<'_> {
    /// A candidate fits when every battle of the fight can avoid banned abilities and still
    /// knows a legal move.
    fn fits(&self, ctx: &mut Ctx, p: Pokemon) -> Result<bool, RandoError> {
        let Some(&abilities) = self.abilities.get(&p) else {
            return Ok(false);
        };
        for &i in self.rows {
            let battle = &self.battles[i];
            if safe_tokusei(abilities, battle.vanilla.tokusei, battle.banned_abilities(self.safe)).is_none() {
                return Ok(false);
            }
            if !self.random_moves && legal_moves(ctx, p, self.targets[i].level, risky_moves(self.safe))?.is_empty() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// A random pool member that fits, preferring species the group has not used yet.
fn pick(
    ctx: &mut Ctx,
    rng: &mut Rng,
    pool: &[Pokemon],
    used: &BTreeSet<Pokemon>,
    choice: &Choice,
) -> Result<Option<Pokemon>, RandoError> {
    let mut order = pool.to_vec();
    rng.shuffle(&mut order);
    for allow_repeat in [false, true] {
        for &p in &order {
            if !allow_repeat && used.contains(&p) {
                continue;
            }
            if choice.fits(ctx, p)? {
                return Ok(Some(p));
            }
        }
    }
    Ok(None)
}

struct GroupResult {
    group: &'static Group,
    replaced: BTreeMap<u16, Pokemon>,
    scenes: Vec<SceneFile>,
}

fn randomize_group(
    ctx: &mut Ctx,
    g: &'static Group,
    battles: &[Battle],
    targets: &mut [Mon],
    abilities: &BTreeMap<Pokemon, [u16; 3]>,
) -> Result<GroupResult, RandoError> {
    let safe = ctx.settings.on(SAFE);
    let random_moves = ctx.settings.on(RANDOM_MOVES);
    let pool = group_pool(ctx, g)?;
    let scenes = load_scenes(ctx, g)?;
    let mut rng = ctx.rng(&format!("bosses.{}", g.key));
    let mut used = BTreeSet::new();
    let mut replaced = BTreeMap::new();
    for (species, rows) in fights(battles, g) {
        let choice = Choice {
            battles,
            targets,
            rows: &rows,
            abilities,
            safe,
            random_moves,
        };
        let new = pick(ctx, &mut rng, &pool, &used, &choice)?.ok_or_else(|| RandoError::Section {
            section: "bosses",
            detail: format!(
                "no species in the {} pool can replace {} without risking a softlock. Widen the pool.",
                g.label,
                ctx.names.species(species)
            ),
        })?;
        used.insert(new);
        let changed: Vec<usize> = rows.iter().copied().filter(|&i| battles[i].vanilla.species != new).collect();
        if changed.is_empty() {
            continue;
        }
        replaced.insert(species, new);
        for i in changed {
            let moves = legal_moves(ctx, new, targets[i].level, risky_moves(safe))?;
            let item = item_after_change(ctx, &battles[i].vanilla, new);
            let target = &mut targets[i];
            target.species = new;
            target.moves = manual(&moves);
            target.item = item;
        }
    }
    Ok(GroupResult { group: g, replaced, scenes })
}

fn random_moves(ctx: &mut Ctx, battles: &[Battle], targets: &mut [Mon], safe: bool) -> Result<(), RandoError> {
    let table = MoveTable::load(ctx)?;
    let breaking = movepool::ids_named(ctx, table.moves.len(), movepool::GAME_BREAKING);
    let allowed: Vec<u16> = table
        .assignable()
        .iter()
        .copied()
        .filter(|m| !risky_moves(safe).contains(m) && !breaking.contains(m))
        .collect();
    let attacks: Vec<u16> = allowed
        .iter()
        .copied()
        .filter(|&m| table.get(m).is_some_and(|info| info.damaging() && info.power > 1))
        .collect();
    let mut rng = ctx.rng("bosses.moves");
    for (battle, target) in battles.iter().zip(targets.iter_mut()) {
        if battle.group.is_none() {
            continue;
        }
        let mut chosen = vec![*rng.pick(&attacks).expect("the move table has attacks")];
        while chosen.len() < 4 {
            let m = *rng.pick(&allowed).expect("the move table has moves");
            if !chosen.contains(&m) {
                chosen.push(m);
            }
        }
        rng.shuffle(&mut chosen);
        target.moves = manual(&chosen);
    }
    Ok(())
}

/// A battle left on game-picked moves can pick a risky move once its level or learnset changed, so
/// it gets safe level-up moves written out instead. Risky moves the game itself picks stay.
fn guard_default_moves(ctx: &mut Ctx, battles: &[Battle], targets: &mut [Mon]) -> Result<(), RandoError> {
    let vanilla_personal = load_table(ctx.game, PERSONAL)?;
    for (battle, target) in battles.iter().zip(targets.iter_mut()) {
        if battle.group.is_none() || target.moves != Moves::Default {
            continue;
        }
        let Some(row) = ctx.dex.get(target.species).map(|f| f.row) else { continue };
        let before = last_four(&vanilla_personal, row, battle.vanilla.level, &[])?;
        let picked = movepool::level_up_moves_at(ctx, target.species, target.level)?;
        if picked.iter().any(|m| RISKY_MOVES.contains(m) && !before.contains(m)) {
            target.moves = manual(&legal_moves(ctx, target.species, target.level, &RISKY_MOVES)?);
        }
    }
    Ok(())
}

fn random_items(ctx: &mut Ctx, battles: &[Battle], targets: &mut [Mon], safe: bool) -> Result<(), RandoError> {
    let catalog = ItemCatalog::load(ctx)?;
    let held: Vec<u16> = catalog.held(true).into_iter().filter(|i| !safe || !RISKY_ITEMS.contains(i)).collect();
    let mut rng = ctx.rng("bosses.items");
    for (battle, target) in battles.iter().zip(targets.iter_mut()) {
        if battle.group.is_none() {
            continue;
        }
        let roll = rng.next_u64();
        if form_item(ctx, target.species).is_some() {
            continue;
        }
        let Some(info) = ctx.dex.get(target.species) else { continue };
        let suited = itempool::sensible_for(&catalog, &held, info.types, ctx.dex.is_fully_evolved(target.species));
        let from = if suited.is_empty() { &held } else { &suited };
        target.item = from[(roll % from.len() as u64) as usize];
    }
    Ok(())
}

/// Tera types, and an ability slot that keeps every fight winnable.
fn finish(ctx: &Ctx, battles: &[Battle], targets: &mut [Mon], abilities: &BTreeMap<Pokemon, [u16; 3]>) -> Result<(), RandoError> {
    let safe = ctx.settings.on(SAFE);
    let random_tera = ctx.settings.on(RANDOM_TERA);
    let mut tera_rng = ctx.rng("bosses.tera");
    for (battle, target) in battles.iter().zip(targets.iter_mut()) {
        if battle.group.is_none() {
            continue;
        }
        let rolled = random_tera.then(|| tera_rng.range(GEM_NORMAL, GEM_FAIRY));
        let species_changed = target.species != battle.vanilla.species;
        match (locked_gem(ctx, target.species), rolled) {
            (Some(gem), _) if species_changed => target.gem = gem,
            (Some(_), _) => {}
            (None, Some(gem)) if target.gem != GEM_STELLAR => target.gem = gem,
            (None, _) => {}
        }
        let Some(&slots) = abilities.get(&target.species) else { continue };
        target.tokusei = safe_tokusei(slots, target.tokusei, battle.banned_abilities(safe)).ok_or_else(|| {
            RandoError::Section {
                section: "bosses",
                detail: format!(
                    "{} in {} can only have an ability that makes the fight unwinnable or lets it flee. Turn on its group, change the ability settings or use another seed.",
                    ctx.names.pokemon(target.species),
                    battle.label
                ),
            }
        })?;
    }
    Ok(())
}

struct SceneSwap {
    group: &'static str,
    path: String,
    map: BTreeMap<Pokemon, Pokemon>,
}

fn patch_scenes(ctx: &mut Ctx, result: &GroupResult) -> Result<Vec<SceneSwap>, RandoError> {
    let mut out = Vec::new();
    for file in &result.scenes {
        let map: BTreeMap<Pokemon, Pokemon> = file
            .found
            .keys()
            .filter_map(|&k| result.replaced.get(&k.species).map(|&new| (k, new)))
            .filter(|(old, new)| old != new)
            .collect();
        if map.is_empty() {
            continue;
        }
        let patch = scene::swap_species(&file.bytes, &map)?;
        ctx.files.insert(file.path.clone(), patch.bytes);
        out.push(SceneSwap {
            group: result.group.label,
            path: file.path.clone(),
            map,
        });
    }
    Ok(out)
}

fn update_release(ctx: &mut Ctx, path: &str, replaced: &BTreeMap<u16, Pokemon>) -> Result<Vec<(u16, u16)>, RandoError> {
    let mut swaps = Vec::new();
    for (i, row) in ctx.store.get(path)?.tables("values").enumerate() {
        let old = row.int("ReleasePokeId").map_err(shape)? as u16;
        if let Some(new) = replaced.get(&old) {
            swaps.push((i, old, new.species));
        }
    }
    if swaps.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows: Vec<_> = ctx.store.get_mut(path)?.tables_mut("values").collect();
    for &(i, _, new) in &swaps {
        rows[i].set_int("ReleasePokeId", i64::from(new)).map_err(shape)?;
    }
    Ok(swaps.into_iter().map(|(_, old, new)| (old, new)).collect())
}

fn write_battles(ctx: &mut Ctx, battles: &[Battle], targets: &[Mon]) -> Result<(), RandoError> {
    if battles.iter().zip(targets).all(|(b, t)| b.vanilla == *t) {
        return Ok(());
    }
    let table = ctx.store.get_mut(BATTLES)?;
    for ((row, battle), new) in table.tables_mut("values").zip(battles).zip(targets) {
        let old = &battle.vanilla;
        if old == new {
            continue;
        }
        let p = row.table_mut("pokeData").map_err(shape)?;
        if new.species != old.species {
            p.set_int("devId", i64::from(new.species.species)).map_err(shape)?;
            p.set_int("formId", i64::from(new.species.form)).map_err(shape)?;
            p.set_int("sex", SEX_DEFAULT).map_err(shape)?;
        }
        if let (Moves::Manual(ids), true) = (&new.moves, new.moves != old.moves) {
            p.set_int("wazaType", WAZA_MANUAL).map_err(shape)?;
            for (k, id) in ids.iter().enumerate() {
                p.table_mut(&format!("waza{}", k + 1))
                    .map_err(shape)?
                    .set_int("wazaId", i64::from(*id))
                    .map_err(shape)?;
            }
        }
        let scalars = [
            ("level", i64::from(old.level), i64::from(new.level)),
            ("item", i64::from(old.item), i64::from(new.item)),
            ("gemType", old.gem, new.gem),
            ("tokusei", old.tokusei, new.tokusei),
        ];
        for (field, before, after) in scalars {
            if before != after {
                p.set_int(field, after).map_err(shape)?;
            }
        }
    }
    Ok(())
}

fn run(ctx: &mut Ctx) -> Result<(), RandoError> {
    let battles = read_battles(ctx)?;
    let abilities = ability_table(ctx)?;
    let safe = ctx.settings.on(SAFE);
    let percent = ctx.settings.number(LEVEL);
    let mut targets: Vec<Mon> = battles.iter().map(|b| b.vanilla.clone()).collect();

    for (battle, target) in battles.iter().zip(targets.iter_mut()) {
        if battle.group.is_some() && percent != 0 {
            target.level = scaled_level(target.level, percent);
        }
    }

    let enabled: Vec<&'static Group> = GROUPS.iter().filter(|g| ctx.settings.on(g.switch)).collect();
    let mut results = Vec::new();
    for g in enabled {
        results.push(randomize_group(ctx, g, &battles, &mut targets, &abilities)?);
    }
    if ctx.settings.on(RANDOM_MOVES) {
        random_moves(ctx, &battles, &mut targets, safe)?;
    }
    if safe {
        guard_default_moves(ctx, &battles, &mut targets)?;
    }
    if ctx.settings.on(RANDOM_ITEMS) {
        random_items(ctx, &battles, &mut targets, safe)?;
    }
    finish(ctx, &battles, &mut targets, &abilities)?;

    write_battles(ctx, &battles, &targets)?;
    let mut scene_log = Vec::new();
    let mut release_log = Vec::new();
    for result in &results {
        scene_log.extend(patch_scenes(ctx, result)?);
        if let Some(path) = result.group.release {
            release_log.extend(update_release(ctx, path, &result.replaced)?);
        }
    }
    log(ctx, &battles, &targets, &scene_log, &release_log);
    Ok(())
}

fn moves_text(ctx: &Ctx, moves: &Moves) -> String {
    match moves {
        Moves::Default => "level-up".into(),
        Moves::Manual(ids) => ids
            .iter()
            .filter(|&&m| m != 0)
            .map(|&m| ctx.names.move_name(m))
            .collect::<Vec<_>>()
            .join(" / "),
    }
}

fn gem_text(ctx: &Ctx, gem: i64) -> String {
    match gem {
        GEM_NORMAL..=GEM_FAIRY => ctx.names.type_name((gem - GEM_NORMAL) as u8),
        GEM_STELLAR => "Stellar".into(),
        _ => "Default".into(),
    }
}

fn change<T: PartialEq>(old: &T, new: &T, text: impl Fn(&T) -> String) -> String {
    if old == new {
        text(new)
    } else {
        format!("{} -> {}", text(old), text(new))
    }
}

fn log(ctx: &mut Ctx, battles: &[Battle], targets: &[Mon], scenes: &[SceneSwap], release: &[(u16, u16)]) {
    let mut rows = Vec::new();
    for (battle, new) in battles.iter().zip(targets) {
        let old = &battle.vanilla;
        if old == new {
            continue;
        }
        rows.push(vec![
            battle.group.map_or("", |g| g.label).to_owned(),
            battle.label.clone(),
            change(&old.species, &new.species, |p| ctx.names.pokemon(*p)),
            change(&old.level, &new.level, |l| l.to_string()),
            change(&old.moves, &new.moves, |m| moves_text(ctx, m)),
            change(&old.item, &new.item, |&i| if i == 0 { "none".into() } else { ctx.names.item(i) }),
            change(&old.gem, &new.gem, |&g| gem_text(ctx, g)),
        ]);
    }
    let scene_rows: Vec<Vec<String>> = scenes
        .iter()
        .map(|swap| {
            let swaps: Vec<String> = swap
                .map
                .iter()
                .map(|(old, new)| format!("{} -> {}", ctx.names.pokemon(*old), ctx.names.pokemon(*new)))
                .collect();
            let kind = if groups::is_demo(&swap.path) { "pre-animated demo" } else { "scene" };
            vec![swap.group.to_owned(), swap.path.clone(), kind.to_owned(), swaps.join(", ")]
        })
        .collect();
    let release_rows: Vec<Vec<String>> = release
        .iter()
        .map(|&(old, new)| vec![ctx.names.species(old), ctx.names.species(new)])
        .collect();
    if rows.is_empty() && scene_rows.is_empty() {
        return;
    }
    let section = ctx.log.section(LOG);
    let table = section.table("Scripted battles", &["Group", "Battle", "Pokémon", "Level", "Moves", "Item", "Tera"]);
    for row in rows {
        table.row(row);
    }
    if !scene_rows.is_empty() {
        let table = section.table("Cutscene models", &["Group", "File", "Kind", "Shows"]);
        for row in scene_rows {
            table.row(row);
        }
    }
    if !release_rows.is_empty() {
        let table = section.table("Snacksworth's list", &["Was", "Now"]);
        for row in release_rows {
            table.row(row);
        }
    }
}
