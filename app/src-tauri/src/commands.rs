//! The Tauri commands the UI calls (see app/ui/src/lib/api.ts). Each one is a thin call into the
//! plain modules. Anything that touches the disk runs off the main thread.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use svr_core::rando::Settings;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::config::AppConfig;
use crate::describe::{describe_changes as describe, ChangeLine};
use crate::edition::Edition;
use crate::games::{PoolCounts, SpeciesEntry};
use crate::job::{self, JobInput, JobSummary, RandoEvent, RandomizeRequest};
use crate::library::{self, ActivateResult, SeedEntry, SeedRoots, SwitchCheck};
use crate::settings::{self, Normalized, RegistryInfo};
use crate::share::{self, DecodedShare};
use crate::state::{AppState, Environment};

async fn blocking<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<AppState>()))
        .await
        .map_err(|_| "Something went wrong inside the app. Try again, and restart the app if it keeps happening.".to_owned())?
}

#[tauri::command]
pub async fn get_environment(app: AppHandle) -> Result<Environment, String> {
    blocking(app, |state| Ok(state.environment())).await
}

#[tauri::command]
pub async fn save_config(app: AppHandle, config: AppConfig) -> Result<Environment, String> {
    blocking(app, move |state| state.save_config(config)).await
}

#[tauri::command]
pub fn get_registry(state: State<'_, AppState>) -> Result<RegistryInfo, String> {
    Ok(settings::registry_info(&state.registry))
}

#[tauri::command]
pub fn normalize_settings(state: State<'_, AppState>, settings: Settings) -> Result<Normalized, String> {
    Ok(settings::normalize(&state.registry, &settings))
}

#[tauri::command]
pub async fn describe_changes(
    app: AppHandle,
    settings: Settings,
    edition: Option<Edition>,
) -> Result<Vec<ChangeLine>, String> {
    blocking(app, move |state| {
        let game = edition.and_then(|edition| state.cached_game(edition));
        let species = game.as_deref().and_then(|game| game.species().ok());
        Ok(describe(&state.registry, &settings, species))
    })
    .await
}

#[tauri::command]
pub async fn load_settings_file(app: AppHandle, path: PathBuf) -> Result<Normalized, String> {
    blocking(app, move |state| settings::load_file(&path, &state.registry)).await
}

#[tauri::command]
pub async fn save_settings_file(app: AppHandle, path: PathBuf, settings: Settings) -> Result<(), String> {
    blocking(app, move |state| settings::save_file(&path, &state.registry, &settings)).await
}

#[tauri::command]
pub fn encode_share_code(
    state: State<'_, AppState>,
    settings: Settings,
    seed: Option<String>,
) -> Result<String, String> {
    Ok(share::encode(&state.registry, &settings, seed.as_deref()))
}

#[tauri::command]
pub fn decode_share_code(state: State<'_, AppState>, code: String) -> Result<DecodedShare, String> {
    share::decode(&state.registry, &code)
}

#[tauri::command]
pub async fn list_species(app: AppHandle, edition: Edition) -> Result<Vec<SpeciesEntry>, String> {
    blocking(app, move |state| Ok(state.game(edition)?.species()?.entries.clone())).await
}

#[tauri::command]
pub async fn pool_counts(app: AppHandle, edition: Edition, settings: Settings) -> Result<PoolCounts, String> {
    blocking(app, move |state| {
        let game = state.game(edition)?;
        Ok(game.species()?.pool_counts(&state.registry, &settings))
    })
    .await
}

#[tauri::command]
pub async fn randomize(
    app: AppHandle,
    request: RandomizeRequest,
    on_event: Channel<RandoEvent>,
) -> Result<JobSummary, String> {
    blocking(app, move |state| {
        let _running = state.begin_job()?;
        let game = state.game(request.edition)?;
        let config = state.config();
        let library_dir = SeedRoots::new(&config, &state.dirs.app_data).library;
        let input = JobInput {
            request: &request,
            game: &game,
            registry: &state.registry,
            library_dir: &library_dir,
            next_batch_index: config.next_batch_index,
        };
        let summary = job::run(&input, &state.cancel, &mut |event| {
            let _ = on_event.send(event);
        })?;
        // The seeds are already written; failing to remember the batch number must not hide them.
        let _ = state.update_config(|config| job::record_batch(config, &request, &summary, &library_dir));
        Ok(summary)
    })
    .await
}

#[tauri::command]
pub fn cancel_randomize(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn list_seeds(app: AppHandle) -> Result<Vec<SeedEntry>, String> {
    blocking(app, |state| {
        let eden = state.eden();
        Ok(library::list_seeds(&state.seed_roots(), eden.load_dir().ok()))
    })
    .await
}

#[tauri::command]
pub async fn check_switch(app: AppHandle, path: PathBuf) -> Result<SwitchCheck, String> {
    blocking(app, move |state| {
        let eden = state.eden();
        library::check_switch(&path, &state.seed_roots(), eden.load_dir().ok(), eden.nand_dir())
    })
    .await
}

#[tauri::command]
pub async fn activate_seed(app: AppHandle, path: PathBuf, backup_save: bool) -> Result<ActivateResult, String> {
    blocking(app, move |state| {
        let eden = state.eden();
        let backups = state.dirs.app_data.join("save-backups");
        library::activate_seed(&path, eden.load_dir()?, eden.nand_dir(), backup_save.then_some(backups.as_path()))
    })
    .await
}

#[tauri::command]
pub async fn deactivate(app: AppHandle, edition: Edition) -> Result<(), String> {
    blocking(app, move |state| library::deactivate(state.eden().load_dir()?, edition)).await
}

#[tauri::command]
pub async fn delete_seed(app: AppHandle, path: PathBuf) -> Result<(), String> {
    blocking(app, move |state| library::delete_seed(&path, &state.seed_roots())).await
}

#[tauri::command]
pub fn open_path(path: PathBuf) -> Result<(), String> {
    library::open_path(&path)
}

#[tauri::command]
pub async fn launch_eden(app: AppHandle, edition: Edition) -> Result<(), String> {
    blocking(app, move |state| {
        let game = state.game(edition)?;
        let exe = state.eden().info.exe.path.ok_or_else(|| "Choose eden.exe in Settings first.".to_owned())?;
        library::launch_eden(&exe, &game.base)
    })
    .await
}
