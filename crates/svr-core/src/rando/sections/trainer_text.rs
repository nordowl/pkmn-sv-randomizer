//! Trainer names, class names and classes. Names and class names are text lines found by label,
//! rewritten with the same string in all ten languages.

use std::collections::{BTreeMap, BTreeSet};

use crate::fb::TableValue;
use crate::rando::{Ctx, RandoError, Rng};
use crate::text::{GameText, Language};

pub const NAME_TEXT: &str = "trname";
pub const CLASS_TEXT: &str = "trtype";
pub const MAX_NAME: usize = 12;
pub const MAX_CLASS: usize = 20;
const FEMALE: i64 = 1;

pub const MALE_NAMES: &[&str] = &[
    "Aaron", "Abel", "Adler", "Alder", "Alvin", "Ansel", "Archer", "Arlo", "Asher", "Barry", "Basil", "Bennet",
    "Blake", "Bram", "Brock", "Caden", "Calvin", "Cedric", "Clay", "Colby", "Conrad", "Cyrus", "Dalton", "Damian",
    "Darius", "Dexter", "Dorian", "Edgar", "Elio", "Emmett", "Ezra", "Felix", "Finn", "Flint", "Garrett", "Gideon",
    "Grant", "Gus", "Hale", "Harvey", "Hector", "Hugo", "Ivan", "Jasper", "Jonah", "Jude", "Kai", "Keaton", "Kellan",
    "Laszlo", "Leon", "Levi", "Lorcan", "Lucas", "Magnus", "Marco", "Milo", "Nash", "Nico", "Noel", "Oakley",
    "Orson", "Otto", "Pablo", "Percy", "Quentin", "Rafe", "Remy", "Rhett", "Roland", "Rowan", "Rufus", "Silas",
    "Soren", "Tobias", "Tristan", "Ulric", "Victor", "Wade", "Wesley", "Xavier", "Yusuf", "Zane",
];

pub const FEMALE_NAMES: &[&str] = &[
    "Abby", "Ada", "Alma", "Amara", "Anya", "Aria", "Astrid", "Aurora", "Bea", "Bianca", "Briar", "Calla", "Camila",
    "Cora", "Dahlia", "Daisy", "Delia", "Elena", "Elise", "Ember", "Esme", "Fern", "Fiona", "Freya", "Gemma",
    "Greta", "Hana", "Hazel", "Helena", "Ida", "Imogen", "Iris", "Isla", "Juniper", "June", "Kira", "Lena", "Lila",
    "Luna", "Lydia", "Mabel", "Maeve", "Maren", "Mila", "Nadia", "Nell", "Nina", "Nova", "Olive", "Opal", "Paige",
    "Pearl", "Petra", "Quinn", "Rhea", "Rosa", "Ruby", "Sadie", "Selma", "Sienna", "Sora", "Talia", "Thea",
    "Tessa", "Una", "Vera", "Violet", "Willa", "Wren", "Xena", "Yara", "Yuki", "Zara", "Zoe",
];

pub const CLASS_NAMES: &[&str] = &[
    "Ace Trainer", "Aroma Lady", "Battle Girl", "Beauty", "Biker", "Bird Keeper", "Breeder", "Bug Catcher",
    "Camper", "Clown", "Collector", "Cowgirl", "Cyclist", "Dancer", "Delinquent", "Expert", "Fairy Tale Girl",
    "Fisher", "Gambler", "Gentleman", "Guitarist", "Harlequin", "Idol", "Juggler", "Kindler", "Lady", "Lass",
    "Madame", "Maid", "Monsieur", "Ninja Boy", "Painter", "Parasol Lady", "Picnicker", "Pilot", "Pokefan",
    "Psychic", "Punk Girl", "Punk Guy", "Ranger", "Rich Boy", "Rocker", "Ruin Maniac", "Sailor", "Sightseer",
    "Skier", "Socialite", "Swimmer", "Tourist", "Veteran", "Worker",
];

/// Parses a comma or newline separated list, keeping entries that fit and that game text can
/// store. An empty result means "use the built-in list".
pub fn parse_list(text: &str, max_len: usize) -> Vec<String> {
    text.split([',', '\n', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= max_len)
        .filter(|s| !s.contains(['[', '\\', '\0', '\u{10}']))
        .map(str::to_owned)
        .collect()
}

/// Hands out entries of a shuffled list without repeats until it runs out, then reshuffles.
pub struct Deck {
    cards: Vec<String>,
    next: usize,
}

impl Deck {
    pub fn new(cards: Vec<String>, rng: &mut Rng) -> Deck {
        let mut cards = cards;
        rng.shuffle(&mut cards);
        Deck { cards, next: 0 }
    }

    pub fn draw(&mut self, rng: &mut Rng) -> String {
        if self.next >= self.cards.len() {
            rng.shuffle(&mut self.cards);
            self.next = 0;
        }
        self.next += 1;
        self.cards[self.next - 1].clone()
    }
}

/// What the plain trainer type table says about each type.
pub struct TypeInfo {
    pub msg_label: String,
    pub female: bool,
    /// No intro timeline, camera or category: a route-style trainer.
    pub plain: bool,
}

pub fn read_types(table: &TableValue) -> BTreeMap<String, TypeInfo> {
    table
        .tables("values")
        .map(|t| {
            let empty = |f: &str| t.string(f).unwrap_or("").is_empty();
            let info = TypeInfo {
                msg_label: t.string("msgLabel").unwrap_or("").to_owned(),
                female: t.int("sex").unwrap_or(0) == FEMALE,
                plain: empty("introTml") && empty("cameraObjectName") && t.int("category").unwrap_or(0) == 0,
            };
            (t.string("nameLabel").unwrap_or("").to_owned(), info)
        })
        .collect()
}

/// New text per label, written into the given common text file of every language. Returns the
/// English lines before and after, for the log.
pub fn rewrite(
    ctx: &mut Ctx,
    name: &str,
    changes: &BTreeMap<String, String>,
) -> Result<Vec<(String, String, String)>, RandoError> {
    let text_err = |e: crate::text::TextError| RandoError::Text(e.to_string());
    let mut log = Vec::new();
    for lang in Language::ALL {
        let mut text = GameText::load(ctx.game, lang, name).map_err(text_err)?;
        for (label, value) in changes {
            let Some(index) = text.labels().and_then(|l| l.index_of(label)) else {
                continue;
            };
            if lang == Language::English {
                log.push((label.clone(), text.line(index).unwrap_or_default(), value.clone()));
            }
            text.file_mut().set_line(index, value).map_err(text_err)?;
        }
        ctx.files.insert(lang.dat_path(name), text.file().to_bytes());
    }
    Ok(log)
}

/// Labels whose English line is plain text, so replacing it cannot drop a game variable.
pub fn plain_labels(ctx: &Ctx, name: &str, labels: &BTreeSet<String>) -> Result<BTreeSet<String>, RandoError> {
    let text = GameText::load(ctx.game, Language::English, name).map_err(|e| RandoError::Text(e.to_string()))?;
    Ok(labels
        .iter()
        .filter(|label| text.by_label(label).is_some_and(|line| !line.is_empty() && !line.contains('[')))
        .cloned()
        .collect())
}
