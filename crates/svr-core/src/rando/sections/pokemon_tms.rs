//! The TM machine: which move each TM teaches, what crafting it costs, and which Pokémon can use it.
//!
//! A TM's move lives in two tables, the item's `MachineWaza` and the machine's `WazaNo`, and both
//! always change together. Compatibility is stored per species as move IDs, so it is tracked here
//! per TM and rewritten in move IDs after the TMs change.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::store::ITEMS;
use crate::fb::FieldError;
use crate::rando::{Ctx, RandoError};

use super::super::pokemon::table::Row;

pub const TM_MACHINE: &str = "world/data/ui/shop/shop_wazamachine/shop_wazamachine_data/shop_wazamachine_data_array.bin";

const MATERIAL_FIELDS: [(&str, &str, &str); 3] = [("Item01", "ItemNum01", "DevNo01"), ("Item02", "ItemNum02", "DevNo02"), ("Item03", "ItemNum03", "DevNo03")];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    pub item: u16,
    pub count: i64,
    pub source: i64,
}

/// One craftable TM. TM00, which no machine row lists, is not one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tm {
    pub item: u16,
    pub move_id: u16,
    pub lp: i64,
    pub materials: [Material; 3],
}

pub struct TmTable {
    pub tms: Vec<Tm>,
    before: Vec<Tm>,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "TM machine",
        detail: e.to_string(),
    }
}

impl TmTable {
    pub fn load(ctx: &mut Ctx) -> Result<TmTable, RandoError> {
        let mut tms = Vec::new();
        for row in ctx.store.get(TM_MACHINE)?.tables("values") {
            let mut materials = [Material { item: 0, count: 0, source: 0 }; 3];
            for (m, (item, count, source)) in materials.iter_mut().zip(MATERIAL_FIELDS) {
                *m = Material {
                    item: row.int(item).map_err(shape)? as u16,
                    count: row.int(count).map_err(shape)?,
                    source: row.int(source).map_err(shape)?,
                };
            }
            tms.push(Tm {
                item: row.int("WazaItemID").map_err(shape)? as u16,
                move_id: row.int("WazaNo").map_err(shape)? as u16,
                lp: row.int("LP").map_err(shape)?,
                materials,
            });
        }
        Ok(TmTable { before: tms.clone(), tms })
    }

    pub fn before(&self) -> &[Tm] {
        &self.before
    }

    pub fn moves(&self) -> Vec<u16> {
        self.tms.iter().map(|t| t.move_id).collect()
    }

    pub fn changed(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.tms.len()).filter(|&i| self.tms[i] != self.before[i])
    }

    pub fn save(&self, ctx: &mut Ctx) -> Result<(), RandoError> {
        if self.changed().next().is_none() {
            return Ok(());
        }
        let machine = ctx.store.get_mut(TM_MACHINE)?;
        for (row, (new, old)) in machine.tables_mut("values").zip(self.tms.iter().zip(&self.before)) {
            if new == old {
                continue;
            }
            row.set_int("WazaNo", i64::from(new.move_id)).map_err(shape)?;
            row.set_int("LP", new.lp).map_err(shape)?;
            for (m, (item, count, source)) in new.materials.iter().zip(MATERIAL_FIELDS) {
                row.set_int(item, i64::from(m.item)).map_err(shape)?;
                row.set_int(count, m.count).map_err(shape)?;
                row.set_int(source, m.source).map_err(shape)?;
            }
        }
        let new_moves: BTreeMap<u16, u16> = self
            .tms
            .iter()
            .zip(&self.before)
            .filter(|(n, o)| n.move_id != o.move_id)
            .map(|(n, _)| (n.item, n.move_id))
            .collect();
        if !new_moves.is_empty() {
            for item in ctx.store.get_mut(ITEMS)?.tables_mut("values") {
                let id = item.int("Id").map_err(shape)? as u16;
                if let Some(&m) = new_moves.get(&id) {
                    item.set_int("MachineWaza", i64::from(m)).map_err(shape)?;
                }
            }
        }
        Ok(())
    }
}

/// Which TMs `row` can use, by TM index, read against the TM moves it was loaded with.
pub fn compatibility(row: &Row, tm_moves: &[u16]) -> Vec<bool> {
    tm_moves.iter().map(|m| row.tm.contains(m)).collect()
}

/// Rewrites `row.tm` so it lists exactly the moves of the TMs in `flags`. Moves that no TM teaches
/// before or after stay, since the game keeps them there for its own reasons.
pub fn write_compatibility(row: &mut Row, flags: &[bool], old_moves: &[u16], new_moves: &[u16]) {
    let old: BTreeSet<u16> = old_moves.iter().copied().collect();
    let new: BTreeSet<u16> = new_moves.iter().copied().collect();
    let mut list: BTreeSet<u16> = row.tm.iter().copied().filter(|m| !old.contains(m) && !new.contains(m)).collect();
    list.extend(new_moves.iter().zip(flags).filter(|(_, &on)| on).map(|(&m, _)| m));
    let current: BTreeSet<u16> = row.tm.iter().copied().collect();
    if list != current {
        row.tm = list.into_iter().collect();
    }
}
