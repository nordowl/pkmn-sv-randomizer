//! Move data and lookups shared by sections.

use std::collections::BTreeSet;

use crate::data::store::{MOVES, PERSONAL};
use crate::data::Pokemon;
use crate::fb::FieldError;

use super::{Ctx, RandoError};

pub const METRONOME: u16 = 118;
pub const STRUGGLE: u16 = 165;
/// The accuracy the game stores for moves that never miss.
pub const NEVER_MISSES: u8 = 101;
/// Normal through Fairy. Stellar exists only for Terastallizing.
pub const TYPE_COUNT: u8 = 18;
pub const NORMAL: u8 = 0;

/// Moves that only work for their own species, so a randomizer never hands them out.
const SIGNATURE_ONLY: &[&str] = &["Dark Void", "Hyperspace Fury", "Aura Wheel"];

/// One-hit KOs and fixed early-game damage.
pub const GAME_BREAKING: &[&str] = &["Sonic Boom", "Dragon Rage", "Fissure", "Sheer Cold", "Horn Drill", "Guillotine"];

/// Damaging moves too situational to count as a Pokémon's attack: they faint the user, need a
/// setup, only work on the first turn, or ignore the user's own stats. UPR's list plus Gen 9.
pub const POOR_ATTACKS: &[&str] = &[
    "Self-Destruct",
    "Explosion",
    "Misty Explosion",
    "Final Gambit",
    "Dream Eater",
    "Snore",
    "False Swipe",
    "Hold Back",
    "Future Sight",
    "Doom Desire",
    "Fake Out",
    "First Impression",
    "Focus Punch",
    "Feint",
    "Last Resort",
    "Sucker Punch",
    "Thunderclap",
    "Upper Hand",
    "Constrict",
    "Rage",
    "Rollout",
    "Ice Ball",
    "Synchronoise",
    "Shell Trap",
    "Foul Play",
    "Spit Up",
    "Belch",
    "Poltergeist",
    "Steel Roller",
    "Sonic Boom",
    "Dragon Rage",
    "Horn Drill",
    "Guillotine",
    "Fissure",
    "Sheer Cold",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Status,
    Physical,
    Special,
}

impl Category {
    fn from_raw(v: i64) -> Category {
        match v {
            1 => Category::Physical,
            2 => Category::Special,
            _ => Category::Status,
        }
    }

    fn raw(self) -> i64 {
        match self {
            Category::Status => 0,
            Category::Physical => 1,
            Category::Special => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Status => "Status",
            Category::Physical => "Physical",
            Category::Special => "Special",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoveInfo {
    pub id: u16,
    pub usable: bool,
    pub type_id: u8,
    pub category: Category,
    /// 1 marks moves whose power the battle works out, like Low Kick.
    pub power: u8,
    pub accuracy: u8,
    pub pp: u8,
    pub hit_min: u8,
    pub hit_max: u8,
}

impl MoveInfo {
    pub fn damaging(&self) -> bool {
        self.category != Category::Status
    }

    pub fn expected_hits(&self) -> f64 {
        match (self.hit_min, self.hit_max) {
            (_, 0 | 1) => 1.0,
            (2, 5) => 19.0 / 6.0,
            (a, b) => f64::from(a + b) / 2.0,
        }
    }

    /// Power times expected hits; 0 for moves without a fixed power.
    pub fn strength(&self) -> f64 {
        if self.damaging() && self.power > 1 {
            f64::from(self.power) * self.expected_hits()
        } else {
            0.0
        }
    }

    /// UPR's test: strong, or decent and accurate.
    pub fn good_damaging(&self) -> bool {
        let s = self.strength();
        s >= 100.0 || (s >= 50.0 && self.accuracy >= 90)
    }
}

pub struct MoveTable {
    pub moves: Vec<MoveInfo>,
    before: Vec<MoveInfo>,
    /// Usable, nonzero, and neither Struggle nor a signature-only move.
    assignable: BTreeSet<u16>,
}

fn shape(e: FieldError) -> RandoError {
    RandoError::Section {
        section: "move table",
        detail: e.to_string(),
    }
}

/// Move IDs whose English name is in `list`.
pub fn ids_named(ctx: &Ctx, count: usize, list: &[&str]) -> BTreeSet<u16> {
    (0..count as u16).filter(|&id| list.contains(&ctx.names.move_name(id).as_str())).collect()
}

impl MoveTable {
    pub fn load(ctx: &mut Ctx) -> Result<MoveTable, RandoError> {
        let table = ctx.store.get(MOVES)?;
        let mut moves = Vec::new();
        for (i, row) in table.tables("Table").enumerate() {
            let small = |field: &str| row.int(field).map(|v| v.clamp(0, 255) as u8).map_err(shape);
            moves.push(MoveInfo {
                id: i as u16,
                usable: row.bool("CanUseMove").unwrap_or(false),
                type_id: small("Type")?,
                category: Category::from_raw(row.int("Category").map_err(shape)?),
                power: small("Power")?,
                accuracy: small("Accuracy")?,
                pp: small("PP")?,
                hit_min: small("HitMin")?,
                hit_max: small("HitMax")?,
            });
        }
        let blocked = ids_named(ctx, moves.len(), SIGNATURE_ONLY);
        let assignable = moves
            .iter()
            .filter(|m| m.usable && m.id != 0 && m.id != STRUGGLE && !blocked.contains(&m.id))
            .map(|m| m.id)
            .collect();
        Ok(MoveTable {
            before: moves.clone(),
            moves,
            assignable,
        })
    }

    pub fn get(&self, id: u16) -> Option<&MoveInfo> {
        self.moves.get(usize::from(id))
    }

    pub fn before(&self, id: u16) -> Option<&MoveInfo> {
        self.before.get(usize::from(id))
    }

    /// Moves a randomizer may hand out.
    pub fn assignable(&self) -> &BTreeSet<u16> {
        &self.assignable
    }

    pub fn changed(&self) -> impl Iterator<Item = u16> + '_ {
        self.moves.iter().zip(&self.before).filter(|(a, b)| a != b).map(|(a, _)| a.id)
    }

    pub fn save(&self, ctx: &mut Ctx) -> Result<(), RandoError> {
        if self.changed().next().is_none() {
            return Ok(());
        }
        let table = ctx.store.get_mut(MOVES)?;
        for (row, (new, old)) in table.tables_mut("Table").zip(self.moves.iter().zip(&self.before)) {
            if new == old {
                continue;
            }
            row.set_int("Type", i64::from(new.type_id)).map_err(shape)?;
            row.set_int("Category", new.category.raw()).map_err(shape)?;
            row.set_int("Power", i64::from(new.power)).map_err(shape)?;
            row.set_int("Accuracy", i64::from(new.accuracy)).map_err(shape)?;
            row.set_int("PP", i64::from(new.pp)).map_err(shape)?;
        }
        Ok(())
    }
}

/// The last four level-up moves `p` knows at `level`, in learn order, as the game would pick them.
pub fn level_up_moves_at(ctx: &mut Ctx, p: Pokemon, level: u16) -> Result<Vec<u16>, RandoError> {
    let Some(row) = ctx.dex.get(p).map(|f| f.row) else {
        return Ok(Vec::new());
    };
    let personal = ctx.store.get(PERSONAL)?;
    let Some(entry) = personal.tables("entry").nth(row) else {
        return Ok(Vec::new());
    };
    let mut learned: Vec<u16> = Vec::new();
    for m in entry.vector("levelup_moves").unwrap_or(&[]) {
        let Some(m) = m.as_struct() else { continue };
        let learn_level = m.int("level").unwrap_or(0);
        let id = m.int("move").unwrap_or(0) as u16;
        if learn_level <= i64::from(level) && id != 0 && !learned.contains(&id) {
            learned.push(id);
        }
    }
    let start = learned.len().saturating_sub(4);
    Ok(learned[start..].to_vec())
}
