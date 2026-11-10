<script lang="ts">
  // Shimmering placeholders shown while a console's listing loads.
  let { view }: { view: "list" | "grid" } = $props();
</script>

<div class="skeleton {view}" aria-busy="true" aria-label="Loading games">
  {#if view === "grid"}
    {#each Array(14) as _, i}
      <div class="card" style="animation-delay: {i * 40}ms">
        <div class="art shimmer"></div>
        <div class="line shimmer" style="width: {60 + ((i * 37) % 30)}%"></div>
        <div class="line short shimmer"></div>
      </div>
    {/each}
  {:else}
    {#each Array(18) as _, i}
      <div class="row" style="animation-delay: {i * 25}ms">
        <span class="box shimmer"></span>
        <span class="line shimmer" style="width: {35 + ((i * 41) % 40)}%"></span>
        <span class="grow"></span>
        <span class="line tiny shimmer"></span>
      </div>
    {/each}
  {/if}
</div>

<style>
  .skeleton {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 14px;
    padding: 14px;
    align-content: start;
  }
  .card,
  .row {
    opacity: 0;
    animation: appear 0.4s ease-out forwards;
  }
  .art {
    aspect-ratio: 2 / 3;
    border-radius: 8px;
  }
  .line {
    height: 11px;
    margin-top: 9px;
    border-radius: 6px;
  }
  .line.short {
    width: 35%;
    margin-top: 6px;
    height: 9px;
  }
  .list .row {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 32px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
  }
  .list .line {
    margin: 0;
  }
  .box {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .grow {
    flex: 1;
  }
  .tiny {
    width: 60px;
    flex-shrink: 0;
  }
  /* A soft band of light sweeping across every placeholder. */
  .shimmer {
    background: linear-gradient(
        100deg,
        transparent 30%,
        color-mix(in srgb, var(--text) 9%, transparent) 50%,
        transparent 70%
      )
      0 0 / 200% 100%,
      var(--field);
    animation: shimmer 1.4s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 150% 0, 0 0;
    }
    to {
      background-position: -50% 0, 0 0;
    }
  }
  @keyframes appear {
    to {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .shimmer {
      animation: none;
    }
    .card,
    .row {
      animation: none;
      opacity: 1;
    }
  }
</style>
