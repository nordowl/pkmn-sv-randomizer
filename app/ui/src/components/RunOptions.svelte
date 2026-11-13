<script lang="ts">
  import { api, EDITIONS } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { batchPlan, batchTotal, EDITION_NAMES, zipName } from '../lib/format';
  import Icon from './Icon.svelte';
  import Stepper from './controls/Stepper.svelte';
  import Toggle from './controls/Toggle.svelte';

  const MAX_SEEDS = 999;

  const batch = $derived(app.batch);
  const zip = $derived(app.zip);
  const next = $derived(app.env?.config.next_batch_index ?? 1);
  const start = $derived(batch.startIndex ?? next);
  const total = $derived(batchTotal(batch.counts));
  const nameMissing = $derived(!batch.prefix.trim());
  const plan = $derived(
    batchPlan({ ...batch.counts, prefix: batch.prefix || 'Run ', start_index: start, output_dir: null }),
  );
  const zipFile = $derived(
    batch.enabled && total
      ? zipName({
          edition: app.edition,
          settings: app.settings,
          seed: '',
          batch: { ...batch.counts, prefix: batch.prefix || 'Run ', start_index: start, output_dir: null },
          zip_dir: null,
        })
      : null,
  );

  async function chooseBatchFolder() {
    try {
      const folder = await api.pickFolder('Choose where the batch goes', batch.outputDir ?? app.env?.library_dir);
      if (folder) batch.outputDir = folder;
    } catch (error) {
      app.fail('No folder was chosen', error);
    }
  }

  async function chooseZipFolder(): Promise<boolean> {
    try {
      const folder = await api.pickFolder('Choose where the zip file goes', zip.dir ?? app.env?.library_dir);
      if (folder) zip.dir = folder;
      return !!folder;
    } catch (error) {
      app.fail('No folder was chosen', error);
      return false;
    }
  }

  async function toggleZip(on: boolean) {
    if (on && !zip.dir && !(await chooseZipFolder())) return;
    zip.enabled = on;
  }
</script>

<div class="options">
  <section>
    <label class="switch-row">
      <span>
        <strong>Make a batch</strong>
        <span class="sub">Several seeds in one go, each in its own folder.</span>
      </span>
      <Toggle checked={batch.enabled} label="Make a batch" onchange={(on) => (batch.enabled = on)} />
    </label>

    <div class="fields" class:off={!batch.enabled} inert={!batch.enabled}>
      <div class="field-row">
        <span class="label">Seeds</span>
        <div class="counts">
          {#each EDITIONS as edition (edition)}
            <div class="count" data-edition={edition}>
              <span class="game"><span class="swatch" aria-hidden="true"></span>{EDITION_NAMES[edition]}</span>
              <Stepper
                value={batch.counts[edition]}
                min={0}
                max={MAX_SEEDS - total + batch.counts[edition]}
                label="Seeds for {EDITION_NAMES[edition]}"
                onchange={(n) => (batch.counts[edition] = n)}
              />
            </div>
          {/each}
        </div>
      </div>

      <div class="field-row">
        <label class="label" for="batch-name">Folder name</label>
        <div class="stack">
          <input id="batch-name" class="field" maxlength="60" bind:value={batch.prefix} placeholder="Run " />
          {#if nameMissing}<span class="error">Give the batch a name, like “Run ”.</span>{/if}
        </div>
      </div>

      <div class="field-row">
        <span class="label">First number</span>
        <div class="inline">
          <Stepper value={start} min={0} max={999_999 - Math.max(total, 1)} label="First number" onchange={(n) => (batch.startIndex = n)} />
          {#if batch.startIndex !== null && batch.startIndex !== next}
            <button class="link small" onclick={() => (batch.startIndex = null)}>Continue from {next}</button>
          {:else}
            <span class="note">Continues from your last batch</span>
          {/if}
        </div>
      </div>

      <div class="field-row">
        <span class="label">Save to</span>
        <div class="stack">
          <div class="folder">
            <Icon name="folder" size={15} />
            <span class="path" class:mono={!!batch.outputDir} title={batch.outputDir ?? app.env?.library_dir}>
              {batch.outputDir ?? 'The seed library'}
            </span>
            <button class="btn sm" onclick={chooseBatchFolder}>Change…</button>
          </div>
          {#if batch.outputDir}
            <button class="link small" onclick={() => (batch.outputDir = null)}>Use the seed library instead</button>
          {/if}
        </div>
      </div>

      <p class="preview">
        {#if !total}
          Set a number of seeds for at least one game.
        {:else}
          Makes <span class="mono">{plan[0]?.name}</span>{#if total > 1}{' to '}<span class="mono">{plan.at(-1)?.name}</span>{/if}{#if batch.counts.scarlet && batch.counts.violet}, Scarlet first{/if}.
        {/if}
      </p>
    </div>
  </section>

  <section>
    <label class="switch-row">
      <span>
        <strong>Pack into a zip</strong>
        <span class="sub">Also saves the seeds of each run in one zip file, each in its own mod folder.</span>
      </span>
      <Toggle checked={zip.enabled} label="Pack into a zip" onchange={toggleZip} />
    </label>

    <div class="fields" class:off={!zip.enabled} inert={!zip.enabled}>
      <div class="field-row">
        <span class="label">Save to</span>
        <div class="folder">
          <Icon name="folder" size={15} />
          <span class="path" class:mono={!!zip.dir} title={zip.dir ?? undefined}>{zip.dir ?? 'Not chosen yet'}</span>
          <button class="btn sm" onclick={chooseZipFolder}>Change…</button>
        </div>
      </div>
      <p class="preview">
        {#if zipFile}
          Makes <span class="mono">{zipFile}</span>.
        {:else}
          The zip is named after the seed, like <span class="mono">{EDITION_NAMES[app.edition]} {app.seed || '1234567890'}.zip</span>.
        {/if}
      </p>
    </div>
  </section>
</div>

<style>
  .options {
    display: flex;
    flex-direction: column;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
  }
  section + section {
    border-top: 1px solid var(--line-2);
  }

  .switch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    cursor: pointer;
  }
  .switch-row strong {
    display: block;
    font-size: 14px;
    font-weight: 600;
  }
  .sub {
    color: var(--text-3);
    font-size: 12.5px;
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    transition: opacity 160ms var(--ease);
  }
  .fields.off {
    opacity: 0.4;
  }

  .field-row {
    display: grid;
    grid-template-columns: 96px 1fr;
    align-items: center;
    justify-items: start;
    gap: 12px;
  }
  .label {
    color: var(--text-2);
    font-size: 13px;
  }

  .counts {
    display: grid;
    grid-template-columns: max-content max-content;
    align-items: center;
    gap: 8px 12px;
  }
  .count {
    display: contents;
  }
  .game {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--text);
    font-size: 13px;
    font-weight: 560;
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    rotate: 45deg;
    background: var(--accent);
  }

  .stack {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
    width: 100%;
    min-width: 0;
  }
  .stack .field {
    width: 100%;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .note {
    color: var(--text-3);
    font-size: 12px;
  }
  .small {
    font-size: 12px;
  }

  .error {
    color: color-mix(in oklab, var(--danger) 75%, white);
    font-size: 12px;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 0;
    color: var(--text-3);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 12.5px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .preview {
    padding: 9px 11px;
    border-radius: var(--r-m);
    background: var(--ink);
    color: var(--text-3);
    font-size: 12.5px;
  }
  .preview .mono {
    color: var(--text);
  }
</style>
