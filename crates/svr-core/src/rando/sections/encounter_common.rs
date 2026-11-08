//! What the starter, gift, wild and static sections share: editing a game Pokémon record when its
//! species changes, Tera types, shiny rolls, where the game lets each species spawn, cutscene
//! patches and species choice with soft preferences.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::store::load_table;
use crate::data::facts::{self, Tera};
use crate::data::{scene, DataError, Dex, FormInfo, Names, Pokemon};
use crate::fb::{FieldError, TableValue};
use crate::rando::options::SpeciesPick;
use crate::rando::pick::Picker;
use crate::rando::{itempool, pool, Ctx, RandoError, Rng};

pub const GIFTS: &str = "world/data/event/event_add_pokemon/eventAddPokemon/eventAddPokemon_array.bin";
pub const PALDEA_WILD: &str = "world/data/encount/pokedata/pokedata/pokedata_array.bin";
pub const KITAKAMI_WILD: &str = "world/data/encount/pokedata/pokedata_su1/pokedata_su1_array.bin";
pub const BLUEBERRY_WILD: &str = "world/data/encount/pokedata/pokedata_su2/pokedata_su2_array.bin";

pub const TYPE_COUNT: u8 = 18;
/// GemType lists the 18 types in type-id order, starting at NORMAL = 2.
const FIRST_TYPE_GEM: i64 = 2;
const STELLAR_GEM: i64 = 101;

/// PersonalGender groups. Mixed-gender species are group 0.
const MALE_ONLY: u8 = 1;
const FEMALE_ONLY: u8 = 2;

pub fn shape(e: FieldError) -> RandoError {
    RandoError::Data(DataError::Shape(e.to_string()))
}

/// The value of a game enum constant, looked up in the schema of the table that uses it.
pub fn enum_value(table: &TableValue, enum_name: &str, constant: &str) -> Result<i64, RandoError> {
    table.schema().enum_value(enum_name, constant).ok_or_else(|| {
        RandoError::Data(DataError::Shape(format!("{} has no {enum_name}.{constant}", table.def().name)))
    })
}

/// Writes only when the value differs, so untouched fields keep their original encoding.
pub fn put(table: &mut TableValue, field: &str, value: i64) -> Result<(), RandoError> {
    if table.int(field).map_err(shape)? != value {
        table.set_int(field, value).map_err(shape)?;
    }
    Ok(())
}

/// The species of a gift, trade or static Pokémon record.
pub fn species_of(poke: &TableValue) -> Result<Pokemon, RandoError> {
    let species = poke.int("devId").map_err(shape)?;
    let form = poke.int("formId").map_err(shape)?;
    Ok(Pokemon {
        species: species as u16,
        form: form as u16,
    })
}

/// Puts `p` into a gift, trade or static Pokémon record. A fixed gender or move set belonged to
/// the old species, so the game rolls the gender again and picks level-up moves.
pub fn set_species(poke: &mut TableValue, p: Pokemon) -> Result<(), RandoError> {
    put(poke, "devId", i64::from(p.species))?;
    put(poke, "formId", i64::from(p.form))?;
    put(poke, "sex", enum_value(poke, "SexType", "DEFAULT")?)?;
    let default_moves = enum_value(poke, "WazaType", "DEFAULT")?;
    if poke.int("wazaType").map_err(shape)? != default_moves {
        poke.set_int("wazaType", default_moves).map_err(shape)?;
        for slot in ["waza1", "waza2", "waza3", "waza4"] {
            put(poke.table_mut(slot).map_err(shape)?, "wazaId", 0)?;
        }
    }
    Ok(())
}

/// The held item `p` needs to keep its form, such as a plate for an Arceus form.
pub fn form_item(dex: &Dex, p: Pokemon) -> Option<u16> {
    dex.get(p).and_then(|f| itempool::form_item(f.national, p.form))
}

/// Gives a starter, gift or trade the item its form needs, over any other item.
pub fn give_form_item(poke: &mut TableValue, dex: &Dex, p: Pokemon) -> Result<(), RandoError> {
    match form_item(dex, p) {
        Some(item) => put(poke, "item", i64::from(item)),
        None => Ok(()),
    }
}

/// For slots that cannot hold an item: drops forms that need one to exist.
pub fn without_form_items(dex: &Dex, mut pool: Vec<Pokemon>, what: &str) -> Result<Vec<Pokemon>, RandoError> {
    pool.retain(|&p| form_item(dex, p).is_none());
    if pool.is_empty() {
        return Err(RandoError::EmptyPool { what: what.to_owned() });
    }
    Ok(pool)
}

/// Whether a species can be `wanted` ("male" or "female").
pub fn can_be(info: &FormInfo, wanted: &str) -> bool {
    match wanted {
        "male" => info.gender_group == 0 || info.gender_group == MALE_ONLY,
        "female" => info.gender_group == 0 || info.gender_group == FEMALE_ONLY,
        _ => false,
    }
}

fn required_gem(info: &FormInfo) -> Option<i64> {
    facts::required_tera(info.national, info.id.form).map(|tera| match tera {
        Tera::Type(t) => FIRST_TYPE_GEM + i64::from(t),
        Tera::Stellar => STELLAR_GEM,
    })
}

/// Sets the Tera type of `poke`, which holds `p`: the species' required type if it has one, else a
/// random type when `random` is given, else the current value stays. A random type is drawn even
/// when unused, so the stream does not depend on which species were placed.
pub fn set_tera(poke: &mut TableValue, dex: &Dex, p: Pokemon, random: Option<&mut Rng>) -> Result<(), RandoError> {
    let rolled = random.map(random_gem);
    let gem = match dex.get(p).and_then(required_gem) {
        Some(gem) => gem,
        None => match rolled {
            Some(gem) => gem,
            None => return Ok(()),
        },
    };
    put(poke, "gemType", gem)
}

pub fn random_gem(rng: &mut Rng) -> i64 {
    FIRST_TYPE_GEM + rng.below(u64::from(TYPE_COUNT)) as i64
}

pub fn is_stellar(poke: &TableValue) -> Result<bool, RandoError> {
    Ok(poke.int("gemType").map_err(shape)? == STELLAR_GEM)
}

pub fn gem_label(names: &Names, gem: i64) -> String {
    match gem {
        STELLAR_GEM => "Stellar".into(),
        g if (FIRST_TYPE_GEM..FIRST_TYPE_GEM + i64::from(TYPE_COUNT)).contains(&g) => {
            names.type_name((g - FIRST_TYPE_GEM) as u8)
        }
        _ => "Default".into(),
    }
}

pub fn make_shiny(poke: &mut TableValue) -> Result<(), RandoError> {
    put(poke, "rareType", enum_value(poke, "RareType", "RARE")?)
}

pub fn is_shiny(poke: &TableValue) -> Result<bool, RandoError> {
    Ok(poke.int("rareType").map_err(shape)? == enum_value(poke, "RareType", "RARE")?)
}

/// Makes `poke` shiny with `percent` chance. A Pokémon that is already shiny stays shiny.
pub fn roll_shiny(poke: &mut TableValue, rng: &mut Rng, percent: i64) -> Result<(), RandoError> {
    if percent > 0 && rng.percent(percent as u32) {
        make_shiny(poke)?;
    }
    Ok(())
}

/// `level` changed by `percent`, rounded, within 1 to 100.
pub fn scaled_level(level: i64, percent: i64) -> i64 {
    ((level * (100 + percent) + 50) / 100).clamp(1, 100)
}

pub fn yes(flag: bool) -> String {
    if flag { "yes" } else { "" }.into()
}

pub fn item_label(names: &Names, item: i64) -> String {
    if item == 0 {
        String::new()
    } else {
        names.item(item as u16)
    }
}

/// A preference on candidate species, from most to least important.
pub type Pref<'a> = &'a dyn Fn(&FormInfo) -> bool;

/// Picks from the pool, keeping as many preferences as some candidate satisfies. The last
/// preference is dropped first. With `target_bst`, the pick is the strength band nearest it.
pub fn choose(picker: &Picker, rng: &mut Rng, target_bst: Option<u32>, prefs: &[Pref]) -> Pokemon {
    for kept in (1..=prefs.len()).rev() {
        let keep = |f: &FormInfo| prefs[..kept].iter().all(|pref| pref(f));
        let found = match target_bst {
            Some(target) => picker.similar_strength(rng, target, keep),
            None => picker.matching(rng, keep),
        };
        if let Some(p) = found {
            return p;
        }
    }
    match target_bst {
        Some(target) => picker.similar_strength(rng, target, |_| true).unwrap_or_else(|| picker.any(rng)),
        None => picker.any(rng),
    }
}

/// Keeps picks from repeating a species already given out.
pub fn other_species(chosen: &[Pokemon]) -> impl Fn(&FormInfo) -> bool + '_ {
    move |f| chosen.iter().all(|c| c.species != f.id.species)
}

/// The section pool with the section's own form and legendary switches applied on top.
pub fn section_pool(
    ctx: &Ctx,
    pool_id: &str,
    what: &str,
    alt_forms: bool,
    legendaries: bool,
) -> Result<Vec<Pokemon>, RandoError> {
    let mut pool = ctx.pool(pool_id, what)?;
    pool.retain(|&p| {
        let info = ctx.dex.get(p).expect("pool only holds dex forms");
        (alt_forms || p.form == 0) && (legendaries || !info.is_legendary_class())
    });
    if pool.is_empty() {
        return Err(RandoError::EmptyPool { what: what.to_owned() });
    }
    Ok(pool)
}

/// For wild and static spawns: drops species the game only shows through their own objects.
pub fn overworld(dex: &Dex, mut pool: Vec<Pokemon>, what: &str) -> Result<Vec<Pokemon>, RandoError> {
    pool.retain(|&p| dex.get(p).is_none_or(|info| !facts::OBJECT_ONLY.iter().any(|&(n, _)| n == info.national)));
    if pool.is_empty() {
        return Err(RandoError::EmptyPool { what: what.to_owned() });
    }
    Ok(pool)
}

/// The form a species picker names, or None for "random". Species the randomizer never places
/// are refused here too.
pub fn resolve_pick(
    ctx: &Ctx,
    pick: SpeciesPick,
    section: &'static str,
    what: &str,
) -> Result<Option<Pokemon>, RandoError> {
    let Some(national) = pick.national else {
        return Ok(None);
    };
    let refuse = |detail: String| RandoError::Section { section, detail };
    let Some(info) = ctx.dex.playable().find(|f| f.national == national && f.id.form == pick.form) else {
        return Err(refuse(match ctx.dex.all().iter().find(|f| f.national == national) {
            Some(f) => format!("{what}: {} form {} is not in the game", ctx.names.species(f.id.species), pick.form),
            None => format!("{what}: National Dex #{national} is not in the game"),
        }));
    };
    if pool::always_excluded(info, ctx.settings.on(crate::rando::ALLOW_TERA_LOCKED)) {
        return Err(refuse(format!("{what}: {} cannot be placed", ctx.names.pokemon(info.id))));
    }
    Ok(Some(info.id))
}

/// Shows `map`'s new species in a cutscene and queues the file for output.
pub fn patch_scene(
    ctx: &mut Ctx,
    path: String,
    map: &BTreeMap<Pokemon, Pokemon>,
    section: &'static str,
) -> Result<(), RandoError> {
    let raw = ctx.game.read(&path).map_err(|e| DataError::Read {
        path: path.clone(),
        detail: e.to_string(),
    })?;
    let patch = scene::swap_species(&raw, map)?;
    if patch.changed == 0 {
        return Err(RandoError::Section {
            section,
            detail: format!("{path} no longer shows {:?}", map.keys().collect::<Vec<_>>()),
        });
    }
    ctx.files.insert(path, patch.bytes);
    Ok(())
}

const MOVE_FIELDS: [&str; 5] = ["land", "up_water", "underwater", "air1", "air2"];

/// Where a wild spawn rule places its Pokémon: a set over the `enabletable` flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moves(pub u8);

impl Moves {
    pub const LAND: Moves = Moves(1);
    pub const AIR: Moves = Moves(1 << 3 | 1 << 4);

    pub fn read(row: &TableValue) -> Result<Moves, RandoError> {
        let mut bits = 0;
        if let Some(table) = row.struct_value("enabletable") {
            for (i, field) in MOVE_FIELDS.iter().enumerate() {
                if table.bool(field).map_err(shape)? {
                    bits |= 1 << i;
                }
            }
        }
        Ok(Moves(bits))
    }

    pub fn write(self, row: &mut TableValue) -> Result<(), RandoError> {
        let table = row.struct_mut("enabletable").map_err(shape)?;
        for (i, field) in MOVE_FIELDS.iter().enumerate() {
            let on = self.0 & (1 << i) != 0;
            if table.bool(field).map_err(shape)? != on {
                table.set_bool(field, on).map_err(shape)?;
            }
        }
        Ok(())
    }

    pub fn overlaps(self, other: Moves) -> bool {
        self.0 & other.0 != 0
    }

    /// The part of this row's spawn modes that `can` covers, or all of `can` when none is, so
    /// the species never spawns somewhere the game does not let it move.
    pub fn fit(self, can: Moves) -> Moves {
        match self.0 & can.0 {
            0 => can,
            both => Moves(both),
        }
    }
}

/// What the vanilla wild tables show about each species: how the game lets it spawn, its voice
/// class, and whether it follows a herd leader somewhere.
pub struct Habits {
    moves: BTreeMap<Pokemon, Moves>,
    voices: BTreeMap<Pokemon, String>,
    followers: BTreeSet<Pokemon>,
}

impl Habits {
    pub fn load(ctx: &Ctx) -> Result<Habits, RandoError> {
        let mut moves: BTreeMap<Pokemon, Moves> = BTreeMap::new();
        let mut voices = BTreeMap::new();
        let mut followers = BTreeSet::new();
        for path in [PALDEA_WILD, KITAKAMI_WILD, BLUEBERRY_WILD] {
            for row in load_table(ctx.game, path)?.tables("values") {
                let p = Pokemon {
                    species: row.int("devid").map_err(shape)? as u16,
                    form: row.int("formno").map_err(shape)? as u16,
                };
                moves.entry(p).or_default().0 |= Moves::read(row)?.0;
                if let Some(voice) = row.string("pokeVoiceClassification").filter(|v| !v.is_empty()) {
                    voices.entry(p).or_insert_with(|| voice.to_owned());
                }
                let follower = row.int("bandpoke").map_err(shape)? as u16;
                if follower != 0 {
                    followers.insert(Pokemon {
                        species: follower,
                        form: row.int("bandFormno").map_err(shape)? as u16,
                    });
                }
            }
        }
        Ok(Habits { moves, voices, followers })
    }

    /// Followers carry no held item, so a species that follows a herd must not become a form
    /// that needs one.
    pub fn follows_a_herd(&self, p: Pokemon) -> bool {
        self.followers.contains(&p)
    }

    /// Species the vanilla tables never spawn count as walkers.
    pub fn moves(&self, p: Pokemon) -> Moves {
        self.moves.get(&p).copied().filter(|m| m.0 != 0).unwrap_or(Moves::LAND)
    }

    pub fn voice(&self, p: Pokemon) -> Option<&str> {
        self.voices.get(&p).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{scaled_level, Moves};

    #[test]
    fn movement_keeps_what_the_species_can_use() {
        let air = Moves(1 << 3);
        let land_and_air = Moves(1 | 1 << 3);
        let water = Moves(1 << 1);
        assert_eq!(land_and_air.fit(Moves::LAND), Moves::LAND);
        assert_eq!(land_and_air.fit(air), air);
        assert_eq!(air.fit(Moves::LAND), Moves::LAND);
        assert_eq!(Moves::LAND.fit(water), water);
    }

    #[test]
    fn levels_scale_and_stay_in_range() {
        assert_eq!(scaled_level(50, 10), 55);
        assert_eq!(scaled_level(3, -50), 2);
        assert_eq!(scaled_level(1, -50), 1);
        assert_eq!(scaled_level(80, 100), 100);
    }
}
