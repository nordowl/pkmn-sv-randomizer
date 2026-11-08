//! Ability lists by English name, mapped to IDs through the game's own text, and the pool the
//! ability randomizer draws from.

use std::collections::{BTreeMap, BTreeSet};

use crate::data::Names;
use crate::rando::Rng;

/// Abilities that change their holder's form. On any other species they would ask for a form that
/// does not exist, which is why the SV reference always bans Zero to Hero and Tera Shift. A
/// Pokémon that has one keeps its abilities.
pub const FORM_CHANGING: &[&str] = &[
    "Multitype",
    "Zen Mode",
    "Stance Change",
    "Shields Down",
    "Schooling",
    "Disguise",
    "Battle Bond",
    "Power Construct",
    "RKS System",
    "Forecast",
    "Flower Gift",
    "Gulp Missile",
    "Ice Face",
    "Hunger Switch",
    "Zero to Hero",
    "Commander",
    "Tera Shift",
    "Tera Shell",
    "Teraform Zero",
    "Embody Aspect",
];

pub const WONDER_GUARD: &[&str] = &["Wonder Guard"];
pub const EXIT: &[&str] = &["Wimp Out", "Emergency Exit"];
pub const TRAPPING: &[&str] = &["Shadow Tag", "Magnet Pull", "Arena Trap"];
pub const NEGATIVE: &[&str] = &["Defeatist", "Slow Start", "Truant", "Klutz", "Stall"];
/// UPR's bad abilities plus the ones that only help a partner, as UPR bans for single battles.
pub const BAD: &[&str] = &[
    "Minus",
    "Plus",
    "Anticipation",
    "Forewarn",
    "Frisk",
    "Honey Gather",
    "Aura Break",
    "Receiver",
    "Power of Alchemy",
    "Friend Guard",
    "Healer",
    "Telepathy",
    "Symbiosis",
    "Battery",
];

/// Abilities with the same effect. The first name stands for its group in the draw.
pub const SAME_EFFECT: &[&[&str]] = &[
    &["Insomnia", "Vital Spirit"],
    &["Clear Body", "White Smoke", "Full Metal Body"],
    &["Huge Power", "Pure Power"],
    &["Battle Armor", "Shell Armor"],
    &["Cloud Nine", "Air Lock"],
    &["Filter", "Solid Rock", "Prism Armor"],
    &["Rough Skin", "Iron Barbs"],
    &["Mold Breaker", "Turboblaze", "Teravolt"],
    &["Wimp Out", "Emergency Exit"],
    &["Queenly Majesty", "Dazzling", "Armor Tail"],
    &["Gooey", "Tangling Hair"],
    &["Receiver", "Power of Alchemy"],
    &["Multiscale", "Shadow Shield"],
    &["Protean", "Libero"],
    &["Stalwart", "Propeller Tail"],
    &["Mummy", "Lingering Aroma"],
    &["Moxie", "Chilling Neigh"],
];

/// Ability IDs below `count` whose English name is in `list`.
pub fn ids(names: &Names, count: u16, list: &[&str]) -> BTreeSet<u16> {
    (1..count).filter(|&id| list.contains(&names.ability(id).as_str())).collect()
}

pub struct AbilityPool {
    draw: Vec<u16>,
    groups: BTreeMap<u16, Vec<u16>>,
}

impl AbilityPool {
    /// Every ability below `count` except `banned`. With `weigh_duplicates`, each same-effect
    /// group is drawn as one ability and a member is chosen afterwards.
    pub fn new(names: &Names, count: u16, banned: &BTreeSet<u16>, weigh_duplicates: bool) -> AbilityPool {
        let mut draw: Vec<u16> = (1..count).filter(|id| !banned.contains(id)).collect();
        let mut groups = BTreeMap::new();
        if weigh_duplicates {
            for group in SAME_EFFECT {
                let members: Vec<u16> = group
                    .iter()
                    .flat_map(|name| ids(names, count, &[name]))
                    .filter(|id| !banned.contains(id))
                    .collect();
                let Some((&lead, rest)) = members.split_first() else { continue };
                draw.retain(|id| !rest.contains(id));
                groups.insert(lead, members.clone());
            }
        }
        AbilityPool { draw, groups }
    }

    pub fn is_empty(&self) -> bool {
        self.draw.len() < 3
    }

    /// A random ability that is not in `taken`.
    pub fn pick(&self, rng: &mut Rng, taken: &[u16]) -> u16 {
        loop {
            let mut id = self.draw[rng.index(self.draw.len())];
            if let Some(members) = self.groups.get(&id) {
                let free: Vec<u16> = members.iter().copied().filter(|m| !taken.contains(m)).collect();
                if let Some(&m) = rng.pick(&free) {
                    id = m;
                }
            }
            if !taken.contains(&id) {
                return id;
            }
        }
    }
}
