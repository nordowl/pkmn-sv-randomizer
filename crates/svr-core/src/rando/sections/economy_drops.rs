//! The crafting material each species drops, kept so every TM recipe stays craftable.

use std::collections::{BTreeMap, BTreeSet};

use crate::fb::{FieldError, TableValue};
use crate::rando::itempool::{ItemCatalog, ItemKind};
use crate::rando::{Ctx, RandoError, Rng};

const DROP_TABLE: &str = "world/data/item/dropitemdata/dropitemdata_array.bin";
const TM_RECIPES: &str = "world/data/ui/shop/shop_wazamachine/shop_wazamachine_data/shop_wazamachine_data_array.bin";
const RECIPE_FIELDS: [&str; 3] = ["Item01", "Item02", "Item03"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DropMode {
    Shuffle,
    Random,
}

/// One drop-table row whose material the randomizer may change.
struct Drop {
    row: usize,
    species: u16,
    item: u16,
    /// A playable species that is not legendary, so players can farm it for the material.
    carrier: bool,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "items",
        detail: e.to_string(),
    }
}

fn drop_item(row: &TableValue) -> u16 {
    row.table("item1")
        .and_then(|item| item.int("itemid").ok())
        .and_then(|id| u16::try_from(id).ok())
        .unwrap_or(0)
}

pub(super) fn run(ctx: &mut Ctx, catalog: &ItemCatalog, mode: DropMode) -> Result<(), RandoError> {
    let is_material = |id: u16| catalog.get(id).is_some_and(|i| i.kind == ItemKind::Material);
    let playable: BTreeSet<u16> = ctx.dex.playable().map(|f| f.id.species).collect();
    let farmable: BTreeSet<u16> = ctx
        .dex
        .playable()
        .filter(|f| !f.is_legendary_class())
        .map(|f| f.id.species)
        .collect();

    let table = ctx.store.get(DROP_TABLE)?;
    let materials: Vec<u16> = table
        .tables("values")
        .map(drop_item)
        .filter(|&id| is_material(id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut drops = Vec::new();
    for (row, values) in table.tables("values").enumerate() {
        let species = u16::try_from(values.int("devid").map_err(shape)?).unwrap_or(0);
        let item = drop_item(values);
        if playable.contains(&species) && is_material(item) {
            drops.push(Drop {
                row,
                species,
                item,
                carrier: farmable.contains(&species),
            });
        }
    }
    let recipe_items: BTreeSet<u16> = ctx
        .store
        .get(TM_RECIPES)?
        .tables("values")
        .flat_map(|recipe| RECIPE_FIELDS.map(|field| recipe.int(field).unwrap_or(0)))
        .filter_map(|id| u16::try_from(id).ok())
        .collect();
    let needed: BTreeSet<u16> = materials.iter().copied().filter(|id| recipe_items.contains(id)).collect();

    let mut rng = ctx.rng("items.drops");
    let mut items: Vec<u16> = drops.iter().map(|d| d.item).collect();
    match mode {
        DropMode::Shuffle => rng.shuffle(&mut items),
        DropMode::Random => {
            for item in &mut items {
                *item = materials[rng.index(materials.len())];
            }
        }
    }
    let carriers: Vec<bool> = drops.iter().map(|d| d.carrier).collect();
    let repaired = cover(&mut items, &carriers, &needed, &mut rng).map_err(|missing| RandoError::Section {
        section: "items",
        detail: format!(
            "too few Pokémon drop materials to keep every TM craftable; {} could not be placed",
            missing.iter().map(|&id| ctx.names.item(id)).collect::<Vec<_>>().join(", ")
        ),
    })?;

    let changed: Vec<(&Drop, u16)> = drops.iter().zip(items).filter(|(d, new)| d.item != *new).collect();
    if !changed.is_empty() {
        let mut rows: Vec<&mut TableValue> = ctx.store.get_mut(DROP_TABLE)?.tables_mut("values").collect();
        for &(drop, new) in &changed {
            rows[drop.row]
                .table_mut("item1")
                .and_then(|item| item.set_int("itemid", i64::from(new)))
                .map_err(shape)?;
        }
    }
    let log = ctx.log.section("Pokémon drops");
    log.note(format!("{} of {} species drop a different material.", changed.len(), drops.len()));
    if repaired > 0 {
        log.note(format!(
            "{repaired} drops were moved so every TM recipe material stays on a Pokémon that is not legendary."
        ));
    }
    let table = log.table("Pokémon drops", &["Pokémon", "Was", "Now"]);
    for (drop, new) in changed {
        table.row(vec![ctx.names.species(drop.species), ctx.names.item(drop.item), ctx.names.item(new)]);
    }
    Ok(())
}

/// Puts every `needed` material on at least one carrier row. A row is only overwritten when its
/// material is not needed or another carrier also drops it, so no recipe loses its last source.
/// Returns how many rows changed, or the materials that found no row.
fn cover(items: &mut [u16], carriers: &[bool], needed: &BTreeSet<u16>, rng: &mut Rng) -> Result<usize, Vec<u16>> {
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for (&item, _) in items.iter().zip(carriers).filter(|(_, &c)| c) {
        *counts.entry(item).or_default() += 1;
    }
    let mut changed = 0;
    let mut missing = Vec::new();
    for &material in needed {
        if counts.get(&material).is_some_and(|&n| n > 0) {
            continue;
        }
        let spare: Vec<usize> = (0..items.len())
            .filter(|&i| carriers[i] && (!needed.contains(&items[i]) || counts[&items[i]] > 1))
            .collect();
        let Some(&i) = rng.pick(&spare) else {
            missing.push(material);
            continue;
        };
        *counts.get_mut(&items[i]).expect("counted above") -= 1;
        items[i] = material;
        *counts.entry(material).or_default() += 1;
        changed += 1;
    }
    if missing.is_empty() {
        Ok(changed)
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_places_missing_materials_without_stealing_last_sources() {
        let needed: BTreeSet<u16> = [1, 2, 3, 4].into();
        let mut items = vec![1, 1, 2, 9, 1, 2];
        let carriers = [true, true, true, true, false, true];
        let changed = cover(&mut items, &carriers, &needed, &mut Rng::new(1)).unwrap();
        assert_eq!(changed, 2);
        for m in needed {
            assert!(items.iter().zip(carriers).any(|(&i, c)| c && i == m), "{m} missing from {items:?}");
        }
        assert_eq!(items[4], 1, "non-carrier rows are never used");
    }

    #[test]
    fn cover_reports_what_cannot_fit() {
        let needed: BTreeSet<u16> = [1, 2, 3].into();
        let mut items = vec![1, 2];
        assert_eq!(cover(&mut items, &[true, true], &needed, &mut Rng::new(1)), Err(vec![3]));
    }
}
