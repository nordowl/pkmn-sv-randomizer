import { api, type Edition, type SeedEntry, type SwitchCheck } from './api';
import { app, type Loadable } from './app.svelte';
import { EDITION_NAMES, errorText, joinPath } from './format';

/** Enough to switch to a seed: a library row, or a seed a run just made. */
export type SeedRef = Pick<SeedEntry, 'path' | 'name'>;

class LibraryState {
  seeds = $state.raw<Loadable<SeedEntry[]> | null>(null);
  /** A switch waiting on the save warning. */
  pendingSwitch = $state.raw<{ seed: SeedRef; check: SwitchCheck } | null>(null);
  pendingDelete = $state.raw<SeedEntry | null>(null);
  /** Path of the seed being switched to, or an edition being turned off or launched. */
  busy = $state<string | null>(null);

  async refresh(): Promise<void> {
    if (this.seeds?.state !== 'ready') this.seeds = { state: 'loading' };
    try {
      this.seeds = { state: 'ready', value: await api.listSeeds() };
    } catch (error) {
      this.seeds = { state: 'failed', error: errorText(error) };
    }
  }

  async activate(seed: SeedRef): Promise<void> {
    this.busy = seed.path;
    try {
      const check = await api.checkSwitch(seed.path);
      if (check.saves.length) {
        this.pendingSwitch = { seed, check };
        return;
      }
      await this.install(seed, false);
    } catch (error) {
      app.fail(`${seed.name} was not switched on`, error);
    } finally {
      if (!this.pendingSwitch) this.busy = null;
    }
  }

  async confirmSwitch(backup: boolean): Promise<void> {
    const pending = this.pendingSwitch;
    if (!pending) return;
    this.pendingSwitch = null;
    try {
      await this.install(pending.seed, backup);
    } catch (error) {
      app.fail(`${pending.seed.name} was not switched on`, error);
    } finally {
      this.busy = null;
    }
  }

  cancelSwitch(): void {
    this.pendingSwitch = null;
    this.busy = null;
  }

  private async install(seed: SeedRef, backup: boolean): Promise<void> {
    const result = await api.activateSeed(seed.path, backup);
    const backupDir = result.backup_dir;
    app.toast({
      tone: 'success',
      title: `${seed.name} is now active in Eden`,
      lines: [`Installed to ${result.installed_to}`, ...(backupDir ? [`Save backed up to ${backupDir}`] : [])],
      action: backupDir ? { label: 'Open backup', run: () => this.open(backupDir) } : undefined,
    });
    await this.refresh();
  }

  async deactivate(edition: Edition): Promise<void> {
    this.busy = `off:${edition}`;
    try {
      await api.deactivate(edition);
      app.toast({ tone: 'success', title: `The randomizer is off for ${EDITION_NAMES[edition]}`, body: 'Eden loads the normal game.' });
      await this.refresh();
    } catch (error) {
      app.fail(`The randomizer is still on for ${EDITION_NAMES[edition]}`, error);
    } finally {
      this.busy = null;
    }
  }

  async confirmDelete(): Promise<void> {
    const seed = this.pendingDelete;
    if (!seed) return;
    this.pendingDelete = null;
    try {
      await api.deleteSeed(seed.path);
      app.toast({ tone: 'success', title: `Deleted ${seed.name}` });
      await this.refresh();
    } catch (error) {
      app.fail(`${seed.name} was not deleted`, error);
    }
  }

  async open(path: string): Promise<void> {
    try {
      await api.openPath(path);
    } catch (error) {
      app.fail('That could not be opened', error);
    }
  }

  openSpoiler(seed: SeedRef): Promise<void> {
    return this.open(joinPath(seed.path, 'spoiler.html'));
  }

  async play(edition: Edition): Promise<void> {
    this.busy = `play:${edition}`;
    try {
      await api.launchEden(edition);
      app.toast({ tone: 'success', title: `Starting ${EDITION_NAMES[edition]} in Eden` });
    } catch (error) {
      app.fail('Eden did not start', error);
    } finally {
      this.busy = null;
    }
  }
}

export const library = new LibraryState();
