//! Rejects a seed whose changed tables reference anything the game cannot use.
//!
//! References are found by enum type in each table's schema, so every table is covered without a
//! list of field names. Only references that are new compared with the vanilla table count, so
//! the game's own test rows never fail a seed.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::scene;
use crate::data::store::{game_items, load_table, MOVES};
use crate::data::{Dex, Pokemon, TableStore};
use crate::fb::{ElementType, FieldType, StructValue, TableValue, Value};
use crate::game::GameFiles;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ref {
    Species(u16),
    Form(Pokemon),
    Move(u16),
    Item(u16),
}

pub struct Rules {
    usable_moves: BTreeSet<u16>,
    items: BTreeSet<u16>,
}

impl Rules {
    pub fn load(store: &mut TableStore) -> Result<Rules, crate::data::DataError> {
        let mut usable_moves = BTreeSet::new();
        for (i, row) in store.get(MOVES)?.tables("Table").enumerate() {
            if row.bool("CanUseMove").unwrap_or(false) {
                usable_moves.insert(u16::try_from(row.int("MoveID").unwrap_or(i as i64)).unwrap_or(0));
            }
        }
        Ok(Rules {
            usable_moves,
            items: game_items(store)?,
        })
    }

    fn valid(&self, dex: &Dex, r: Ref) -> bool {
        match r {
            Ref::Species(0) | Ref::Move(0) | Ref::Item(0) => true,
            Ref::Form(p) if p.species == 0 => true,
            Ref::Species(s) => dex.playable().any(|f| f.id.species == s),
            Ref::Form(p) => dex.is_playable(p),
            Ref::Move(m) => self.usable_moves.contains(&m),
            Ref::Item(i) => self.items.contains(&i),
        }
    }
}

fn form_sibling(field: &str) -> Vec<String> {
    let mut out = vec![
        field.replace("devId", "formId"),
        field.replace("devid", "formno"),
        field.replace("DevId", "FormId"),
        field.replace("devId", "formNo"),
    ];
    if field == "bandpoke" {
        out.push("bandFormno".into());
    }
    out.retain(|s| s != field);
    out
}

fn enum_kind(table_schema: &crate::fb::Schema, ty: ElementType) -> Option<&'static str> {
    let ElementType::Scalar(_, Some(id)) = ty else {
        return None;
    };
    let name = table_schema.enum_at(id).name.as_str();
    let short = name.rsplit('.').next().unwrap_or(name);
    match short {
        "DevID" => Some("species"),
        "WazaID" => Some("move"),
        "ItemID" => Some("item"),
        _ => None,
    }
}

fn int(value: &Value) -> Option<i64> {
    value.as_int().or_else(|| value.as_uint().map(|v| v as i64))
}

fn collect_table(t: &TableValue, out: &mut BTreeMap<Ref, usize>) {
    let schema = t.schema().clone();
    let fields: Vec<_> = t.fields().map(|(d, v)| (d.clone(), v.cloned())).collect();
    let by_name: BTreeMap<&str, Option<&Value>> = fields.iter().map(|(d, v)| (d.name.as_str(), v.as_ref())).collect();
    for (def, value) in &fields {
        let Some(value) = value else { continue };
        visit(&schema, def.ty, &def.name, value, &|name| by_name.get(name).copied().flatten().and_then(int), out);
    }
}

fn collect_struct(s: &StructValue, out: &mut BTreeMap<Ref, usize>) {
    let schema = s.schema().clone();
    let fields: Vec<_> = s.fields().map(|(d, v)| (d.clone(), v.clone())).collect();
    let by_name: BTreeMap<&str, &Value> = fields.iter().map(|(d, v)| (d.name.as_str(), v)).collect();
    for (def, value) in &fields {
        visit(&schema, def.ty, &def.name, value, &|name| by_name.get(name).and_then(|v| int(v)), out);
    }
}

fn visit(
    schema: &crate::fb::Schema,
    ty: FieldType,
    name: &str,
    value: &Value,
    sibling: &dyn Fn(&str) -> Option<i64>,
    out: &mut BTreeMap<Ref, usize>,
) {
    let element = match ty {
        FieldType::Single(e) | FieldType::Vector(e) | FieldType::Array(e, _) => e,
    };
    let items: Vec<&Value> = match value {
        Value::Vector(v) | Value::Array(v) => v.iter().collect(),
        other => vec![other],
    };
    for item in items {
        match item {
            Value::Table(t) => collect_table(t, out),
            Value::Struct(s) => collect_struct(s, out),
            scalar => {
                let Some(v) = int(scalar) else { continue };
                let reference = match enum_kind(schema, element) {
                    Some("species") => {
                        let form = form_sibling(name).iter().find_map(|f| sibling(f));
                        match form {
                            Some(form) => Ref::Form(Pokemon {
                                species: v as u16,
                                form: form as u16,
                            }),
                            None => Ref::Species(v as u16),
                        }
                    }
                    Some("move") => Ref::Move(v as u16),
                    Some("item") => Ref::Item(v as u16),
                    _ => continue,
                };
                *out.entry(reference).or_default() += 1;
            }
        }
    }
}

fn references(t: &TableValue) -> BTreeMap<Ref, usize> {
    let mut out = BTreeMap::new();
    collect_table(t, &mut out);
    out
}

/// Returns one message per new invalid reference. An empty list means the seed is safe to write.
pub fn check(
    game: &GameFiles,
    dex: &Dex,
    rules: &Rules,
    changed: &BTreeMap<String, TableValue>,
    other_files: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, table) in changed {
        let vanilla = load_table(game, path).map(|t| references(&t)).unwrap_or_default();
        for (r, count) in references(table) {
            if !vanilla.contains_key(&r) && !rules.valid(dex, r) {
                problems.push(format!("{path}: {count} new reference(s) to {r:?}, which the game cannot use"));
            }
        }
    }
    for (path, bytes) in other_files {
        if path.ends_with(".trsog") || path.ends_with(".trscn") || path.ends_with(".trsot") {
            match scene::swap_species(bytes, &BTreeMap::new()) {
                Ok(found) => {
                    for p in found.found.keys() {
                        let in_vanilla = game
                            .read(path)
                            .ok()
                            .and_then(|raw| scene::swap_species(&raw, &BTreeMap::new()).ok())
                            .is_some_and(|v| v.found.contains_key(p));
                        if !in_vanilla && !dex.is_playable(*p) {
                            problems.push(format!("{path}: scene shows {p:?}, which has no usable model"));
                        }
                    }
                }
                Err(e) => problems.push(format!("{path}: scene no longer parses: {e}")),
            }
        }
    }
    problems
}
