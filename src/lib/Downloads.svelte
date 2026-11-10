<script lang="ts">
  import { api, formatBytes, formatEta, type Job } from "./api";
  import Icon from "./Icon.svelte";
  import { isAndroid } from "./platform";

  let { jobs, onError, limit = 5 }: { jobs: Job[]; onError: (msg: string) => void; limit?: number } = $props();

  const IN_PROGRESS: Job["state"][] = ["resolving", "queued", "starting", "downloading", "paused", "moving", "extracting"];
  let showAll = $state(false);
  /** In-progress downloads first (in queue order), then finished ones newest first. */
  let ordered = $derived([
    ...jobs.filter((j) => IN_PROGRESS.includes(j.state)),
    ...jobs.filter((j) => !IN_PROGRESS.includes(j.state)).reverse(),
  ]);
  let shown = $derived(showAll ? ordered : ordered.slice(0, limit));

  const labels: Record<Job["state"], string> = {
    resolving: "looking up",
    queued: "queued",
    starting: "starting",
    downloading: "downloading",
    paused: "paused",
    moving: "finishing",
    extracting: "extracting",
    done: "done",
    failed: "failed",
    cancelled: "cancelled",
  };

  let active = $derived(jobs.filter((j) => j.state === "downloading"));
  let totalSpeed = $derived(active.reduce((s, j) => s + j.speed, 0));
  let hasFinished = $derived(jobs.some((j) => ["done", "failed", "cancelled"].includes(j.state)));

  function run(p: Promise<unknown>) {
    p.catch((e) => onError(String(e)));
  }

  function pct(j: Job) {
    return j.total ? Math.min(100, (j.completed / j.total) * 100) : 0;
  }
</script>

<div class="panel">
  <header>
    <h2>Downloads</h2>
    {#if active.length}
      <span class="rate">{formatBytes(totalSpeed)}/s</span>
    {/if}
    <div class="grow"></div>
    {#if hasFinished}
      <button class="text" onclick={() => run(api.clearFinished())}>Clear finished</button>
    {/if}
    <button class="icon" class:hidden={isAndroid} title="Open download folder" onclick={() => run(api.openDownloads())}>
      <Icon name="folder" />
    </button>
  </header>

  <div class="list">
    {#each shown as j (j.id)}
      <div class="job {j.state}">
        <div class="top">
          <div class="name" title={j.name}>{j.name.replace(/\.zip$/i, "")}</div>
          <div class="btns">
            {#if j.state === "downloading"}
              <button class="icon" title="Pause" onclick={() => run(api.pause(j.id))}><Icon name="pause" size={14} /></button>
            {:else if j.state === "paused"}
              <button class="icon" title="Resume" onclick={() => run(api.resume(j.id))}><Icon name="play" size={14} /></button>
            {/if}
            {#if j.state === "failed" || j.state === "cancelled"}
              <button class="icon" title="Retry" onclick={() => run(api.retry(j.id))}><Icon name="retry" size={14} /></button>
            {/if}
            {#if j.state === "done" && j.output && !isAndroid}
              <button class="icon" title="Show in folder" onclick={() => run(api.revealFile(j.output!))}><Icon name="reveal" size={14} /></button>
            {/if}
            {#if !["done", "moving", "extracting", "cancelled"].includes(j.state)}
              <button class="icon" title="Cancel" onclick={() => run(api.cancel(j.id))}><Icon name="x" size={14} /></button>
            {/if}
          </div>
        </div>
        {#if j.state !== "done" && j.state !== "cancelled"}
          <div class="bar"><div style="width: {pct(j)}%"></div></div>
        {/if}
        <div class="meta">
          <span class="state">{labels[j.state]}</span>
          {#if j.state === "downloading" || j.state === "paused"}
            <span>{formatBytes(j.completed)} of {formatBytes(j.total)}</span>
          {:else if j.state === "extracting" && j.total}
            <span>{Math.round(pct(j))}%</span>
          {:else if j.total && j.state !== "failed"}
            <span>{formatBytes(j.total)}</span>
          {/if}
          {#if j.state === "downloading"}
            <span class="grow"></span>
            <span>{formatBytes(j.speed)}/s</span>
            <span>{j.peers}p</span>
            <span>{formatEta(j.total - j.completed, j.speed)}</span>
          {/if}
        </div>
        {#if j.error}<div class="error">{j.error}</div>{/if}
      </div>
    {:else}
      <p class="empty">Queue is empty.</p>
    {/each}
  </div>
  {#if ordered.length > limit}
    <button class="text more" onclick={() => (showAll = !showAll)}>
      {showAll ? "Show fewer" : `Show all ${ordered.length}`}
    </button>
  {/if}
</div>

<style>
  .panel {
    width: 380px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .more {
    border-top: 1px solid var(--line) !important;
    border-radius: 0;
    padding: 9px 14px !important;
    text-align: left;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 41px;
    padding: 0 8px 0 14px;
    border-bottom: 1px solid var(--line);
    box-sizing: border-box;
  }
  h2 {
    margin: 0;
    font: 700 14px var(--display);
    letter-spacing: -0.01em;
  }
  .rate {
    font: 600 13px var(--mono);
    background: var(--grad);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .grow {
    flex: 1;
  }
  .list {
    overflow-y: auto;
    max-height: min(60vh, 520px);
  }
  .job {
    padding: 9px 10px 9px 14px;
    border-bottom: 1px solid var(--line);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .name {
    flex: 1;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btns {
    display: flex;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    margin: 7px 0 5px;
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    border-radius: 2px;
    background: var(--grad);
    transition: width 0.4s;
  }
  /* Slow shimmer while bytes are flowing. */
  .downloading .bar div,
  .extracting .bar div {
    background:
      linear-gradient(90deg, transparent 0%, rgb(255 255 255 / 0.35) 50%, transparent 100%) 0 0 / 40% 100% no-repeat,
      var(--grad);
    animation: shimmer 1.8s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: -60% 0, 0 0;
    }
    to {
      background-position: 160% 0, 0 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .downloading .bar div {
      animation: none;
    }
  }
  .paused .bar div {
    background: var(--muted);
  }
  .meta {
    display: flex;
    gap: 10px;
    font: 11px var(--mono);
    color: var(--muted);
    margin-top: 2px;
  }
  .state {
    color: var(--text);
  }
  .done .state {
    color: var(--ok);
  }
  .done .error {
    color: var(--muted);
  }
  .failed .state {
    color: var(--danger);
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin-top: 4px;
    word-break: break-word;
  }
  .empty {
    color: var(--muted);
    padding: 12px 14px;
    margin: 0;
  }
  .hidden {
    display: none;
  }
</style>
