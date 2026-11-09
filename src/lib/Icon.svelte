<script lang="ts">
  const paths = {
    download: "M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 19h14",
    pause: "M9 5v14M15 5v14",
    play: "M8 5.5v13l10-6.5z",
    x: "M6 6l12 12M18 6 6 18",
    retry: "M4 12a8 8 0 1 0 2.4-5.7M4 4v4h4",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    reveal: "M14 4h6v6M20 4l-9 9M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4",
    gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 13a7.5 7.5 0 0 0 0-2l2-1.5-2-3.4-2.3.9a7.5 7.5 0 0 0-1.7-1L15 3.6h-4l-.4 2.4a7.5 7.5 0 0 0-1.7 1l-2.3-.9-2 3.4 2 1.5a7.5 7.5 0 0 0 0 2l-2 1.5 2 3.4 2.3-.9a7.5 7.5 0 0 0 1.7 1l.4 2.4h4l.4-2.4a7.5 7.5 0 0 0 1.7-1l2.3.9 2-3.4z",
    up: "M12 19V5m0 0-6 6m6-6 6 6",
    search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4",
    min: "M6 12h12",
    max: "M6 6h12v12H6z",
    restore: "M8 8h10v10H8zM6 16V6h10",
    arrow: "M5 12h14m0 0-5-5m5 5-5 5",
    list: "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
    grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
    check: "M5 12.5l4.5 4.5L19 7.5",
  } as const;

  let { name, size = 16 }: { name: keyof typeof paths; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={name === "play" ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name]} />
</svg>

<style>
  svg {
    display: block;
    flex-shrink: 0;
  }
</style>
