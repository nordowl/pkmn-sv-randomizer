//! Game data as the randomizer sees it: tables loaded on demand, a species index, scene patching
//! and classifications the game does not store.

pub mod dex;
pub mod facts;
pub mod names;
pub mod scene;
pub mod store;

pub use dex::{Dex, Evolution, FormInfo, Pokemon};
pub use names::Names;
pub use store::TableStore;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("could not read {path}: {detail}")]
    Read { path: String, detail: String },
    #[error("{0} has no schema")]
    NoSchema(String),
    #[error("schema {path} is invalid: {detail}")]
    Schema { path: String, detail: String },
    #[error("could not parse {path}: {detail}")]
    Parse { path: String, detail: String },
    #[error("could not write {path}: {detail}")]
    Serialize { path: String, detail: String },
    #[error("unexpected table shape: {0}")]
    Shape(String),
}
