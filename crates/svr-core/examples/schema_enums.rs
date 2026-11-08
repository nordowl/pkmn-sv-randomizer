//! Prints the enums and fields of a table's schema, optionally only enums whose name contains a filter.
//! cargo run -p svr-core --release --example schema_enums -- <table path> [enum filter]

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::load_table;
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &keys,
    )
    .unwrap();
    let table = load_table(&game, &args[0]).unwrap();
    let filter = args.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
    for e in table.schema().enums().iter().filter(|e| e.name.to_lowercase().contains(&filter) && e.values.len() < 200) {
        println!("enum {}: {:?}", e.name, e.values);
    }
    if let Some(row) = table.tables("values").next() {
        println!("row fields: {:?}", row.fields().map(|(d, _)| d.name.clone()).collect::<Vec<_>>());
    };
}
