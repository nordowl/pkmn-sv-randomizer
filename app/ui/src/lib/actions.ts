import type { Action } from 'svelte/action';

/** Calls `close` on a press outside the node or on Escape. Put it on the wrapper of trigger and popover. */
export const dismissable: Action<HTMLElement, () => void> = (node, close) => {
  let current = close;
  const press = (event: PointerEvent) => {
    if (!node.contains(event.target as Node)) current();
  };
  const key = (event: KeyboardEvent) => {
    if (event.key === 'Escape') current();
  };
  window.addEventListener('pointerdown', press, true);
  window.addEventListener('keydown', key);
  return {
    update(next) {
      current = next;
    },
    destroy() {
      window.removeEventListener('pointerdown', press, true);
      window.removeEventListener('keydown', key);
    },
  };
};
