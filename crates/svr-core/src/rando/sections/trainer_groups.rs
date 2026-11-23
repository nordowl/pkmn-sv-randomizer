//! Which group and region each trainer row belongs to, and the limits scripted battles need.
//! Everything is keyed by `trid`, the row's stable name.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Rivals,
    Gyms,
    League,
    TeamStar,
    Club,
    Story,
    Route,
    Raid,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Rivals => "Rivals",
            Group::Gyms => "Gyms",
            Group::League => "Pokémon League",
            Group::TeamStar => "Team Star",
            Group::Club => "League Club",
            Group::Story => "Story & school",
            Group::Route => "Route trainers",
            Group::Raid => "Raid helpers",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Region {
    Paldea,
    Kitakami,
    Blueberry,
}

/// Limits that keep a scripted battle working, ported from the SV reference's protective cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Protection {
    /// The script fixes singles, doubles or a multi battle, so `battleType` and `aiDouble` stay.
    pub fixed_battle: bool,
    /// Team size never grows past this.
    pub max_team: Option<usize>,
    /// Team size never grows at all.
    pub fixed_team: bool,
    /// `changeGem` stays off.
    pub no_tera: bool,
    /// Must-win fights where a Wonder Guard opponent could be unbeatable.
    pub no_wonder_guard: bool,
    /// The battle adopts a scene's Pokémon object as the lead's model, so the scene must show the
    /// lead's species. The team stays one Pokémon, never shiny, as the object is.
    pub scene_lead: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub group: Group,
    pub region: Region,
    pub protection: Protection,
    /// Rows of the Paldea story before the credits, for the main-story even distribution.
    pub main_story: bool,
    /// Gym leaders, the League, Team Star bosses and story characters.
    pub named_character: bool,
}

/// `*x*` matches a substring, `a*b` a prefix and suffix (either may be empty), anything else the
/// whole trid.
fn matches(pattern: &str, trid: &str) -> bool {
    if let Some(inner) = pattern.strip_prefix('*').and_then(|p| p.strip_suffix('*')) {
        return trid.contains(inner);
    }
    match pattern.split_once('*') {
        Some((prefix, suffix)) => {
            trid.len() >= prefix.len() + suffix.len() && trid.starts_with(prefix) && trid.ends_with(suffix)
        }
        None => trid == pattern,
    }
}

fn any(patterns: &[&str], trid: &str) -> bool {
    patterns.iter().any(|p| matches(p, trid))
}

const GROUPS: &[(&str, Group)] = &[
    ("raid_assist_*", Group::Raid),
    ("su2_bukatu_*", Group::Club),
    ("rival_*", Group::Rivals),
    ("pepper_*", Group::Rivals),
    ("botan_*", Group::Rivals),
    ("brother_*", Group::Rivals),
    ("sister_*", Group::Rivals),
    ("s2_side_brother", Group::Rivals),
    ("gym_*", Group::Gyms),
    ("e4_*", Group::League),
    ("chairperson_*", Group::League),
    ("*challenge_*", Group::League),
    ("*4_02_*", Group::League),
    ("dan_*", Group::TeamStar),
    ("area*", Group::Route),
];

const KITAKAMI: &[&str] = &[
    "*_dlc1*",
    "brother_01*",
    "sister_01*",
    "sister_muruchi*",
    "sister_onitaizi*",
    "camera_*",
    "serebu*",
    "serevy*",
    "sp_trainer*",
];

const BLUEBERRY: &[&str] = &[
    "*_dlc2*",
    "brother_02*",
    "brother_kodaigame",
    "sister_02*",
    "s2_*",
    "su2_*",
    "*challenge_*",
    "*4_02_*",
    "shiano",
    "botan_02_01",
    "botan_schoolwars",
    "pepper_02_01",
    "pepper_schoolwars",
    "rival_02_01*",
    "rival_schoolwars*",
];

/// Paldea rows fought after the credits: rematches and the Academy Ace Tournament.
const POST_GAME: &[&str] = &[
    "gym_*_leader_02",
    "dan_*_boss_02",
    "chairperson_02",
    "chairperson_03",
    "clavel_02*",
    "rival_08*",
    "pepper_02",
    "pepper_03",
    "botan_02",
    "e4_dragon_02",
    "kihada_*",
    "mimoza_*",
    "rehoru_*",
    "sawaro_*",
    "seizi_*",
    "taimu_*",
    "zinia_*",
];

/// Multi battles and partner battles, whose battle type the script decides. None of these trainers
/// Terastallizes in the game, and they stay that way.
const MULTI: &[&str] = &[
    "botan_multi",
    "pepper_multi",
    "pepper_nusi_*",
    "pepper_02_01",
    "rival_multi_*",
    "brother_kodaigame",
    "s2_side_brother",
    "sister_muruchi_01*",
    "sister_onitaizi*",
    "sister_02_02",
    "s2_side_grandfather",
    "s2_side_grandmother",
    "s2_side_villager0*",
];

/// Battles that must stay singles: the first Nemona battle, Carmine's and Kieran's Kitakami fights.
const SINGLES: &[&str] = &["rival_01_*", "sister_01_01", "sister_01_01_strong", "brother_01_*"];

/// One-Pokémon fights that must stay one Pokémon without Terastallizing.
const LONE: &[&str] = &["rival_01_hono", "rival_01_kusa", "rival_01_mizu", "sister_02_02"];

/// The Paradise Protocol fights and the Terapagos fight.
const NO_WONDER_GUARD: &[&str] = &["professor_A_02", "professor_B_02", "brother_02_02"];

/// Arven's partner in each Titan's second battle, and the Titan's clash scene, without the version
/// suffix, whose `frend_partner` object becomes the partner's model in battle.
pub const TITAN_PARTNERS: [(&str, &str); 5] = [
    ("pepper_nusi_01", "world/scene/parts/field/field_contents/nushi/common/RockClashEvent_/RockClashEvent.trscn"),
    ("pepper_nusi_02", "world/scene/parts/field/field_contents/nushi/hagane/HaganeRockClashEvent_/HaganeRockClashEvent.trscn"),
    ("pepper_nusi_03", "world/scene/parts/field/field_contents/nushi/hiko/HikoRockClashEvent_/HikoRockClashEvent.trscn"),
    ("pepper_nusi_04", "world/scene/parts/field/field_contents/nushi/jimen/JimenRockClashEvent_/JimenRockClashEvent.trscn"),
    ("pepper_nusi_05", "world/scene/parts/field/field_contents/nushi/dragon/DragonRockClashEvent_/DragonRockClashEvent.trscn"),
];

/// Team Star bosses whose sixth slot belongs to the Starmobile.
const STAR_BOSS_FIRST: &str = "dan_*_boss_01";

/// The game's own test rows.
pub fn is_test(trid: &str) -> bool {
    trid.is_empty() || trid.starts_with("00_test") || trid.starts_with("test")
}

fn region(trid: &str) -> Region {
    if let Some(n) = trid.strip_prefix("raid_assist_NPC_").and_then(|n| n.parse::<u32>().ok()) {
        return match n {
            0..=16 => Region::Paldea,
            17..=34 => Region::Kitakami,
            _ => Region::Blueberry,
        };
    }
    if any(KITAKAMI, trid) {
        Region::Kitakami
    } else if any(BLUEBERRY, trid) {
        Region::Blueberry
    } else {
        Region::Paldea
    }
}

/// Group, region and protections of a row, or None for the game's test rows.
pub fn place(trid: &str) -> Option<Placement> {
    if is_test(trid) {
        return None;
    }
    let group = GROUPS
        .iter()
        .find(|(p, _)| matches(p, trid))
        .map_or(Group::Story, |&(_, g)| g);
    let region = region(trid);
    let mut protection = Protection {
        fixed_battle: any(MULTI, trid) || any(SINGLES, trid) || group == Group::Raid,
        no_tera: any(MULTI, trid),
        ..Protection::default()
    };
    if group == Group::Raid {
        protection.fixed_team = true;
        protection.no_tera = true;
    }
    if any(LONE, trid) {
        protection.fixed_battle = true;
        protection.max_team = Some(1);
        protection.no_tera = true;
    }
    if matches(STAR_BOSS_FIRST, trid) {
        protection.fixed_battle = true;
        protection.max_team = Some(5);
    }
    if any(NO_WONDER_GUARD, trid) {
        protection.fixed_battle = true;
        protection.fixed_team = true;
        protection.no_tera = true;
        protection.no_wonder_guard = true;
    }
    if TITAN_PARTNERS.iter().any(|&(id, _)| id == trid) {
        protection.fixed_team = true;
        protection.scene_lead = true;
    }
    let post_game = any(POST_GAME, trid);
    let named_character = match group {
        Group::Rivals | Group::Club | Group::Story => true,
        Group::League => !trid.contains("challenge_"),
        Group::Gyms => trid.contains("_leader_"),
        Group::TeamStar => trid.contains("_boss_"),
        Group::Route | Group::Raid => false,
    };
    Some(Placement {
        group,
        region,
        protection,
        main_story: region == Region::Paldea && !post_game,
        named_character,
    })
}

/// The gym a row belongs to, such as `denki` for `gym_denki_leader_01`.
pub fn gym_key(trid: &str) -> Option<&str> {
    trid.strip_prefix("gym_")?.split('_').next()
}

/// The Team Star crew of a row, such as `aku` for `dan_aku_boss_01`.
pub fn crew_key(trid: &str) -> Option<&str> {
    trid.strip_prefix("dan_")?.split('_').next()
}

/// The player's starter a per-starter row answers: 0 grass, 1 fire, 2 water.
pub fn player_starter(trid: &str) -> Option<usize> {
    [("kusa", 0), ("honoo", 1), ("hono", 1), ("mizu", 2)]
        .iter()
        .find(|(suffix, _)| trid.ends_with(suffix))
        .map(|&(_, slot)| slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_regions_follow_the_trid() {
        let p = |t| place(t).unwrap();
        assert_eq!(p("area01_trainer_cleaning").group, Group::Route);
        assert_eq!(p("area01_dlc1_trainer_j_child_m").region, Region::Kitakami);
        assert_eq!(p("area04_dlc2_trainer_b_adult_f").region, Region::Blueberry);
        assert_eq!(p("rival_02_01kusa").region, Region::Blueberry);
        assert_eq!(p("dragonchallenge_01").group, Group::League);
        assert_eq!(p("fairy4_02_01").group, Group::League);
        assert_eq!(p("su2_bukatu_nemo_kusa").group, Group::Club);
        assert_eq!(p("raid_assist_NPC_20").region, Region::Kitakami);
        assert_eq!(p("kihada_01").group, Group::Story);
        assert!(place("00_test_data").is_none() && place("test2").is_none());
    }

    #[test]
    fn protections_match_the_reference_cases() {
        let p = |t| place(t).unwrap().protection;
        assert_eq!(p("rival_01_kusa").max_team, Some(1));
        assert!(p("rival_01_kusa").no_tera && p("rival_01_kusa").fixed_battle);
        assert_eq!(p("dan_aku_boss_01").max_team, Some(5));
        assert_eq!(p("dan_aku_boss_02").max_team, None);
        assert!(p("brother_01_03_strong").fixed_battle);
        assert!(p("sister_onitaizi_strong").fixed_battle);
        assert!(p("brother_02_02").no_wonder_guard && p("professor_B_02").no_wonder_guard);
        assert!(p("pepper_nusi_05").scene_lead && p("pepper_nusi_05").fixed_team && p("pepper_nusi_05").fixed_battle);
        assert!(p("pepper_nusi_05").no_tera && p("pepper_multi").no_tera && p("s2_side_villager02").no_tera);
        assert!(!p("pepper_multi").scene_lead);
        assert!(!p("gym_denki_01").fixed_battle);
        assert!(!place("gym_denki_leader_02").unwrap().main_story);
        assert!(place("gym_denki_leader_01").unwrap().main_story);
    }

    #[test]
    fn starter_suffixes_name_the_players_choice() {
        assert_eq!(player_starter("rival_01_kusa"), Some(0));
        assert_eq!(player_starter("su2_bukatu_claver_honoo"), Some(1));
        assert_eq!(player_starter("rival_02_01mizu"), Some(2));
        assert_eq!(player_starter("rival_02"), None);
        assert_eq!(gym_key("gym_denki_leader_01"), Some("denki"));
        assert_eq!(crew_key("dan_fairy_butler_01"), Some("fairy"));
    }
}
