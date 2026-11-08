<script lang="ts">
  let {
    checked,
    label,
    disabled = false,
    onchange,
  }: { checked: boolean; label: string; disabled?: boolean; onchange: (checked: boolean) => void } = $props();
</script>

<button class="switch" role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={() => onchange(!checked)}>
  <span class="thumb"></span>
</button>

<style>
  .switch {
    position: relative;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--surface-4);
    box-shadow: inset 0 0 0 1px var(--line-2);
    transition:
      background-color 180ms var(--ease),
      box-shadow 180ms var(--ease);
  }
  /* A larger invisible target, so the small switch is easy to hit. */
  .switch::before {
    content: '';
    position: absolute;
    inset: -9px -4px;
  }
  .switch[aria-checked='true'] {
    background: var(--accent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent) 70%, black);
  }
  .switch:disabled {
    opacity: 0.45;
  }

  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #d9d5e0;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.4);
    transition:
      translate 200ms var(--ease),
      background-color 180ms var(--ease);
  }
  .switch[aria-checked='true'] .thumb {
    translate: 16px 0;
    background: #fff;
  }
  .switch:active:not(:disabled) .thumb {
    scale: 0.92;
  }
</style>
