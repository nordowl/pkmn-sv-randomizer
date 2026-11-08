import { untrack } from 'svelte';
import {
  api,
  EDITIONS,
  type AppConfig,
  type Edition,
  type Environment,
  type FileFilter,
  type GameStatus,
  type OptionDef,
  type OptValue,
  type PoolCounts,
  type Preset,
  type Registry,
  type Settings,
  type SpeciesEntry,
} from './api';
import { EDITION_NAMES, errorText, folderName } from './format';
import { applies, Catalog, GLOBAL_POOL, poolName, SearchIndex } from './options';

export type View = { kind: 'page'; id: string } | { kind: 'library' } | { kind: 'settings' };

export interface BatchForm {
  enabled: boolean;
  count: number;
  prefix: string;
  /** null follows the backend's `next_batch_index`. */
  startIndex: number | null;
  /** null writes into the seed library. */
  outputDir: string | null;
}

export type Loadable<T> = { state: 'loading' } | { state: 'ready'; value: T } | { state: 'failed'; error: string };

export type Tone = 'error' | 'success' | 'info' | 'warning';

export interface Toast {
  id: number;
  tone: Tone;
  title: string;
  body?: string;
  lines?: string[];
  action?: { label: string; run: () => void };
}

/** Why Randomize cannot start right now. */
export type Block =
  | { kind: 'checking' }
  | { kind: 'env-failed' }
  | { kind: 'game'; status: GameStatus }
  | { kind: 'pool'; def: OptionDef }
  | { kind: 'batch-name' }
  | { kind: 'running' };

export function blockReason(block: Block, catalog: Catalog | null): string {
  switch (block.kind) {
    case 'checking':
      return 'Checking your games first…';
    case 'env-failed':
      return 'Your games could not be checked. Open App settings and try again.';
    case 'game':
      return `${EDITION_NAMES[block.status.edition]} is not ready yet.`;
    case 'pool':
      return `${poolName(block.def, catalog)} has no species left.`;
    case 'batch-name':
      return 'Give the batch a folder name first.';
    case 'running':
      return 'A run is already going.';
  }
}

interface PoolCountState {
  edition: Edition;
  state: 'loading' | 'ready' | 'failed';
  /** Kept from the previous answer while a new one loads, so counts do not flicker. */
  value: PoolCounts | null;
  error: string | null;
}

const SETTINGS_FILES: FileFilter[] = [{ name: 'Randomizer settings', extensions: ['json'] }];
const STORE_KEY = 'svr.ui.v1';

function readStored(): Record<string, unknown> {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : null;
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function writeStored(value: object): void {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(value));
  } catch {
    // Blocked or full storage only means the working state is not remembered.
  }
}

function integer(raw: unknown, min: number, max: number, fallback: number): number {
  return typeof raw === 'number' && Number.isInteger(raw) ? Math.min(max, Math.max(min, raw)) : fallback;
}

function parseView(raw: unknown, catalog: Catalog): View {
  const view = raw as Partial<{ kind: string; id: unknown }> | null;
  if (view?.kind === 'library') return { kind: 'library' };
  if (view?.kind === 'settings') return { kind: 'settings' };
  if (view?.kind === 'page' && typeof view.id === 'string' && catalog.hasPage(view.id)) return { kind: 'page', id: view.id };
  return { kind: 'page', id: catalog.registry.pages[0]?.id ?? '' };
}

function parseBatch(raw: unknown): BatchForm {
  const batch = (raw && typeof raw === 'object' ? raw : {}) as Partial<Record<keyof BatchForm, unknown>>;
  return {
    enabled: batch.enabled === true,
    count: integer(batch.count, 1, 999, 5),
    prefix: typeof batch.prefix === 'string' ? batch.prefix : 'Run ',
    startIndex: typeof batch.startIndex === 'number' ? integer(batch.startIndex, 0, 999_999, 1) : null,
    outputDir: typeof batch.outputDir === 'string' && batch.outputDir ? batch.outputDir : null,
  };
}

function isSettings(raw: unknown): raw is Settings {
  const values = (raw as Settings | null)?.values;
  return typeof values === 'object' && values !== null && !Array.isArray(values);
}

class AppState {
  registry = $state.raw<Registry | null>(null);
  catalog = $derived(this.registry ? new Catalog(this.registry) : null);
  searchIndex = $derived(this.catalog ? new SearchIndex(this.catalog) : null);
  booted = $state(false);
  bootError = $state<string | null>(null);

  env = $state.raw<Environment | null>(null);
  envError = $state<string | null>(null);

  settings = $state.raw<Settings>({ values: {} });
  edition = $state<Edition>('scarlet');
  seed = $state('');
  view = $state.raw<View>({ kind: 'page', id: '' });
  batch = $state<BatchForm>(parseBatch(null));

  species = $state.raw<Partial<Record<Edition, Loadable<SpeciesEntry[]>>>>({});
  poolCounts = $state.raw<PoolCountState | null>(null);
  highlight = $state.raw<{ id: string; at: number } | null>(null);
  toasts = $state.raw<Toast[]>([]);
  jobRunning = $state(false);

  gameStatus = $derived(this.statusOf(this.edition));
  gameReady = $derived(this.gameStatus?.state === 'ready');

  speciesList = $derived.by(() => {
    const loaded = this.species[this.edition];
    return loaded?.state === 'ready' ? loaded.value : null;
  });

  speciesNames = $derived.by(() => {
    const names = new Map<number, string>();
    for (const entry of this.speciesList ?? []) if (!names.has(entry.national)) names.set(entry.national, entry.name);
    return names;
  });

  /** The global pool when it is empty, else the first applying override that is. */
  emptyPool = $derived.by((): OptionDef | null => {
    const counts = this.poolCounts;
    if (!this.catalog || counts?.edition !== this.edition || !counts.value) return null;
    const values = this.settings.values;
    const known = counts.value.counts;
    const empty = this.catalog.pools.filter((def) => applies(def, values) && known[def.id] === 0);
    return empty.find((def) => def.id === GLOBAL_POOL) ?? empty[0] ?? null;
  });

  block = $derived.by((): Block | null => {
    if (this.jobRunning) return { kind: 'running' };
    if (!this.env) return this.envError ? { kind: 'env-failed' } : { kind: 'checking' };
    if (this.gameStatus && !this.gameReady) return { kind: 'game', status: this.gameStatus };
    if (this.emptyPool) return { kind: 'pool', def: this.emptyPool };
    if (this.batch.enabled && !this.batch.prefix.trim()) return { kind: 'batch-name' };
    return null;
  });

  statusOf(edition: Edition): GameStatus | null {
    return this.env?.games.find((game) => game.edition === edition) ?? null;
  }

  async boot(): Promise<void> {
    const stored = readStored();
    let registry: Registry;
    try {
      registry = await api.getRegistry();
    } catch (error) {
      this.bootError = errorText(error);
      return;
    }
    this.registry = registry;
    void this.refreshEnv();
    const catalog = new Catalog(registry);
    this.edition = EDITIONS.find((edition) => edition === stored.edition) ?? 'scarlet';
    this.seed = typeof stored.seed === 'string' ? stored.seed : '';
    this.view = parseView(stored.view, catalog);
    this.batch = parseBatch(stored.batch);
    this.settings = registry.defaults;
    if (isSettings(stored.settings)) {
      try {
        const normalized = await api.normalizeSettings(stored.settings);
        this.settings = normalized.settings;
        if (normalized.warnings.length) {
          this.toast({ tone: 'warning', title: 'Some saved settings were repaired', lines: normalized.warnings });
        }
      } catch (error) {
        this.fail('Your saved settings could not be read, so the defaults are in place', error);
      }
    }
    this.booted = true;
  }

  persist(): void {
    if (!this.booted) return;
    writeStored({ settings: this.settings, seed: this.seed, edition: this.edition, view: this.view, batch: { ...this.batch } });
  }

  async refreshEnv(): Promise<void> {
    try {
      this.env = await api.getEnvironment();
      this.envError = null;
    } catch (error) {
      this.envError = errorText(error);
    }
  }

  async saveConfig(config: AppConfig, done = 'App settings saved'): Promise<boolean> {
    try {
      this.env = await api.saveConfig(config);
      this.species = {};
      this.poolCounts = null;
      this.toast({ tone: 'success', title: done });
      return true;
    } catch (error) {
      this.fail('The app settings were not saved', error);
      return false;
    }
  }

  private async patchConfig(pick: () => Promise<string | null>, patch: (config: AppConfig, path: string) => AppConfig, done: string) {
    const config = this.env?.config;
    if (!config) return;
    try {
      const path = await pick();
      if (path) await this.saveConfig(patch(config, path), done);
    } catch (error) {
      this.fail('Nothing was changed', error);
    }
  }

  chooseKeysFile() {
    return this.patchConfig(
      () => api.pickFile('Choose your prod.keys file', [{ name: 'Switch keys', extensions: ['keys'] }]),
      (config, path) => ({ ...config, keys_file: path }),
      'Keys file saved',
    );
  }

  addGameFolder() {
    return this.patchConfig(
      () => api.pickFolder('Add a folder with your game dumps'),
      (config, path) => ({ ...config, game_dirs: [...(config.game_dirs ?? this.env?.eden.game_dirs ?? []), path] }),
      'Game folder added',
    );
  }

  chooseGameFile(edition: Edition, which: 'base' | 'update') {
    const name = edition === 'scarlet' ? 'Scarlet' : 'Violet';
    const filter: FileFilter = which === 'base'
      ? { name: `${name} cartridge dump`, extensions: ['xci'] }
      : { name: `${name} 4.0.0 update`, extensions: ['nsp'] };
    return this.patchConfig(
      () => api.pickFile(which === 'base' ? `Choose the ${name} .xci` : `Choose the ${name} 4.0.0 update .nsp`, [filter]),
      (config, path) => ({ ...config, [edition]: { ...config[edition], [which]: path } }),
      which === 'base' ? `${name} game file saved` : `${name} update file saved`,
    );
  }

  chooseEdenExe() {
    return this.patchConfig(
      () => api.pickFile('Choose eden.exe', [{ name: 'Eden', extensions: ['exe'] }]),
      (config, path) => ({ ...config, eden_exe: path }),
      'Eden location saved',
    );
  }

  ensureSpecies(edition: Edition): void {
    if (this.species[edition]) return;
    this.species = { ...this.species, [edition]: { state: 'loading' } };
    api.listSpecies(edition).then(
      (value) => (this.species = { ...this.species, [edition]: { state: 'ready', value } }),
      (error) => (this.species = { ...this.species, [edition]: { state: 'failed', error: errorText(error) } }),
    );
  }

  private countRequest = 0;

  /** Call from an effect: reads edition, settings and the environment, and returns the debounce cleanup. */
  schedulePoolCounts(): () => void {
    const { edition, settings, gameReady, env } = this;
    if (!env || !gameReady || !this.catalog?.pools.length) {
      this.poolCounts = null;
      return () => {};
    }
    const request = ++this.countRequest;
    const previous = untrack(() => this.poolCounts);
    this.poolCounts = { edition, state: 'loading', value: previous?.edition === edition ? previous.value : null, error: null };
    const timer = setTimeout(() => {
      api.poolCounts(edition, settings).then(
        (value) => {
          if (request === this.countRequest) this.poolCounts = { edition, state: 'ready', value, error: null };
        },
        (error) => {
          if (request === this.countRequest) this.poolCounts = { edition, state: 'failed', value: null, error: errorText(error) };
        },
      );
    }, 250);
    return () => clearTimeout(timer);
  }

  setValue(id: string, value: OptValue): void {
    this.settings = { values: { ...this.settings.values, [id]: value } };
  }

  /** Whole-settings changes are undoable from the toast instead of asking first. */
  replaceSettings(next: Settings, title: string, extra: Partial<Omit<Toast, 'id'>> = {}, undoAlso?: () => void): void {
    const previous = this.settings;
    this.settings = next;
    this.toast({
      tone: 'success',
      title,
      ...extra,
      action: {
        label: 'Undo',
        run: () => {
          this.settings = previous;
          undoAlso?.();
        },
      },
    });
  }

  resetPage(page: string): void {
    const catalog = this.catalog;
    if (!catalog) return;
    const values = { ...this.settings.values };
    for (const def of catalog.options(page)) {
      const fallback = catalog.defaultOf(def.id);
      if (fallback !== undefined) values[def.id] = fallback;
    }
    this.replaceSettings({ values }, `${catalog.pageTitle(page)} is back to its defaults`);
  }

  resetAll(): void {
    if (this.registry) this.replaceSettings(this.registry.defaults, 'All settings are back to their defaults');
  }

  applyPreset(preset: Preset): void {
    this.replaceSettings(preset.settings, `Applied the ${preset.name} preset`);
  }

  async loadSettingsFile(): Promise<void> {
    try {
      const path = await api.pickFile('Load settings', SETTINGS_FILES);
      if (!path) return;
      const result = await api.loadSettingsFile(path);
      const repaired = result.warnings.length > 0;
      this.replaceSettings(result.settings, `Loaded ${folderName(path)}`, {
        tone: repaired ? 'warning' : 'success',
        body: repaired ? 'Some values in the file were repaired:' : undefined,
        lines: repaired ? result.warnings : undefined,
      });
    } catch (error) {
      this.fail('The settings file was not loaded', error);
    }
  }

  async saveSettingsFile(): Promise<void> {
    try {
      const path = await api.pickSaveFile('Save settings', SETTINGS_FILES, 'randomizer-settings.json');
      if (!path) return;
      await api.saveSettingsFile(path, this.settings);
      this.toast({ tone: 'success', title: 'Settings saved', body: path });
    } catch (error) {
      this.fail('The settings were not saved', error);
    }
  }

  goTo(view: View): void {
    this.view = view;
  }

  /** Opens the option's page and flashes it. */
  reveal(id: string): void {
    const def = this.catalog?.byId.get(id);
    if (!def) return;
    this.view = { kind: 'page', id: def.page };
    this.highlight = { id, at: Date.now() };
  }

  private toastId = 0;

  toast(toast: Omit<Toast, 'id'>): number {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { ...toast, id }].slice(-4);
    return id;
  }

  dismiss(id: number): void {
    this.toasts = this.toasts.filter((toast) => toast.id !== id);
  }

  fail(title: string, error: unknown): void {
    this.toast({ tone: 'error', title, body: errorText(error) });
  }
}

export const app = new AppState();
