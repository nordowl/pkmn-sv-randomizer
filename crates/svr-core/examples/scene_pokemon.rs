//! Lists the Pokémon objects in each given scene file, as species/form and count.
//! cargo run -p svr-core --release --example scene_pokemon -- <path>...

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::scene::swap_species;
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(&GamePaths::find(&games, Edition::Scarlet).expect("Scarlet files in SVR_TEST_GAMES"), &keys).unwrap();
    for path in std::env::args().skip(1) {
        if !game.contains(&path) {
            println!(" -  {path}");
            continue;
        }
        let found = swap_species(&game.read(&path).unwrap(), &BTreeMap::new()).map(|patch| patch.found);
        match found {
            Ok(found) => {
                let list: Vec<String> = found.iter().map(|(p, n)| format!("{}/{} x{n}", p.species, p.form)).collect();
                println!("yes {path}: {}", list.join(", "));
            }
            Err(e) => println!("err {path}: {e}"),
        }
    }
}
