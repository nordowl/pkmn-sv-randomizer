//! Writes one archive file, known by its path hash, to disk.
//! cargo run -p svr-core --release --example dump_hash -- <hash hex> <out file>

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hash = u64::from_str_radix(&args[0], 16).unwrap();
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &keys,
    )
    .unwrap();
    let bytes = game.archive().read_hash(hash).unwrap();
    std::fs::write(&args[1], &bytes).unwrap();
    println!("wrote {} bytes", bytes.len());
}
