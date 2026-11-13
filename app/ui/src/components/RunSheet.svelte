<script lang="ts">
  import type { ChangeLine } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { batchPlan, batchTotal, duration, EDITION_NAMES, joinPath, plural, zipName } from '../lib/format';
  import { library } from '../lib/library.svelte';
  import { run, STAGE_TEXT, type SeedRow } from '../lib/run.svelte';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  const phase = $derived(run.phase);
  const request = $derived(phase.is === 'closed' ? null : phase.request);
  const plan = $derived(request?.batch ? batchPlan(request.batch) : []);
  const games = $derived.by(() => {
    if (!request) return '';
    const editions = request.batch ? [...new Set(plan.map((seed) => seed.edition))] : [request.edition];
    return editions.map((edition) => EDITION_NAMES[edition]).join(' and ');
  });
  const total = $derived(request?.batch ? batchTotal(request.batch) : 1);
  const zipFile = $derived(request?.zip_dir ? zipName(request) : null);
  const packed = $derived(phase.is === 'finished' ? (phase.summary?.zip ?? null) : null);

  const grouped = $derived.by(() => {
    if (phase.is !== 'review' || phase.lines.state !== 'ready') return [];
    const pages = new Map<string, ChangeLine[]>();
    for (const line of phase.lines.value) pages.set(line.page, [...(pages.get(line.page) ?? []), line]);
    return [...pages].map(([page, lines]) => ({ title: app.catalog?.pageTitle(page) ?? page, lines }));
  });

  const title = $derived.by(() => {
    if (phase.is === 'review') return `Randomize ${games}`;
    if (phase.is === 'running') return total > 1 ? `Making ${plural(total, 'seed')}` : `Making your ${games} seed`;
    if (phase.is === 'finished') {
      if (phase.error && !run.done) return 'The run did not finish';
      if (phase.summary?.cancelled) return `Run cancelled · ${plural(run.done, 'seed')} made`;
      if (run.failed) return `${plural(run.done, 'seed')} made, ${run.failed} failed`;
      return total > 1 ? `${plural(run.done, 'seed')} ready` : 'Your seed is ready';
    }
    return '';
  });

  const where = $derived(request?.batch?.output_dir ?? app.env?.library_dir ?? null);
  const single = $derived(run.rows.length === 1 && run.rows[0]?.state.is === 'done' ? run.rows[0] : null);
  const singleFolder = $derived(single?.state.is === 'done' ? single.state.folder : null);

  function status(row: SeedRow): string {
    switch (row.state.is) {
      case 'queued':
        return 'Queued';
      case 'working':
        return STAGE_TEXT[row.state.stage];
      case 'done':
        return `Done in ${duration(row.state.millis)}`;
      case 'failed':
        return 'Failed';
      case 'skipped':
        return 'Skipped';
    }
  }

  function activate() {
    if (!single || !singleFolder) return;
    const seed = { path: singleFolder, name: single.name };
    run.close();
    void library.activate(seed);
  }

  function toLibrary() {
    run.close();
    app.goTo({ kind: 'library' });
  }
</script>

<Dialog
  open={run.visible && phase.is !== 'closed'}
  {title}
  width={620}
  onclose={() => run.close()}
>
  {#snippet lead()}
    <span class="lead" data-edition={request?.edition}>
      {#if phase.is === 'running'}
        <Icon name="spinner" size={17} stroke={2.2} class="spin" />
      {:else if phase.is === 'finished' && (phase.error || run.failed) && !run.done}
        <Icon name="alert" size={17} />
      {:else if phase.is === 'finished'}
        <Icon name="check" size={17} stroke={2.2} />
      {:else}
        <Icon name="play" size={15} stroke={2} />
      {/if}
    </span>
  {/snippet}

  {#if phase.is === 'review' && request}
    <dl class="facts">
      <div>
        <dt>Seed</dt>
        <dd>
          {#if request.seed}
            <span class="mono">{request.seed}</span>{#if request.batch}<span class="dim"> with -{request.batch.start_index}, -{request.batch.start_index + 1}… added</span>{/if}
          {:else}
            Random{request.batch ? ' for each seed' : ''}
          {/if}
        </dd>
      </div>
      {#if request.batch}
        <div>
          <dt>Batch</dt>
          <dd>
            {#if request.batch.scarlet && request.batch.violet}
              {request.batch.scarlet} Scarlet and {request.batch.violet} Violet,
            {:else}
              {plural(total, 'seed')},
            {/if}
            <span class="mono">{plan[0]?.name}</span>{#if total > 1}{' to '}<span class="mono">{plan.at(-1)?.name}</span>{/if}
          </dd>
        </div>
      {/if}
      <div>
        <dt>Saved to</dt>
        <dd class="path" class:mono={!!request.batch?.output_dir} title={where ?? undefined}>
          {request.batch?.output_dir ?? 'The seed library'}
        </dd>
      </div>
      {#if request.zip_dir}
        <div>
          <dt>Zip</dt>
          <dd class="path mono" title={request.zip_dir}>
            {zipFile ? joinPath(request.zip_dir, zipFile) : `${request.zip_dir}, named after the seed`}
          </dd>
        </div>
      {/if}
    </dl>

    <h3 class="section">Changed settings</h3>
    {#if phase.lines.state === 'loading'}
      <div class="skeleton" aria-label="Loading the changed settings"><span></span><span></span><span></span></div>
    {:else if phase.lines.state === 'failed'}
      <p class="error">{phase.lines.error}</p>
    {:else if !grouped.length}
      <p class="defaults">All settings are at their defaults.</p>
    {:else}
      <div class="changes">
        {#each grouped as page (page.title)}
          <section>
            <h4>{page.title}</h4>
            <ul>
              {#each page.lines as line (line.id)}
                <li>
                  <span class="what">{line.label}</span>
                  <span class="value">{line.value}</span>
                  <span class="was">was {line.default}</span>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    {/if}
  {:else if phase.is === 'running' || phase.is === 'finished'}
    <div class="overall">
      <div class="bar" aria-hidden="true"><span style:scale="{run.progress} 1"></span></div>
      <p class="num">
        {run.done} of {run.rows.length} done
        {#if run.failed}<span class="sep">·</span><span class="bad">{run.failed} failed</span>{/if}
        {#if phase.is === 'running' && phase.cancelling}<span class="sep">·</span>Stopping after this seed…{/if}
        {#if run.packing}<span class="sep">·</span>Packing the zip…{/if}
      </p>
    </div>

    {#if phase.is === 'finished' && phase.error}
      <p class="error block">{phase.error}</p>
    {/if}
    {#if packed?.state === 'failed'}
      <p class="error block">The seeds were made, but the zip was not written. {packed.error}</p>
    {:else if packed?.state === 'done'}
      <p class="zip-done"><Icon name="check" size={14} stroke={2.4} /><span>Packed into <span class="mono">{packed.path}</span></span></p>
    {/if}

    <ol class="rows">
      {#each run.rows as row (row.position)}
        <li class="row {row.state.is}">
          <span class="state-icon">
            {#if row.state.is === 'working'}
              <Icon name="spinner" size={15} stroke={2.2} class="spin" />
            {:else if row.state.is === 'done'}
              <Icon name="check" size={15} stroke={2.4} />
            {:else if row.state.is === 'failed'}
              <Icon name="alert" size={15} />
            {:else if row.state.is === 'skipped'}
              <Icon name="dash" size={15} />
            {:else}
              <Icon name="clock" size={15} />
            {/if}
          </span>
          <span class="name">{row.name}</span>
          <span class="seed mono">{row.seed && !row.name.endsWith(row.seed) ? row.seed : ''}</span>
          <span class="status num">{status(row)}</span>
          {#if row.state.is === 'failed'}
            <p class="row-error">{row.state.error}</p>
          {:else if row.state.is === 'done' && row.state.warnings.length}
            <ul class="row-warnings">
              {#each row.state.warnings as warning, i (i)}<li>{warning}</li>{/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}

  {#snippet footer()}
    {#if phase.is === 'review'}
      <button class="btn ghost" onclick={() => run.close()}>Cancel</button>
      <button class="btn primary" onclick={() => run.start()}>
        <Icon name="play" size={14} stroke={2} />{total > 1 ? `Randomize ${plural(total, 'seed')}` : 'Randomize'}
      </button>
    {:else if phase.is === 'running'}
      <button class="btn ghost" onclick={() => run.close()}>Hide</button>
      <button class="btn danger" disabled={phase.cancelling} onclick={() => run.cancel()}>
        {phase.cancelling ? 'Stopping…' : 'Cancel run'}
      </button>
    {:else if phase.is === 'finished'}
      {#if packed?.state === 'done' && request?.zip_dir}
        <button class="btn ghost" onclick={() => library.open(request.zip_dir!)}>
          <Icon name="folder" size={15} />Open zip folder
        </button>
      {/if}
      {#if single && singleFolder}
        <button class="btn ghost" onclick={() => library.open(singleFolder)}>
          <Icon name="folder" size={15} />Open folder
        </button>
        <button class="btn" onclick={toLibrary}><Icon name="stack" size={15} />Go to Library</button>
        <button class="btn primary" onclick={activate}><Icon name="power" size={15} />Switch it on in Eden</button>
      {:else}
        {#if where && run.done}
          <button class="btn ghost" onclick={() => library.open(where)}>
            <Icon name="folder" size={15} />Open folder
          </button>
        {/if}
        <button class="btn" onclick={toLibrary}><Icon name="stack" size={15} />Go to Library</button>
        <button class="btn primary" onclick={() => run.close()}>Done</button>
      {/if}
    {/if}
  {/snippet}
</Dialog>

<style>
  .zip-done {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: -4px 0 14px;
    color: var(--text-2);
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .zip-done :global(svg) {
    flex: none;
    translate: 0 2px;
    color: var(--ok);
  }
  .zip-done .mono {
    color: var(--text);
  }

  .lead {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-m);
    background: var(--accent-soft);
    color: var(--accent-text);
  }

  .facts {
    display: grid;
    gap: 8px;
    margin: 0 0 20px;
    padding: 12px 14px;
    border-radius: var(--r-l);
    background: var(--ink);
  }
  .facts div {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: 12px;
    font-size: 13px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    min-width: 0;
    color: var(--text);
  }
  .dim {
    color: var(--text-3);
  }
  .path {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .section {
    margin-bottom: 10px;
    color: var(--text-3);
    font: 600 11px/1 var(--font-ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .changes {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .changes h4 {
    margin: 0 0 6px;
    color: var(--text);
    font-size: 13px;
    font-weight: 620;
  }
  .changes ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .changes li {
    display: grid;
    grid-template-columns: minmax(150px, 0.8fr) 1.2fr;
    column-gap: 16px;
    padding: 7px 0;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .what {
    color: var(--text-2);
  }
  .value {
    color: var(--text);
    font-weight: 540;
  }
  .was {
    grid-column: 2;
    color: var(--text-3);
    font-size: 12px;
  }

  .defaults {
    padding: 14px;
    border-radius: var(--r-l);
    border: 1px dashed var(--line-2);
    color: var(--text-2);
    font-size: 13px;
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .skeleton span {
    height: 14px;
    border-radius: 4px;
    background: var(--surface-4);
    animation: breathe 1.2s ease-in-out infinite alternate;
  }
  .skeleton span:nth-child(2) {
    width: 70%;
  }
  .skeleton span:nth-child(3) {
    width: 45%;
  }
  @keyframes breathe {
    to {
      opacity: 0.4;
    }
  }

  .overall {
    margin-bottom: 14px;
  }
  .bar {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-4);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
    transform-origin: left center;
    transition: scale 420ms var(--ease-out);
  }
  .overall p {
    margin-top: 8px;
    color: var(--text-2);
    font-size: 13px;
  }
  .bad {
    color: color-mix(in oklab, var(--danger) 75%, white);
  }
  .sep {
    margin: 0 6px;
    color: var(--text-4);
  }

  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
    border-radius: var(--r-l);
    background: var(--ink);
    overflow: hidden;
  }

  .row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto auto;
    align-items: center;
    column-gap: 12px;
    padding: 10px 14px;
    font-size: 13px;
    transition: background-color 200ms var(--ease);
  }
  .row + .row {
    border-top: 1px solid var(--line);
  }
  .row.working {
    background: var(--accent-softer);
  }

  .state-icon {
    display: grid;
    place-items: center;
    color: var(--text-4);
  }
  .working .state-icon {
    color: var(--accent-text);
  }
  .done .state-icon {
    color: var(--ok);
  }
  .failed .state-icon {
    color: var(--danger);
  }

  .name {
    overflow: hidden;
    color: var(--text);
    font-weight: 540;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .queued .name,
  .skipped .name {
    color: var(--text-3);
  }

  .seed {
    color: var(--text-3);
    font-size: 11.5px;
  }

  .status {
    min-width: 130px;
    color: var(--text-3);
    text-align: right;
  }
  .working .status {
    color: var(--accent-text);
  }
  .done .status {
    color: var(--text-2);
  }
  .failed .status {
    color: color-mix(in oklab, var(--danger) 75%, white);
  }

  .row-error,
  .row-warnings {
    grid-column: 2 / -1;
    margin: 4px 0 0;
    font-size: 12.5px;
  }
  .row-error {
    color: color-mix(in oklab, var(--danger) 70%, white);
  }
  .row-warnings {
    padding-left: 16px;
    color: var(--warn);
  }

  .error {
    color: color-mix(in oklab, var(--danger) 75%, white);
    font-size: 13px;
  }
  .error.block {
    margin-bottom: 12px;
  }
</style>
