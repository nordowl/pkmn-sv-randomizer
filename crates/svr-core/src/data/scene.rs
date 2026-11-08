//! Swaps the species shown by Pokémon objects inside scene files (`.trsog`, `.trscn`).
//!
//! Scenes are a tree of typed components whose payload is itself a FlatBuffer. Only the species and
//! form ushorts of Pokémon components change, in place, so every other byte stays as the game wrote it.
//! A payload that stores no form (form 0 is the default, so the game omits it) cannot take one in
//! place; it gets a copy with a form field appended to the file instead.

use std::collections::BTreeMap;

use super::{DataError, Pokemon};

const POKEMON_COMPONENTS: [&str; 2] = ["ti_PokemonModelComponent", "ti_FieldPokemonComponent"];
const MAX_DEPTH: usize = 64;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScenePatch {
    pub bytes: Vec<u8>,
    /// Every Pokémon object found, before patching, with how often it occurs.
    pub found: BTreeMap<Pokemon, usize>,
    pub changed: usize,
}

/// Replaces every Pokémon object whose species and form appear in `map`.
pub fn swap_species(raw: &[u8], map: &BTreeMap<Pokemon, Pokemon>) -> Result<ScenePatch, DataError> {
    let mut patch = ScenePatch {
        bytes: raw.to_vec(),
        ..ScenePatch::default()
    };
    let reader = Flat(raw);
    let root = reader.root(0)?;
    let objects = reader.table_vector(root, 4)?;
    let mut edits = Vec::new();
    walk(&reader, &objects, 0, &mut patch.found, &mut |site, current| {
        if let Some(&target) = map.get(&current) {
            edits.push((site, target));
        }
    })?;
    // Appending never moves a byte that a later edit's positions point at.
    for (site, target) in edits {
        patch.bytes[site.species..site.species + 2].copy_from_slice(&target.species.to_le_bytes());
        match site.form {
            Some(pos) => patch.bytes[pos..pos + 2].copy_from_slice(&target.form.to_le_bytes()),
            None if target.form != 0 => add_form(&mut patch.bytes, &site, target.form)?,
            None => {}
        }
        patch.changed += 1;
    }
    Ok(patch)
}

/// One Pokémon component: its ushorts, and where its nested payload sits in the file.
struct Site {
    species: usize,
    form: Option<usize>,
    /// The component's uoffset field that points at the payload vector.
    payload_ref: usize,
    payload: usize,
    len: usize,
    /// The payload's root table.
    table: usize,
}

/// Points the component at a copy of its payload, placed at the end of the file, whose root table
/// has a new vtable with a form slot and the form value after it. The old payload stays unused.
fn add_form(bytes: &mut Vec<u8>, site: &Site, form: u16) -> Result<(), DataError> {
    let shape = |what: &str| DataError::Shape(format!("scene Pokémon payload: {what}"));
    let flat = Flat(bytes.as_slice());
    let vtable = site.table as i64 - i64::from(i32::from_le_bytes(flat.bytes(site.table)?));
    let vtable = usize::try_from(vtable).map_err(|_| shape("vtable out of bounds"))?;
    let vsize = usize::from(flat.u16(vtable)?);
    let object_size = usize::from(flat.u16(vtable + 2)?);
    let end = site.payload + site.len;
    if vtable < site.payload || vtable + vsize > end || site.table + 4 > end {
        return Err(shape("root table or vtable lies outside the payload"));
    }

    let table = site.table - site.payload;
    let mut copy = bytes[site.payload..end].to_vec();
    copy.resize(copy.len().next_multiple_of(4), 0);
    let new_vtable = copy.len();
    let new_vsize = vsize.max(4 + 2 * 2);
    copy.extend_from_slice(&bytes[vtable..vtable + vsize]);
    copy.resize(new_vtable + new_vsize, 0);
    let value = copy.len();
    copy.extend_from_slice(&form.to_le_bytes());
    copy.resize(copy.len().next_multiple_of(4), 0);

    let u16_at = |n: usize| u16::try_from(n).map_err(|_| shape("payload too large for a form field"));
    let header = [u16_at(new_vsize)?, u16_at(object_size.max(value + 2 - table))?];
    copy[new_vtable..new_vtable + 2].copy_from_slice(&header[0].to_le_bytes());
    copy[new_vtable + 2..new_vtable + 4].copy_from_slice(&header[1].to_le_bytes());
    copy[new_vtable + 6..new_vtable + 8].copy_from_slice(&u16_at(value - table)?.to_le_bytes());
    let soffset = i32::try_from(table as i64 - new_vtable as i64).map_err(|_| shape("vtable too far"))?;
    copy[table..table + 4].copy_from_slice(&soffset.to_le_bytes());

    // Same alignment as the original vector, so the nested buffer's own alignment still holds.
    let prefix = site.payload - 4;
    while bytes.len() % 16 != prefix % 16 {
        bytes.push(0);
    }
    let moved = bytes.len();
    let len = u32::try_from(copy.len()).map_err(|_| shape("payload too large"))?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(&copy);
    let offset = u32::try_from(moved - site.payload_ref).map_err(|_| shape("file too large"))?;
    bytes[site.payload_ref..site.payload_ref + 4].copy_from_slice(&offset.to_le_bytes());
    Ok(())
}

fn walk(
    reader: &Flat,
    tables: &[usize],
    depth: usize,
    found: &mut BTreeMap<Pokemon, usize>,
    visit: &mut dyn FnMut(Site, Pokemon),
) -> Result<(), DataError> {
    if depth > MAX_DEPTH {
        return Err(DataError::Shape("scene nesting is too deep".into()));
    }
    for &table in tables {
        let kind = reader.string(table, 0)?;
        if POKEMON_COMPONENTS.contains(&kind.as_str()) {
            let payload_ref = reader
                .field(table, 1)?
                .ok_or_else(|| DataError::Shape(format!("{kind} has no payload")))?;
            let (data, len) = reader.vector(table, 1)?;
            if len < 4 {
                return Err(DataError::Shape(format!("{kind} payload is too short")));
            }
            let inner = reader.root(data)?;
            if let Some(species) = reader.field(inner, 0)? {
                let form = reader.field(inner, 1)?;
                let current = Pokemon {
                    species: reader.u16(species)?,
                    form: form.map(|p| reader.u16(p)).transpose()?.unwrap_or(0),
                };
                *found.entry(current).or_default() += 1;
                let site = Site {
                    species,
                    form,
                    payload_ref,
                    payload: data,
                    len,
                    table: inner,
                };
                visit(site, current);
            }
        }
        let children = reader.table_vector(table, 2)?;
        walk(reader, &children, depth + 1, found, visit)?;
    }
    Ok(())
}

struct Flat<'a>(&'a [u8]);

impl Flat<'_> {
    fn bytes<const N: usize>(&self, pos: usize) -> Result<[u8; N], DataError> {
        self.0
            .get(pos..pos + N)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| DataError::Shape(format!("scene read out of bounds at {pos:#x}")))
    }

    fn u16(&self, pos: usize) -> Result<u16, DataError> {
        Ok(u16::from_le_bytes(self.bytes(pos)?))
    }

    fn u32(&self, pos: usize) -> Result<u32, DataError> {
        Ok(u32::from_le_bytes(self.bytes(pos)?))
    }

    fn indirect(&self, pos: usize) -> Result<usize, DataError> {
        Ok(pos + self.u32(pos)? as usize)
    }

    fn root(&self, base: usize) -> Result<usize, DataError> {
        self.indirect(base)
    }

    fn field(&self, table: usize, index: usize) -> Result<Option<usize>, DataError> {
        let soffset = i32::from_le_bytes(self.bytes(table)?);
        let vtable = usize::try_from(table as i64 - i64::from(soffset))
            .map_err(|_| DataError::Shape("scene vtable out of bounds".into()))?;
        let vsize = usize::from(self.u16(vtable)?);
        let slot = 4 + 2 * index;
        if slot + 2 > vsize {
            return Ok(None);
        }
        let offset = usize::from(self.u16(vtable + slot)?);
        Ok((offset != 0).then_some(table + offset))
    }

    fn vector(&self, table: usize, index: usize) -> Result<(usize, usize), DataError> {
        match self.field(table, index)? {
            None => Ok((0, 0)),
            Some(pos) => {
                let start = self.indirect(pos)?;
                Ok((start + 4, self.u32(start)? as usize))
            }
        }
    }

    fn table_vector(&self, table: usize, index: usize) -> Result<Vec<usize>, DataError> {
        let (start, len) = self.vector(table, index)?;
        if start + len.saturating_mul(4) > self.0.len() {
            return Err(DataError::Shape("scene vector out of bounds".into()));
        }
        (0..len).map(|i| self.indirect(start + i * 4)).collect()
    }

    fn string(&self, table: usize, index: usize) -> Result<String, DataError> {
        let (start, len) = self.vector(table, index)?;
        let bytes = self
            .0
            .get(start..start + len)
            .ok_or_else(|| DataError::Shape("scene string out of bounds".into()))?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}
