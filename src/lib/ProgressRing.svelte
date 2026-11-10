<script lang="ts">
  // A small circular progress indicator with the percentage in the middle.
  let { value, size = 28 }: { value: number; size?: number } = $props();

  const R = 10;
  const C = 2 * Math.PI * R;
  let pct = $derived(Math.max(0, Math.min(1, value)));
</script>

<svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label="{Math.round(pct * 100)}% downloaded">
  <circle cx="12" cy="12" r={R} class="track" />
  <circle
    cx="12"
    cy="12"
    r={R}
    class="fill"
    stroke-dasharray={C}
    stroke-dashoffset={C * (1 - pct)}
    transform="rotate(-90 12 12)"
  />
  <text x="12" y="12.5" dominant-baseline="middle" text-anchor="middle">{Math.round(pct * 100)}</text>
</svg>

<style>
  svg {
    display: block;
  }
  circle {
    fill: none;
    stroke-width: 2.6;
  }
  .track {
    stroke: rgb(255 255 255 / 0.18);
  }
  .fill {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.6s;
  }
  text {
    fill: #fff;
    font: 700 7px var(--mono);
  }
</style>
