//! Species classifications that the game data does not record, keyed by National Dex number.
//! Names are kept beside each number so a test can check them against the game's own text.

pub const MYTHICAL: &[(u16, &str)] = &[
    (151, "Mew"),
    (251, "Celebi"),
    (385, "Jirachi"),
    (386, "Deoxys"),
    (489, "Phione"),
    (490, "Manaphy"),
    (491, "Darkrai"),
    (492, "Shaymin"),
    (493, "Arceus"),
    (494, "Victini"),
    (647, "Keldeo"),
    (648, "Meloetta"),
    (649, "Genesect"),
    (719, "Diancie"),
    (720, "Hoopa"),
    (721, "Volcanion"),
    (801, "Magearna"),
    (802, "Marshadow"),
    (807, "Zeraora"),
    (808, "Meltan"),
    (809, "Melmetal"),
    (893, "Zarude"),
    (1025, "Pecharunt"),
];

pub const LEGENDARY: &[(u16, &str)] = &[
    (150, "Mewtwo"),
    (249, "Lugia"),
    (250, "Ho-Oh"),
    (382, "Kyogre"),
    (383, "Groudon"),
    (384, "Rayquaza"),
    (483, "Dialga"),
    (484, "Palkia"),
    (487, "Giratina"),
    (643, "Reshiram"),
    (644, "Zekrom"),
    (646, "Kyurem"),
    (716, "Xerneas"),
    (717, "Yveltal"),
    (718, "Zygarde"),
    (789, "Cosmog"),
    (790, "Cosmoem"),
    (791, "Solgaleo"),
    (792, "Lunala"),
    (800, "Necrozma"),
    (888, "Zacian"),
    (889, "Zamazenta"),
    (890, "Eternatus"),
    (898, "Calyrex"),
    (1007, "Koraidon"),
    (1008, "Miraidon"),
    (1024, "Terapagos"),
];

pub const SUB_LEGENDARY: &[(u16, &str)] = &[
    (144, "Articuno"),
    (145, "Zapdos"),
    (146, "Moltres"),
    (243, "Raikou"),
    (244, "Entei"),
    (245, "Suicune"),
    (377, "Regirock"),
    (378, "Regice"),
    (379, "Registeel"),
    (380, "Latias"),
    (381, "Latios"),
    (480, "Uxie"),
    (481, "Mesprit"),
    (482, "Azelf"),
    (485, "Heatran"),
    (486, "Regigigas"),
    (488, "Cresselia"),
    (638, "Cobalion"),
    (639, "Terrakion"),
    (640, "Virizion"),
    (641, "Tornadus"),
    (642, "Thundurus"),
    (645, "Landorus"),
    (772, "Type: Null"),
    (773, "Silvally"),
    (785, "Tapu Koko"),
    (786, "Tapu Lele"),
    (787, "Tapu Bulu"),
    (788, "Tapu Fini"),
    (891, "Kubfu"),
    (892, "Urshifu"),
    (894, "Regieleki"),
    (895, "Regidrago"),
    (896, "Glastrier"),
    (897, "Spectrier"),
    (905, "Enamorus"),
    (1001, "Wo-Chien"),
    (1002, "Chien-Pao"),
    (1003, "Ting-Lu"),
    (1004, "Chi-Yu"),
    (1014, "Okidogi"),
    (1015, "Munkidori"),
    (1016, "Fezandipiti"),
    (1017, "Ogerpon"),
];

pub const ULTRA_BEAST: &[(u16, &str)] = &[
    (793, "Nihilego"),
    (794, "Buzzwole"),
    (795, "Pheromosa"),
    (796, "Xurkitree"),
    (797, "Celesteela"),
    (798, "Kartana"),
    (799, "Guzzlord"),
    (803, "Poipole"),
    (804, "Naganadel"),
    (805, "Stakataka"),
    (806, "Blacephalon"),
];

pub const PARADOX: &[(u16, &str)] = &[
    (984, "Great Tusk"),
    (985, "Scream Tail"),
    (986, "Brute Bonnet"),
    (987, "Flutter Mane"),
    (988, "Slither Wing"),
    (989, "Sandy Shocks"),
    (990, "Iron Treads"),
    (991, "Iron Bundle"),
    (992, "Iron Hands"),
    (993, "Iron Jugulis"),
    (994, "Iron Moth"),
    (995, "Iron Thorns"),
    (1005, "Roaring Moon"),
    (1006, "Iron Valiant"),
    (1009, "Walking Wake"),
    (1010, "Iron Leaves"),
    (1020, "Gouging Fire"),
    (1021, "Raging Bolt"),
    (1022, "Iron Boulder"),
    (1023, "Iron Crown"),
];

/// Story-critical species the randomizer never places anywhere: the ride legends and the two
/// species the SV reference warns must not be Terastallized after capture.
pub const NEVER_RANDOM: &[(u16, &str)] = &[(1007, "Koraidon"), (1008, "Miraidon")];
/// Species the game shows in the overworld only through their own objects, never as ordinary wild
/// or static spawns: Gimmighoul sits in treasure chests and roams as a coin collector. Those
/// objects' scripts crash the game when another species takes Gimmighoul's place, so seeds leave
/// them alone, and as a precaution never spawn the species as an ordinary overworld Pokémon.
pub const OBJECT_ONLY: &[(u16, &str)] = &[(999, "Gimmighoul")];
pub const OGERPON: u16 = 1017;
pub const TERAPAGOS: u16 = 1024;
pub const TERA_LOCKED: &[(u16, &str)] = &[(OGERPON, "Ogerpon"), (TERAPAGOS, "Terapagos")];

/// A Tera type the game insists on for a species.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tera {
    /// One of the 18 types, by type id.
    Type(u8),
    Stellar,
}

/// Ogerpon's masks by form, Teal, Wellspring, Hearthflame and Cornerstone, as the type ids of Grass,
/// Water, Fire and Rock. Its Terastal forms 4 to 7 wear the same masks in the same order.
pub const OGERPON_MASK_TYPES: [u8; 4] = [11, 10, 9, 5];

/// The game expects Ogerpon's Tera type to match its mask and Terapagos to be Stellar.
pub fn required_tera(national_dex: u16, form: u16) -> Option<Tera> {
    match national_dex {
        OGERPON => Some(Tera::Type(OGERPON_MASK_TYPES[usize::from(form % 4)])),
        TERAPAGOS => Some(Tera::Stellar),
        _ => None,
    }
}

/// Battle-only or transformation forms (UPR's "irregular" forms): only ever meant to exist mid-battle
/// or tied to a gimmick, so random pools leave them out unless the user allows them.
/// Entries are (National Dex, first form, last form, species name).
pub const IRREGULAR_FORMS: &[(u16, u16, u16, &str)] = &[
    (351, 1, 3, "Castform"),
    (421, 1, 1, "Cherrim"),
    (555, 1, 1, "Darmanitan"),
    (555, 3, 3, "Darmanitan"),
    (648, 1, 1, "Meloetta"),
    (681, 1, 1, "Aegislash"),
    (746, 1, 1, "Wishiwashi"),
    (774, 7, 13, "Minior"),
    (778, 1, 3, "Mimikyu"),
    (800, 3, 3, "Necrozma"),
    (845, 1, 2, "Cramorant"),
    (875, 1, 1, "Eiscue"),
    (877, 1, 1, "Morpeko"),
    (888, 1, 1, "Zacian"),
    (889, 1, 1, "Zamazenta"),
    (890, 1, 1, "Eternatus"),
    (964, 1, 1, "Palafin"),
    (1017, 4, 7, "Ogerpon"),
    (1024, 1, 2, "Terapagos"),
];

pub fn is_irregular_form(national_dex: u16, form: u16) -> bool {
    form != 0
        && IRREGULAR_FORMS
            .iter()
            .any(|&(n, first, last, _)| n == national_dex && (first..=last).contains(&form))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Rarity {
    Standard,
    SubLegendary,
    Legendary,
    Mythical,
}

pub fn rarity(national_dex: u16) -> Rarity {
    let has = |list: &[(u16, &str)]| list.iter().any(|&(n, _)| n == national_dex);
    if has(MYTHICAL) {
        Rarity::Mythical
    } else if has(LEGENDARY) {
        Rarity::Legendary
    } else if has(SUB_LEGENDARY) || has(ULTRA_BEAST) {
        Rarity::SubLegendary
    } else {
        Rarity::Standard
    }
}

pub fn is_paradox(national_dex: u16) -> bool {
    PARADOX.iter().any(|&(n, _)| n == national_dex)
}

pub fn generation(national_dex: u16) -> u8 {
    const LAST: [u16; 8] = [151, 251, 386, 493, 649, 721, 809, 905];
    LAST.iter().position(|&last| national_dex <= last).map_or(9, |i| i as u8 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_follow_the_dex_boundaries() {
        assert_eq!(generation(1), 1);
        assert_eq!(generation(151), 1);
        assert_eq!(generation(152), 2);
        assert_eq!(generation(905), 8);
        assert_eq!(generation(906), 9);
        assert_eq!(generation(1025), 9);
    }

    #[test]
    fn classification_lists_do_not_overlap() {
        let all = [MYTHICAL, LEGENDARY, SUB_LEGENDARY, ULTRA_BEAST, PARADOX];
        let mut seen = std::collections::BTreeSet::new();
        for list in all {
            for &(n, name) in list {
                assert!(seen.insert(n), "{name} ({n}) is in two lists");
            }
        }
    }

    #[test]
    fn rarity_ranks_mythical_above_legendary() {
        assert_eq!(rarity(151), Rarity::Mythical);
        assert_eq!(rarity(150), Rarity::Legendary);
        assert_eq!(rarity(1001), Rarity::SubLegendary);
        assert_eq!(rarity(793), Rarity::SubLegendary);
        assert_eq!(rarity(25), Rarity::Standard);
        assert!(is_paradox(984) && !is_paradox(25));
    }
}
