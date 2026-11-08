<script lang="ts">
  import { app, type Toast } from '../lib/app.svelte';
  import Icon from './Icon.svelte';

  const ICON = { error: 'alert', warning: 'alert', success: 'check', info: 'info' } as const;

  let layer: HTMLElement | undefined = $state();

  // Modal dialogs sit in the top layer; re-showing this popover lifts toasts above any open one.
  $effect(() => {
    const count = app.toasts.length;
    if (!layer) return;
    const shown = layer.matches(':popover-open');
    if (!count) {
      if (shown) layer.hidePopover();
      return;
    }
    if (shown && document.querySelector('dialog[open]')) layer.hidePopover();
    if (!layer.matches(':popover-open')) layer.showPopover();
  });

  /** Errors wait to be dismissed; the rest leave on their own unless hovered. */
  function lifetime(toast: Toast): number | null {
    if (toast.tone === 'error') return null;
    return toast.lines?.length || toast.action ? 9000 : 5000;
  }

  function autoDismiss(node: HTMLElement, toast: Toast) {
    const life = lifetime(toast);
    if (life === null) return;
    let left = life;
    let started = Date.now();
    let timer = setTimeout(() => app.dismiss(toast.id), left);
    const pause = () => {
      clearTimeout(timer);
      left -= Date.now() - started;
    };
    const resume = () => {
      started = Date.now();
      timer = setTimeout(() => app.dismiss(toast.id), Math.max(left, 1500));
    };
    node.addEventListener('pointerenter', pause);
    node.addEventListener('pointerleave', resume);
    return {
      destroy() {
        clearTimeout(timer);
        node.removeEventListener('pointerenter', pause);
        node.removeEventListener('pointerleave', resume);
      },
    };
  }
</script>

<div class="toasts" aria-live="polite" popover="manual" bind:this={layer}>
  {#each app.toasts as toast (toast.id)}
    <div class="toast {toast.tone}" role={toast.tone === 'error' ? 'alert' : 'status'} use:autoDismiss={toast}>
      <span class="icon"><Icon name={ICON[toast.tone]} size={15} stroke={toast.tone === 'success' ? 2.2 : 1.8} /></span>
      <div class="text">
        <p class="title">{toast.title}</p>
        {#if toast.body}<p class="body">{toast.body}</p>{/if}
        {#if toast.lines?.length}
          <ul>{#each toast.lines as line, i (i)}<li>{line}</li>{/each}</ul>
        {/if}
        {#if toast.action}
          {@const action = toast.action}
          <button
            class="action"
            onclick={() => {
              action.run();
              app.dismiss(toast.id);
            }}
          >
            {action.label}
          </button>
        {/if}
      </div>
      <button class="close" aria-label="Dismiss" onclick={() => app.dismiss(toast.id)}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    inset: auto 18px 18px auto;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    overflow: visible;
    color: var(--text);
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    pointer-events: none;
  }

  .toast {
    --tone: var(--info);
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    width: 410px;
    padding: 12px 10px 12px 12px;
    border-radius: var(--r-l);
    background: var(--surface-3);
    box-shadow: var(--shadow-pop);
    pointer-events: auto;
    animation: toast-in 240ms var(--ease-out);
  }
  .toast.error {
    --tone: var(--danger);
  }
  .toast.warning {
    --tone: var(--warn);
  }
  .toast.success {
    --tone: var(--ok);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: color-mix(in oklab, var(--tone) 18%, transparent);
    color: var(--tone);
  }

  .title {
    padding-top: 2px;
    font-size: 13.5px;
    font-weight: 600;
  }
  .body,
  ul {
    margin-top: 3px;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  ul {
    padding-left: 16px;
  }

  .action {
    margin-top: 8px;
    padding: 4px 10px;
    border-radius: var(--r-s);
    background: rgb(255 255 255 / 0.07);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 600;
    transition: background-color 120ms var(--ease);
  }
  .action:hover {
    background: rgb(255 255 255 / 0.12);
  }

  .close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--r-s);
    color: var(--text-3);
  }
  .close:hover {
    color: var(--text);
    background: rgb(255 255 255 / 0.06);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      translate: 0 8px;
      scale: 0.98;
    }
  }
</style>
