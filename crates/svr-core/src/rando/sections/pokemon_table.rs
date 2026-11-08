//! A working copy of the species table for the sections that edit Pokémon data. A section loads
//! it, edits plain rows, and `save` writes back only the fields that changed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::data::dex::STAT_FIELDS;
use crate::data::store::PERSONAL;
use crate::data::{Dex, Evolution, Pokemon};
use crate::fb::{FieldError, TableValue, Value};
use crate::rando::{Ctx, RandoError};

/// The level the game stores for a move learned on evolution.
pub const ON_EVOLUTION: i8 = -3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelUpMove {
    pub move_id: u16,
    pub level: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: Pokemon,
    pub present: bool,
    pub playable: bool,
    pub type_locked: bool,
    pub stats: [u8; 6],
    /// Equal types mean a single type.
    pub types: [u8; 2],
    /// First, second and hidden. Equal first and second mean a single regular ability.
    pub abilities: [u16; 3],
    pub xp_growth: u8,
    pub catch_rate: u8,
    pub hatch_cycles: u8,
    pub evolutions: Vec<Evolution>,
    /// Sorted by level, with moves learned on evolution first.
    pub levelup: Vec<LevelUpMove>,
    pub egg: Vec<u16>,
    pub reminder: Vec<u16>,
    /// TM compatibility by move ID. The game also lists moves here that no TM teaches.
    pub tm: Vec<u16>,
}

impl Row {
    pub fn bst(&self) -> u32 {
        self.stats.iter().map(|&s| u32::from(s)).sum()
    }

    pub fn has_type(&self, t: u8) -> bool {
        self.types.contains(&t)
    }

    pub fn learns_by_level(&self, move_id: u16) -> bool {
        self.levelup.iter().any(|m| m.move_id == move_id)
    }
}

pub struct Species {
    pub rows: Vec<Row>,
    before: Vec<Row>,
    index: BTreeMap<Pokemon, usize>,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "species table",
        detail: e.to_string(),
    }
}

fn int<T: TryFrom<i64>>(v: Result<i64, FieldError>) -> Result<T, RandoError> {
    let v = v.map_err(shape)?;
    T::try_from(v).map_err(|_| RandoError::Section {
        section: "species table",
        detail: format!("value {v} is out of range"),
    })
}

fn ushorts(entry: &TableValue, field: &str) -> Vec<u16> {
    entry
        .vector(field)
        .unwrap_or(&[])
        .iter()
        .filter_map(Value::as_uint)
        .map(|v| v as u16)
        .collect()
}

fn read_row(dex: &Dex, i: usize, entry: &TableValue) -> Result<Row, RandoError> {
    let info = &dex.all()[i];
    let mut stats = [0u8; 6];
    if let Some(block) = entry.struct_value("base_stats") {
        for (slot, name) in stats.iter_mut().zip(STAT_FIELDS) {
            *slot = int(block.int(name))?;
        }
    }
    let mut levelup = Vec::new();
    for m in entry.vector("levelup_moves").unwrap_or(&[]).iter().filter_map(Value::as_struct) {
        levelup.push(LevelUpMove {
            move_id: int(m.int("move"))?,
            level: int(m.int("level"))?,
        });
    }
    let mut evolutions = Vec::new();
    for e in entry.vector("evolutions").unwrap_or(&[]).iter().filter_map(Value::as_struct) {
        evolutions.push(Evolution {
            condition: int(e.int("condition"))?,
            parameter: int(e.int("parameter"))?,
            level: int(e.int("level"))?,
            into: Pokemon {
                species: int(e.int("species"))?,
                form: int(e.int("form"))?,
            },
        });
    }
    Ok(Row {
        id: info.id,
        present: info.present,
        playable: info.playable(),
        type_locked: entry.bool("type_change_disallowed").unwrap_or(false),
        stats,
        types: [int(entry.int("type_1"))?, int(entry.int("type_2"))?],
        abilities: [
            int(entry.int("ability_1"))?,
            int(entry.int("ability_2"))?,
            int(entry.int("ability_hidden"))?,
        ],
        xp_growth: int(entry.int("xp_growth"))?,
        catch_rate: int(entry.int("catch_rate"))?,
        hatch_cycles: int(entry.int("egg_hatch_steps"))?,
        evolutions,
        levelup,
        egg: ushorts(entry, "egg_moves"),
        reminder: ushorts(entry, "reminder_moves"),
        tm: ushorts(entry, "tm_moves"),
    })
}

fn set_ushorts(entry: &mut TableValue, field: &str, values: &[u16]) -> Result<(), FieldError> {
    *entry.vector_mut(field)? = values.iter().map(|&v| Value::UInt(u64::from(v))).collect();
    Ok(())
}

fn write_row(entry: &mut TableValue, old: &Row, new: &Row) -> Result<(), FieldError> {
    if new.stats != old.stats {
        let block = entry.struct_mut("base_stats")?;
        for (&v, name) in new.stats.iter().zip(STAT_FIELDS) {
            block.set_int(name, i64::from(v))?;
        }
    }
    if new.types != old.types {
        entry.set_int("type_1", i64::from(new.types[0]))?;
        entry.set_int("type_2", i64::from(new.types[1]))?;
    }
    if new.abilities != old.abilities {
        for (&a, field) in new.abilities.iter().zip(["ability_1", "ability_2", "ability_hidden"]) {
            entry.set_int(field, i64::from(a))?;
        }
    }
    if new.xp_growth != old.xp_growth {
        entry.set_int("xp_growth", i64::from(new.xp_growth))?;
    }
    if new.catch_rate != old.catch_rate {
        entry.set_int("catch_rate", i64::from(new.catch_rate))?;
    }
    if new.hatch_cycles != old.hatch_cycles {
        entry.set_int("egg_hatch_steps", i64::from(new.hatch_cycles))?;
    }
    if new.evolutions != old.evolutions {
        let template = entry.new_struct("evolutions")?;
        let existing: Vec<Value> = entry.vector("evolutions").unwrap_or(&[]).to_vec();
        let mut values = Vec::new();
        for (k, evo) in new.evolutions.iter().enumerate() {
            let mut s = existing.get(k).and_then(Value::as_struct).cloned().unwrap_or_else(|| template.clone());
            s.set_int("level", i64::from(evo.level))?;
            s.set_int("condition", i64::from(evo.condition))?;
            s.set_int("parameter", i64::from(evo.parameter))?;
            s.set_int("species", i64::from(evo.into.species))?;
            s.set_int("form", i64::from(evo.into.form))?;
            values.push(Value::Struct(s));
        }
        *entry.vector_mut("evolutions")? = values;
    }
    if new.levelup != old.levelup {
        let template = entry.new_struct("levelup_moves")?;
        let mut values = Vec::new();
        for m in &new.levelup {
            let mut s = template.clone();
            s.set_int("move", i64::from(m.move_id))?;
            s.set_int("level", i64::from(m.level))?;
            values.push(Value::Struct(s));
        }
        *entry.vector_mut("levelup_moves")? = values;
    }
    if new.egg != old.egg {
        set_ushorts(entry, "egg_moves", &new.egg)?;
    }
    if new.reminder != old.reminder {
        set_ushorts(entry, "reminder_moves", &new.reminder)?;
    }
    if new.tm != old.tm {
        set_ushorts(entry, "tm_moves", &new.tm)?;
    }
    Ok(())
}

impl Species {
    pub fn load(ctx: &mut Ctx) -> Result<Species, RandoError> {
        let dex = &ctx.dex;
        let mut rows = Vec::new();
        for (i, entry) in ctx.store.get(PERSONAL)?.tables("entry").enumerate() {
            rows.push(read_row(dex, i, entry)?);
        }
        let index = rows.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
        Ok(Species {
            before: rows.clone(),
            rows,
            index,
        })
    }

    pub fn row_of(&self, p: Pokemon) -> Option<usize> {
        self.index.get(&p).copied()
    }

    /// Row `i` as it was when loaded.
    pub fn before(&self, i: usize) -> &Row {
        &self.before[i]
    }

    pub fn changed(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| self.rows[i] != self.before[i])
    }

    /// Writes changed rows into the species table and refreshes the dex. Untouched rows stay
    /// byte-identical, and nothing is written when nothing changed.
    pub fn save(&self, ctx: &mut Ctx) -> Result<(), RandoError> {
        if self.changed().next().is_none() {
            return Ok(());
        }
        let table = ctx.store.get_mut(PERSONAL)?;
        for (i, entry) in table.tables_mut("entry").enumerate() {
            if self.rows[i] != self.before[i] {
                write_row(entry, &self.before[i], &self.rows[i]).map_err(shape)?;
            }
        }
        ctx.refresh_dex()
    }

    /// For each row, the row it must copy because it is a cosmetic form: same species, lower
    /// form, and the same `key` as loaded. Visual variants such as Vivillon patterns stay alike.
    pub fn twins<K: PartialEq>(&self, key: impl Fn(&Row) -> K) -> Vec<Option<usize>> {
        let mut leaders: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
        let mut out = vec![None; self.rows.len()];
        for (i, row) in self.before.iter().enumerate() {
            if !row.present {
                continue;
            }
            let group = leaders.entry(row.id.species).or_default();
            let k = key(row);
            match group.iter().find(|&&l| key(&self.before[l]) == k) {
                Some(&leader) => out[i] = Some(leader),
                None => group.push(i),
            }
        }
        out
    }

    /// Lets each cosmetic form from `twins` take over fields from its base form.
    pub fn copy_twins(&mut self, twins: &[Option<usize>], copy: impl Fn(&mut Row, &Row)) {
        for (i, leader) in twins.iter().enumerate() {
            if let Some(l) = *leader {
                let from = self.rows[l].clone();
                copy(&mut self.rows[i], &from);
            }
        }
    }

    /// The evolution graph of the present rows that `include` keeps, from the current rows.
    pub fn families(&self, include: impl Fn(usize) -> bool) -> Families {
        let n = self.rows.len();
        let mut parent = vec![None; n];
        let mut children = vec![Vec::new(); n];
        for (i, row) in self.rows.iter().enumerate() {
            if !row.present || !include(i) {
                continue;
            }
            for evo in &row.evolutions {
                let Some(t) = self.row_of(evo.into) else { continue };
                if t == i || !self.rows[t].present || !include(t) || children[i].contains(&t) {
                    continue;
                }
                children[i].push(t);
                if parent[t].is_none() {
                    parent[t] = Some(i);
                }
            }
        }
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        let roots: Vec<usize> = (0..n).filter(|&i| parent[i].is_none()).collect();
        let others: Vec<usize> = (0..n).filter(|&i| parent[i].is_some()).collect();
        for start in roots.into_iter().chain(others) {
            if !self.rows[start].present || !include(start) || !seen.insert(start) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            while let Some(i) = queue.pop_front() {
                order.push(i);
                for &c in &children[i] {
                    if parent[c] == Some(i) && seen.insert(c) {
                        queue.push_back(c);
                    }
                }
            }
        }
        Families { parent, children, order }
    }
}

/// An evolution graph over row indices. Each row has at most one parent, its first pre-evolution
/// in row order.
pub struct Families {
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    order: Vec<usize>,
}

impl Families {
    pub fn parent(&self, i: usize) -> Option<usize> {
        self.parent[i]
    }

    /// True when `i`'s parent evolves into more than one row, like Eevee or Tyrogue.
    pub fn is_split(&self, i: usize) -> bool {
        self.parent[i].is_some_and(|p| self.children[p].len() > 1)
    }

    /// Every included row once, each parent before its children. Rows on a cycle with no way in
    /// come after the rest, in row order.
    pub fn top_down(&self) -> &[usize] {
        &self.order
    }
}
