<script lang="ts">
  import { coverFor, coverState, knownCover, loadedImages } from "./covers.svelte";

  // Shows cover art from the game data when it exists, otherwise a placeholder.
  // `slug` names the console (see listingSlug); omit it for no lookup.
  let {
    title,
    slug = "",
    folder = false,
    glow = false,
  }: { title: string; slug?: string; folder?: boolean; glow?: boolean } = $props();

  let src = $state<string | null>(null);
  let loaded = $state(false);

  $effect(() => {
    coverState.version;
    const t = title;
    const sl = slug;
    if (!sl || folder) return;
    // Already known: show it straight away (no delay, no fade if it loaded before).
    const known = knownCover(t, sl);
    if (known !== undefined) {
      src = known;
      loaded = !!known && loadedImages.has(known);
      return;
    }
    src = null;
    loaded = false;
    let alive = true;
    // Wait briefly so cards that fly past while scrolling don't trigger lookups.
    const timer = setTimeout(() => {
      coverFor(t, sl).then((u) => {
        if (alive) src = u;
      });
    }, 150);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  });

  function hash(s: string) {
    let h = 0;
    for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
    return Math.abs(h);
  }

  let hue = $derived(hash(title) % 360);
  let letter = $derived((title.match(/[A-Za-z0-9]/)?.[0] ?? "?").toUpperCase());
</script>

<div class="wrap">
  {#if glow && src && loaded}
    <img class="glow" {src} alt="" aria-hidden="true" />
  {/if}
  <div
    class="cover"
    class:folder
    style="--h: {hue}"
  >
    {#if folder}
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
    {:else}
      <span class="letter">{letter}</span>
      <span class="title">{title}</span>
    {/if}
    {#if src}
      <img
        {src}
        alt={title}
        decoding="sync"
        class:loaded
        onload={() => {
          loaded = true;
          if (src) loadedImages.add(src);
        }}
        onerror={() => (src = null)}
      />
    {/if}
  </div>
</div>

<style>
  .wrap {
    position: relative;
    width: 100%;
    height: 100%;
  }
  /* A blurred copy of the art behind the card, shown on hover. */
  .glow {
    position: absolute;
    left: 6%;
    right: 6%;
    top: 10%;
    bottom: -4%;
    width: 88%;
    height: 94%;
    object-fit: cover;
    filter: blur(22px) saturate(1.7);
    opacity: 0;
    transition: opacity 0.3s;
    pointer-events: none;
  }
  :global(.card:hover) .glow {
    opacity: 0.7;
  }
  .cover {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: 8px;
    background:
      radial-gradient(120% 80% at 20% 0%, hsl(var(--h) 45% 42% / 0.9), transparent 70%),
      linear-gradient(160deg, hsl(var(--h) 35% 28%), hsl(calc(var(--h) + 40) 30% 14%));
    color: #fff;
  }
  .folder {
    display: grid;
    place-items: center;
    background: var(--chip);
    color: var(--accent);
  }
  .folder svg {
    width: 40%;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linejoin: round;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity 0.3s;
  }
  img.loaded {
    opacity: 1;
  }
  .letter {
    position: absolute;
    right: -4%;
    bottom: -14%;
    font-size: 9em;
    font-weight: 800;
    line-height: 1;
    opacity: 0.14;
    letter-spacing: -0.05em;
  }
  .title {
    position: absolute;
    left: 10px;
    right: 10px;
    top: 10px;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.25;
    opacity: 0.85;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
