//! Maps the archive reads in Eden logs to pack names and prints the packs one log read after a
//! given time that another log never read.
//! cargo run -p svr-core --release --example eden_log_packs -- <bad log> <good log> <bad log start seconds>

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::trinity::parse_index;

fn reads(log: &Path) -> Vec<(f64, u64, u64)> {
    let text = String::from_utf8_lossy(&std::fs::read(log).unwrap()).into_owned();
    text.lines()
        .filter_map(|l| {
            let t: f64 = l.get(1..l.find(']')?)?.trim().parse().ok()?;
            let o = l.find("offset=0X")? + 9;
            let off = u64::from_str_radix(&l[o..o + l[o..].find(',')?], 16).ok()?;
            let n = l.find("length=")? + 7;
            let len = l[n..].trim().parse().ok()?;
            Some((t, off, len))
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (bad, good, after) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]), args[2].parse::<f64>().unwrap());
    let games = PathBuf::from(std::env::var_os("SVR_TEST_GAMES").unwrap());
    let keys = Keys::load(Path::new(&std::env::var_os("SVR_TEST_KEYS").unwrap())).unwrap();
    let game = GameFiles::open(
        &GamePaths::find(&games, Edition::Scarlet).expect("the Scarlet base XCI and update NSP are in SVR_TEST_GAMES"),
        &keys,
    )
    .unwrap();
    let names = parse_index(game.raw_index()).unwrap().pack_paths;
    let extents = game.archive().pack_extents();
    let by_len: BTreeMap<u64, Vec<usize>> = extents.iter().enumerate().fold(BTreeMap::new(), |mut m, (i, &(_, len))| {
        m.entry(len).or_default().push(i);
        m
    });
    let bad_reads = reads(&bad);
    let good_reads = reads(&good);
    let mut bases: BTreeMap<u64, usize> = BTreeMap::new();
    for &(_, off, len) in bad_reads.iter().chain(&good_reads) {
        for &i in by_len.get(&len).into_iter().flatten() {
            if let Some(base) = off.checked_sub(extents[i].0) {
                *bases.entry(base).or_default() += 1;
            }
        }
    }
    let (&base, &votes) = bases.iter().max_by_key(|(_, &v)| v).unwrap();
    println!("data.trpfs base {base:#x} ({votes} reads agree); {} packs", extents.len());
    let starts: BTreeMap<u64, usize> = extents.iter().enumerate().map(|(i, &(s, _))| (base + s, i)).collect();
    let pack_at = |off: u64| starts.range(..=off).next_back().map(|(_, &i)| i).filter(|&i| off < base + extents[i].0 + extents[i].1);
    let good_packs: BTreeSet<usize> = good_reads.iter().filter_map(|&(_, o, _)| pack_at(o)).collect();
    let mut shown = BTreeSet::new();
    let mut unknown = 0;
    for &(t, off, len) in bad_reads.iter().filter(|r| r.0 >= after) {
        match pack_at(off) {
            Some(i) if !good_packs.contains(&i) && shown.insert(i) => {
                println!("{t:9.3}s {len:>9} {}", names.get(i).map_or("?", String::as_str))
            }
            None => unknown += 1,
            _ => {}
        }
    }
    println!("{unknown} reads after {after}s fall outside the archive (loose mod files or other romfs files)");
}
