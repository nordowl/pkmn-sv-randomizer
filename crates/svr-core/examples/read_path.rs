//! Reports which of the given archive paths exist, and writes the first match to `out` if given.
//! cargo run -p svr-core --release --example read_path -- [--out FILE] <path>...

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.iter().position(|a| a == "--out").map(|i| {
        let file = args.remove(i + 1);
        args.remove(i);
        file
    });
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(&GamePaths::find(&games, Edition::Scarlet).expect("Scarlet files in SVR_TEST_GAMES"), &keys).unwrap();
    let mut written = false;
    for path in &args {
        let found = game.contains(path);
        println!("{} {path}", if found { "yes" } else { " - " });
        if found && !written {
            if let Some(out) = &out {
                std::fs::write(out, game.read(path).unwrap()).unwrap();
                written = true;
            }
        }
    }
}
