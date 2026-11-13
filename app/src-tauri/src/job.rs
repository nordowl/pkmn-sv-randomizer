//! A randomize run: one seed or a batch, written into seed folders, reported through events.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use svr_core::output::{write_mod, SeedMeta};
use svr_core::rando::options::{OptionDef, Settings};
use svr_core::rando::{self, Seed};

use crate::config::AppConfig;
use crate::describe::describe_changes;
use crate::edition::Edition;
use crate::errors::{io_failure, sentence};
use crate::games::OpenedGame;
use crate::library::free_folder;
use crate::pack;
use crate::settings::file_bytes;
use crate::spoiler;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_BATCH: u32 = 999;
const MAX_NAME_CHARS: usize = 80;

#[derive(Clone, Debug, Deserialize)]
pub struct RandomizeRequest {
    /// The game a single seed is made for. A batch says how many seeds each game gets instead.
    pub edition: Edition,
    pub settings: Settings,
    pub seed: String,
    pub batch: Option<BatchRequest>,
    /// Also packs every seed the run makes into one zip file in this folder.
    pub zip_dir: Option<PathBuf>,
}

/// Scarlet's seeds come first, then Violet's, numbered on from Scarlet's.
#[derive(Clone, Debug, Deserialize)]
pub struct BatchRequest {
    pub scarlet: u32,
    pub violet: u32,
    pub prefix: String,
    pub start_index: u32,
    pub output_dir: Option<PathBuf>,
}

impl BatchRequest {
    pub fn count(&self, edition: Edition) -> u32 {
        match edition {
            Edition::Scarlet => self.scarlet,
            Edition::Violet => self.violet,
        }
    }

    pub fn total(&self) -> u32 {
        self.scarlet.saturating_add(self.violet)
    }

    /// Seeds for both games carry the game in their name, so the two halves can be told apart.
    fn mixed(&self) -> bool {
        self.scarlet > 0 && self.violet > 0
    }
}

impl RandomizeRequest {
    /// The games this run opens.
    pub fn editions(&self) -> Vec<Edition> {
        match &self.batch {
            Some(batch) => Edition::ALL.into_iter().filter(|&edition| batch.count(edition) > 0).collect(),
            None => vec![self.edition],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedStage {
    Randomizing,
    Writing,
    Logging,
}

/// `position` counts seeds in this run from 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RandoEvent {
    Started {
        total: u32,
    },
    SeedStarted {
        position: u32,
        name: String,
        seed: String,
    },
    SeedStage {
        position: u32,
        stage: SeedStage,
    },
    SeedDone {
        position: u32,
        name: String,
        seed: String,
        folder: PathBuf,
        warnings: Vec<String>,
        millis: u64,
    },
    SeedFailed {
        position: u32,
        name: String,
        seed: String,
        error: String,
    },
    /// The seeds are done and the zip file is being written.
    Packing {
        path: PathBuf,
    },
}

/// The zip file of a run, or why it could not be written. The seed folders are kept either way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Packed {
    Done { path: PathBuf },
    Failed { path: PathBuf, error: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JobSummary {
    pub total: u32,
    pub done: u32,
    pub failed: u32,
    pub cancelled: bool,
    pub folders: Vec<PathBuf>,
    pub next_batch_index: u32,
    /// None when no zip was asked for or no seed was made.
    pub zip: Option<Packed>,
}

fn validate(request: &RandomizeRequest) -> Result<(), String> {
    if let Some(batch) = &request.batch {
        if !(1..=MAX_BATCH).contains(&batch.total()) {
            return Err(format!("A batch makes between 1 and {MAX_BATCH} seeds. Change the number of seeds."));
        }
        if batch.prefix.trim().is_empty() {
            return Err("Give the batch a name, for example \"Run \", so its seed folders can be told apart.".to_owned());
        }
        if batch.start_index.checked_add(batch.total()).is_none() {
            return Err("The batch start number is too large. Choose a smaller one.".to_owned());
        }
    }
    Ok(())
}

/// Windows refuses some characters and names in folders, and core stages each seed folder at
/// `<name>.partial`, so dots become underscores too.
fn folder_name(raw: &str) -> String {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let replaced: String = raw
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*."#.contains(c) { '_' } else { c })
        .take(MAX_NAME_CHARS)
        .collect();
    let mut name = replaced.trim().to_owned();
    if name.is_empty() {
        name = "Seed".to_owned();
    }
    if RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(&name)) {
        name.push('_');
    }
    name
}

/// Random seeds are drawn when each seed starts, so seeds made in quick succession still differ.
enum SeedText {
    Random,
    Fixed(String),
}

struct Planned {
    edition: Edition,
    seed: SeedText,
    /// The batch folder name; `None` for a single run, whose name comes from its seed.
    batch_name: Option<String>,
}

fn plan(request: &RandomizeRequest) -> Vec<Planned> {
    let typed = request.seed.trim();
    let Some(batch) = &request.batch else {
        let seed = if typed.is_empty() { SeedText::Random } else { SeedText::Fixed(typed.to_owned()) };
        return vec![Planned {
            edition: request.edition,
            seed,
            batch_name: None,
        }];
    };
    let editions = Edition::ALL
        .into_iter()
        .flat_map(|edition| (0..batch.count(edition)).map(move |_| edition));
    (batch.start_index..)
        .zip(editions)
        .map(|(index, edition)| {
            let number = format!("{}{index:03}", batch.prefix.trim_start());
            Planned {
                edition,
                seed: if typed.is_empty() {
                    SeedText::Random
                } else {
                    SeedText::Fixed(format!("{typed}-{index}"))
                },
                batch_name: Some(if batch.mixed() {
                    format!("{number} {}", edition.short_name())
                } else {
                    number
                }),
            }
        })
        .collect()
}

/// "Scarlet 1234" for one seed, "Run 001-010" for a batch.
fn zip_name(request: &RandomizeRequest, first_folder: &str) -> String {
    let Some(batch) = &request.batch else {
        return first_folder.to_owned();
    };
    let prefix = batch.prefix.trim_start();
    let first = batch.start_index;
    match batch.total() {
        1 => format!("{prefix}{first:03}"),
        total => format!("{prefix}{first:03}-{:03}", first + total - 1),
    }
}

pub struct JobInput<'a> {
    pub request: &'a RandomizeRequest,
    /// Every game in `request.editions()`.
    pub games: &'a [&'a OpenedGame],
    pub registry: &'a [OptionDef],
    pub library_dir: &'a Path,
    pub next_batch_index: u32,
}

/// Runs the whole request. Each seed reports `seed_started`, its stages, then `seed_done` or
/// `seed_failed`; a failed seed does not stop the batch. `cancel` is checked before each seed.
pub fn run(input: &JobInput, cancel: &AtomicBool, emit: &mut dyn FnMut(RandoEvent)) -> Result<JobSummary, String> {
    let request = input.request;
    validate(request)?;
    let game_for = |edition: Edition| {
        input
            .games
            .iter()
            .copied()
            .find(|game| game.edition == edition)
            .ok_or_else(|| format!("{edition} is not loaded. Check the game files in Settings."))
    };
    for edition in request.editions() {
        game_for(edition)?;
    }
    let root = request
        .batch
        .as_ref()
        .and_then(|batch| batch.output_dir.as_deref())
        .unwrap_or(input.library_dir);
    fs::create_dir_all(root).map_err(|e| io_failure("create the output folder", root, &e))?;

    let planned = plan(request);
    let total = planned.len() as u32;
    let mut summary = JobSummary {
        total,
        done: 0,
        failed: 0,
        cancelled: false,
        folders: Vec::new(),
        next_batch_index: input.next_batch_index,
        zip: None,
    };
    emit(RandoEvent::Started { total });

    let mut used_seeds = HashSet::new();
    for (position, planned) in (0u32..).zip(planned) {
        if cancel.load(Ordering::SeqCst) {
            summary.cancelled = true;
            break;
        }
        let seed = match planned.seed {
            SeedText::Fixed(text) => Seed(text),
            SeedText::Random => loop {
                let seed = Seed::random();
                if !used_seeds.contains(&seed.0) {
                    break seed;
                }
            },
        };
        used_seeds.insert(seed.0.clone());
        let raw_name = planned
            .batch_name
            .unwrap_or_else(|| format!("{} {}", planned.edition.short_name(), seed.0));
        let dir = free_folder(root, &folder_name(&raw_name));
        let name = dir.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        emit(RandoEvent::SeedStarted {
            position,
            name: name.clone(),
            seed: seed.0.clone(),
        });

        let started = Instant::now();
        let game = game_for(planned.edition)?;
        match make_seed(input, game, &seed, &dir, &mut |stage| emit(RandoEvent::SeedStage { position, stage })) {
            Ok(warnings) => {
                summary.done += 1;
                summary.folders.push(dir.clone());
                emit(RandoEvent::SeedDone {
                    position,
                    name,
                    seed: seed.0,
                    folder: dir,
                    warnings,
                    millis: started.elapsed().as_millis() as u64,
                });
            }
            Err(error) => {
                summary.failed += 1;
                let _ = fs::remove_dir_all(dir.with_extension("partial"));
                emit(RandoEvent::SeedFailed {
                    position,
                    name,
                    seed: seed.0,
                    error,
                });
            }
        }
    }

    if let Some(batch) = &request.batch {
        summary.next_batch_index = batch.start_index + summary.done + summary.failed;
    }
    if let (Some(zip_dir), Some(first)) = (&request.zip_dir, summary.folders.first()) {
        let first_name = first.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let path = pack::free_zip(zip_dir, &folder_name(&zip_name(request, &first_name)));
        emit(RandoEvent::Packing { path: path.clone() });
        summary.zip = Some(match pack::zip_folders(&path, &summary.folders) {
            Ok(()) => Packed::Done { path },
            Err(error) => Packed::Failed { path, error },
        });
    }
    Ok(summary)
}

fn make_seed(
    input: &JobInput,
    game: &OpenedGame,
    seed: &Seed,
    dir: &Path,
    stage: &mut dyn FnMut(SeedStage),
) -> Result<Vec<String>, String> {
    stage(SeedStage::Randomizing);
    let output = rando::randomize(&game.files, &input.request.settings, seed.clone())
        .map_err(|error| format!("The randomizer stopped: {}", sentence(error.to_string())))?;

    stage(SeedStage::Logging);
    let created = chrono::Local::now();
    let changes = describe_changes(input.registry, &output.settings, game.species().ok());
    let created_text = created.format("%Y-%m-%d %H:%M").to_string();
    let header = spoiler::Header {
        edition: game.edition,
        seed: &seed.0,
        created: &created_text,
        app_version: APP_VERSION,
    };
    let spoiler_text = spoiler::text(&header, &changes, &output.log);
    let spoiler_html = spoiler::html(&header, &changes, &output.log);
    let settings_json = file_bytes(&output.settings);

    stage(SeedStage::Writing);
    let meta = SeedMeta {
        app_version: APP_VERSION.to_owned(),
        edition: game.edition.id().to_owned(),
        title_id: game.edition.title_id_hex(),
        seed: seed.0.clone(),
        created_unix: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        file_count: output.files.len(),
    };
    let extras: [(&str, &[u8]); 3] = [
        ("spoiler.txt", spoiler_text.as_bytes()),
        ("spoiler.html", spoiler_html.as_bytes()),
        ("settings.json", &settings_json),
    ];
    write_mod(dir, &game.files, &output.files, &meta, &extras)
        .map_err(|error| format!("Could not save the seed: {}", sentence(error.to_string())))?;
    Ok(output.warnings)
}

/// Remembers where the next batch starts and any batch folder outside the library.
pub fn record_batch(config: &mut AppConfig, request: &RandomizeRequest, summary: &JobSummary, library_dir: &Path) {
    let Some(batch) = &request.batch else {
        return;
    };
    config.next_batch_index = summary.next_batch_index;
    if let Some(dir) = &batch.output_dir {
        if dir != library_dir && !config.extra_seed_dirs.contains(dir) {
            config.extra_seed_dirs.push(dir.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(seed: &str, batch: Option<BatchRequest>) -> RandomizeRequest {
        RandomizeRequest {
            edition: Edition::Scarlet,
            settings: Settings::default(),
            seed: seed.to_owned(),
            batch,
            zip_dir: None,
        }
    }

    fn batch(count: u32, prefix: &str) -> BatchRequest {
        BatchRequest {
            scarlet: count,
            violet: 0,
            prefix: prefix.to_owned(),
            start_index: 7,
            output_dir: None,
        }
    }

    #[test]
    fn names_are_safe_for_windows() {
        assert_eq!(folder_name("Scarlet a/b:c?.d"), "Scarlet a_b_c__d");
        assert_eq!(folder_name("  "), "Seed");
        assert_eq!(folder_name("con"), "con_");
        assert_eq!(folder_name(&"x".repeat(200)).chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn batches_number_their_seeds_and_folders() {
        let planned = plan(&request(" hello ", Some(batch(3, "Run "))));
        let names: Vec<_> = planned.iter().map(|p| p.batch_name.clone().unwrap()).collect();
        assert_eq!(names, ["Run 007", "Run 008", "Run 009"]);
        assert!(matches!(&planned[2].seed, SeedText::Fixed(text) if text == "hello-9"));
        assert!(matches!(plan(&request("", Some(batch(1, "B"))))[0].seed, SeedText::Random));
        assert!(matches!(&plan(&request("abc", None))[0].seed, SeedText::Fixed(text) if text == "abc"));
    }

    #[test]
    fn mixed_batches_make_scarlet_then_violet_and_say_which_is_which() {
        let mut both = batch(2, "Run ");
        both.violet = 1;
        let mixed = request("", Some(both));
        let names: Vec<_> = plan(&mixed).into_iter().map(|p| (p.edition, p.batch_name.unwrap())).collect();
        assert_eq!(
            names,
            [
                (Edition::Scarlet, "Run 007 Scarlet".to_owned()),
                (Edition::Scarlet, "Run 008 Scarlet".to_owned()),
                (Edition::Violet, "Run 009 Violet".to_owned()),
            ]
        );
        assert_eq!(mixed.editions(), [Edition::Scarlet, Edition::Violet]);
        assert_eq!(zip_name(&mixed, "Run 007 Scarlet"), "Run 007-009");

        let mut violet = batch(0, "Run ");
        violet.violet = 1;
        let violet_only = request("", Some(violet));
        assert_eq!(violet_only.editions(), [Edition::Violet]);
        assert_eq!(plan(&violet_only)[0].batch_name.as_deref(), Some("Run 007"));
        assert_eq!(zip_name(&violet_only, "Run 007"), "Run 007");
        assert_eq!(zip_name(&request("", None), "Scarlet 42"), "Scarlet 42");
    }

    #[test]
    fn bad_batches_are_refused_plainly() {
        assert!(validate(&request("", Some(batch(0, "Run ")))).is_err());
        let mut too_many = batch(999, "Run ");
        too_many.violet = 1;
        assert!(validate(&request("", Some(too_many))).is_err());
        assert!(validate(&request("", Some(batch(1000, "Run ")))).is_err());
        assert!(validate(&request("", Some(batch(5, "   ")))).is_err());
        assert!(validate(&request("", Some(batch(999, "Run ")))).is_ok());
        assert!(validate(&request("", None)).is_ok());
    }

    #[test]
    fn batches_advance_the_start_index_and_remember_outside_folders() {
        let mut config = AppConfig::default();
        let mut request = request("", Some(batch(4, "Run ")));
        request.batch.as_mut().unwrap().output_dir = Some(PathBuf::from("D:/Batches"));
        let summary = JobSummary {
            total: 4,
            done: 2,
            failed: 1,
            cancelled: true,
            folders: Vec::new(),
            next_batch_index: 10,
            zip: None,
        };
        record_batch(&mut config, &request, &summary, Path::new("C:/Library"));
        record_batch(&mut config, &request, &summary, Path::new("C:/Library"));
        assert_eq!(config.next_batch_index, 10);
        assert_eq!(config.extra_seed_dirs, [PathBuf::from("D:/Batches")]);
    }
}
