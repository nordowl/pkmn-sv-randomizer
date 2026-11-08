//! Missing-species engine test: turns on Victini with Pikachu's model and data, gives it as the grass
//! starter, and writes the mod. Usage: engine_test <keys> <games dir> <mod dir>

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::{CATALOG, PERSONAL};
use svr_core::data::{Dex, Pokemon, TableStore};
use svr_core::fb::Value;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::output::{write_mod, SeedMeta};

const GIFTS: &str = "world/data/event/event_add_pokemon/eventAddPokemon/eventAddPokemon_array.bin";
const ICON_SCALE: &str = "world/data/ui/pokeicon/poke_icon_general_scale/poke_icon_general_scale_array.bin";
const SPECIES_DATA: [&str; 11] = [
    "ai/pokemon_species_behavior_data",
    "ai/pokemon_species_table",
    "collision/pokemon_collision_unique_data",
    "partner/partner_movement_data",
    "partner/partner_pokemon_data",
    "pokemon_common_data",
    "pokemon_contact_data",
    "stain/pokemon_stain_unique_data",
    "wild/wild_pokemon_group_data",
    "wild/wild_pokemon_single_data",
    "wild/wild_pokemon_special_data",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let keys = Keys::load(Path::new(&args[1])).expect("keys");
    let games = PathBuf::from(&args[2]);
    let out_dir = PathBuf::from(&args[3]);
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &keys,
    )
    .expect("open game");
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).expect("dex");
    let donor = Pokemon { species: 25, form: 0 };
    let target = dex.all().iter().find(|f| f.national == 494 && f.id.form == 0).expect("victini row");
    let new = target.id;
    println!("target internal id {} (present {}, model {}), donor {:?}", new.species, target.present, target.has_model, donor);

    store
        .get_mut(PERSONAL)
        .unwrap()
        .tables_mut("entry")
        .nth(target.row)
        .unwrap()
        .set_bool("is_present", true)
        .unwrap();

    let catalog = store.get_mut(CATALOG).unwrap();
    let entries = catalog.vector_mut("Table").unwrap();
    let key = |v: &Value| {
        let info = v.as_table().unwrap().table("Info").unwrap();
        (info.int("SpeciesInternal").unwrap() as u16, info.int("Form").unwrap() as u16)
    };
    let sorted = entries.windows(2).all(|w| key(&w[0]) <= key(&w[1]));
    let mut clones: Vec<Value> = entries
        .iter()
        .filter(|v| key(v) == (donor.species, donor.form))
        .cloned()
        .collect();
    for clone in &mut clones {
        clone.as_table_mut().unwrap().table_mut("Info").unwrap().set_int("SpeciesInternal", i64::from(new.species)).unwrap();
    }
    let at = entries.iter().position(|v| key(v) > (new.species, 0)).unwrap_or(entries.len());
    let added = clones.len();
    entries.splice(at..at, clones);
    println!("catalog was sorted: {sorted}; inserted {added} entries at {at}");

    let icons = store.get_mut(ICON_SCALE).unwrap();
    let rows = icons.vector_mut("values").unwrap();
    let dev = |v: &Value| v.as_table().unwrap().int("DevID").unwrap();
    if let Some(row) = rows.iter().find(|v| dev(v) == i64::from(donor.species)).cloned() {
        let mut row = row;
        row.as_table_mut().unwrap().set_int("DevID", i64::from(new.species)).unwrap();
        let at = rows.iter().position(|v| dev(v) > i64::from(new.species)).unwrap_or(rows.len());
        rows.insert(at, row);
        println!("icon scale row inserted at {at}");
    }

    let gifts = store.get_mut(GIFTS).unwrap();
    for row in gifts.tables_mut("values") {
        if row.string("label") == Some("common_0065_kusa") {
            row.table_mut("pokeData").unwrap().set_int("devId", i64::from(new.species)).unwrap();
            println!("grass starter gift -> internal {}", new.species);
        }
    }

    let donor_key = format!("{:04}_{:02}_00", donor.species, donor.form);
    let new_key = format!("{:04}_{:02}_00", new.species, new.form);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for kind in SPECIES_DATA {
        let src = format!("world/data/pokemon/{kind}/{donor_key}.bin");
        let dst = format!("world/data/pokemon/{kind}/{new_key}.bin");
        // The game still ships some data for removed species; it describes their own model, so it
        // must be replaced by the donor's to match the borrowed model.
        if game.contains(&dst) {
            println!("  {kind}: replacing the target's leftover data with the donor's");
        }
        files.insert(dst, game.read(&src).expect("donor data"));
        let list = format!("world/data/pokemon/pokemon_unique_path_data/{}.bin", kind.rsplit('/').next().unwrap());
        let table = store.get_mut(&list).unwrap();
        let slots: Vec<String> = table.fields().map(|(d, _)| d.name.clone()).collect();
        let mut keys: Vec<String> = slots
            .iter()
            .filter_map(|name| table.string(name).filter(|s| !s.is_empty()).map(String::from))
            .collect();
        let present = keys.contains(&new_key);
        if !present {
            let at = keys.partition_point(|k| *k < new_key);
            keys.insert(at, new_key.clone());
            assert!(keys.len() <= slots.len(), "{kind}: path list is full");
            for (slot, key) in slots.iter().zip(&keys) {
                table.set_string(slot, key.clone()).unwrap();
            }
        }
        println!("  {kind}: had it {present}, now {} keys", keys.len());
    }

    for (path, bytes) in store.into_files().expect("serialize") {
        files.insert(path, bytes);
    }
    let meta = SeedMeta {
        app_version: "engine-test".into(),
        edition: "Scarlet".into(),
        title_id: "0100A3D008C5C000".into(),
        seed: "victini-as-pikachu".into(),
        created_unix: 0,
        file_count: files.len(),
    };
    write_mod(&out_dir, &game, &files, &meta, &[]).expect("write mod");
    println!("wrote {} files to {}", files.len(), out_dir.display());
}
