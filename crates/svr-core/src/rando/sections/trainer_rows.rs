//! Trainer rows as plain values. Sections read a snapshot, change copies, and write back only the
//! fields that differ, so untouched rows keep their exact bytes.

use crate::data::Pokemon;
use crate::fb::{FieldError, TableValue};

pub const TRDATA: &str = "world/data/trainer/trdata/trdata_array.bin";
pub const TRTYPE: &str = "world/data/trainer/trtype/trtype_array.bin";
pub const SLOTS: usize = 6;

const STAT_FIELDS: [&str; 6] = ["hp", "atk", "def", "spAtk", "spDef", "agi"];

// Enum values from the trainer table's schema.
pub const BATTLE_DOUBLES: i64 = 1;
pub const WAZA_MANUAL: i64 = 1;
pub const TALENT_VALUE: i64 = 2;
pub const RARE: i64 = 2;
pub const SEX_DEFAULT: i64 = 0;
pub const GEM_DEFAULT: i64 = 0;
/// `gemType` of type id 0 (Normal); the others follow in type order.
pub const GEM_FIRST_TYPE: i64 = 2;
/// `tokusei` values that fix ability slot 1, 2 or the hidden ability.
pub const TOKUSEI_SET: [i64; 3] = [2, 3, 4];
pub const TOKUSEI_RANDOM_12: i64 = 0;
pub const TOKUSEI_RANDOM_123: i64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mon {
    pub species: Pokemon,
    pub level: i64,
    pub item: i64,
    pub gem: i64,
    pub tokusei: i64,
    pub sex: i64,
    pub rare: i64,
    pub manual_moves: bool,
    pub moves: [u16; 4],
    pub talent_type: i64,
    pub talent: [i64; 6],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ai {
    pub basic: bool,
    pub high: bool,
    pub expert: bool,
    pub double: bool,
    pub weak: bool,
    pub item: bool,
    pub change: bool,
}

const AI_FIELDS: [&str; 7] = ["aiBasic", "aiHigh", "aiExpert", "aiDouble", "aiWeak", "aiItem", "aiChange"];

impl Ai {
    fn flags(&self) -> [bool; 7] {
        [self.basic, self.high, self.expert, self.double, self.weak, self.item, self.change]
    }

    fn from_flags(f: [bool; 7]) -> Ai {
        Ai {
            basic: f[0],
            high: f[1],
            expert: f[2],
            double: f[3],
            weak: f[4],
            item: f[5],
            change: f[6],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trainer {
    pub index: usize,
    pub trid: String,
    pub label: String,
    pub class: String,
    pub doubles: bool,
    pub change_gem: bool,
    pub ai: Ai,
    /// Present slots in order, starting at `poke1`.
    pub team: Vec<Mon>,
    /// False when an empty slot sits before a filled one; such rows are never edited.
    pub contiguous: bool,
}

fn int(t: &TableValue, field: &str) -> i64 {
    t.int(field).unwrap_or(0)
}

fn read_mon(p: &TableValue) -> Mon {
    let mut moves = [0u16; 4];
    for (k, slot) in moves.iter_mut().enumerate() {
        *slot = p.table(&format!("waza{}", k + 1)).map_or(0, |w| int(w, "wazaId") as u16);
    }
    let mut talent = [0i64; 6];
    if let Some(tv) = p.table("talentValue") {
        for (slot, name) in talent.iter_mut().zip(STAT_FIELDS) {
            *slot = int(tv, name);
        }
    }
    Mon {
        species: Pokemon {
            species: int(p, "devId") as u16,
            form: int(p, "formId") as u16,
        },
        level: int(p, "level"),
        item: int(p, "item"),
        gem: int(p, "gemType"),
        tokusei: int(p, "tokusei"),
        sex: int(p, "sex"),
        rare: int(p, "rareType"),
        manual_moves: int(p, "wazaType") == WAZA_MANUAL,
        moves,
        talent_type: int(p, "talentType"),
        talent,
    }
}

pub fn read(index: usize, row: &TableValue) -> Trainer {
    let mut team = Vec::new();
    let mut contiguous = true;
    let mut gap = false;
    for k in 1..=SLOTS {
        let present = row.table(&format!("poke{k}")).filter(|p| int(p, "devId") != 0);
        match present {
            Some(p) if gap => {
                contiguous = false;
                team.push(read_mon(p));
            }
            Some(p) => team.push(read_mon(p)),
            None => gap = true,
        }
    }
    let flags = AI_FIELDS.map(|f| row.bool(f).unwrap_or(false));
    Trainer {
        index,
        trid: row.string("trid").unwrap_or_default().to_owned(),
        label: row.string("trNameLabel").unwrap_or_default().to_owned(),
        class: row.string("trainerType").unwrap_or_default().to_owned(),
        doubles: int(row, "battleType") == BATTLE_DOUBLES,
        change_gem: row.bool("changeGem").unwrap_or(false),
        ai: Ai::from_flags(flags),
        team,
        contiguous,
    }
}

fn write_mon(p: &mut TableValue, old: &Mon, new: &Mon) -> Result<(), FieldError> {
    if old.species != new.species {
        p.set_int("devId", i64::from(new.species.species))?;
        p.set_int("formId", i64::from(new.species.form))?;
    }
    let scalars = [
        ("level", old.level, new.level),
        ("item", old.item, new.item),
        ("gemType", old.gem, new.gem),
        ("tokusei", old.tokusei, new.tokusei),
        ("sex", old.sex, new.sex),
        ("rareType", old.rare, new.rare),
        ("talentType", old.talent_type, new.talent_type),
    ];
    for (field, before, after) in scalars {
        if before != after {
            p.set_int(field, after)?;
        }
    }
    if old.manual_moves != new.manual_moves {
        p.set_int("wazaType", if new.manual_moves { WAZA_MANUAL } else { 0 })?;
    }
    for k in 0..4 {
        if old.moves[k] != new.moves[k] {
            p.table_mut(&format!("waza{}", k + 1))?.set_int("wazaId", i64::from(new.moves[k]))?;
        }
    }
    if old.talent != new.talent {
        let tv = p.table_mut("talentValue")?;
        for (name, v) in STAT_FIELDS.iter().zip(new.talent) {
            tv.set_int(name, v)?;
        }
    }
    Ok(())
}

/// Writes the differences between `old` and `new` into `row`. A slot past the old team is first
/// filled with a copy of the old team's last slot, so ball, size and EVs look like its teammates.
pub fn write(row: &mut TableValue, old: &Trainer, new: &Trainer) -> Result<(), FieldError> {
    if old.class != new.class {
        row.set_string("trainerType", new.class.clone())?;
    }
    if old.doubles != new.doubles {
        row.set_int("battleType", if new.doubles { BATTLE_DOUBLES } else { 0 })?;
    }
    if old.change_gem != new.change_gem {
        row.set_bool("changeGem", new.change_gem)?;
    }
    for ((field, before), after) in AI_FIELDS.iter().zip(old.ai.flags()).zip(new.ai.flags()) {
        if before != after {
            row.set_bool(field, after)?;
        }
    }
    let template_slot = old.team.len().max(1);
    let template_table = row.table(&format!("poke{template_slot}")).cloned();
    for (k, mon) in new.team.iter().enumerate() {
        let field = format!("poke{}", k + 1);
        let before = match old.team.get(k) {
            Some(before) => before.clone(),
            None => {
                let template = old.team.last().expect("rows with Pokémon only");
                if let Some(table) = &template_table {
                    *row.table_mut(&field)? = table.clone();
                }
                template.clone()
            }
        };
        if &before != mon {
            write_mon(row.table_mut(&field)?, &before, mon)?;
        }
    }
    Ok(())
}
