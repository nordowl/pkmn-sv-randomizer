use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::fb::{self, builtin, Schema, TableValue};
use crate::game::GameFiles;

use super::DataError;

pub const PERSONAL: &str = "avalon/data/personal_array.bin";
pub const MOVES: &str = "avalon/data/waza_array.bin";
pub const ABILITIES: &str = "avalon/data/tokusei_array.bin";
pub const CATALOG: &str = "pokemon/catalog/catalog/poke_resource_table.trpmcatalog";
pub const ITEMS: &str = "world/data/item/itemdata/itemdata_array.bin";
/// A table whose schema declares the game's `ItemID` enum. The item table's own schema does not.
const ITEM_ENUM_SOURCE: &str = "world/data/item/hiddenItemDataTable/hiddenItemDataTable_array.bin";

/// The items Scarlet and Violet know: the values of the game's `ItemID` enum. The item table also
/// keeps rows for items these games dropped, such as the Apricorns, which shops show as "None".
pub fn game_items(store: &mut TableStore) -> Result<BTreeSet<u16>, DataError> {
    let entries = store
        .get(ITEM_ENUM_SOURCE)?
        .schema()
        .enum_entries("ItemID")
        .ok_or_else(|| DataError::Shape(format!("{ITEM_ENUM_SOURCE} declares no ItemID enum")))?;
    Ok(entries.into_iter().filter_map(|(_, v)| u16::try_from(v).ok()).collect())
}

fn builtin_schema(path: &str) -> Option<Arc<Schema>> {
    match path {
        PERSONAL => Some(builtin::personal()),
        MOVES => Some(builtin::waza()),
        ABILITIES => Some(builtin::tokusei()),
        CATALOG => Some(builtin::pokemon_catalog()),
        _ => None,
    }
}

struct Entry {
    root: TableValue,
    dirty: bool,
}

/// Game tables loaded on first use. Only tables handed out mutably are written back.
pub struct TableStore<'g> {
    game: &'g GameFiles,
    tables: BTreeMap<String, Entry>,
}

impl<'g> TableStore<'g> {
    pub fn new(game: &'g GameFiles) -> TableStore<'g> {
        TableStore {
            game,
            tables: BTreeMap::new(),
        }
    }

    pub fn game(&self) -> &'g GameFiles {
        self.game
    }

    fn load(&mut self, path: &str) -> Result<&mut Entry, DataError> {
        if !self.tables.contains_key(path) {
            let root = load_table(self.game, path)?;
            self.tables.insert(path.to_owned(), Entry { root, dirty: false });
        }
        Ok(self.tables.get_mut(path).expect("inserted above"))
    }

    pub fn get(&mut self, path: &str) -> Result<&TableValue, DataError> {
        Ok(&self.load(path)?.root)
    }

    pub fn get_mut(&mut self, path: &str) -> Result<&mut TableValue, DataError> {
        let entry = self.load(path)?;
        entry.dirty = true;
        Ok(&mut entry.root)
    }

    pub fn is_dirty(&self, path: &str) -> bool {
        self.tables.get(path).is_some_and(|entry| entry.dirty)
    }

    pub fn changed(&self) -> BTreeMap<String, TableValue> {
        self.tables
            .iter()
            .filter(|(_, entry)| entry.dirty)
            .map(|(path, entry)| (path.clone(), entry.root.clone()))
            .collect()
    }

    pub fn into_files(self) -> Result<BTreeMap<String, Vec<u8>>, DataError> {
        self.tables
            .into_iter()
            .filter(|(_, entry)| entry.dirty)
            .map(|(path, entry)| {
                let bytes = fb::try_serialize(&entry.root).map_err(|source| DataError::Serialize {
                    path: path.clone(),
                    detail: source.to_string(),
                })?;
                Ok((path, bytes))
            })
            .collect()
    }
}

pub fn load_table(game: &GameFiles, path: &str) -> Result<TableValue, DataError> {
    let schema = match builtin_schema(path) {
        Some(schema) => schema,
        None => {
            let schema_path = path
                .strip_suffix(".bin")
                .map(|stem| format!("{stem}.bfbs"))
                .ok_or_else(|| DataError::NoSchema(path.to_owned()))?;
            let bytes = game.read(&schema_path).map_err(|_| DataError::NoSchema(path.to_owned()))?;
            Schema::from_bfbs(&bytes).map_err(|source| DataError::Schema {
                path: schema_path,
                detail: source.to_string(),
            })?
        }
    };
    let bytes = game.read(path).map_err(|source| DataError::Read {
        path: path.to_owned(),
        detail: source.to_string(),
    })?;
    fb::parse(&schema, &bytes).map_err(|source| DataError::Parse {
        path: path.to_owned(),
        detail: source.to_string(),
    })
}
