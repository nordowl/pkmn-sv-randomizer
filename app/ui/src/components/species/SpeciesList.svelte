<script lang="ts">
  import { app } from '../../lib/app.svelte';
  import Icon from '../Icon.svelte';
  import SpeciesSearch from './SpeciesSearch.svelte';

  let {
    title,
    help,
    list,
    blocked,
    onchange,
  }: {
    title: string;
    help: string;
    list: number[];
    blocked: Map<number, string>;
    onchange: (list: number[]) => void;
  } = $props();

  const nameOf = (national: number) => app.speciesNames.get(national) ?? `No. ${national}`;
</script>

<section class="species-list">
  <header>
    <h4>{title}</h4>
    {#if list.length}
      <span class="count num">{list.length}</span>
      <button class="link clear" onclick={() => onchange([])}>Clear</button>
    {/if}
  </header>
  <p class="help">{help}</p>
  <SpeciesSearch placeholder="Add by name or number" {blocked} onpick={(national) => onchange([...list, national])} />
  {#if list.length}
    <ul class="chips">
      {#each list as national (national)}
        <li>
          <span class="chip-name">{nameOf(national)}</span>
          <span class="mono num">#{national}</span>
          <button aria-label="Remove {nameOf(national)}" onclick={() => onchange(list.filter((n) => n !== national))}>
            <Icon name="x" size={12} stroke={2} />
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="none">No species yet.</p>
  {/if}
</section>

<style>
  .species-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  h4 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .count {
    color: var(--text-3);
    font-size: 12px;
  }

  .clear {
    margin-left: auto;
    font-size: 12px;
  }

  .help {
    color: var(--text-3);
    font-size: 12.5px;
    margin-top: -4px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 2px 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 4px 0 10px;
    border-radius: 999px;
    background: var(--surface-4);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 13px;
  }

  .chip-name {
    font-weight: 520;
  }

  .mono {
    color: var(--text-3);
    font-size: 11px;
  }

  li button {
    position: relative;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: var(--text-3);
    transition:
      background-color 120ms var(--ease),
      color 120ms var(--ease);
  }
  li button::before {
    content: '';
    position: absolute;
    inset: -6px;
  }
  li button:hover {
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }

  .none {
    color: var(--text-4);
    font-size: 12.5px;
  }
</style>
