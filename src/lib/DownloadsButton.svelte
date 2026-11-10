<script lang="ts">
  import { fly } from "svelte/transition";
  import { SvelteSet } from "svelte/reactivity";
  import type { Job } from "./api";
  import Downloads from "./Downloads.svelte";
  import Icon from "./Icon.svelte";

  let { jobs, onError }: { jobs: Job[]; onError: (msg: string) => void } = $props();

  let open = $state(false);

  /** Opens or closes the popover (also used by the D shortcut). */
  export function toggle() {
    open = !open;
  }
  let root = $state<HTMLDivElement>();

  const IN_PROGRESS: Job["state"][] = ["resolving", "queued", "starting", "downloading", "paused", "moving", "extracting"];
  let inProgress = $derived(jobs.filter((j) => IN_PROGRESS.includes(j.state)));

  /** Overall progress of everything in flight, 0..1. */
  let progress = $derived.by(() => {
    const total = inProgress.reduce((s, j) => s + j.total, 0);
    return total ? inProgress.reduce((s, j) => s + j.completed, 0) / total : 0;
  });
  let downloading = $derived(inProgress.some((j) => j.state === "downloading" || j.state === "extracting"));

  // A dot for finished downloads you haven't looked at yet.
  const seen = new SvelteSet<number>();
  let unseen = $derived(jobs.some((j) => j.state === "done" && !seen.has(j.id)));
  $effect(() => {
    if (open) for (const j of jobs) if (j.state === "done") seen.add(j.id);
  });

  // A small bump when something new is queued.
  let bump = $state(false);
  let lastCount = -1; // -1 = first run, so existing jobs don't bump on startup
  $effect(() => {
    const n = jobs.length;
    if (lastCount >= 0 && n > lastCount) {
      bump = true;
      setTimeout(() => (bump = false), 450);
    }
    lastCount = n;
  });

  // Ring geometry.
  const R = 14;
  const C = 2 * Math.PI * R;
</script>

<svelte:window
  onmousedown={(e) => {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

<div class="wrap" bind:this={root}>
  <button
    class="trigger"
    data-downloads-button
    class:open
    class:bump
    title={inProgress.length ? `${inProgress.length} downloading (D)` : "Downloads (D)"}
    onclick={() => (open = !open)}
  >
    {#if inProgress.length}
      <svg class="ring" viewBox="0 0 32 32" aria-hidden="true">
        <circle cx="16" cy="16" r={R} class="track" />
        <circle
          cx="16"
          cy="16"
          r={R}
          class="fill"
          class:spin={!downloading || progress === 0}
          stroke-dasharray={C}
          stroke-dashoffset={progress > 0 ? C * (1 - progress) : C * 0.75}
        />
      </svg>
    {/if}
    <Icon name="download" size={16} />
    {#if inProgress.length}
      <span class="count">{inProgress.length}</span>
    {:else if unseen}
      <span class="dot"></span>
    {/if}
  </button>

  {#if open}
    <div class="pop" transition:fly={{ y: -6, duration: 140 }}>
      <Downloads {jobs} {onError} />
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .trigger {
    position: relative;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .trigger:hover,
  .trigger.open {
    background: var(--hover);
    color: var(--text);
  }
  .bump {
    animation: bump 0.45s cubic-bezier(0.3, 1.6, 0.5, 1);
  }
  @keyframes bump {
    40% {
      transform: scale(1.18);
    }
  }
  .ring {
    position: absolute;
    inset: 1px;
    width: 32px;
    height: 32px;
    transform: rotate(-90deg);
    pointer-events: none;
  }
  .ring circle {
    fill: none;
    stroke-width: 2.4;
  }
  .track {
    stroke: var(--line);
  }
  .fill {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.6s;
  }
  /* Waiting (looking up, queued, paused): a spinning arc instead of progress. */
  .fill.spin {
    transform-origin: 16px 16px;
    animation: ring-spin 1.2s linear infinite;
  }
  @keyframes ring-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .count {
    position: absolute;
    top: -2px;
    right: -3px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    box-sizing: border-box;
    border-radius: 8px;
    background: var(--grad);
    color: #fff;
    font: 700 10px/16px var(--mono);
    text-align: center;
  }
  .dot {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 2px var(--panel);
  }
  .pop {
    position: absolute;
    top: calc(100% + 8px);
    right: -6px;
    z-index: 45;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 16px 44px rgb(0 0 0 / 0.38);
    overflow: hidden;
  }
  @media (prefers-reduced-motion: reduce) {
    .bump,
    .fill.spin {
      animation: none;
    }
  }
</style>
