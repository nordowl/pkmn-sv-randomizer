<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    open,
    title,
    onclose,
    width = 520,
    children,
    footer,
    lead,
  }: {
    open: boolean;
    title: string;
    onclose: () => void;
    width?: number;
    children: Snippet;
    footer?: Snippet;
    lead?: Snippet;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  /** Escape goes through onclose, so the owner's state stays the one source of truth. */
  function cancel(event: Event) {
    event.preventDefault();
    onclose();
  }
</script>

<dialog bind:this={dialog} class="dialog" style:--dialog-width="{width}px" oncancel={cancel} aria-label={title}>
  {#if open}
    <div class="frame">
      <header>
        {#if lead}{@render lead()}{/if}
        <h2>{title}</h2>
        <button class="btn ghost sm icon close" onclick={onclose} aria-label="Close">
          <Icon name="x" />
        </button>
      </header>
      <div class="body">
        {@render children()}
      </div>
      {#if footer}
        <footer>{@render footer()}</footer>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  .frame {
    display: flex;
    flex-direction: column;
    max-height: min(760px, calc(100vh - 64px));
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 20px 0 24px;
  }

  h2 {
    flex: 1;
    font: 600 19px/1.25 var(--font-display);
    letter-spacing: -0.01em;
  }

  .close {
    margin: -4px -4px 0 0;
    color: var(--text-3);
  }

  .body {
    padding: 14px 24px 22px;
    overflow: auto;
    color: var(--text-2);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--line);
    background: color-mix(in oklab, var(--ink) 35%, transparent);
    border-radius: 0 0 var(--r-xl) var(--r-xl);
  }
</style>
