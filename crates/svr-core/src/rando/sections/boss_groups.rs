//! Which scripted battles belong to which switch, and which scene files show their Pokémon.
//!
//! Scene paths are listed once without the version suffix; every scene exists as `_0` (Scarlet)
//! and `_1` (Violet). A scene is patched by original species, so a group lists every file in which
//! any of its battle species appears, and objects of other species in those files stay as they are.

pub const BATTLES: &str = "world/data/battle/eventBattlePokemon/eventBattlePokemon_array.bin";
pub const RELEASE: &str = "world/data/event/s2_sub_012_legend_poke/release_poke/release_poke_array.bin";

pub struct Group {
    pub key: &'static str,
    pub switch: &'static str,
    pub pool: &'static str,
    pub pool_label: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// Battle labels belong to the group when they start with one of these.
    pub labels: &'static [&'static str],
    pub scenes: &'static [&'static str],
    /// A table whose species field follows the group's battles.
    pub release: Option<&'static str>,
}

/// Koraidon and Miraidon fights stay vanilla, and so do Gimmighoul's coin battles, because its
/// chest and roaming objects crash the game with any other species (see `facts::OBJECT_ONLY`).
pub const UNTOUCHED: [&str; 3] = ["lastbattle_", "sub018_", "coin_976_"];

macro_rules! main {
    ($dir:literal, $file:literal) => {
        concat!("world/scene/parts/event/event_scenario/main_scenario/", $dir, "_/", $file, ".trsog")
    };
}

macro_rules! sub {
    ($dir:literal, $file:literal) => {
        concat!("world/scene/parts/event/event_scenario/sub_scenario/", $dir, "_/", $file, ".trsog")
    };
}

macro_rules! titan_object {
    ($kind:literal, $file:literal) => {
        concat!("world/obj_template/parts/nushi/", $kind, "/", $file, "_/", $file, ".trsot")
    };
}

pub const GROUPS: &[Group] = &[
    Group {
        key: "titans",
        switch: "bosses.titans",
        pool: "bosses.titans.pool",
        pool_label: "Titan species",
        label: "Titans",
        help: "Klawf, Bombirdier, Orthworm, Great Tusk or Iron Treads, Dondozo and Tatsugiri. Both phases of a Titan fight get the same new Pokémon, and its overworld model and cutscenes change with it.",
        labels: &["nusi_"],
        scenes: &[
            main!("nushi_iwa_010", "nushi_iwa_010_pre_start"),
            main!("nushi_iwa_020", "nushi_iwa_020_pre_start"),
            titan_object!("iwa", "nushi_iwa_fp_1066"),
            titan_object!("iwa", "nushi_iwa_fp_1066_020"),
            sub!("sub_037", "sub_037_pre_start"),
            "world/scene/parts/field/field_contents/nushi/hiko/HikoNushi_/HikoNushi.trscn",
            main!("nushi_hikou_010", "nushi_hikou_010_pre_start"),
            main!("nushi_hikou_020", "nushi_hikou_020_pre_start"),
            main!("nushi_hikou_020", "nushi_hikou_020_main"),
            titan_object!("hiko", "nushi_hiko_fp_1063_010"),
            titan_object!("hiko", "nushi_hiko_fp_1063_020"),
            sub!("sub_038", "sub_038_pre_start"),
            main!("nushi_hagane_010", "nushi_hagane_010_pre_start"),
            main!("nushi_hagane_020", "nushi_hagane_020_pre_start"),
            titan_object!("hagane", "nushi_hagane_fp_1048_010"),
            titan_object!("hagane", "nushi_hagane_fp_1048_020"),
            sub!("sub_039", "sub_039_pre_start"),
            main!("nushi_jimen_010", "nushi_jimen_010_pre_start"),
            main!("nushi_jimen_020", "nushi_jimen_020_pre_start"),
            titan_object!("jimen", "nushi_jimen_fp_1082_010"),
            titan_object!("jimen", "nushi_jimen_fp_1082_020"),
            titan_object!("jimen", "nushi_jimen_fp_1090_010"),
            titan_object!("jimen", "nushi_jimen_fp_1090_020"),
            sub!("sub_041", "sub_041_pre_start"),
            main!("nushi_dragon_010", "nushi_dragon_010_always"),
            main!("nushi_dragon_010", "nushi_dragon_010_pre_start"),
            main!("nushi_dragon_020", "nushi_dragon_020_pre_start"),
            titan_object!("dragon", "nushi_dragon_fp_1035_010"),
            titan_object!("dragon", "nushi_dragon_fp_1035_020"),
            titan_object!("dragon", "nushi_dragon_fp_1056_010"),
            titan_object!("dragon", "nushi_dragon_fp_1056_020"),
            sub!("sub_040", "sub_040_pre_start"),
        ],
        release: None,
    },
    Group {
        key: "ruin",
        switch: "bosses.ruin",
        pool: "bosses.ruin.pool",
        pool_label: "Treasures of Ruin species",
        label: "Treasures of Ruin",
        help: "Ting-Lu, Chien-Pao, Wo-Chien and Chi-Yu behind the sealed shrines, including their shrine cutscenes.",
        labels: &["semi_legend_"],
        scenes: &[
            sub!("sub_014", "sub_014_main"),
            sub!("sub_015", "sub_015_main"),
            sub!("sub_016", "sub_016_main"),
            sub!("sub_017", "sub_017_main"),
        ],
        release: None,
    },
    Group {
        key: "area_zero",
        switch: "bosses.area_zero",
        pool: "bosses.area_zero.pool",
        pool_label: "Area Zero species",
        label: "Area Zero ambushes",
        help: "The Paradox Pokémon that attack on the way down to the Zero Lab, including the big fight with your friends. A species that appears twice gets the same new Pokémon both times.",
        labels: &["1055_multi_", "1075_multi_", "1095_multi_", "1180_multi_"],
        scenes: &[
            main!("common_1055", "common_1055_main"),
            main!("common_1075", "common_1075_main"),
            main!("common_1095", "common_1095_main"),
            main!("common_1170", "common_1170_always"),
        ],
        release: None,
    },
    Group {
        key: "story",
        switch: "bosses.story",
        pool: "bosses.story.pool",
        pool_label: "Lechonk and Houndoom species",
        label: "Lechonk and Houndoom",
        help: "The Lechonk in Nemona's catching lesson and the Houndoom that attacks Koraidon or Miraidon on the beach. Every Lechonk in that lesson's scene changes with it.",
        labels: &["common_0100_BUTA1", "common_0150_HERUGAA"],
        scenes: &[main!("common_0100", "common_0100_main"), main!("common_0150", "common_0150_main")],
        release: None,
    },
    Group {
        key: "sunflora",
        switch: "bosses.sunflora",
        pool: "bosses.sunflora.pool",
        pool_label: "Sunflora species",
        label: "Sunflora gym test",
        help: "The five Sunflora you find and battle in the Cortondo gym test. All five become the same Pokémon.",
        labels: &["gym_kusa_020_KIMAWARI_"],
        scenes: &["world/scene/parts/event/event_scenario/main_scenario/gym_kusa_poke_finding_/pokes.trsog"],
        release: None,
    },
    Group {
        key: "kitakami",
        switch: "bosses.kitakami",
        pool: "bosses.kitakami.pool",
        pool_label: "Kitakami story species",
        label: "Kitakami story",
        help: "Okidogi, Munkidori, Fezandipiti, Ogerpon and the Milotic from The Teal Mask, including their stronger rematches. Ogerpon's opening cutscene is pre-animated and may look odd with another Pokémon.",
        labels: &["SDC01_"],
        scenes: &[
            "world/scene/parts/demo/ev/d610_/d610.trscn",
            main!("sdc01_0105", "sdc01_0105_main"),
            main!("sdc01_0130", "sdc01_0130_always"),
            main!("sdc01_0160", "sdc01_0160_always"),
            main!("sdc01_0300", "sdc01_0300_main"),
            main!("sdc01_0330", "sdc01_0330_always"),
            main!("sdc01_0360", "sdc01_0360_pre_start"),
            main!("sdc01_0370", "sdc01_0370_post_end"),
            main!("sdc01_0390", "sdc01_0390_main"),
            main!("sdc01_0400", "sdc01_0400_main"),
            main!("sdc01_0410", "sdc01_0410_main"),
            main!("sdc01_0420", "sdc01_0420_main"),
            main!("sdc01_0430", "sdc01_0430_main"),
            main!("sdc01_0440", "sdc01_0440_always"),
            main!("sdc01_0460", "sdc01_0460_main"),
            main!("sdc01_0480", "sdc01_0480_main"),
            main!("sdc01_3gods_a", "sdc01_3gods_a_pre_start"),
            main!("sdc01_3gods_a", "sdc01_3gods_a_main"),
            main!("sdc01_3gods_b", "sdc01_3gods_b_pre_start"),
            main!("sdc01_3gods_b", "sdc01_3gods_b_main"),
            main!("sdc01_3gods_c_before", "sdc01_3gods_c_before_pre_start"),
            main!("sdc01_3gods_c", "sdc01_3gods_c_pre_start"),
            main!("sdc01_3gods_c", "sdc01_3gods_c_main"),
            sub!("s1_sub_011", "s1_sub_011_pre_start"),
            sub!("s1_sub_012", "s1_sub_012_pre_start"),
            sub!("s1_sub_016", "s1_sub_016_pre_start"),
        ],
        release: None,
    },
    Group {
        key: "kitakami_side",
        switch: "bosses.kitakami_side",
        pool: "bosses.kitakami_side.pool",
        pool_label: "Side quest species",
        label: "Kitakami side quest",
        help: "The Ariados and the Bloodmoon Ursaluna from Perrin's photo quest in Kitakami.",
        labels: &["S1_SIDE02_"],
        scenes: &[sub!("s1_side02_0030", "s1_side02_0030_main"), sub!("s1_side02_0050", "s1_side02_0050_main")],
        release: None,
    },
    Group {
        key: "indigo",
        switch: "bosses.indigo",
        pool: "bosses.indigo.pool",
        pool_label: "Indigo Disk species",
        label: "Indigo Disk story",
        help: "The Stellar Pokémon in the Area Zero Underdepths, the four Paradox legends, and both Terapagos fights. Stellar fights stay Stellar. Terapagos's arrival cutscene is pre-animated and may look odd with another Pokémon.",
        labels: &["SDC02_"],
        scenes: &[
            sub!("s2_sub_005", "s2_sub_005_pre_start"),
            main!("sdc02_0262", "sdc02_0262_main"),
            main!("sdc02_0263", "sdc02_0263_pre_start"),
            main!("sdc02_0265", "sdc02_0265_pre_start"),
            main!("sdc02_0267", "sdc02_0267_pre_start"),
            sub!("s2_side02_0010", "s2_side02_0010_pre_start"),
            sub!("s2_side02_0020", "s2_side02_0020_pre_start"),
            "world/scene/parts/demo/ev/d730_/d730.trscn",
            main!("sdc02_0320", "sdc02_0320_always"),
        ],
        release: None,
    },
    Group {
        key: "snacksworth",
        switch: "bosses.snacksworth",
        pool: "bosses.snacksworth.pool",
        pool_label: "Snacksworth species",
        label: "Snacksworth legendaries",
        help: "The 25 legendaries Snacksworth sends you to find after the Indigo Disk. His list of unlocked legendaries follows the new Pokémon.",
        labels: &["su2_legend_"],
        scenes: &[
            sub!("s2_sub_013", "s2_sub_013_pre_start"),
            sub!("s2_sub_014", "s2_sub_014_pre_start"),
            sub!("s2_sub_015", "s2_sub_015_pre_start"),
            sub!("s2_sub_016", "s2_sub_016_pre_start"),
            sub!("s2_sub_017", "s2_sub_017_pre_start"),
            sub!("s2_sub_018", "s2_sub_018_pre_start"),
            sub!("s2_sub_019", "s2_sub_019_pre_start"),
            sub!("s2_sub_020", "s2_sub_020_pre_start"),
            sub!("s2_sub_021", "s2_sub_021_pre_start"),
            sub!("s2_sub_022", "s2_sub_022_pre_start"),
            sub!("s2_sub_023", "s2_sub_023_pre_start"),
            sub!("s2_sub_024", "s2_sub_024_pre_start"),
            sub!("s2_sub_025", "s2_sub_025_pre_start"),
            sub!("s2_sub_026", "s2_sub_026_pre_start"),
            sub!("s2_sub_027", "s2_sub_027_pre_start"),
            sub!("s2_sub_028", "s2_sub_028_pre_start"),
            sub!("s2_sub_029", "s2_sub_029_pre_start"),
            sub!("s2_sub_030", "s2_sub_030_pre_start"),
            sub!("s2_sub_031", "s2_sub_031_pre_start"),
            sub!("s2_sub_032", "s2_sub_032_pre_start"),
            sub!("s2_sub_033", "s2_sub_033_pre_start"),
            sub!("s2_sub_034", "s2_sub_034_pre_start"),
            sub!("s2_sub_035", "s2_sub_035_pre_start"),
            sub!("s2_sub_036", "s2_sub_036_pre_start"),
            sub!("s2_sub_037", "s2_sub_037_pre_start"),
        ],
        release: Some(RELEASE),
    },
    Group {
        key: "mythicals",
        switch: "bosses.mythicals",
        pool: "bosses.mythicals.pool",
        pool_label: "Meloetta and Pecharunt species",
        label: "Meloetta and Pecharunt",
        help: "The Meloetta and Pecharunt encounters from the Indigo Disk side stories.",
        labels: &["su2_sub_Meloetta", "su2_dokutarou"],
        scenes: &[
            "world/scene/parts/event/event_scenario/sub_scenario/s2_sub_003_pop_/s2_sub_003_pop.trscn",
            sub!("s2_side01_0160", "s2_side01_0160_always"),
            sub!("s2_side01_0180", "s2_side01_0180_always"),
        ],
        release: None,
    },
];

impl Group {
    pub fn owns(&self, label: &str) -> bool {
        self.labels.iter().any(|prefix| label.starts_with(prefix))
    }

    /// Every scene file of the group, both versions.
    pub fn scene_files(&self) -> impl Iterator<Item = String> + '_ {
        self.scenes.iter().flat_map(|path| {
            let (stem, ext) = path.rsplit_once('.').expect("scene paths have an extension");
            [format!("{stem}_0.{ext}"), format!("{stem}_1.{ext}")]
        })
    }
}

pub fn group_of(label: &str) -> Option<&'static Group> {
    if UNTOUCHED.iter().any(|prefix| label.starts_with(prefix)) {
        return None;
    }
    GROUPS.iter().find(|g| g.owns(label))
}

pub fn is_demo(path: &str) -> bool {
    path.contains("/scene/parts/demo/")
}
