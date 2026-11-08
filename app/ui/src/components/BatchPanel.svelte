<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { batchName } from '../lib/format';
  import Icon from './Icon.svelte';
  import Stepper from './controls/Stepper.svelte';
  import Toggle from './controls/Toggle.svelte';

  const batch = $derived(app.batch);
  const next = $derived(app.env?.config.next_batch_index ?? 1);
  const start = $derived(batch.startIndex ?? next);
  const nameMissing = $derived(!batch.prefix.trim());
  const first = $derived(batchName(batch.prefix || 'Run ', start));
  const last = $derived(batchName(batch.prefix || 'Run ', start + batch.count - 1));

  async function chooseFolder() {
    try {
      const folder = await api.pickFolder('Choose where the batch goes', batch.outputDir ?? app.env?.library_dir);
      if (folder) batch.outputDir = folder;
    } catch (error) {
      app.fail('No folder was chosen', error);
    }
  }
</script>

<div class="batch">
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
      <Stepper value={batch.count} min={1} max={999} label="Number of seeds" onchange={(n) => (batch.count = n)} />
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
        <Stepper value={start} min={0} max={999_999 - batch.count} label="First number" onchange={(n) => (batch.startIndex = n)} />
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
          <button class="btn sm" onclick={chooseFolder}>Change…</button>
        </div>
        {#if batch.outputDir}
          <button class="link small" onclick={() => (batch.outputDir = null)}>Use the seed library instead</button>
        {/if}
      </div>
    </div>

    <p class="preview">
      Makes <span class="mono">{first}</span>{#if batch.count > 1}{' to '}<span class="mono">{last}</span>{/if}.
    </p>
  </div>
</div>

<style>
  .batch {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
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
