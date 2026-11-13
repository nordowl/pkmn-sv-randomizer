<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import AppSettingsPage from './components/AppSettingsPage.svelte';
  import BrandMark from './components/BrandMark.svelte';
  import Icon from './components/Icon.svelte';
  import LibraryPage from './components/LibraryPage.svelte';
  import Notices from './components/Notices.svelte';
  import OptionsPage from './components/OptionsPage.svelte';
  import RunSheet from './components/RunSheet.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import SwitchDialog from './components/SwitchDialog.svelte';
  import Toasts from './components/Toasts.svelte';
  import TopBar from './components/TopBar.svelte';
  import { app } from './lib/app.svelte';

  let main: HTMLElement | undefined = $state();

  onMount(() => {
    void app.boot();
  });

  $effect(() => {
    document.documentElement.dataset.edition = app.edition;
  });

  $effect(() => {
    void app.settings;
    void app.seed;
    void app.edition;
    void app.view;
    void { ...app.batch, ...app.batch.counts };
    void { ...app.zip };
    const timer = setTimeout(() => app.persist(), 300);
    return () => clearTimeout(timer);
  });

  $effect(() => app.schedulePoolCounts());

  $effect(() => {
    const edition = app.edition;
    if (app.gameReady && !app.species[edition]) untrack(() => app.ensureSpecies(edition));
  });

  $effect(() => {
    void app.view;
    if (untrack(() => app.highlight && Date.now() - app.highlight.at < 1500)) return;
    main?.scrollTo({ top: 0 });
  });

  function shortcuts(event: KeyboardEvent) {
    const typing = event.target instanceof HTMLElement && event.target.closest('input, textarea, select, [contenteditable]');
    if ((event.ctrlKey && event.key.toLowerCase() === 'k') || (event.key === '/' && !typing)) {
      event.preventDefault();
      const search = document.getElementById('option-search');
      search?.focus();
      (search as HTMLInputElement | null)?.select();
    }
  }
</script>

<svelte:window onkeydown={shortcuts} />

{#if app.bootError}
  <div class="splash">
    <BrandMark edition={app.edition} size={44} />
    <h1>The randomizer could not start</h1>
    <p>{app.bootError}</p>
    <button class="btn primary" onclick={() => location.reload()}><Icon name="reset" size={15} />Try again</button>
  </div>
{:else if !app.booted}
  <div class="splash" aria-busy="true">
    <BrandMark edition={app.edition} size={44} />
    <p class="loading"><Icon name="spinner" size={15} stroke={2.2} class="spin" />Starting up…</p>
  </div>
{:else}
  <div class="shell">
    <TopBar />
    <Sidebar />
    <main bind:this={main}>
      <Notices />
      {#if app.view.kind === 'page'}
        {#key app.view.id}
          <OptionsPage pageId={app.view.id} />
        {/key}
      {:else if app.view.kind === 'library'}
        <LibraryPage />
      {:else}
        <AppSettingsPage />
      {/if}
    </main>
  </div>
  <RunSheet />
  <SwitchDialog />
{/if}
<Toasts />

<style>
  .shell {
    --sidebar-width: 252px;
    display: grid;
    grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    height: 100vh;
    min-width: 1040px;
  }
  .shell::before {
    content: '';
    position: fixed;
    inset: 0 auto auto 0;
    width: 720px;
    height: 420px;
    background: radial-gradient(closest-side, color-mix(in oklab, var(--accent) 9%, transparent), transparent);
    translate: -38% -52%;
    pointer-events: none;
  }

  main {
    --page-top: 34px;
    position: relative;
    overflow-y: auto;
    padding: var(--page-top) 44px 60px;
    border-top-left-radius: 18px;
    background: var(--surface);
    box-shadow:
      inset 1px 1px 0 var(--line),
      -1px -1px 0 rgb(0 0 0 / 0.25);
    scrollbar-gutter: stable;
  }

  .splash {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 14px;
    height: 100vh;
    padding: 24px;
    text-align: center;
  }
  .splash h1 {
    font: 620 22px/1.2 var(--font-display);
  }
  .splash p {
    max-width: 520px;
    color: var(--text-2);
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-3);
  }
</style>
