// The contract between the UI and the Rust backend (app/src-tauri). Field names are the serde
// names on the Rust side, so they stay snake_case. Every command rejects with a plain-language
// string the UI can show as is.

import { Channel, invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';

export type Edition = 'scarlet' | 'violet';
export const EDITIONS: Edition[] = ['scarlet', 'violet'];

// ---- Option registry (serialized straight from svr_core::rando::options) ----

export type Source = 'upr' | 'sv' | 'pknx' | 'new';
export type Status = 'data' | 'test' | 'research';

export interface ChoiceDef {
  value: string;
  label: string;
  help: string;
}

export type Kind =
  | { type: 'toggle'; default: boolean }
  | { type: 'choice'; default: string; choices: ChoiceDef[] }
  | { type: 'number'; default: number; min: number; max: number; step: number; unit: string }
  | { type: 'pool' }
  | { type: 'species' }
  | { type: 'text'; default: string; max_len: number };

export type Req =
  | { when: 'on'; id: string }
  | { when: 'off'; id: string }
  | { when: 'is'; id: string; value: string }
  | { when: 'not'; id: string; value: string };

export interface OptionDef {
  id: string;
  page: string;
  group: string;
  label: string;
  help: string;
  kind: Kind;
  requires: Req[];
  sources: Source[];
  status: Status;
}

/** Index order: generations and legendaries are Gen 1..9; stages are single-stage, first, middle,
 * final; regional_dex is Paldea, Kitakami, Blueberry. banned and allowed hold National Dex numbers. */
export interface PoolFilter {
  inherit: boolean;
  generations: boolean[];
  legendaries: boolean[];
  paradox: boolean;
  stages: boolean[];
  alt_forms: boolean;
  irregular_forms: boolean;
  regional_dex: boolean[];
  banned: number[];
  allowed: number[];
}

/** national null means "random". */
export interface SpeciesPick {
  national: number | null;
  form: number;
}

export type OptValue = boolean | number | string | PoolFilter | SpeciesPick;

export interface Settings {
  values: Record<string, OptValue>;
}

export interface PageInfo {
  id: string;
  title: string;
}

export interface Preset {
  id: string;
  name: string;
  description: string;
  /** Defaults with the preset applied, already normalized. */
  settings: Settings;
}

export interface Registry {
  /** Every page in Page::ALL order, including pages with no options yet. */
  pages: PageInfo[];
  options: OptionDef[];
  defaults: Settings;
  presets: Preset[];
}

export interface Normalized {
  settings: Settings;
  /** One plain sentence per repaired or dropped value. */
  warnings: string[];
}

/** One option whose value differs from its default, described in plain words. */
export interface ChangeLine {
  id: string;
  page: string;
  group: string;
  label: string;
  value: string;
  default: string;
}

export interface DecodedShare {
  settings: Settings;
  seed: string | null;
  warnings: string[];
}

// ---- Environment: Eden, keys, games, app config ----

export type PathSource = 'override' | 'detected' | 'default';

export interface ResolvedPath {
  /** null when nothing was found and nothing was set. */
  path: string | null;
  source: PathSource;
  exists: boolean;
}

export interface GameFileOverride {
  base: string | null;
  update: string | null;
}

/** Persisted in the app's own config dir. null means "detect it". */
export interface AppConfig {
  eden_exe: string | null;
  eden_data_dir: string | null;
  keys_file: string | null;
  load_dir: string | null;
  nand_dir: string | null;
  game_dirs: string[] | null;
  scarlet: GameFileOverride;
  violet: GameFileOverride;
  library_dir: string | null;
  /** Batch output folders outside the library. The library lists seeds found in them too. */
  extra_seed_dirs: string[];
  next_batch_index: number;
}

export interface EdenInfo {
  exe: ResolvedPath;
  data_dir: ResolvedPath;
  /** True when a `user` folder beside eden.exe is used as the data dir. */
  portable: boolean;
  keys: ResolvedPath;
  load_dir: ResolvedPath;
  nand_dir: ResolvedPath;
  game_dirs: string[];
  game_dirs_source: PathSource;
}

export type GameState = 'ready' | 'error' | 'missing';

export interface GameStatus {
  edition: Edition;
  /** Base title ID as 16 uppercase hex digits, e.g. 0100A3D008C5C000. */
  title_id: string;
  state: GameState;
  base: string | null;
  update: string | null;
  /** Plain-language problem for 'error' and 'missing'; null when ready. */
  message: string | null;
  /** Eden's add-on list has the "SV Randomizer" mod switched off for this game. */
  mod_disabled: boolean;
}

export interface Environment {
  config: AppConfig;
  eden: EdenInfo;
  /** Why the keys could not be loaded, or null when they loaded. */
  keys_error: string | null;
  library_dir: string;
  /** Always two entries, Scarlet then Violet. */
  games: GameStatus[];
}

// ---- Species data for the pool editor and species picker ----

export interface SpeciesEntry {
  national: number;
  form: number;
  /** Internal species id; names come from the game's own text by this id. */
  species_id: number;
  name: string;
  generation: number;
  legendary: boolean;
  paradox: boolean;
  /** Evolution stage slot: 0 single-stage, 1 first, 2 middle, 3 final. */
  stage: number;
  /** In the Paldea, Kitakami, Blueberry dex. */
  regional: boolean[];
}

export interface PoolCounts {
  /** Species forms allowed by each Kind::Pool option after inheritance, keyed by option id. */
  counts: Record<string, number>;
  /** Playable forms in the game, for "N of total". */
  total: number;
}

// ---- Randomizing ----

/** Scarlet's seeds come first, then Violet's, numbered on from Scarlet's. */
export interface BatchRequest {
  scarlet: number;
  violet: number;
  prefix: string;
  start_index: number;
  /** null writes into the seed library. */
  output_dir: string | null;
}

export interface RandomizeRequest {
  /** The game a single seed is made for. A batch says how many seeds each game gets instead. */
  edition: Edition;
  settings: Settings;
  /** Empty means a random seed per run. In a batch, a typed seed becomes `<seed>-<index>`. */
  seed: string;
  batch: BatchRequest | null;
  /** Also packs every seed the run makes into one zip file in this folder. */
  zip_dir: string | null;
}

export type SeedStage = 'randomizing' | 'writing' | 'logging';

export type RandoEvent =
  | { kind: 'started'; total: number }
  | { kind: 'seed_started'; position: number; name: string; seed: string }
  | { kind: 'seed_stage'; position: number; stage: SeedStage }
  | { kind: 'seed_done'; position: number; name: string; seed: string; folder: string; warnings: string[]; millis: number }
  | { kind: 'seed_failed'; position: number; name: string; seed: string; error: string }
  | { kind: 'packing'; path: string };

/** The seed folders are kept even when the zip fails. */
export type Packed = { state: 'done'; path: string } | { state: 'failed'; path: string; error: string };

export interface JobSummary {
  total: number;
  done: number;
  failed: number;
  cancelled: boolean;
  folders: string[];
  /** The start index the next batch should default to. */
  next_batch_index: number;
  /** null when no zip was asked for or no seed was made. */
  zip: Packed | null;
}

// ---- Seed library ----

export interface SeedEntry {
  /** Absolute seed folder path; also the seed's id in every library command. */
  path: string;
  name: string;
  /** Name of the batch output folder when the seed is outside the library, else null. */
  group: string | null;
  edition: Edition | null;
  title_id: string;
  seed: string;
  created_unix: number;
  file_count: number;
  /** True when this seed is the one installed in Eden's mod folder. */
  active: boolean;
  has_spoiler: boolean;
}

export interface SwitchCheck {
  edition: Edition;
  /** Name of the seed active in Eden for this game now, if any. */
  active_seed: string | null;
  /** Eden save folders for this game. Non-empty means warn before switching. */
  saves: string[];
}

export interface ActivateResult {
  installed_to: string;
  /** Folder the save was copied to, when a backup was requested. */
  backup_dir: string | null;
}

// ---- Commands ----

export interface FileFilter {
  name: string;
  extensions: string[];
}

const tauriApi = {
  getEnvironment: () => invoke<Environment>('get_environment'),
  saveConfig: (config: AppConfig) => invoke<Environment>('save_config', { config }),

  getRegistry: () => invoke<Registry>('get_registry'),
  normalizeSettings: (settings: Settings) => invoke<Normalized>('normalize_settings', { settings }),
  /** Species names resolve when that edition's game is ready; otherwise numbers are shown. */
  describeChanges: (settings: Settings, edition: Edition | null) =>
    invoke<ChangeLine[]>('describe_changes', { settings, edition }),
  loadSettingsFile: (path: string) => invoke<Normalized>('load_settings_file', { path }),
  saveSettingsFile: (path: string, settings: Settings) => invoke<void>('save_settings_file', { path, settings }),
  encodeShareCode: (settings: Settings, seed: string | null) => invoke<string>('encode_share_code', { settings, seed }),
  decodeShareCode: (code: string) => invoke<DecodedShare>('decode_share_code', { code }),

  listSpecies: (edition: Edition) => invoke<SpeciesEntry[]>('list_species', { edition }),
  poolCounts: (edition: Edition, settings: Settings) => invoke<PoolCounts>('pool_counts', { edition, settings }),

  randomize: (request: RandomizeRequest, onEvent: (event: RandoEvent) => void) => {
    const channel = new Channel<RandoEvent>();
    channel.onmessage = onEvent;
    return invoke<JobSummary>('randomize', { request, onEvent: channel });
  },
  cancelRandomize: () => invoke<void>('cancel_randomize'),

  listSeeds: () => invoke<SeedEntry[]>('list_seeds'),
  checkSwitch: (path: string) => invoke<SwitchCheck>('check_switch', { path }),
  activateSeed: (path: string, backupSave: boolean) => invoke<ActivateResult>('activate_seed', { path, backupSave }),
  deactivate: (edition: Edition) => invoke<void>('deactivate', { edition }),
  deleteSeed: (path: string) => invoke<void>('delete_seed', { path }),
  openPath: (path: string) => invoke<void>('open_path', { path }),
  launchEden: (edition: Edition) => invoke<void>('launch_eden', { edition }),

  pickFile: async (title: string, filters: FileFilter[], defaultPath?: string) =>
    (await open({ title, filters, defaultPath, multiple: false, directory: false })) as string | null,
  pickFolder: async (title: string, defaultPath?: string) =>
    (await open({ title, defaultPath, multiple: false, directory: true })) as string | null,
  pickSaveFile: (title: string, filters: FileFilter[], defaultPath?: string) => save({ title, filters, defaultPath }),
};

export type Api = typeof tauriApi;

/** Live binding: the dev build swaps in a mock when the page runs outside Tauri. */
export let api: Api = tauriApi;

export function useApi(impl: Api) {
  api = impl;
}
