import { api, type ChangeLine, type JobSummary, type RandoEvent, type RandomizeRequest, type SeedStage } from './api';
import { app, type Loadable } from './app.svelte';
import { batchName, EDITION_NAMES, errorText, plural } from './format';
import { library } from './library.svelte';

export type RowState =
  | { is: 'queued' }
  | { is: 'working'; stage: SeedStage }
  | { is: 'done'; millis: number; folder: string; warnings: string[] }
  | { is: 'failed'; error: string }
  | { is: 'skipped' };

export interface SeedRow {
  position: number;
  name: string;
  seed: string | null;
  state: RowState;
}

/** review -> running -> finished; closing from review or finished returns to closed. */
export type RunPhase =
  | { is: 'closed' }
  | { is: 'review'; request: RandomizeRequest; lines: Loadable<ChangeLine[]> }
  | { is: 'running'; request: RandomizeRequest; cancelling: boolean }
  | { is: 'finished'; request: RandomizeRequest; summary: JobSummary | null; error: string | null };

/** How far through one seed each stage is, for the overall bar. Stages arrive in this order. */
const STAGE_PROGRESS: Record<SeedStage, number> = { randomizing: 0.15, logging: 0.6, writing: 0.8 };

export const STAGE_TEXT: Record<SeedStage, string> = {
  randomizing: 'Randomizing',
  logging: 'Writing spoiler log',
  writing: 'Writing files',
};

function buildRequest(): RandomizeRequest {
  const batch = app.batch;
  return {
    edition: app.edition,
    settings: app.settings,
    seed: app.seed.trim(),
    batch: batch.enabled
      ? {
          count: batch.count,
          prefix: batch.prefix,
          start_index: batch.startIndex ?? app.env?.config.next_batch_index ?? 1,
          output_dir: batch.outputDir,
        }
      : null,
  };
}

function plannedRows(request: RandomizeRequest): SeedRow[] {
  const typed = request.seed;
  const batch = request.batch;
  if (!batch) {
    const name = typed ? `${EDITION_NAMES[request.edition]} ${typed}` : `New ${EDITION_NAMES[request.edition]} seed`;
    return [{ position: 0, name, seed: typed || null, state: { is: 'queued' } }];
  }
  return Array.from({ length: batch.count }, (_, position) => {
    const index = batch.start_index + position;
    return { position, name: batchName(batch.prefix, index), seed: typed ? `${typed}-${index}` : null, state: { is: 'queued' } };
  });
}

class RunState {
  phase = $state.raw<RunPhase>({ is: 'closed' });
  rows = $state<SeedRow[]>([]);
  /** The sheet can be hidden while a run keeps going. */
  visible = $state(false);

  progress = $derived.by(() => {
    if (!this.rows.length) return 0;
    let sum = 0;
    for (const row of this.rows) {
      if (row.state.is === 'working') sum += STAGE_PROGRESS[row.state.stage];
      else if (row.state.is === 'done' || row.state.is === 'failed') sum += 1;
    }
    return sum / this.rows.length;
  });

  done = $derived(this.rows.filter((row) => row.state.is === 'done').length);
  failed = $derived(this.rows.filter((row) => row.state.is === 'failed').length);

  review(): void {
    if (app.block || this.phase.is === 'running') return;
    const request = buildRequest();
    this.phase = { is: 'review', request, lines: { state: 'loading' } };
    this.visible = true;
    api.describeChanges(request.settings, request.edition).then(
      (lines) => this.setLines(request, { state: 'ready', value: lines }),
      (error) => this.setLines(request, { state: 'failed', error: errorText(error) }),
    );
  }

  private setLines(request: RandomizeRequest, lines: Loadable<ChangeLine[]>): void {
    if (this.phase.is === 'review' && this.phase.request === request) this.phase = { ...this.phase, lines };
  }

  async start(): Promise<void> {
    if (this.phase.is !== 'review') return;
    const { request } = this.phase;
    this.rows = plannedRows(request);
    this.phase = { is: 'running', request, cancelling: false };
    app.jobRunning = true;
    let summary: JobSummary | null = null;
    let error: string | null = null;
    try {
      summary = await api.randomize(request, (event) => this.apply(event));
    } catch (failure) {
      error = errorText(failure);
    }
    for (const row of this.rows) {
      if (row.state.is === 'queued' || row.state.is === 'working') row.state = { is: 'skipped' };
    }
    this.phase = { is: 'finished', request, summary, error };
    app.jobRunning = false;
    if (request.batch) app.batch.startIndex = null;
    // The backend moved next_batch_index and may have remembered a new batch folder.
    await app.refreshEnv();
    void library.refresh();
    if (!this.visible) this.announce(summary, error);
  }

  private announce(summary: JobSummary | null, error: string | null): void {
    const show = { label: 'Show', run: () => (this.visible = true) };
    if (error || !summary) {
      app.toast({ tone: 'error', title: 'The run stopped', body: error ?? undefined, action: show });
      return;
    }
    const tone = summary.failed ? 'warning' : 'success';
    const failed = summary.failed ? `, ${summary.failed} failed` : '';
    app.toast({ tone, title: `${plural(summary.done, 'seed')} made${failed}`, action: show });
  }

  private apply(event: RandoEvent): void {
    if (event.kind === 'started') {
      if (this.rows.length > event.total) this.rows.length = event.total;
      return;
    }
    const row = this.rows[event.position];
    if (!row) return;
    switch (event.kind) {
      case 'seed_started':
        row.name = event.name;
        row.seed = event.seed;
        row.state = { is: 'working', stage: 'randomizing' };
        break;
      case 'seed_stage':
        row.state = { is: 'working', stage: event.stage };
        break;
      case 'seed_done':
        row.name = event.name;
        row.state = { is: 'done', millis: event.millis, folder: event.folder, warnings: event.warnings };
        break;
      case 'seed_failed':
        row.name = event.name;
        row.state = { is: 'failed', error: event.error };
        break;
    }
  }

  cancel(): void {
    if (this.phase.is !== 'running' || this.phase.cancelling) return;
    this.phase = { ...this.phase, cancelling: true };
    api.cancelRandomize().catch((error) => app.fail('The run could not be cancelled', error));
  }

  close(): void {
    if (this.phase.is === 'running') {
      this.visible = false;
      return;
    }
    this.visible = false;
    this.phase = { is: 'closed' };
  }
}

export const run = new RunState();
