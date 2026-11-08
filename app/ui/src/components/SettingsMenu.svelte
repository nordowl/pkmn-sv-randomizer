<script lang="ts">
  import { dismissable } from '../lib/actions';
  import { app } from '../lib/app.svelte';
  import { sameValue } from '../lib/options';
  import Icon from './Icon.svelte';
  import ShareDialog from './ShareDialog.svelte';

  let open = $state(false);
  let sharing = $state(false);

  const presets = $derived(app.registry?.presets ?? []);
  const atDefaults = $derived(!!app.registry && sameValue(app.settings.values, app.registry.defaults.values));
  const current = $derived(presets.find((preset) => sameValue(preset.settings.values, app.settings.values)));
  /** A preset equal to the defaults is not news, so the trigger names presets only once something changed. */
  const badge = $derived(atDefaults ? undefined : current);

  function run(action: () => void) {
    open = false;
    action();
  }
</script>

<div class="menu" use:dismissable={() => (open = false)}>
  <button class="btn trigger" aria-expanded={open} aria-haspopup="true" onclick={() => (open = !open)}>
    <Icon name="sliders" size={16} />
    <span>Settings</span>
    {#if badge}<span class="current">{badge.name}</span>{/if}
    <Icon name="chevronDown" size={14} />
  </button>

  {#if open}
    <div class="popover panel">
      <p class="heading">Presets</p>
      {#each presets as preset (preset.id)}
        <button class="item preset" onclick={() => run(() => app.applyPreset(preset))}>
          <span class="title">
            <Icon name="spark" size={14} />{preset.name}
            {#if current?.id === preset.id}<span class="tag accent">In use</span>{/if}
          </span>
          <span class="description">{preset.description}</span>
        </button>
      {/each}
      <div class="rule"></div>
      <button class="item" onclick={() => run(() => app.loadSettingsFile())}>
        <Icon name="upload" size={15} />Load settings file…
      </button>
      <button class="item" onclick={() => run(() => app.saveSettingsFile())}>
        <Icon name="download" size={15} />Save settings file…
      </button>
      <button class="item" onclick={() => run(() => (sharing = true))}>
        <Icon name="link" size={15} />Share code…
      </button>
      <div class="rule"></div>
      <button class="item" disabled={atDefaults} onclick={() => run(() => app.resetAll())}>
        <Icon name="reset" size={15} />Reset all settings
      </button>
    </div>
  {/if}
</div>

<ShareDialog open={sharing} onclose={() => (sharing = false)} />

<style>
  .menu {
    position: relative;
  }

  .trigger {
    padding: 0 10px 0 12px;
  }
  .trigger :global(svg:last-child) {
    color: var(--text-3);
  }

  .current {
    padding: 2px 7px;
    border-radius: var(--r-xs);
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: 11.5px;
    font-weight: 600;
  }

  .panel {
    top: calc(100% + 8px);
    right: 0;
    width: 360px;
    padding: 6px;
  }

  .heading {
    padding: 8px 10px 6px;
    color: var(--text-3);
    font: 600 11px/1 var(--font-ui);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 34px;
    padding: 0 10px;
    border-radius: var(--r-s);
    color: var(--text);
    font-size: 13.5px;
    text-align: left;
    transition: background-color 120ms var(--ease);
  }
  .item:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.05);
  }
  .item:disabled {
    color: var(--text-4);
  }
  .item :global(svg) {
    color: var(--text-3);
  }

  .preset {
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    padding: 8px 10px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .description {
    display: -webkit-box;
    overflow: hidden;
    padding-left: 22px;
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.4;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .rule {
    height: 1px;
    margin: 6px 8px;
    background: var(--line);
  }
</style>
