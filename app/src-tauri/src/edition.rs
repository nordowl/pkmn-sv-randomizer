use std::fmt;

use serde::{Deserialize, Serialize};
use svr_core::game;

/// The app's serializable view of `svr_core::game::Edition`; the UI sends `scarlet` or `violet`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    Scarlet,
    Violet,
}

impl Edition {
    pub const ALL: [Edition; 2] = [Edition::Scarlet, Edition::Violet];

    pub fn core(self) -> game::Edition {
        match self {
            Edition::Scarlet => game::Edition::Scarlet,
            Edition::Violet => game::Edition::Violet,
        }
    }

    pub fn from_core(edition: game::Edition) -> Edition {
        match edition {
            game::Edition::Scarlet => Edition::Scarlet,
            game::Edition::Violet => Edition::Violet,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Edition::Scarlet => "scarlet",
            Edition::Violet => "violet",
        }
    }

    pub fn from_id(id: &str) -> Option<Edition> {
        Edition::ALL.into_iter().find(|edition| edition.id() == id)
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Edition::Scarlet => "Scarlet",
            Edition::Violet => "Violet",
        }
    }

    pub fn title_id(self) -> u64 {
        self.core().base_title_id()
    }

    pub fn title_id_hex(self) -> String {
        format!("{:016X}", self.title_id())
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.core().fmt(f)
    }
}
