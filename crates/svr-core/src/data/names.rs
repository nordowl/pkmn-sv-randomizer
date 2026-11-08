use crate::game::GameFiles;
use crate::text::{GameText, Language, TextError};

use super::Pokemon;

/// Display names from the game's own text. Species lines are indexed by internal species id.
#[derive(Clone, Debug, Default)]
pub struct Names {
    species: Vec<String>,
    moves: Vec<String>,
    items: Vec<String>,
    abilities: Vec<String>,
    types: Vec<String>,
}

fn lines(game: &GameFiles, lang: Language, name: &str) -> Result<Vec<String>, TextError> {
    let text = GameText::load(game, lang, name)?;
    Ok((0..text.file().len()).map(|i| text.line(i).unwrap_or_default()).collect())
}

fn lookup(list: &[String], id: usize, kind: &str) -> String {
    match list.get(id) {
        Some(name) if !name.is_empty() => name.clone(),
        _ => format!("{kind} #{id}"),
    }
}

impl Names {
    pub fn load(game: &GameFiles, lang: Language) -> Result<Names, TextError> {
        Ok(Names {
            species: lines(game, lang, "monsname")?,
            moves: lines(game, lang, "wazaname")?,
            items: lines(game, lang, "itemname")?,
            abilities: lines(game, lang, "tokusei")?,
            types: lines(game, lang, "typename")?,
        })
    }

    pub fn species(&self, species: u16) -> String {
        lookup(&self.species, usize::from(species), "Species")
    }

    pub fn pokemon(&self, id: Pokemon) -> String {
        let name = self.species(id.species);
        if id.form == 0 {
            name
        } else {
            format!("{name} (form {})", id.form)
        }
    }

    pub fn move_name(&self, id: u16) -> String {
        lookup(&self.moves, usize::from(id), "Move")
    }

    pub fn item(&self, id: u16) -> String {
        lookup(&self.items, usize::from(id), "Item")
    }

    pub fn ability(&self, id: u16) -> String {
        lookup(&self.abilities, usize::from(id), "Ability")
    }

    pub fn type_name(&self, id: u8) -> String {
        lookup(&self.types, usize::from(id), "Type")
    }
}
