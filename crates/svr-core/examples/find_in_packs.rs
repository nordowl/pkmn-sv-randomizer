//! Lists archive packs whose path contains a filter, or with search strings, decompresses those
//! packs and reports which contain each string.
//! cargo run -p svr-core --release --example find_in_packs -- <pack path filter> [search string]...

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::trinity::parse_index;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = args[0].to_lowercase();
    let needles: Vec<&[u8]> = args[1..].iter().map(|s| s.as_bytes()).collect();
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &keys,
    )
    .unwrap();
    let index = parse_index(game.raw_index()).unwrap();
    let packs: Vec<(usize, &String)> = index.pack_paths.iter().enumerate().filter(|(_, p)| p.to_lowercase().contains(&filter)).collect();
    println!("{} packs match {filter:?}", packs.len());
    if needles.is_empty() {
        for (_, p) in packs.iter().take(60) {
            println!("  {p}");
        }
        return;
    }
    for (i, path) in packs {
        for entry in index.entries.iter().filter(|e| e.pack == i as u64) {
            let Ok(bytes) = game.archive().read_hash(entry.hash) else { continue };
            for needle in &needles {
                if let Some(at) = bytes.windows(needle.len()).position(|w| w == *needle) {
                    println!("{path} file {:016X} ({} bytes): {} at {at:#x}", entry.hash, bytes.len(), String::from_utf8_lossy(needle));
                }
            }
        }
    }
}
