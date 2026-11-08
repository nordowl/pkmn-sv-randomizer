<script lang="ts">
  import type { SpeciesEntry } from '../../lib/api';
  import { dismissable } from '../../lib/actions';
  import { app } from '../../lib/app.svelte';
  import { EDITION_NAMES } from '../../lib/format';
  import { fold } from '../../lib/options';
  import Icon from '../Icon.svelte';

  const MAX_NATIONAL = 1025;

  interface Result {
    national: number;
    name: string;
    entry: SpeciesEntry | null;
    /** Why it cannot be picked, e.g. "Already banned". */
    blocked: string | null;
  }

  let {
    placeholder,
    onpick,
    blocked = new Map(),
    inline = false,
    random,
  }: {
    placeholder: string;
    onpick: (national: number) => void;
    blocked?: Map<number, string>;
    /** Results flow under the field instead of floating, for use inside a popover. */
    inline?: boolean;
    /** Offers a "Random" row first. */
    random?: () => void;
  } = $props();

  const uid = $props.id();
  let query = $state('');
  let open = $state(false);
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();

  export function focus() {
    input?.focus();
  }

  const unique = $derived.by(() => {
    const list = app.speciesList;
    if (!list) return null;
    const first = new Map<number, SpeciesEntry>();
    for (const entry of list) if (!first.has(entry.national)) first.set(entry.national, entry);
    return [...first.values()];
  });

  const results = $derived.by((): Result[] => {
    const text = fold(query.trim()).replace(/^#/, '');
    if (!text) return [];
    const number = /^\d+$/.test(text) ? Number(text) : null;
    const result = (national: number, name: string, entry: SpeciesEntry | null): Result => ({
      national,
      name,
      entry,
      blocked: blocked.get(national) ?? null,
    });
    if (!unique) {
      return number !== null && number >= 1 && number <= MAX_NATIONAL ? [result(number, `No. ${number}`, null)] : [];
    }
    const matches = unique.filter((entry) =>
      number !== null ? String(entry.national).startsWith(text) : fold(entry.name).includes(text),
    );
    const rank = (entry: SpeciesEntry) =>
      number !== null ? (entry.national === number ? 0 : 1) : fold(entry.name).startsWith(text) ? 0 : 1;
    matches.sort((a, b) => rank(a) - rank(b) || a.national - b.national);
    return matches.slice(0, 8).map((entry) => result(entry.national, entry.name, entry));
  });

  const rows = $derived((random ? 1 : 0) + results.length);
  const showList = $derived((inline || open) && (query.trim() !== '' || !!random));

  function choose(index: number) {
    if (random && index === 0) {
      random();
      return;
    }
    const picked = results[index - (random ? 1 : 0)];
    if (!picked || picked.blocked) return;
    onpick(picked.national);
    query = '';
    active = 0;
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      open = true;
      const by = event.key === 'ArrowDown' ? 1 : -1;
      active = Math.min(Math.max(active + by, 0), Math.max(rows - 1, 0));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      choose(active);
    } else if (event.key === 'Escape' && query) {
      event.stopPropagation();
      query = '';
    }
  }
</script>

<div class="species-search" class:inline use:dismissable={() => (open = false)}>
  <div class="input">
    <Icon name="search" size={15} />
    <input
      bind:this={input}
      bind:value={query}
      {placeholder}
      role="combobox"
      aria-expanded={showList}
      aria-controls="{uid}-list"
      aria-autocomplete="list"
      aria-activedescendant={showList ? `${uid}-${active}` : undefined}
      onfocus={() => (open = true)}
      oninput={() => {
        open = true;
        active = 0;
      }}
      onkeydown={key}
    />
  </div>

  {#if showList}
    <ul id="{uid}-list" class="results" class:popover={!inline} role="listbox">
      {#if random}
        <li id="{uid}-0" role="option" aria-selected={active === 0}>
          <button class:active={active === 0} onclick={() => choose(0)} onpointermove={() => (active = 0)}>
            <Icon name="dice" size={15} />
            <span class="name">Random</span>
            <span class="meta">A different species each seed</span>
          </button>
        </li>
      {/if}
      {#each results as result, i (result.national)}
        {@const index = i + (random ? 1 : 0)}
        <li id="{uid}-{index}" role="option" aria-selected={active === index} aria-disabled={!!result.blocked}>
          <button
            class:active={active === index}
            disabled={!!result.blocked}
            onclick={() => choose(index)}
            onpointermove={() => (active = index)}
          >
            <span class="dex mono num">#{String(result.national).padStart(4, '0')}</span>
            <span class="name">{result.name}</span>
            {#if result.blocked}
              <span class="meta">{result.blocked}</span>
            {:else if result.entry}
              <span class="meta">
                Gen {result.entry.generation}{result.entry.legendary ? ' · Legendary' : ''}{result.entry.paradox ? ' · Paradox' : ''}
              </span>
            {/if}
          </button>
        </li>
      {:else}
        {#if query.trim()}
          <li class="empty">
            {unique ? `No species called “${query.trim()}”.` : 'Type a National Dex number, like 25.'}
          </li>
        {/if}
      {/each}
    </ul>
  {/if}

  {#if !unique}
    <p class="hint">Names show once {EDITION_NAMES[app.edition]} is ready. Until then, add species by National Dex number.</p>
  {/if}
</div>

<style>
  .species-search {
    position: relative;
  }

  .input {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--r-m);
    background: var(--ink);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--text-3);
    transition: box-shadow 140ms var(--ease);
  }
  .input:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-softer);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    background: none;
    outline: none;
    color: var(--text);
    font-size: 13.5px;
  }
  input::placeholder {
    color: var(--text-4);
  }

  .results {
    margin: 0;
    padding: 4px;
    list-style: none;
  }
  .results.popover {
    left: 0;
    right: 0;
    top: calc(100% + 6px);
    max-height: 300px;
    overflow: auto;
    transform-origin: top center;
  }
  .inline .results {
    margin-top: 6px;
  }

  li button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--r-s);
    text-align: left;
    color: var(--text);
  }
  li button.active:not(:disabled) {
    background: var(--accent-soft);
  }
  li button:disabled {
    color: var(--text-3);
    cursor: default;
  }

  .dex {
    color: var(--text-3);
    font-size: 11.5px;
  }
  .name {
    font-weight: 540;
  }
  .meta {
    margin-left: auto;
    color: var(--text-3);
    font-size: 12px;
    white-space: nowrap;
  }

  .empty {
    padding: 9px 10px;
    color: var(--text-3);
    font-size: 13px;
  }

  .hint {
    margin-top: 7px;
    color: var(--text-3);
    font-size: 12px;
  }
</style>
