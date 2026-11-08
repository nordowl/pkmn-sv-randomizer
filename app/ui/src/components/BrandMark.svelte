<script lang="ts">
  import type { Edition } from '../lib/api';

  let { edition, size = 30 }: { edition: Edition; size?: number } = $props();

  const SHAPES = {
    scarlet: { x: 9, y: 12, cx: 22, cy: 25, fill: 'url(#mark-warm)', pip: '#fff6ee' },
    violet: { x: 29, y: 26, cx: 42, cy: 39, fill: 'url(#mark-cool)', pip: '#f4eeff' },
  } as const;

  /** The selected game's diamond is drawn last, so it sits in front. */
  const order = $derived(edition === 'scarlet' ? (['violet', 'scarlet'] as const) : (['scarlet', 'violet'] as const));
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <defs>
    <linearGradient id="mark-warm" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ff8a4c" />
      <stop offset="1" stop-color="#e8452c" />
    </linearGradient>
    <linearGradient id="mark-cool" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#b48cff" />
      <stop offset="1" stop-color="#7446e0" />
    </linearGradient>
  </defs>
  {#each order as key (key)}
    {@const shape = SHAPES[key]}
    <g class:dim={key !== edition}>
      <rect x={shape.x} y={shape.y} width="26" height="26" rx="5.5" transform="rotate(45 {shape.cx} {shape.cy})" fill={shape.fill} />
      <circle cx={shape.cx} cy={shape.cy} r="3" fill={shape.pip} />
    </g>
  {/each}
</svg>

<style>
  g {
    transition: opacity 420ms var(--ease);
  }
  .dim {
    opacity: 0.35;
  }
</style>
