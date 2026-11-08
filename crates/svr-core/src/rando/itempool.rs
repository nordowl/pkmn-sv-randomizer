//! Item classification shared by sections. Kinds come from the item table; the few judgments the
//! table does not record are curated lists below, each id kept beside its English name so a test
//! can check them against the game text.

use std::collections::BTreeMap;

use crate::data::store::{game_items, ITEMS};
use crate::text::{GameText, Language};

use super::{Ctx, RandoError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Ball,
    Medicine,
    Battle,
    Held,
    Berry,
    Treasure,
    Other,
    Tm,
    Material,
    Picnic,
    Key,
}

#[derive(Clone, Debug)]
pub struct ItemInfo {
    pub id: u16,
    /// English name; empty for placeholder rows.
    pub name: String,
    pub kind: ItemKind,
    pub price: i64,
    equip_effect: i64,
    evolves: bool,
}

// ItemType, FieldPocket, FieldFunctionType and EquipEffect values from the item table's schema.
const TYPE_POCKET: i64 = 0;
const TYPE_DRUG: i64 = 1;
const TYPE_EQUIP: i64 = 2;
const TYPE_BATTLE: i64 = 4;
const TYPE_BALL: i64 = 5;
const TYPE_MAIL: i64 = 6;
const TYPE_WAZA: i64 = 7;
const TYPE_NUTS: i64 = 8;
const TYPE_EVENT: i64 = 9;
const TYPE_MATERIAL: i64 = 12;
const POCKET_TREASURE: i64 = 6;
const POCKET_PICNIC: i64 = 7;
const POCKET_EVENT: i64 = 8;
const POCKET_RECIPE: i64 = 10;
const FIELD_EVOLUTION: i64 = 6;
const EQUIP_NONE: i64 = 0;

pub const MASTER_BALL: (u16, &str) = (1, "Master Ball");
pub const RARE_CANDY: (u16, &str) = (50, "Rare Candy");
pub const LUCKY_EGG: (u16, &str) = (231, "Lucky Egg");
pub const AMULET_COIN: (u16, &str) = (223, "Amulet Coin");

/// Battle items the UPR "guarantee X items" rule covers.
pub const X_ITEMS: &[(u16, &str)] = &[
    (55, "Guard Spec."),
    (56, "Dire Hit"),
    (57, "X Attack"),
    (58, "X Defense"),
    (59, "X Speed"),
    (60, "X Accuracy"),
    (61, "X Sp. Atk"),
    (62, "X Sp. Def"),
];

/// Held items that trigger an evolution. Stones and other used items are flagged in the table.
pub const EVOLUTION_HELD: &[(u16, &str)] = &[
    (110, "Oval Stone"),
    (221, "King’s Rock"),
    (233, "Metal Coat"),
    (235, "Dragon Scale"),
    (252, "Upgrade"),
    (321, "Protector"),
    (322, "Electirizer"),
    (323, "Magmarizer"),
    (324, "Dubious Disc"),
    (325, "Reaper Cloth"),
    (326, "Razor Claw"),
    (327, "Razor Fang"),
    (537, "Prism Scale"),
    (1109, "Strawberry Sweet"),
    (1110, "Love Sweet"),
    (1111, "Berry Sweet"),
    (1112, "Clover Sweet"),
    (1113, "Flower Sweet"),
    (1114, "Star Sweet"),
    (1115, "Ribbon Sweet"),
];

/// Evolution items that also work in battle.
const EVOLVES_AND_FIGHTS: &[(u16, &str)] = &[
    (221, "King’s Rock"),
    (233, "Metal Coat"),
    (326, "Razor Claw"),
    (327, "Razor Fang"),
];

/// Items that change a species' form or only work for one species. Never handed out as random
/// held items: a plate turns Arceus into another form, a mask is meaningless without Ogerpon.
pub const FORM_OR_SPECIES_ITEMS: &[(u16, &str)] = &[
    (112, "Griseous Orb"),
    (135, "Adamant Orb"),
    (136, "Lustrous Orb"),
    (225, "Soul Dew"),
    (236, "Light Ball"),
    (298, "Flame Plate"),
    (299, "Splash Plate"),
    (300, "Zap Plate"),
    (301, "Meadow Plate"),
    (302, "Icicle Plate"),
    (303, "Fist Plate"),
    (304, "Toxic Plate"),
    (305, "Earth Plate"),
    (306, "Sky Plate"),
    (307, "Mind Plate"),
    (308, "Insect Plate"),
    (309, "Stone Plate"),
    (310, "Spooky Plate"),
    (311, "Draco Plate"),
    (312, "Dread Plate"),
    (313, "Iron Plate"),
    (644, "Pixie Plate"),
    (1103, "Rusted Sword"),
    (1104, "Rusted Shield"),
    (1777, "Adamant Crystal"),
    (1778, "Lustrous Globe"),
    (1779, "Griseous Core"),
    (1880, "Booster Energy"),
    (2406, "Cornerstone Mask"),
    (2407, "Wellspring Mask"),
    (2408, "Hearthflame Mask"),
];

/// Items that only hurt a holder or have no use in this game.
pub const BAD_ITEMS: &[(u16, &str)] = &[
    (111, "Odd Keystone"),
    (224, "Cleanse Tag"),
    (228, "Smoke Ball"),
    (278, "Iron Ball"),
    (279, "Lagging Tail"),
    (288, "Sticky Barb"),
    (543, "Ring Target"),
];

/// Held items with no battle effect, or one that hurts a trainer's Pokémon.
const NO_BATTLE_EFFECT: &[(u16, &str)] = &[
    (218, "Soothe Bell"),
    (223, "Amulet Coin"),
    (229, "Everstone"),
    (231, "Lucky Egg"),
    (272, "Toxic Orb"),
    (273, "Flame Orb"),
    (289, "Power Bracer"),
    (290, "Power Belt"),
    (291, "Power Lens"),
    (292, "Power Band"),
    (293, "Power Anklet"),
    (294, "Power Weight"),
];

/// Held items added after the table's effect column stopped being filled in.
const MODERN_HELD: &[(u16, &str)] = &[
    (1118, "Throat Spray"),
    (1119, "Eject Pack"),
    (1120, "Heavy-Duty Boots"),
    (1121, "Blunder Policy"),
    (1122, "Room Service"),
    (1123, "Utility Umbrella"),
    (1881, "Ability Shield"),
    (1882, "Clear Amulet"),
    (1883, "Mirror Herb"),
    (1884, "Punching Glove"),
    (1885, "Covert Cloak"),
    (1886, "Loaded Dice"),
    (2401, "Fairy Feather"),
];

/// Single-use held items besides berries.
const CONSUMABLE_HELD: &[(u16, &str)] = &[
    (214, "White Herb"),
    (219, "Mental Herb"),
    (271, "Power Herb"),
    (275, "Focus Sash"),
    (541, "Air Balloon"),
    (542, "Red Card"),
    (545, "Absorb Bulb"),
    (546, "Cell Battery"),
    (547, "Eject Button"),
    (564, "Normal Gem"),
    (639, "Weakness Policy"),
    (648, "Luminous Moss"),
    (649, "Snowball"),
    (846, "Adrenaline Orb"),
    (881, "Electric Seed"),
    (882, "Psychic Seed"),
    (883, "Misty Seed"),
    (884, "Grassy Seed"),
    (1118, "Throat Spray"),
    (1119, "Eject Pack"),
    (1121, "Blunder Policy"),
    (1122, "Room Service"),
    (1883, "Mirror Herb"),
];

/// Held items that boost one type, by the game's type id (0 Normal ... 17 Fairy).
pub const TYPE_BOOSTERS: &[(u16, &str, u8)] = &[
    (251, "Silk Scarf", 0),
    (241, "Black Belt", 1),
    (244, "Sharp Beak", 2),
    (245, "Poison Barb", 3),
    (237, "Soft Sand", 4),
    (238, "Hard Stone", 5),
    (222, "Silver Powder", 6),
    (247, "Spell Tag", 7),
    (233, "Metal Coat", 8),
    (249, "Charcoal", 9),
    (243, "Mystic Water", 10),
    (239, "Miracle Seed", 11),
    (242, "Magnet", 12),
    (248, "Twisted Spoon", 13),
    (246, "Never-Melt Ice", 14),
    (250, "Dragon Fang", 15),
    (240, "Black Glasses", 16),
    (2401, "Fairy Feather", 17),
];

pub const BLACK_SLUDGE: (u16, &str) = (281, "Black Sludge");
pub const EVIOLITE: (u16, &str) = (538, "Eviolite");
const POISON: u8 = 3;

/// Every curated list with its name, for the test that checks them against the game text.
pub fn curated_names() -> Vec<(u16, &'static str)> {
    let mut all = vec![MASTER_BALL, RARE_CANDY, LUCKY_EGG, AMULET_COIN, BLACK_SLUDGE, EVIOLITE];
    for list in [
        X_ITEMS,
        EVOLUTION_HELD,
        EVOLVES_AND_FIGHTS,
        FORM_OR_SPECIES_ITEMS,
        BAD_ITEMS,
        NO_BATTLE_EFFECT,
        MODERN_HELD,
        CONSUMABLE_HELD,
        SHOP_OVERPOWERED,
    ] {
        all.extend_from_slice(list);
    }
    all.extend(TYPE_BOOSTERS.iter().map(|&(id, name, _)| (id, name)));
    all.extend(FORM_ITEMS.iter().map(|&(_, _, id, name)| (id, name)));
    all
}

fn listed(list: &[(u16, &str)], id: u16) -> bool {
    list.iter().any(|&(i, _)| i == id)
}

/// The item a form must hold to exist in battle, by National Dex number and form.
const FORM_ITEMS: &[(u16, u16, u16, &str)] = &[
    (483, 1, 1777, "Adamant Crystal"),
    (484, 1, 1778, "Lustrous Globe"),
    (487, 1, 1779, "Griseous Core"),
    (493, 1, 303, "Fist Plate"),
    (493, 2, 306, "Sky Plate"),
    (493, 3, 304, "Toxic Plate"),
    (493, 4, 305, "Earth Plate"),
    (493, 5, 309, "Stone Plate"),
    (493, 6, 308, "Insect Plate"),
    (493, 7, 310, "Spooky Plate"),
    (493, 8, 313, "Iron Plate"),
    (493, 9, 298, "Flame Plate"),
    (493, 10, 299, "Splash Plate"),
    (493, 11, 301, "Meadow Plate"),
    (493, 12, 300, "Zap Plate"),
    (493, 13, 307, "Mind Plate"),
    (493, 14, 302, "Icicle Plate"),
    (493, 15, 311, "Draco Plate"),
    (493, 16, 312, "Dread Plate"),
    (493, 17, 644, "Pixie Plate"),
    (888, 1, 1103, "Rusted Sword"),
    (889, 1, 1104, "Rusted Shield"),
    (1017, 1, 2407, "Wellspring Mask"),
    (1017, 2, 2408, "Hearthflame Mask"),
    (1017, 3, 2406, "Cornerstone Mask"),
];

/// The held item a form needs, such as a plate for Arceus forms. Sections that place a form must
/// give it this item, whatever their held-item settings say.
pub fn form_item(national: u16, form: u16) -> Option<u16> {
    FORM_ITEMS
        .iter()
        .find(|&&(n, f, _, _)| n == national && f == form)
        .map(|&(_, _, item, _)| item)
}

impl ItemInfo {
    pub fn named(&self) -> bool {
        !self.name.is_empty()
    }

    /// Stones and the like, plus held items that trigger an evolution.
    pub fn is_evolution(&self) -> bool {
        self.evolves || listed(EVOLUTION_HELD, self.id)
    }

    pub fn is_x_item(&self) -> bool {
        listed(X_ITEMS, self.id)
    }

    pub fn is_bad(&self) -> bool {
        listed(BAD_ITEMS, self.id)
    }

    /// Lucky Egg, Amulet Coin and treasures worth at least 10,000 when bought.
    pub fn is_big_money(&self) -> bool {
        self.id == LUCKY_EGG.0 || self.id == AMULET_COIN.0 || (self.kind == ItemKind::Treasure && self.price >= 10_000)
    }

    pub fn is_form_or_species_item(&self) -> bool {
        listed(FORM_OR_SPECIES_ITEMS, self.id)
    }

    /// The type this item boosts, if it is a type booster.
    pub fn boosted_type(&self) -> Option<u8> {
        TYPE_BOOSTERS.iter().find(|&&(id, _, _)| id == self.id).map(|&(_, _, t)| t)
    }

    pub fn is_consumable(&self) -> bool {
        (self.kind == ItemKind::Berry && self.equip_effect != EQUIP_NONE) || listed(CONSUMABLE_HELD, self.id)
    }

    fn holdable(&self) -> bool {
        matches!(self.kind, ItemKind::Held | ItemKind::Berry) && self.named() && !self.is_form_or_species_item()
    }

    /// Whether holding it does something useful in battle.
    fn has_battle_effect(&self) -> bool {
        if self.is_bad() || listed(NO_BATTLE_EFFECT, self.id) {
            return false;
        }
        if self.is_evolution() && !listed(EVOLVES_AND_FIGHTS, self.id) {
            return false;
        }
        self.equip_effect != EQUIP_NONE || listed(MODERN_HELD, self.id)
    }
}

pub struct ItemCatalog {
    items: Vec<ItemInfo>,
    by_id: BTreeMap<u16, usize>,
}

impl ItemCatalog {
    pub fn load(ctx: &mut Ctx) -> Result<ItemCatalog, RandoError> {
        let names = GameText::load(ctx.game, Language::English, "itemname").map_err(|e| RandoError::Text(e.to_string()))?;
        let known = game_items(&mut ctx.store)?;
        let table = ctx.store.get(ITEMS)?;
        let mut items = Vec::new();
        for row in table.tables("values") {
            let int = |field: &str| row.int(field).unwrap_or(0);
            let Ok(id) = u16::try_from(int("Id")) else { continue };
            if id == 0 || !known.contains(&id) {
                continue;
            }
            let (ty, pocket) = (int("ItemType"), int("FieldPocket"));
            let kind = match (ty, pocket) {
                (TYPE_EVENT | TYPE_POCKET | TYPE_MAIL, _) | (_, POCKET_EVENT) => ItemKind::Key,
                (TYPE_MATERIAL, _) => ItemKind::Material,
                (TYPE_WAZA, _) => ItemKind::Tm,
                (_, POCKET_PICNIC | POCKET_RECIPE) => ItemKind::Picnic,
                (TYPE_BALL, _) => ItemKind::Ball,
                (TYPE_DRUG, _) => ItemKind::Medicine,
                (TYPE_BATTLE, _) => ItemKind::Battle,
                (TYPE_NUTS, _) => ItemKind::Berry,
                (TYPE_EQUIP, _) => ItemKind::Held,
                (_, POCKET_TREASURE) => ItemKind::Treasure,
                _ => ItemKind::Other,
            };
            items.push(ItemInfo {
                id,
                name: names.line(usize::from(id)).unwrap_or_default(),
                kind,
                price: int("Price"),
                equip_effect: int("EquipEffect"),
                evolves: int("FieldFunctionType") == FIELD_EVOLUTION,
            });
        }
        let by_id = items.iter().enumerate().map(|(i, item)| (item.id, i)).collect();
        Ok(ItemCatalog { items, by_id })
    }

    pub fn get(&self, id: u16) -> Option<&ItemInfo> {
        self.by_id.get(&id).map(|&i| &self.items[i])
    }

    pub fn all(&self) -> &[ItemInfo] {
        &self.items
    }

    /// Items a Pokémon can hold. `ban_bad` keeps only items that do something useful in battle.
    pub fn held(&self, ban_bad: bool) -> Vec<u16> {
        self.items
            .iter()
            .filter(|i| i.holdable() && (!ban_bad || i.has_battle_effect()))
            .map(|i| i.id)
            .collect()
    }

    /// Items that can lie on the ground or be picked up: everything a player can carry except key
    /// items, TMs, crafting materials, picnic goods and form items.
    pub fn field(&self, ban_bad: bool, ban_money: bool) -> Vec<u16> {
        self.items
            .iter()
            .filter(|i| {
                let carried = i.named() && !matches!(i.kind, ItemKind::Key | ItemKind::Tm | ItemKind::Material | ItemKind::Picnic);
                let banned = (i.kind == ItemKind::Held && i.is_form_or_species_item())
                    || (ban_bad && i.is_bad())
                    || (ban_money && i.is_big_money());
                carried && !banned
            })
            .map(|i| i.id)
            .collect()
    }
}

/// Items a Pokémon can sensibly hold. `ban_bad` drops items that only hurt or do nothing.
pub fn held_items(ctx: &mut Ctx, ban_bad: bool) -> Result<Vec<u16>, RandoError> {
    Ok(ItemCatalog::load(ctx)?.held(ban_bad))
}

/// Narrows a held-item pool for one Pokémon: type boosters only for its types, Black Sludge only
/// for Poison types, Eviolite only if it can still evolve.
pub fn sensible_for(catalog: &ItemCatalog, pool: &[u16], types: [u8; 2], fully_evolved: bool) -> Vec<u16> {
    pool.iter()
        .copied()
        .filter(|&id| {
            let Some(item) = catalog.get(id) else { return false };
            if let Some(t) = item.boosted_type() {
                return types.contains(&t);
            }
            if id == BLACK_SLUDGE.0 {
                return types.contains(&POISON);
            }
            if id == EVIOLITE.0 {
                return !fully_evolved;
            }
            true
        })
        .collect()
}

/// Items too strong to buy freely, for the UPR "ban overpowered shop items" rule.
pub const SHOP_OVERPOWERED: &[(u16, &str)] = &[
    MASTER_BALL,
    RARE_CANDY,
    (795, "Bottle Cap"),
    (796, "Gold Bottle Cap"),
    (1128, "Exp. Candy XL"),
    (1606, "Ability Patch"),
];

impl ItemInfo {
    /// The list above plus big-money items, which a shop would turn into a money loop.
    pub fn is_shop_overpowered(&self) -> bool {
        listed(SHOP_OVERPOWERED, self.id) || self.is_big_money()
    }
}
