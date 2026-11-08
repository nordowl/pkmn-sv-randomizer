<script lang="ts">
  import { tick } from 'svelte';
  import type { SpeciesPick } from '../../lib/api';
  import { dismissable } from '../../lib/actions';
  import { app } from '../../lib/app.svelte';
  import Icon from '../Icon.svelte';
  import Select from '../controls/Select.svelte';
  import SpeciesSearch from './SpeciesSearch.svelte';

  let { value, label, onchange }: { value: SpeciesPick; label: string; onchange: (value: SpeciesPick) => void } = $props();

  let open = $state(false);
  let search: SpeciesSearch | undefined = $state();

  const name = $derived(value.national === null ? null : (app.speciesNames.get(value.national) ?? `No. ${value.national}`));
  const forms = $derived(
    value.national === null || !app.speciesList ? [] : app.speciesList.filter((entry) => entry.national === value.national),
  );

  async function toggle() {
    open = !open;
    if (open) {
      await tick();
      search?.focus();
    }
  }

  function pick(national: number | null) {
    const first = app.speciesList?.find((entry) => entry.national === national);
    onchange({ national, form: first?.form ?? 0 });
    open = false;
  }
</script>

<div class="species-picker" use:dismissable={() => (open = false)}>
  <button class="current" aria-label="{label}: {name ?? 'Random'}" aria-expanded={open} onclick={toggle}>
    {#if name === null}
      <Icon name="dice" size={15} />
      <span class="name random">Random</span>
    {:else}
      <span class="dex mono num">#{value.national}</span>
      <span class="name">{name}</span>
    {/if}
    <Icon name="chevronDown" size={15} class="chevron" />
  </button>

  {#if forms.length > 1}
    <Select
      choices={forms.map((entry) => ({ value: String(entry.form), label: entry.form === 0 ? 'Standard form' : `Form ${entry.form}`, help: '' }))}
      value={String(value.form)}
      label="{label}: form"
      onchange={(form) => onchange({ national: value.national, form: Number(form) })}
    />
  {/if}

  {#if open}
    <div class="popover panel">
      <SpeciesSearch bind:this={search} inline placeholder="Find by name or number" random={() => pick(null)} onpick={pick} />
    </div>
  {/if}
</div>

<style>
  .species-picker {
    position: relative;
    display: flex;
    gap: 8px;
  }

  .current {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 220px;
    height: 34px;
    padding: 0 10px 0 11px;
    border-radius: var(--r-m);
    background: var(--surface-3);
    box-shadow: var(--shadow-raise);
    text-align: left;
    transition: background-color 140ms var(--ease);
  }
  .current:hover {
    background: var(--surface-4);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 540;
  }
  .name.random {
    color: var(--text-2);
  }

  .dex {
    color: var(--text-3);
    font-size: 11.5px;
  }

  .current :global(.chevron) {
    color: var(--text-3);
  }

  .panel {
    top: calc(100% + 6px);
    right: 0;
    width: 320px;
    padding: 8px;
  }
</style>
