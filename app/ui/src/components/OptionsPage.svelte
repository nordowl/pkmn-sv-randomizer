<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { plural } from '../lib/format';
  import { changedOn, groupsOf, REGIONS, type Group, type Region } from '../lib/options';
  import Icon from './Icon.svelte';
  import OptionRow from './OptionRow.svelte';

  let { pageId }: { pageId: string } = $props();

  const catalog = $derived(app.catalog!);
  const title = $derived(catalog.pageTitle(pageId));
  const options = $derived(catalog.options(pageId));
  const groups = $derived(groupsOf(options));
  const regions = $derived(REGIONS.filter((region) => groups.some((group) => group.region === region)));
  const tabs = $derived<(Region | 'all')[]>(['all', ...regions]);
  const changed = $derived(changedOn(options, app.settings.values, catalog));

  let region = $state<Region | 'all'>('all');
  const shown = $derived(groups.filter((group) => region === 'all' || group.region === null || group.region === region));

  /** Consecutive slot groups render as one section with a tab per slot. */
  type Block = { kind: 'group'; group: Group } | { kind: 'slots'; groups: [Group, ...Group[]] };
  const blocks = $derived.by(() => {
    const out: Block[] = [];
    for (const group of shown) {
      const last = out.at(-1);
      if (group.slot && last?.kind === 'slots') last.groups.push(group);
      else out.push(group.slot ? { kind: 'slots', groups: [group] } : { kind: 'group', group });
    }
    return out;
  });
  let slot = $state<string | null>(null);

  $effect(() => {
    const target = app.highlight && catalog.byId.get(app.highlight.id);
    if (!target || target.page !== pageId) return;
    const group = groups.find((g) => g.name === target.group);
    if (group?.region && region !== 'all' && region !== group.region) region = group.region;
    if (group?.slot) slot = group.slot;
  });
</script>

<article class="page">
  <header>
    <div class="titles">
      <h1>{title}</h1>
      {#if options.length}
        <p class="meta">
          {plural(options.length, 'option')}{#if changed}<span class="sep">·</span><span class="changed">{changed} changed</span>{/if}
        </p>
      {/if}
    </div>
    {#if options.length}
      <button class="btn ghost sm" disabled={!changed} onclick={() => app.resetPage(pageId)}>
        <Icon name="reset" size={14} />Reset page
      </button>
    {/if}
  </header>

  {#if regions.length}
    <div class="tabs" role="tablist" aria-label="Region">
      {#each tabs as tab (tab)}
        <button role="tab" aria-selected={region === tab} onclick={() => (region = tab)}>
          {tab === 'all' ? 'All regions' : tab}
        </button>
      {/each}
    </div>
  {/if}

  {#if !options.length}
    <div class="building">
      <div class="glyph" aria-hidden="true"><Icon name="layers" size={22} /></div>
      <h2>This section is still being built</h2>
      <p>Its options will show up here once they are ready. Until then, randomizing leaves this part of the game as it is.</p>
    </div>
  {:else}
    {#each blocks as block (block.kind === 'group' ? block.group.name : `slots:${block.groups[0].name}`)}
      {#if block.kind === 'group'}
        <section class="group">
          <h2>{block.group.name}</h2>
          {#each block.group.options as def (def.id)}
            <OptionRow {def} />
          {/each}
        </section>
      {:else}
        {@const active = block.groups.find((g) => g.slot === slot) ?? block.groups[0]}
        <section class="group">
          <h2>Slots</h2>
          <div class="tabs slot-tabs" role="tablist" aria-label="Slots">
            {#each block.groups as g (g.name)}
              <button role="tab" aria-selected={g === active} onclick={() => (slot = g.slot)}>
                {g.slot}
                {#if changedOn(g.options, app.settings.values, catalog)}<span class="dot" title="Changed from the default"></span>{/if}
              </button>
            {/each}
          </div>
          {#each active.options as def (def.id)}
            <OptionRow {def} />
          {/each}
        </section>
      {/if}
    {/each}
  {/if}
</article>

<style>
  .page {
    max-width: 880px;
  }

  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 26px;
  }

  h1 {
    font: 620 30px/1.1 var(--font-display);
    font-stretch: 92%;
    letter-spacing: -0.02em;
  }

  .meta {
    margin-top: 8px;
    color: var(--text-3);
    font-size: 13px;
  }
  .sep {
    margin: 0 7px;
  }
  .changed {
    color: var(--accent-text);
  }

  .tabs {
    display: flex;
    gap: 4px;
    margin: -8px 0 22px;
    padding-bottom: 0;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    position: relative;
    height: 36px;
    padding: 0 12px;
    color: var(--text-3);
    font-size: 13.5px;
    font-weight: 540;
    transition: color 140ms var(--ease);
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
    scale: 0 1;
    transition: scale 200ms var(--ease);
  }
  .tabs button[aria-selected='true'] {
    color: var(--text);
  }
  .tabs button[aria-selected='true']::after {
    scale: 1 1;
  }

  .group {
    margin-bottom: 34px;
  }
  .slot-tabs {
    margin: 2px 0 6px;
  }
  .slot-tabs button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .group h2 {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 8px;
    color: var(--accent-text);
    font: 650 13.5px/1 var(--font-ui);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .group h2::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--line-2);
  }

  .building {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    max-width: 520px;
    margin-top: 12px;
    padding: 30px 30px 32px;
    border-radius: var(--r-xl);
    border: 1px dashed var(--line-2);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-bottom: 6px;
    border-radius: var(--r-l);
    background: var(--surface-3);
    color: var(--text-3);
  }
  .building h2 {
    font: 600 18px/1.3 var(--font-display);
  }
  .building p {
    color: var(--text-2);
    font-size: 13.5px;
  }
</style>
