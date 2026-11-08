//! Random learnsets: level-up, egg and reminder moves, after UPR's same-type weighting and pkNX's
//! learnset-size options.

use std::collections::BTreeSet;

use crate::rando::movepool::{Category, MoveTable, NORMAL, TYPE_COUNT};
use crate::rando::Rng;

use super::super::pokemon::table::{LevelUpMove, Row, ON_EVOLUTION};

/// The moves a learnset may get, grouped the ways the picker asks for them.
pub struct MovePool<'a> {
    table: &'a MoveTable,
    all: Vec<u16>,
    /// Good damaging moves that are not too situational to rely on.
    good: Vec<u16>,
    by_type: Vec<Vec<u16>>,
    good_by_type: Vec<Vec<u16>>,
    attacks_by_type: Vec<Vec<u16>>,
}

#[derive(Clone, Copy)]
pub struct Pick {
    /// Prefer these types the way UPR does; None picks from every type.
    pub types: Option<[u8; 2]>,
    pub damaging: bool,
    /// Physical or special, for damaging picks.
    pub physical: bool,
}

impl<'a> MovePool<'a> {
    pub fn new(table: &'a MoveTable, banned: &BTreeSet<u16>, poor: &BTreeSet<u16>) -> MovePool<'a> {
        let all: Vec<u16> = table.assignable().iter().copied().filter(|m| !banned.contains(m)).collect();
        let info = |m: u16| table.get(m).expect("assignable moves exist");
        let good: Vec<u16> = all.iter().copied().filter(|&m| info(m).good_damaging() && !poor.contains(&m)).collect();
        let group = |list: &[u16]| {
            let mut out = vec![Vec::new(); usize::from(TYPE_COUNT)];
            for &m in list {
                if let Some(slot) = out.get_mut(usize::from(info(m).type_id)) {
                    slot.push(m);
                }
            }
            out
        };
        let attacks: Vec<u16> = all.iter().copied().filter(|&m| info(m).strength() > 0.0).collect();
        MovePool {
            table,
            by_type: group(&all),
            good_by_type: group(&good),
            attacks_by_type: group(&attacks),
            all,
            good,
        }
    }

    pub fn all(&self) -> &[u16] {
        &self.all
    }

    pub fn good(&self) -> &[u16] {
        &self.good
    }

    pub fn table(&self) -> &MoveTable {
        self.table
    }

    /// UPR's odds: a Normal dual type gets Normal 10% and its other type 30%; other dual types get
    /// each type 20%; a single type gets its type 40%. The rest of the time any type is fine.
    fn roll_type(rng: &mut Rng, types: [u8; 2]) -> Option<u8> {
        let roll = rng.below(100);
        let [a, b] = types;
        if a == b {
            return (roll < 40).then_some(a);
        }
        if a == NORMAL || b == NORMAL {
            let other = if a == NORMAL { b } else { a };
            return if roll < 10 {
                Some(NORMAL)
            } else if roll < 40 {
                Some(other)
            } else {
                None
            };
        }
        if roll < 20 {
            Some(a)
        } else if roll < 40 {
            Some(b)
        } else {
            None
        }
    }

    pub fn pick(&self, rng: &mut Rng, how: Pick, taken: &BTreeSet<u16>) -> u16 {
        let free = |list: &[u16]| list.iter().copied().filter(|m| !taken.contains(m)).collect::<Vec<_>>();
        let rolled = how.types.and_then(|t| Self::roll_type(rng, t)).map(usize::from);
        let mut options = Vec::new();
        if how.damaging {
            if let Some(t) = rolled {
                options = free(&self.good_by_type[t]);
            }
            if options.is_empty() {
                options = free(&self.good);
            }
            let wanted = if how.physical { Category::Physical } else { Category::Special };
            let narrowed: Vec<u16> = options
                .iter()
                .copied()
                .filter(|&m| self.table.get(m).is_some_and(|i| i.category == wanted))
                .collect();
            if !narrowed.is_empty() {
                options = narrowed;
            }
        } else if let Some(t) = rolled {
            options = free(&self.by_type[t]);
        }
        if options.is_empty() {
            options = free(&self.all);
        }
        if options.is_empty() {
            options = self.all.clone();
        }
        options[rng.index(options.len())]
    }

    /// A damaging move of one of `types`, or None if every such move is taken.
    pub fn pick_stab(&self, rng: &mut Rng, types: [u8; 2], taken: &BTreeSet<u16>) -> Option<u16> {
        let mut order = vec![types[0]];
        if types[1] != types[0] {
            order.insert(rng.index(2), types[1]);
        }
        for t in order.into_iter().map(usize::from) {
            for list in [&self.good_by_type[t], &self.attacks_by_type[t]] {
                let free: Vec<u16> = list.iter().copied().filter(|m| !taken.contains(m)).collect();
                if let Some(&m) = rng.pick(&free) {
                    return Some(m);
                }
            }
        }
        None
    }

    pub fn of_type(&self, t: u8) -> &[u16] {
        self.by_type.get(usize::from(t)).map_or(&[], Vec::as_slice)
    }
}

pub struct LevelUpRules {
    pub prefer_type: bool,
    pub guaranteed_level1: usize,
    pub good_percent: u32,
    pub add_evolution_move: bool,
    pub min_damaging: usize,
    pub min_stab: usize,
    pub expand_to: usize,
    /// 0 keeps the levels.
    pub spread_to: u8,
}

fn physical_roll(rng: &mut Rng, row: &Row) -> bool {
    let atk = u64::from(row.stats[1]);
    let spa = u64::from(row.stats[3]);
    rng.chance(atk, (atk + spa).max(1))
}

/// The levels of the new learnset: on-evolution slots first, then ascending levels.
fn new_levels(row: &Row, rules: &LevelUpRules) -> (usize, Vec<i8>) {
    let mut evolution = row.levelup.iter().filter(|m| m.level == ON_EVOLUTION).count();
    if rules.add_evolution_move && evolution == 0 {
        evolution = 1;
    }
    let mut regular: Vec<i8> = row.levelup.iter().filter(|m| m.level != ON_EVOLUTION).map(|m| m.level).collect();
    let ones = regular.iter().filter(|&&l| l <= 1).count();
    for _ in ones..rules.guaranteed_level1 {
        regular.insert(0, 1);
    }
    if regular.len() < rules.expand_to {
        let last = i32::from(regular.last().copied().unwrap_or(1));
        let added = rules.expand_to - regular.len();
        let gap = ((100 - last) / (added as i32 + 1)).max(1);
        for k in 1..=added as i32 {
            regular.push((last + gap * k).min(100) as i8);
        }
    }
    if rules.spread_to > 0 && !regular.is_empty() {
        let n = regular.len() as i32;
        let top = i32::from(rules.spread_to.max(1));
        for (k, level) in regular.iter_mut().enumerate() {
            *level = if n == 1 { 1 } else { (1 + k as i32 * (top - 1) / (n - 1)) as i8 };
        }
        for level in regular.iter_mut().take(rules.guaranteed_level1) {
            *level = 1;
        }
    }
    regular.sort_unstable();
    (evolution, regular)
}

pub fn randomize_levelup(rng: &mut Rng, pool: &MovePool, row: &Row, rules: &LevelUpRules) -> Vec<LevelUpMove> {
    let (evolution, regular) = new_levels(row, rules);
    let levels: Vec<i8> = std::iter::repeat_n(ON_EVOLUTION, evolution).chain(regular.iter().copied()).collect();
    let total = levels.len();
    if total == 0 {
        return Vec::new();
    }
    let first_attack = levels
        .iter()
        .rposition(|&l| l == 1)
        .or_else(|| (evolution < total).then_some(evolution));
    let types = rules.prefer_type.then_some(row.types);
    let mut good_left = (f64::from(rules.good_percent) * total as f64 / 100.0).round() as usize;
    let mut taken = BTreeSet::new();
    let mut moves = Vec::with_capacity(total);
    for k in 0..total {
        let attack = first_attack == Some(k) || good_left > 0;
        let physical = physical_roll(rng, row);
        let m = pool.pick(rng, Pick { types, damaging: attack, physical }, &taken);
        if first_attack != Some(k) {
            good_left = good_left.saturating_sub(1);
        }
        taken.insert(m);
        moves.push(m);
    }
    let guaranteed = first_attack.map(|k| moves[k]);
    rng.shuffle(&mut moves);
    if let (Some(k), Some(m)) = (first_attack, guaranteed) {
        let at = moves.iter().position(|&x| x == m).expect("shuffled in place");
        moves.swap(at, k);
    }
    enforce_minimums(rng, pool, row, &mut moves, rules, first_attack);
    levels
        .into_iter()
        .zip(moves)
        .map(|(level, move_id)| LevelUpMove { move_id, level })
        .collect()
}

/// pkNX's minimums: replaces other moves until enough are damaging and enough are STAB.
fn enforce_minimums(rng: &mut Rng, pool: &MovePool, row: &Row, moves: &mut [u16], rules: &LevelUpRules, keep: Option<usize>) {
    let info = |m: u16| pool.table().get(m).expect("picked from the table");
    let attack = |m: u16| info(m).strength() > 0.0;
    let stab = |m: u16| attack(m) && row.has_type(info(m).type_id);
    let mut taken: BTreeSet<u16> = moves.iter().copied().collect();
    let want = rules.min_damaging.min(moves.len());
    let mut slots: Vec<usize> = (0..moves.len()).filter(|&k| !attack(moves[k]) && Some(k) != keep).collect();
    rng.shuffle(&mut slots);
    for k in slots {
        if moves.iter().filter(|&&m| attack(m)).count() >= want {
            break;
        }
        let physical = physical_roll(rng, row);
        let types = rules.prefer_type.then_some(row.types);
        let m = pool.pick(rng, Pick { types, damaging: true, physical }, &taken);
        taken.remove(&moves[k]);
        taken.insert(m);
        moves[k] = m;
    }
    let want = rules.min_stab.min(moves.len());
    let mut slots: Vec<usize> = (0..moves.len()).filter(|&k| !stab(moves[k]) && Some(k) != keep).collect();
    rng.shuffle(&mut slots);
    slots.sort_by_key(|&k| attack(moves[k]));
    for k in slots {
        if moves.iter().filter(|&&m| stab(m)).count() >= want {
            break;
        }
        let Some(m) = pool.pick_stab(rng, row.types, &taken) else { break };
        taken.remove(&moves[k]);
        taken.insert(m);
        moves[k] = m;
    }
}

/// UPR's reorder: damaging moves with a fixed power are learned weakest first. Ties keep their
/// order, so the result needs no randomness and cosmetic forms stay alike.
pub fn order_by_strength(table: &MoveTable, levelup: &mut [LevelUpMove]) {
    let strength = |m: u16| table.get(m).map_or(0.0, |i| i.strength());
    let slots: Vec<usize> = (0..levelup.len())
        .filter(|&k| levelup[k].level != ON_EVOLUTION && strength(levelup[k].move_id) > 0.0)
        .collect();
    let mut moves: Vec<u16> = slots.iter().map(|&k| levelup[k].move_id).collect();
    moves.sort_by(|a, b| strength(*a).total_cmp(&strength(*b)));
    for (k, m) in slots.into_iter().zip(moves) {
        levelup[k].move_id = m;
    }
}

/// A new list of the same length with no repeats, for egg and reminder moves.
pub fn randomize_list(rng: &mut Rng, pool: &MovePool, row: &Row, len: usize, prefer_type: bool) -> Vec<u16> {
    let mut taken = BTreeSet::new();
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let physical = physical_roll(rng, row);
        let m = pool.pick(
            rng,
            Pick {
                types: prefer_type.then_some(row.types),
                damaging: false,
                physical,
            },
            &taken,
        );
        taken.insert(m);
        out.push(m);
    }
    out
}
