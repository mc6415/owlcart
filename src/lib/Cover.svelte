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
  // Up to two initials, skipping little words: "The Legend of Zelda" -> "LZ";
  // a leading number stays whole: "007 Shitou" -> "007", "3.15 Firmware" -> "3.15".
  // (The second screen does the same in second-screen.html.)
  let initials = $derived.by(() => {
    const words = title.split(/[\s\-:]+/).filter((w) => /[A-Za-z0-9]/.test(w) && !/^(the|a|an|of)$/i.test(w));
    if (/^\d[\d.]{0,4}$/.test(words[0] ?? "")) return words[0];
    const first = (w: string) => w.match(/[A-Za-z0-9]/)?.[0] ?? "";
    return (words.length ? words : [title]).slice(0, 2).map(first).join("").toUpperCase() || "?";
  });
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
      <span class="frame"></span>
      <span class="initials">{initials}</span>
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
  :global(html:not(.android) .card:hover) .glow {
    opacity: 0.7;
  }
  /* The keyboard / D-pad highlight glows in the cover's own colours, gently
     breathing so it's easy to spot. */
  :global(.card.cursor) .glow {
    opacity: 0.8;
    animation: breathe 2.4s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.45;
      transform: scale(0.96);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.card.cursor) .glow {
      animation: none;
    }
  }
  .cover {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: 10px;
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
  /* No art: the initials and title in a thin inset frame, like a box. */
  .frame {
    position: absolute;
    inset: 7px;
    border: 1.5px solid rgb(255 255 255 / 0.18);
    border-radius: 6px;
  }
  .initials {
    position: absolute;
    left: 0;
    right: 0;
    top: 30%;
    text-align: center;
    font-size: 38px;
    font-weight: 800;
    line-height: 1;
    letter-spacing: -0.04em;
    text-shadow: 0 2px 12px rgb(0 0 0 / 0.3);
  }
  .title {
    position: absolute;
    left: 14px;
    right: 14px;
    top: calc(30% + 48px);
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    line-height: 1.25;
    opacity: 0.8;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
