use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use svr_core::container::Keys;
use svr_core::data::store::CATALOG;
use svr_core::data::{Dex, TableStore};
use svr_core::game::{Edition, GameFiles, GamePaths};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let games = PathBuf::from(&a[2]);
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &Keys::load(Path::new(&a[1])).unwrap(),
    )
    .unwrap();
    let mut store = TableStore::new(&game);
    let dex = Dex::load(&mut store).unwrap();
    let mut genders: BTreeMap<(u16, u16), Vec<i64>> = BTreeMap::new();
    for e in store.get(CATALOG).unwrap().tables("Table") {
        let i = e.table("Info").unwrap();
        genders.entry((i.int("SpeciesInternal").unwrap() as u16, i.int("Form").unwrap() as u16)).or_default().push(i.int("Gender").unwrap());
    }
    let mut by_group: BTreeMap<u8, BTreeMap<Vec<i64>, usize>> = BTreeMap::new();
    for f in dex.playable() {
        if let Some(g) = genders.get(&(f.id.species, f.id.form)) {
            *by_group.entry(f.gender_group).or_default().entry(g.clone()).or_default() += 1;
        }
    }
    println!("catalog Gender values per personal gender group (group: {{gender list: forms}}):");
    for (group, m) in by_group {
        println!("  group {group}: {m:?}");
    }
    let v = dex.all().iter().find(|f| f.national == 494).unwrap();
    println!("Victini gender group {} ratio {}; Pikachu entries {:?}", v.gender_group, v.gender_ratio, genders.get(&(25, 0)));
    for (name, n) in [("Magnemite", 81), ("Ditto", 132), ("Tauros", 128), ("Mew", 151)] {
        let f = dex.all().iter().find(|f| f.national == n && f.id.form == 0).unwrap();
        println!("  {name}: group {} entries {:?}", f.gender_group, genders.get(&(f.id.species, 0)));
    }
}
