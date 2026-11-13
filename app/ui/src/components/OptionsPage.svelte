<script lang="ts">
  import type { OptionDef } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { plural } from '../lib/format';
  import { applies, changedOn, groupsOf, itemsOf, regionOf, sectionsOf, type Item } from '../lib/options';
  import Icon from './Icon.svelte';
  import OptionRow from './OptionRow.svelte';
  import SwitchCards from './SwitchCards.svelte';
  import Tabs from './Tabs.svelte';

  let { pageId }: { pageId: string } = $props();

  const catalog = $derived(app.catalog!);
  const title = $derived(catalog.pageTitle(pageId));
  const options = $derived(catalog.options(pageId));
  const sections = $derived(sectionsOf(groupsOf(options)));
  const changed = $derived(changedOn(options, app.settings.values, catalog));

  /** The open tab of each tabbed section, by section id, and of each tabbed item, by its first option id. */
  let open = $state<Record<string, number>>({});

  const tab = (label: string, defs: OptionDef[]) => ({
    label,
    changed: changedOn(defs, app.settings.values, catalog) > 0,
    locked: defs.every((def) => !applies(def, app.settings.values)),
  });

  $effect(() => {
    const target = app.highlight && catalog.byId.get(app.highlight.id);
    if (!target || target.page !== pageId) return;
    for (const section of sections) {
      const index = section.groups.findIndex((group) => group.options.includes(target));
      if (index < 0) continue;
      open[section.id] = index;
      for (const item of itemsOf(section.groups[index]!)) {
        if (item.kind === 'regional' && item.defs.includes(target)) open[item.defs[0].id] = item.defs.indexOf(target);
        if (item.kind === 'switched') {
          const part = item.parts.findIndex((defs) => defs.includes(target));
          if (part >= 0) open[item.parts[0]![0].id] = part;
        }
      }
    }
  });

  const itemKey = (item: Item) => {
    switch (item.kind) {
      case 'option':
        return item.def.id;
      case 'switched':
        return item.parts[0]![0].id;
      default:
        return item.defs[0]!.id;
    }
  };

  let page: HTMLElement | undefined = $state();

  /** Tells the sidebar which section is under the top edge. */
  $effect(() => {
    const scroller = page?.closest('main');
    if (!scroller) return;
    const ids = sections.map((section) => section.id);
    const spy = () => {
      const line = scroller.getBoundingClientRect().top + 64;
      const atEnd = scroller.scrollTop > 0 && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
      let current = ids[0] ?? null;
      for (const id of ids) {
        const element = document.getElementById(`section-${id}`);
        if (element && (atEnd || element.getBoundingClientRect().top <= line)) current = id;
      }
      app.section = current;
    };
    spy();
    scroller.addEventListener('scroll', spy, { passive: true });
    return () => scroller.removeEventListener('scroll', spy);
  });
</script>

<article class="page" bind:this={page}>
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

  {#if !options.length}
    <div class="building">
      <div class="glyph" aria-hidden="true"><Icon name="layers" size={22} /></div>
      <h2>This section is still being built</h2>
      <p>Its options will show up here once they are ready. Until then, randomizing leaves this part of the game as it is.</p>
    </div>
  {:else}
    {#each sections as section (section.id)}
      {@const index = Math.min(open[section.id] ?? 0, section.groups.length - 1)}
      <section class="group" id="section-{section.id}">
        <div class="section-head">
          <h2>{section.title}</h2>
          {#if section.groups.length > 1}
            <Tabs
              label={section.title}
              tabs={section.groups.map((group) => tab(group.tab, group.options))}
              active={index}
              onselect={(next) => (open[section.id] = next)}
            />
          {/if}
        </div>
        {#each itemsOf(section.groups[index]!) as item (itemKey(item))}
          {#if item.kind === 'option'}
            <OptionRow def={item.def} />
          {:else if item.kind === 'switches'}
            <SwitchCards defs={item.defs} help={item.help} />
          {:else if item.kind === 'switched'}
            {@const key = itemKey(item)}
            {@const at = Math.min(open[key] ?? 0, item.parts.length - 1)}
            <div class="switched">
              <Tabs
                label={section.title}
                tabs={item.parts.map((defs) => tab(defs[0].label, defs))}
                active={at}
                onselect={(next) => (open[key] = next)}
              />
              {#each item.parts[at]! as def (def.id)}
                <OptionRow {def} />
              {/each}
            </div>
          {:else}
            {@const key = itemKey(item)}
            {@const at = Math.min(open[key] ?? 0, item.defs.length - 1)}
            {#key item.defs[at]!.id}
              <OptionRow def={item.defs[at]!}>
                {#snippet tabs()}
                  <Tabs
                    compact
                    label="Region"
                    tabs={item.defs.map((def) => tab(regionOf(def.label) ?? 'Paldea', [def]))}
                    active={at}
                    onselect={(next) => (open[key] = next)}
                  />
                {/snippet}
              </OptionRow>
            {/key}
          {/if}
        {/each}
      </section>
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
    margin-bottom: 12px;
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

  .group {
    margin-bottom: 20px;
  }
  .section-head {
    position: sticky;
    /* Sticks to the edge of the scroll area, not below its padding. */
    top: calc(-1 * var(--page-top, 0px));
    z-index: 3;
    margin: 0 -16px;
    padding: 16px 16px 0;
    background: var(--surface);
  }
  .section-head :global([role='tablist']) {
    margin-top: 2px;
  }
  .switched {
    margin-top: 10px;
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
