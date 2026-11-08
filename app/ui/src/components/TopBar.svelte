<script lang="ts">
  import { app } from '../lib/app.svelte';
  import BrandMark from './BrandMark.svelte';
  import GamePicker from './GamePicker.svelte';
  import RunControls from './RunControls.svelte';
  import SettingsMenu from './SettingsMenu.svelte';
</script>

<header class="topbar">
  <div class="brand">
    <BrandMark edition={app.edition} />
    <span class="word">SV Randomizer</span>
  </div>
  <div class="bar">
    <GamePicker />
    <div class="end">
      <SettingsMenu />
      <RunControls />
    </div>
  </div>
</header>

<style>
  .topbar {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: var(--sidebar-width) 1fr;
    align-items: center;
    height: 64px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-left: 20px;
  }

  .word {
    font: 650 16px/1 var(--font-display);
    font-stretch: 86%;
    letter-spacing: -0.015em;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 0 16px 0 8px;
    min-width: 0;
  }

  .end {
    display: flex;
    align-items: center;
    gap: 10px;
  }
</style>
