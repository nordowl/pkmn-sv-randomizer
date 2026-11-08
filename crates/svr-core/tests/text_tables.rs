use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

use svr_core::container::Keys;
use svr_core::game::{Edition, GameFiles, GamePaths};
use svr_core::text::{GameText, Language, LabelTable, TextError, TextFile};

const GAMES: [(&str, Edition); 2] = [("Scarlet", Edition::Scarlet), ("Violet", Edition::Violet)];

const CANDIDATE_NAMES: [&str; 22] = [
    "monsname",
    "wazaname",
    "itemname",
    "tokusei",
    "typename",
    "trname",
    "trtype",
    "seikaku",
    "tokuseiinfo",
    "iteminfo",
    "wazainfo",
    "trmsg",
    "place_name",
    "zkn_form",
    "zkn_type",
    "zukan_comment_A",
    "zukan_comment_B",
    "ribbon",
    "poke_memory_feeling",
    "monsname_en",
    "pokemon_type",
    "trainer_name",
];

static OPENED: [OnceLock<GameFiles>; 2] = [const { OnceLock::new() }; 2];

fn game(case: usize) -> Option<&'static GameFiles> {
    let keys = env::var_os("SVR_TEST_KEYS").map(PathBuf::from);
    let dir = env::var_os("SVR_TEST_GAMES").map(PathBuf::from);
    let (Some(keys), Some(dir)) = (keys, dir) else {
        println!("SKIP: set SVR_TEST_KEYS and SVR_TEST_GAMES to run the real-game text tests");
        return None;
    };
    Some(OPENED[case].get_or_init(|| {
        let paths = GamePaths::find(&dir, GAMES[case].1).expect("the base XCI and update NSP are in SVR_TEST_GAMES");
        GameFiles::open(&paths, &Keys::load(&keys).expect("load keys")).expect("open game")
    }))
}

#[test]
fn every_common_file_round_trips_byte_exactly() {
    for (case, (name, _)) in GAMES.iter().enumerate() {
        let Some(game) = game(case) else { return };
        let (mut dats, mut tbls, mut lines, mut labels) = (0, 0, 0, 0);
        for lang in Language::ALL {
            let mut found = Vec::new();
            for file in CANDIDATE_NAMES {
                let dat_path = lang.dat_path(file);
                if !game.contains(&dat_path) {
                    assert!(!game.contains(&lang.tbl_path(file)), "{file}.tbl without .dat");
                    continue;
                }
                let raw = game.read(&dat_path).expect("read dat");
                let parsed = TextFile::parse(&raw).expect("parse dat");
                assert_eq!(parsed.to_bytes(), raw, "{name} {}", dat_path);
                dats += 1;
                lines += parsed.len();

                let tbl_path = lang.tbl_path(file);
                if game.contains(&tbl_path) {
                    let raw = game.read(&tbl_path).expect("read tbl");
                    let table = LabelTable::parse(&raw).expect("parse tbl");
                    assert_eq!(table.to_bytes(), raw, "{name} {tbl_path}");
                    assert!(table.len() == parsed.len() || table.len() == parsed.len() + 1, "{tbl_path} label count {} vs {}", table.len(), parsed.len());
                    for entry in table.entries() {
                        assert_eq!(entry.hash, LabelTable::label_hash(&entry.name), "{tbl_path}");
                    }
                    tbls += 1;
                    labels += table.len();
                    found.push(format!("{file}(+tbl)"));
                } else {
                    found.push(file.to_owned());
                }
            }
            println!("FOUND {name} {:<12} {}", lang.folder(), found.join(" "));
        }
        println!("OK {name}: {dats} .dat and {tbls} .tbl round-trip byte-exactly ({lines} lines, {labels} labels)");
    }
}

#[test]
fn empty_label_hashes_to_the_standard_basis() {
    assert_eq!(LabelTable::label_hash(""), 14695981039346656837);
}

#[test]
fn editing_a_trainer_name_changes_only_that_line() {
    for (case, (name, _)) in GAMES.iter().enumerate() {
        let Some(game) = game(case) else { return };
        let text = GameText::load(game, Language::English, "trname").expect("load trname");
        let index = text.labels().expect("trname labels").index_of("TRNAME_NEMO").expect("TRNAME_NEMO");
        let before = text.file().clone();
        let original = before.line(index).expect("line");

        for replacement in ["Nemo", "A Much Longer Trainer Name Than Nemona Ever Had"] {
            let mut edited = before.clone();
            edited.set_line(index, replacement).expect("set_line");
            let bytes = edited.to_bytes();
            let reparsed = TextFile::parse(&bytes).expect("reparse");
            assert_eq!(reparsed.len(), before.len());
            for i in 0..before.len() {
                if i == index {
                    assert_eq!(reparsed.line(i).as_deref(), Some(replacement));
                } else {
                    assert_eq!(reparsed.line(i), before.line(i), "line {i}");
                }
                assert_eq!(reparsed.flags(i), before.flags(i));
            }
            assert_eq!(reparsed.to_bytes(), bytes);
        }
        println!("OK {}: edited trname line {index} (was {original:?}), shorter and longer names", name);
    }
}

#[test]
fn english_names_resolve() {
    for (case, (name, _)) in GAMES.iter().enumerate() {
        let Some(game) = game(case) else { return };
        let mons = GameText::load(game, Language::English, "monsname").expect("monsname");
        assert_eq!(mons.line(25).as_deref(), Some("Pikachu"));
        assert_eq!(mons.line(906).as_deref(), Some("Sprigatito"));
        assert_eq!(mons.by_label("MONSNAME_1022").as_deref(), Some("Pecharunt"));
        let moves = GameText::load(game, Language::English, "wazaname").expect("wazaname");
        assert_eq!(moves.file().len(), 920);
        assert_eq!(moves.labels().map(|l| l.len()), Some(921));
        let trainers = GameText::load(game, Language::English, "trname").expect("trname");
        assert_eq!(trainers.by_label("TRNAME_NEMO").as_deref(), Some("Nemona"));
        assert_eq!(trainers.by_label("TRNAME_DOES_NOT_EXIST"), None);
        let german = GameText::load(game, Language::German, "monsname").expect("german monsname");
        assert_eq!(german.line(25).as_deref(), Some("Pikachu"));
        assert_eq!(german.line(4).as_deref(), Some("Glumanda"));
        println!(
            "OK {}: monsname 25/906/1025 = {:?}/{:?}/{:?}, wazaname {} lines, TRNAME_NEMO = {:?}, German 4 = {:?}",
            name,
            mons.line(25),
            mons.line(906),
            mons.line(1025),
            moves.file().len(),
            trainers.by_label("TRNAME_NEMO"),
            german.line(4),
        );
    }
}

#[test]
fn readable_form_survives_set_line_byte_exactly() {
    for (case, (name, _)) in GAMES.iter().enumerate() {
        let Some(game) = game(case) else { return };
        let (mut files, mut lines, mut with_vars, mut unreadable) = (0, 0, 0, 0);
        let mut sample = None;
        for lang in Language::ALL {
            for name in CANDIDATE_NAMES {
                let path = lang.dat_path(name);
                if !game.contains(&path) {
                    continue;
                }
                let raw = game.read(&path).expect("read");
                let mut file = TextFile::parse(&raw).expect("parse");
                for index in 0..file.len() {
                    lines += 1;
                    match file.line(index) {
                        Some(text) => {
                            if text.contains("[VAR") {
                                with_vars += 1;
                                sample.get_or_insert_with(|| text.clone());
                            }
                            file.set_line(index, &text).expect("set_line");
                        }
                        None => unreadable += 1,
                    }
                }
                assert_eq!(file.to_bytes(), raw, "{path}");
                files += 1;
            }
        }
        assert_eq!(unreadable, 0, "lines that line() rejects");
        println!(
            "OK {}: {files} files, {lines} lines through line()/set_line() unchanged, {with_vars} with [VAR ..], e.g. {:?}",
            name, sample
        );
    }
}

fn synthetic(lines: &[(&[u16], u16)]) -> Vec<u8> {
    let mut section = vec![0u8; 4 + lines.len() * 8];
    for (index, (units, flags)) in lines.iter().enumerate() {
        let entry = 4 + index * 8;
        let offset = section.len() as u32;
        section[entry..entry + 4].copy_from_slice(&offset.to_le_bytes());
        section[entry + 4..entry + 6].copy_from_slice(&(units.len() as u16).to_le_bytes());
        section[entry + 6..entry + 8].copy_from_slice(&flags.to_le_bytes());
        let mut key = 0x7C89u16.wrapping_add(0x2983u16.wrapping_mul(index as u16));
        for unit in units.iter() {
            section.extend_from_slice(&(unit ^ key).to_le_bytes());
            key = key.rotate_left(3);
        }
        if section.len() % 4 == 2 {
            section.extend_from_slice(&[0, 0]);
        }
    }
    let total = section.len() as u32;
    section[..4].copy_from_slice(&total.to_le_bytes());
    let mut out = Vec::new();
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(lines.len() as u16).to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x10u32.to_le_bytes());
    out.extend_from_slice(&section);
    out
}

#[test]
fn variables_and_escapes_round_trip_on_synthetic_data() {
    let name: Vec<u16> = "Hi \\ [x]\n".encode_utf16().collect();
    let mut with_vars = name.clone();
    with_vars.extend([0x10, 2, 0x0100, 0x0000, 0x10, 1, 0xBE00, 0x10, 3, 0x0101, 0, 0xFFFF, 0]);
    let raw = synthetic(&[(&[0], 0), (&with_vars, 7), (&[b'A' as u16, 0], 1)]);

    let mut file = TextFile::parse(&raw).expect("parse");
    assert_eq!(file.to_bytes(), raw);
    let text = file.line(1).expect("readable");
    assert_eq!(text, "Hi \\\\ \\[x]\\n[VAR 0100(0000)]\\r[VAR 0101(0000,FFFF)]");
    file.set_line(1, &text).expect("set_line");
    assert_eq!(file.to_bytes(), raw);
    assert_eq!(file.line(0).as_deref(), Some(""));
    assert_eq!(file.line(3), None);
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    let raw = synthetic(&[(&[b'A' as u16, 0], 0), (&[0x10, 2, 0x0100, 0, 0], 0)]);
    for cut in 0..raw.len() {
        assert!(TextFile::parse(&raw[..cut]).is_err(), "cut at {cut}");
    }
    for at in 0..raw.len() {
        for value in [0x00, 0x7F, 0xFF] {
            let mut damaged = raw.clone();
            damaged[at] = value;
            if let Ok(file) = TextFile::parse(&damaged) {
                for i in 0..file.len() {
                    let _ = file.line(i);
                }
            }
        }
    }

    let mut file = TextFile::parse(&raw).expect("parse");
    for text in ["\\x", "[VAR", "[VAR 12345]", "[VAR 0100(zz)]", "[NOPE]", "a\0b", "a\\"] {
        assert!(matches!(file.set_line(0, text), Err(TextError::BadText(_))), "{text:?}");
    }
    assert!(matches!(file.set_line(9, "x"), Err(TextError::LineOutOfRange { index: 9 })));

    let tbl: Vec<u8> = [b"AHTB".as_slice(), &1i32.to_le_bytes(), &5u64.to_le_bytes(), &2u16.to_le_bytes(), b"A\0"].concat();
    let table = LabelTable::parse(&tbl).expect("parse tbl");
    assert_eq!(table.to_bytes(), tbl);
    for cut in 0..tbl.len() {
        assert!(LabelTable::parse(&tbl[..cut]).is_err(), "tbl cut at {cut}");
    }
}
