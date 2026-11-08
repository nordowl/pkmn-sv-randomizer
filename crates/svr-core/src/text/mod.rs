//! Game text: the `.dat` line tables and `.tbl` label tables under `message/dat/<Lang>/common/`.

mod dat;
mod tbl;

pub use dat::TextFile;
pub use tbl::{Label, LabelTable};

use crate::game::GameFiles;
use crate::trinity::ArchiveError;

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("Invalid text file: {0}.")]
    BadDat(String),
    #[error("Invalid label table: {0}.")]
    BadTable(String),
    #[error("Invalid line text: {0}.")]
    BadText(String),
    #[error("The text file has no line {index}.")]
    LineOutOfRange { index: usize },
    #[error(transparent)]
    Archive(#[from] ArchiveError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Japanese,
    JapaneseKanji,
    English,
    French,
    Italian,
    German,
    Spanish,
    Korean,
    ChineseSimplified,
    ChineseTraditional,
}

impl Language {
    pub const ALL: [Language; 10] = [
        Language::Japanese,
        Language::JapaneseKanji,
        Language::English,
        Language::French,
        Language::Italian,
        Language::German,
        Language::Spanish,
        Language::Korean,
        Language::ChineseSimplified,
        Language::ChineseTraditional,
    ];

    pub fn folder(self) -> &'static str {
        match self {
            Language::Japanese => "JPN",
            Language::JapaneseKanji => "JPN_KANJI",
            Language::English => "English",
            Language::French => "French",
            Language::Italian => "Italian",
            Language::German => "German",
            Language::Spanish => "Spanish",
            Language::Korean => "Korean",
            Language::ChineseSimplified => "Simp_Chinese",
            Language::ChineseTraditional => "Trad_Chinese",
        }
    }

    pub fn dat_path(self, name: &str) -> String {
        format!("message/dat/{}/common/{name}.dat", self.folder())
    }

    pub fn tbl_path(self, name: &str) -> String {
        format!("message/dat/{}/common/{name}.tbl", self.folder())
    }
}

/// One common text file with its label table, when the game ships one.
#[derive(Clone, Debug)]
pub struct GameText {
    file: TextFile,
    labels: Option<LabelTable>,
}

impl GameText {
    pub fn new(file: TextFile, labels: Option<LabelTable>) -> GameText {
        GameText { file, labels }
    }

    pub fn load(game: &GameFiles, lang: Language, name: &str) -> Result<GameText, TextError> {
        let file = TextFile::parse(&game.read(&lang.dat_path(name))?)?;
        let tbl_path = lang.tbl_path(name);
        let labels = if game.contains(&tbl_path) {
            Some(LabelTable::parse(&game.read(&tbl_path)?)?)
        } else {
            None
        };
        Ok(GameText { file, labels })
    }

    pub fn file(&self) -> &TextFile {
        &self.file
    }

    pub fn file_mut(&mut self) -> &mut TextFile {
        &mut self.file
    }

    pub fn labels(&self) -> Option<&LabelTable> {
        self.labels.as_ref()
    }

    pub fn labels_mut(&mut self) -> Option<&mut LabelTable> {
        self.labels.as_mut()
    }

    pub fn line(&self, index: usize) -> Option<String> {
        self.file.line(index)
    }

    pub fn by_label(&self, label: &str) -> Option<String> {
        self.file.line(self.labels.as_ref()?.index_of(label)?)
    }
}
