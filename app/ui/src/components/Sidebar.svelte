<script lang="ts">
  import { slide } from 'svelte/transition';
  import { app, type View } from '../lib/app.svelte';
  import { changedOn, groupsOf, sectionsOf } from '../lib/options';
  import Icon from './Icon.svelte';
  import SearchBox from './SearchBox.svelte';

  const catalog = $derived(app.catalog!);
  const pages = $derived(
    catalog.registry.pages.map((page) => {
      const options = catalog.options(page.id);
      return { ...page, size: options.length, changed: changedOn(options, app.settings.values, catalog) };
    }),
  );

  const sections = $derived(
    app.view.kind === 'page' ? sectionsOf(groupsOf(catalog.options(app.view.id))) : [],
  );

  function jump(id: string) {
    app.section = id;
    document.getElementById(`section-${id}`)?.scrollIntoView({
      block: 'start',
      behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
    });
  }

  const envTrouble = $derived(
    !!app.envError || !!app.env?.keys_error || !!app.env?.games.some((game) => game.state !== 'ready'),
  );

  const isView = (view: View) =>
    app.view.kind === view.kind && (view.kind !== 'page' || (app.view.kind === 'page' && app.view.id === view.id));
</script>

<nav class="sidebar" aria-label="Sections">
  <div class="search-slot">
    <SearchBox />
  </div>

  <div class="scroll">
    <p class="heading">What to randomize</p>
    <ul>
      {#each pages as page (page.id)}
        <li>
          <button
            class="item"
            class:quiet={!page.size}
            aria-current={isView({ kind: 'page', id: page.id }) ? 'page' : undefined}
            title={page.size ? undefined : 'Still being built'}
            onclick={() => app.goTo({ kind: 'page', id: page.id })}
          >
            <span class="name">{page.title}</span>
            {#if page.changed}<span class="badge num" title="{page.changed} changed from the defaults">{page.changed}</span>{/if}
          </button>
          {#if sections.length > 1 && isView({ kind: 'page', id: page.id })}
            <ul class="sections" transition:slide={{ duration: 180 }}>
              {#each sections as section (section.id)}
                <li>
                  <button
                    class="section"
                    aria-current={app.section === section.id ? 'location' : undefined}
                    onclick={() => jump(section.id)}
                  >
                    {section.title}
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </div>

  <ul class="tail">
    <li>
      <button class="item" aria-current={isView({ kind: 'library' }) ? 'page' : undefined} onclick={() => app.goTo({ kind: 'library' })}>
        <Icon name="stack" size={16} />
        <span class="name">Library</span>
      </button>
    </li>
    <li>
      <button class="item" aria-current={isView({ kind: 'settings' }) ? 'page' : undefined} onclick={() => app.goTo({ kind: 'settings' })}>
        <Icon name="cog" size={16} />
        <span class="name">App settings</span>
        {#if envTrouble}<span class="alert" title="Something needs your attention"></span>{/if}
      </button>
    </li>
  </ul>
</nav>

<style>
  .sidebar {
    position: relative;
    z-index: 5;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .search-slot {
    padding: 4px 12px 0 14px;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    padding: 0 12px 10px 14px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .heading {
    margin: 22px 10px 8px;
    color: var(--text-3);
    font: 600 11px/1 var(--font-ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .tail {
    padding: 4px 12px 14px 14px;
  }
  .tail::before {
    content: '';
    display: block;
    height: 1px;
    margin: 0 10px 10px;
    background: var(--line);
  }

  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--r-s);
    color: var(--text-2);
    font-size: 13.5px;
    font-weight: 500;
    text-align: left;
    transition:
      background-color 120ms var(--ease),
      color 120ms var(--ease);
  }
  .item:hover {
    background: rgb(255 255 255 / 0.04);
    color: var(--text);
  }
  .item[aria-current='page'] {
    background: var(--surface-3);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .item[aria-current='page']::before {
    content: '';
    position: absolute;
    left: -8px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 3px;
    background: var(--accent);
  }
  .item.quiet {
    color: var(--text-4);
  }
  .item.quiet:hover,
  .item.quiet[aria-current='page'] {
    color: var(--text-2);
  }

  .sections {
    position: relative;
    margin: 2px 0 6px 18px;
    padding-left: 10px;
  }
  .sections::before {
    content: '';
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 1px;
    background: var(--line-2);
  }

  .section {
    position: relative;
    display: block;
    width: 100%;
    height: 27px;
    padding: 0 10px;
    overflow: hidden;
    border-radius: var(--r-s);
    color: var(--text-3);
    font-size: 12.5px;
    font-weight: 500;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color 120ms var(--ease);
  }
  .section:hover {
    color: var(--text);
  }
  .section[aria-current='location'] {
    color: var(--accent-text);
  }
  .section[aria-current='location']::before {
    content: '';
    position: absolute;
    left: -10px;
    top: 7px;
    bottom: 7px;
    width: 1px;
    background: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    min-width: 20px;
    height: 18px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 11px;
    font-weight: 620;
    line-height: 18px;
    text-align: center;
  }

  .alert {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--warn) 18%, transparent);
  }
</style>
