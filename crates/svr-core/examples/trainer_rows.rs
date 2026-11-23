//! Prints the vanilla team of every trainer row whose trid starts with a prefix.
//! cargo run -p svr-core --release --example trainer_rows -- <trid prefix>...

use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::data::store::load_table;
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let prefixes: Vec<String> = std::env::args().skip(1).collect();
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(&GamePaths::find(&games, Edition::Scarlet).expect("Scarlet files in SVR_TEST_GAMES"), &keys).unwrap();
    let table = load_table(&game, "world/data/trainer/trdata/trdata_array.bin").unwrap();
    for row in table.tables("values") {
        let trid = row.string("trid").unwrap_or_default();
        if !prefixes.iter().any(|p| trid.starts_with(p.as_str())) {
            continue;
        }
        let team: Vec<String> = (1..=6)
            .filter_map(|k| row.table(&format!("poke{k}")))
            .filter(|p| p.int("devId").unwrap_or(0) != 0)
            .map(|p| {
                let get = |f: &str| p.int(f).unwrap_or(0);
                format!("dev {}/{} Lv{} rare {} sex {}", get("devId"), get("formId"), get("level"), get("rareType"), get("sex"))
            })
            .collect();
        let flags: Vec<String> = ["changeGem", "aiDouble"].iter().map(|f| format!("{f}={}", row.bool(f).unwrap_or(false))).collect();
        println!("{trid} [{} battleType={}]: {}", flags.join(" "), row.int("battleType").unwrap_or(0), team.join(", "));
    }
}
