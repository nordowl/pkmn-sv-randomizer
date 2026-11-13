import { app } from './app.svelte';

/** Which of `ids` the search or a lock link just revealed. `element` scrolls into view and the id
 * stays set for a moment, so the option can flash. */
export function revealOf(ids: () => string[], element: () => HTMLElement | undefined): { readonly id: string | null } {
  let flashing = $state<string | null>(null);

  $effect(() => {
    const highlight = app.highlight;
    const node = element();
    if (!highlight || !ids().includes(highlight.id) || Date.now() - highlight.at > 1500 || !node) return;
    node.scrollIntoView({ block: 'center', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    flashing = highlight.id;
    const timer = setTimeout(() => (flashing = null), 1600);
    return () => clearTimeout(timer);
  });

  return {
    get id() {
      return flashing;
    },
  };
}
