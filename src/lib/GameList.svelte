<script lang="ts">
  import { parseName, type Entry } from "./api";
  import type { SvelteSet } from "svelte/reactivity";
  import Icon from "./Icon.svelte";
  import Flag from "./Flag.svelte";
  import { untrack } from "svelte";
  import ProgressRing from "./ProgressRing.svelte";
  import { reduceMotion } from "./motion";

  interface Props {
    entries: Entry[];
    selected: SvelteSet<string>;
    /** URLs of games already in the download queue. */
    queued: Set<string>;
    onOpenFolder: (url: string) => void;
    onDownload: (e: Entry) => void;
    /** Show the details modal for a game. */
    onOpen: (e: Entry) => void;
    /** Keyboard-highlighted index into `entries` (-1 for none). */
    cursor?: number;
    /** URLs of games already in the download folder. */
    downloaded?: Set<string>;
    /** Download progress (0..1) of games being fetched right now. */
    progress?: Map<string, number>;
    /** Games that have just finished, for a brief highlight. */
    justDone?: Set<string>;
    /** Changes when a new console opens, to play the entrance animation. */
    introKey?: string;
  }
  let {
    entries,
    selected,
    queued,
    onOpenFolder,
    onDownload,
    onOpen,
    cursor = -1,
    downloaded = new Set(),
    progress = new Map(),
    justDone = new Set(),
    introKey = "",
  }: Props = $props();

  // A staggered entrance when a new console's games appear.
  let intro = $state(false);
  let introTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    introKey;
    if (reduceMotion || !introKey) return;
    untrack(() => {
      intro = true;
      clearTimeout(introTimer);
      introTimer = setTimeout(() => (intro = false), 900);
    });
  });

  let viewport = $state<HTMLDivElement>();
  // Keep the keyboard highlight on screen.
  $effect(() => {
    if (cursor < 0 || !viewport) return;
    const top = cursor * ROW;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + ROW > viewport.scrollTop + viewport.clientHeight)
      viewport.scrollTop = top + ROW - viewport.clientHeight;
  });

  const ROW = 32;
  const OVERSCAN = 10;
  let scrollTop = $state(0);
  let height = $state(400);

  let start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  let end = $derived(Math.min(entries.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  let visible = $derived(entries.slice(start, end).map((e, k) => ({ e, i: start + k })));

  // Animate only when the list itself changes (filter/sort), not while scrolling.
  const MOVE_MS = 220;
  let animating = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let first = true;
  $effect(() => {
    entries; // re-run whenever the list changes
    if (first || reduceMotion) {
      first = false;
      return;
    }
    untrack(() => {
      animating = true;
      clearTimeout(timer);
      timer = setTimeout(() => (animating = false), MOVE_MS + 60);
    });
  });

  function fadeIn(_node: Element, k: number) {
    if (intro) {
      return {
        duration: 300,
        delay: Math.min(k * 16, 420),
        easing: (t: number) => 1 - Math.pow(1 - t, 3),
        css: (t: number) => `opacity: ${t}; transform: translateY(${(1 - t) * 10}px)`,
      };
    }
    return {
      duration: animating ? 200 : 0,
      delay: animating ? 60 : 0,
      css: (t: number) => `opacity: ${t}; transform: translateX(${(1 - t) * -8}px)`,
    };
  }
  function fadeOut(_node: Element) {
    return {
      duration: animating ? 140 : 0,
      css: (t: number) => `opacity: ${t}; transform: scale(${0.98 + t * 0.02})`,
    };
  }

  function toggle(url: string) {
    if (selected.has(url)) selected.delete(url);
    else selected.add(url);
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
>
  <div class="spacer" style="height: {entries.length * ROW}px">
    <div class="rows" class:animating>
      {#each visible as { e, i }, k (e.url)}
        {#if e.kind === "folder"}
          <button
            class="row folder"
            class:odd={i % 2}
            class:cursor={i === cursor}
            style="height: {ROW}px; top: {i * ROW}px"
            in:fadeIn={k}
            out:fadeOut
            onclick={() => onOpenFolder(e.url)}>
            <span class="lead"><Icon name="folder" size={15} /></span>
            <span class="title">{e.name}</span>
            <span class="size">{e.size}</span>
            <span></span>
          </button>
        {:else}
          {@const n = parseName(e.name)}
          <div
            class="row"
            class:sel={selected.has(e.url)}
            class:cursor={i === cursor}
            class:odd={i % 2}
            class:just-done={justDone.has(e.url)}
            data-url={e.url}
            style="height: {ROW}px; top: {i * ROW}px"
            in:fadeIn={k}
            out:fadeOut
            role="row"
            tabindex="-1"
            onclick={() => onOpen(e)}
            onkeydown={(k) => (k.key === " " ? toggle(e.url) : k.key === "Enter" && onOpen(e))}
          >
            <span class="lead">
              <input
                type="checkbox"
                checked={selected.has(e.url)}
                onclick={(ev) => ev.stopPropagation()}
                onchange={() => toggle(e.url)}
              />
            </span>
            <span class="title" title={e.name}>
              {n.title}
              {#each n.tags as t}
                {#if n.regions.length && t.split(/,\s*/).join(", ") === n.regions.join(", ")}
                  <span class="flags">
                    {#each n.regions as r}<Flag region={r} height={11} />{/each}
                  </span>
                {:else}
                  <span class="tag">{t}</span>
                {/if}
              {/each}
            </span>
            <span class="size">{e.size}</span>
            <span class="act">
              {#if progress.has(e.url)}
                <span class="ring" title="Downloading"><ProgressRing value={progress.get(e.url) ?? 0} size={24} /></span>
              {:else if queued.has(e.url)}
                <span class="queued">queued</span>
              {:else if downloaded.has(e.url)}
                <span class="have" title="Already in your download folder"><Icon name="check" size={15} /></span>
              {:else}
                <button
                  class="icon"
                  title="Download"
                  onclick={(ev) => {
                    ev.stopPropagation();
                    onDownload(e);
                  }}
                >
                  <Icon name="download" />
                </button>
              {/if}
            </span>
          </div>
        {/if}
      {/each}
    </div>
  </div>
  {#if entries.length === 0}
    <p class="empty">No matches.</p>
  {/if}
</div>

<style>
  .viewport {
    flex: 1;
    overflow-y: auto;
    position: relative;
    min-height: 0;
  }
  .spacer {
    position: relative;
  }
  .rows {
    position: absolute;
    inset: 0;
  }
  .row {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: grid;
    grid-template-columns: 30px 1fr 84px 40px;
    align-items: center;
    gap: 8px;
    padding: 0 10px 0 12px;
    cursor: pointer;
    position: absolute;
    left: 0;
  }
  .animating .row {
    transition: top 220ms cubic-bezier(0.2, 0, 0, 1);
  }
  .row.odd {
    background: var(--stripe);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--sel);
  }
  .row.cursor {
    box-shadow: inset 3px 0 0 var(--accent);
    background: var(--hover);
  }
  .row.cursor.sel {
    background: var(--sel);
  }
  .folder {
    cursor: pointer;
  }
  .folder .title {
    font-weight: 500;
  }
  .lead {
    display: grid;
    place-items: center;
    color: var(--accent);
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    display: inline-block;
    margin-left: 6px;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 11px;
    line-height: 17px;
    color: var(--muted);
    background: var(--chip);
    vertical-align: 1px;
  }
  .flags {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 8px;
    vertical-align: -1px;
  }
  .size {
    text-align: right;
    color: var(--muted);
    font: 12px var(--mono);
  }
  .act {
    display: grid;
    place-items: center;
  }
  .act .icon {
    opacity: 0;
  }
  .row:hover .icon,
  .icon:focus-visible {
    opacity: 1;
  }
  .ring {
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(10 6 20 / 0.55);
  }
  .ring :global(.track) {
    stroke: var(--line);
  }
  .ring :global(text) {
    fill: var(--text);
  }
  /* Just finished: the row glows green for a moment and the tick pops in. */
  .row.just-done {
    animation: done-glow 1.4s ease-out;
  }
  .just-done .have {
    animation: pop 0.55s cubic-bezier(0.3, 1.6, 0.5, 1);
  }
  @keyframes done-glow {
    0% {
      background: color-mix(in srgb, var(--ok) 28%, transparent);
    }
  }
  @keyframes pop {
    0% {
      transform: scale(0);
    }
    100% {
      transform: scale(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .row.just-done,
    .just-done .have {
      animation: none;
    }
  }
  .have {
    color: var(--ok);
    display: grid;
    place-items: center;
  }
  .queued {
    font-size: 11px;
    color: var(--accent);
  }
  .empty {
    color: var(--muted);
    padding: 16px 12px;
    margin: 0;
  }
</style>
