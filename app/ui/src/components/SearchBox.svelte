<script lang="ts">
  import { dismissable } from '../lib/actions';
  import { app } from '../lib/app.svelte';
  import { plural } from '../lib/format';
  import { fold, type SearchHit } from '../lib/options';
  import Icon from './Icon.svelte';

  const uid = $props.id();
  let query = $state('');
  let open = $state(false);
  let active = $state(0);

  const hits = $derived(app.searchIndex?.search(query) ?? []);
  const terms = $derived(fold(query).split(/\s+/).filter(Boolean));

  /** Splits text into plain and matched parts; folding keeps indices aligned for precomposed text. */
  function marked(text: string): { text: string; hit: boolean }[] {
    const folded = fold(text);
    const hit = new Array<boolean>(text.length).fill(false);
    for (const term of terms) {
      let at = folded.indexOf(term);
      while (at !== -1) {
        for (let i = at; i < at + term.length && i < hit.length; i++) hit[i] = true;
        at = folded.indexOf(term, at + term.length);
      }
    }
    const parts: { text: string; hit: boolean }[] = [];
    for (let i = 0; i < text.length; i++) {
      const last = parts[parts.length - 1];
      if (last && last.hit === hit[i]) last.text += text[i];
      else parts.push({ text: text[i]!, hit: hit[i]! });
    }
    return parts;
  }

  function choose(hit: SearchHit | undefined) {
    if (!hit) return;
    if (hit.kind === 'page') app.goTo({ kind: 'page', id: hit.page.id });
    else app.reveal(hit.def.id);
    open = false;
    (document.activeElement as HTMLElement | null)?.blur();
  }

  function key(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const by = event.key === 'ArrowDown' ? 1 : -1;
      active = Math.min(Math.max(active + by, 0), Math.max(hits.length - 1, 0));
      document.getElementById(`${uid}-${active}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') {
      choose(hits[active]);
    } else if (event.key === 'Escape') {
      query = '';
      (event.currentTarget as HTMLInputElement).blur();
    }
  }
</script>

<div class="search" use:dismissable={() => (open = false)}>
  <label class="box">
    <Icon name="search" size={15} />
    <input
      id="option-search"
      placeholder="Search options"
      autocomplete="off"
      spellcheck="false"
      role="combobox"
      aria-expanded={open && !!query.trim()}
      aria-controls="{uid}-list"
      aria-activedescendant={open && hits.length ? `${uid}-${active}` : undefined}
      bind:value={query}
      onfocus={() => (open = true)}
      oninput={() => {
        open = true;
        active = 0;
      }}
      onkeydown={key}
    />
    <kbd>Ctrl K</kbd>
  </label>

  {#if open && query.trim()}
    <div class="popover results" id="{uid}-list" role="listbox" aria-label="Search results">
      {#each hits as hit, i (hit.kind === 'page' ? `page:${hit.page.id}` : hit.def.id)}
        <button
          id="{uid}-{i}"
          role="option"
          aria-selected={i === active}
          class:active={i === active}
          onclick={() => choose(hit)}
          onpointermove={() => (active = i)}
        >
          {#if hit.kind === 'page'}
            <span class="line">
              <span class="tag">Page</span>
              <span class="label">{#each marked(hit.page.title) as part, p (p)}{#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>
            </span>
            <span class="where">{hit.optionCount ? plural(hit.optionCount, 'option') : 'Still being built'}</span>
          {:else}
            <span class="line">
              <span class="label">{#each marked(hit.def.label) as part, p (p)}{#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>
            </span>
            <span class="where">{hit.pageTitle}<Icon name="chevronRight" size={11} />{hit.def.group}</span>
            {#if hit.matched === 'choice' && hit.choice}
              <span class="snippet">Choice: {hit.choice}</span>
            {:else if hit.matched === 'help'}
              <span class="snippet">{hit.def.help}</span>
            {/if}
          {/if}
        </button>
      {:else}
        <p class="none">No options match “{query.trim()}”.</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .search {
    position: relative;
  }

  .box {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px 0 11px;
    border-radius: var(--r-m);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--text-3);
    cursor: text;
    transition: box-shadow 140ms var(--ease);
  }
  .box:focus-within {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-softer);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font-size: 13.5px;
  }
  input::placeholder {
    color: var(--text-3);
  }

  kbd {
    padding: 2px 6px;
    border-radius: var(--r-xs);
    background: rgb(255 255 255 / 0.06);
    color: var(--text-3);
    font: 500 10.5px/1.4 var(--font-mono);
  }

  .results {
    top: calc(100% + 8px);
    left: 0;
    width: 440px;
    max-height: min(520px, calc(100vh - 140px));
    overflow: auto;
    padding: 6px;
    transform-origin: top left;
  }

  .results button {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 9px 11px;
    border-radius: var(--r-m);
    text-align: left;
  }
  .results button.active {
    background: var(--accent-soft);
  }

  .line {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .label {
    font-size: 13.5px;
    font-weight: 560;
  }

  mark {
    background: none;
    color: var(--accent-text);
  }

  .where {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text-3);
    font-size: 12px;
  }

  .snippet {
    overflow: hidden;
    color: var(--text-3);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .none {
    padding: 12px;
    color: var(--text-3);
    font-size: 13px;
  }
</style>
