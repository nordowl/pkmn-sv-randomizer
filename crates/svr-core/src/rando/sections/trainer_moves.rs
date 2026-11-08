//! "Better movesets": four moves a trainer's Pokémon can legally know, picked for strength.

use std::collections::BTreeMap;

use crate::data::store::{MOVES, PERSONAL};
use crate::data::{FormInfo, TableStore};
use crate::rando::{RandoError, Rng};

const STATUS: u8 = 0;
const PHYSICAL: u8 = 1;

#[derive(Clone, Copy, Debug)]
struct MoveInfo {
    ty: u8,
    category: u8,
    power: u8,
    accuracy: u8,
    slow: bool,
}

struct Learnset {
    levelup: Vec<(i64, u16)>,
    /// TM, egg and reminder moves.
    other: Vec<u16>,
}

/// Usable moves and every species row's learnable moves.
pub struct MoveBook {
    moves: BTreeMap<u16, MoveInfo>,
    learnsets: Vec<Learnset>,
}

impl MoveBook {
    pub fn load(store: &mut TableStore) -> Result<MoveBook, RandoError> {
        let mut moves = BTreeMap::new();
        for (i, row) in store.get(MOVES)?.tables("Table").enumerate() {
            let int = |f: &str| row.int(f).unwrap_or(0);
            let flag = |f: &str| row.bool(f).unwrap_or(false);
            let id = u16::try_from(row.int("MoveID").unwrap_or(i as i64)).unwrap_or(0);
            if id == 0 || !flag("CanUseMove") {
                continue;
            }
            moves.insert(
                id,
                MoveInfo {
                    ty: int("Type") as u8,
                    category: int("Category") as u8,
                    power: int("Power") as u8,
                    accuracy: int("Accuracy") as u8,
                    slow: flag("Flag_Charge") || flag("Flag_Recharge"),
                },
            );
        }
        let mut learnsets = Vec::new();
        for entry in store.get(PERSONAL)?.tables("entry") {
            let ids = |field: &str| -> Vec<u16> {
                entry
                    .vector(field)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|v| v.as_uint().map(|u| u as u16))
                    .collect()
            };
            let mut other = ids("tm_moves");
            other.extend(ids("egg_moves"));
            other.extend(ids("reminder_moves"));
            let levelup = entry
                .vector("levelup_moves")
                .unwrap_or(&[])
                .iter()
                .filter_map(|m| m.as_struct())
                .map(|m| (m.int("level").unwrap_or(0), m.int("move").unwrap_or(0) as u16))
                .collect();
            learnsets.push(Learnset { levelup, other });
        }
        Ok(MoveBook { moves, learnsets })
    }

    /// Moves `form` can know at `level`: level-up moves up to that level, plus TM, egg and
    /// reminder moves. Those last three have no level, so attacks among them are capped by power
    /// to keep early trainers from opening with Draco Meteor.
    fn learnable(&self, form: &FormInfo, level: i64) -> Vec<u16> {
        let Some(set) = self.learnsets.get(form.row) else {
            return Vec::new();
        };
        let cap = 40 + 2 * level;
        let mut out: Vec<u16> = set.levelup.iter().filter(|(l, _)| *l <= level).map(|&(_, m)| m).collect();
        out.extend(set.other.iter().filter(|m| self.moves.get(m).is_some_and(|i| i64::from(i.power) <= cap)));
        out.sort_unstable();
        out.dedup();
        out.retain(|m| self.moves.contains_key(m));
        out
    }

    fn score(&self, form: &FormInfo, id: u16) -> u32 {
        let m = self.moves[&id];
        let accuracy = u32::from(m.accuracy.clamp(1, 100));
        let mut score = u32::from(m.power) * accuracy;
        if form.types.contains(&m.ty) {
            score = score * 3 / 2;
        }
        let physical_side = form.stats[1] >= form.stats[3];
        if (m.category == PHYSICAL) != physical_side {
            score = score * 3 / 4;
        }
        if m.slow {
            score /= 2;
        }
        score
    }

    /// Two strong same-type attacks where possible, a coverage attack, and a status move. None
    /// when the species knows no usable attack, in which case the game's own choice is kept.
    pub fn better_moveset(&self, form: &FormInfo, level: i64, rng: &mut Rng) -> Option<[u16; 4]> {
        let learnable = self.learnable(form, level);
        let mut attacks: Vec<u16> = learnable
            .iter()
            .copied()
            .filter(|m| self.moves[m].category != STATUS && self.moves[m].power > 0)
            .collect();
        if attacks.is_empty() {
            return None;
        }
        attacks.sort_by_key(|&m| std::cmp::Reverse((self.score(form, m), m)));
        let mut chosen: Vec<u16> = Vec::new();
        let mut covered: Vec<u8> = Vec::new();
        for &stab in &form.types {
            if covered.contains(&stab) {
                continue;
            }
            if let Some(&m) = attacks.iter().find(|&&m| self.moves[&m].ty == stab) {
                chosen.push(m);
                covered.push(stab);
            }
        }
        if let Some(&m) = attacks.iter().find(|&&m| !covered.contains(&self.moves[&m].ty)) {
            chosen.push(m);
            covered.push(self.moves[&m].ty);
        }
        let status: Vec<u16> = learnable.iter().copied().filter(|m| self.moves[m].category == STATUS).collect();
        if let Some(&m) = rng.pick(&status) {
            chosen.push(m);
        }
        for &m in &attacks {
            if chosen.len() >= 4 {
                break;
            }
            if !chosen.contains(&m) {
                chosen.push(m);
            }
        }
        chosen.truncate(4);
        let mut out = [0u16; 4];
        out[..chosen.len()].copy_from_slice(&chosen);
        Some(out)
    }
}
