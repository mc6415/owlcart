<script lang="ts">
  import { fade } from "svelte/transition";
  import { coverFor } from "./covers.svelte";
  import { reduceMotion } from "./motion";

  // A slow-drifting wall of the console's covers behind its header.
  // `slim`: behind a one-line header, smaller and softer.
  let { slug, titles, slim = false }: { slug: string; titles: string[]; slim?: boolean } = $props();

  const COUNT = 16;
  let urls = $state<string[]>([]);

  $effect(() => {
    const s = slug;
    // An even spread across the alphabet, not just the first few A-titles.
    const step = Math.max(1, Math.floor(titles.length / COUNT));
    const pick = titles.filter((_, i) => i % step === 0).slice(0, COUNT);
    let alive = true;
    urls = [];
    Promise.all(pick.map((t) => coverFor(t, s).catch(() => null))).then((found) => {
      if (alive) urls = found.filter((u): u is string => !!u);
    });
    return () => {
      alive = false;
    };
  });

  // Two rows, each repeated so the drift loops seamlessly.
  let rowA = $derived(urls.filter((_, i) => i % 2 === 0));
  let rowB = $derived(urls.filter((_, i) => i % 2 === 1));
</script>

{#if urls.length >= 6}
  {#key slug}
    <div class="collage" class:still={reduceMotion} class:slim transition:fade={{ duration: 600 }} aria-hidden="true">
      {#each [rowA, rowB] as row, r}
        <div class="row" class:reverse={r === 1}>
          {#each [...row, ...row] as u}
            <img src={u} alt="" loading="lazy" decoding="async" />
          {/each}
        </div>
      {/each}
    </div>
  {/key}
{/if}

<style>
  .collage {
    position: absolute;
    inset: -50px -20px -50px 22%;
    z-index: -2;
    display: flex;
    flex-direction: column;
    gap: 12px;
    transform: rotate(-8deg);
    opacity: 0.75;
    filter: saturate(1.2) blur(0.6px);
    /* Fade out toward the console name so the text stays clear. */
    -webkit-mask-image: linear-gradient(to right, transparent 0%, black 35%, black 80%, transparent 100%);
    mask-image: linear-gradient(to right, transparent 0%, black 35%, black 80%, transparent 100%);
    pointer-events: none;
  }
  .row {
    display: flex;
    gap: 12px;
    width: max-content;
    animation: drift 90s linear infinite;
  }
  .row.reverse {
    animation-direction: reverse;
    margin-left: -60px;
  }
  .slim {
    inset: -30px -20px -30px 52%;
    gap: 8px;
    opacity: 0.5;
    filter: saturate(1.3) blur(1.2px);
    overflow: hidden;
  }
  .slim .row {
    gap: 8px;
  }
  .slim .row.reverse {
    margin-left: 0;
  }
  .slim img {
    width: 58px;
  }
  .still .row {
    animation: none;
  }
  img {
    width: 104px;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 6px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.5);
  }
  @keyframes drift {
    to {
      transform: translateX(-50%);
    }
  }
</style>
