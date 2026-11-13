<script lang="ts">
  import type { OptionDef } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { sameValue } from '../lib/options';
  import { revealOf } from '../lib/reveal.svelte';
  import Toggle from './controls/Toggle.svelte';

  let { defs, help }: { defs: OptionDef[]; help: string } = $props();

  let element: HTMLElement | undefined = $state();
  const reveal = revealOf(
    () => defs.map((def) => def.id),
    () => element,
  );
</script>

<div class="switches" bind:this={element}>
  {#if help}<p class="help">{help}</p>{/if}
  <div class="cards">
    {#each defs as def (def.id)}
      {@const on = app.settings.values[def.id] === true}
      <label class="card" class:on class:flash={reveal.id === def.id} id="option-{def.id}">
        {#if app.catalog && !sameValue(on, app.catalog.defaultOf(def.id))}
          <span class="changed" title="Changed from the default"></span>
        {/if}
        <span class="label">{def.label}</span>
        <Toggle checked={on} label={def.label} onchange={(next) => app.setValue(def.id, next)} />
      </label>
    {/each}
  </div>
</div>

<style>
  .switches {
    padding: 16px 0 18px;
  }

  .help {
    margin-bottom: 12px;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(104px, 1fr));
    gap: 8px;
  }

  .card {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 11px 12px 11px 14px;
    border-radius: var(--r-m);
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--line);
    cursor: pointer;
    transition:
      background-color 140ms var(--ease),
      box-shadow 140ms var(--ease);
  }
  .card:hover {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .card.on {
    background: var(--accent-softer);
    box-shadow: inset 0 0 0 1px var(--accent-line);
  }
  .card.flash {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }

  .label {
    min-width: 0;
    font-size: 13.5px;
    font-weight: 600;
  }

  .changed {
    position: absolute;
    top: 6px;
    left: 6px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
  }
</style>
