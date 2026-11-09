<script lang="ts">
  import { parseName, type Entry } from "./api";
  import type { SvelteSet } from "svelte/reactivity";
  import { untrack } from "svelte";
  import Cover from "./Cover.svelte";
  import Flag from "./Flag.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    entries: Entry[];
    /** Cover manifest name for this listing. */
    slug: string;
    selected: SvelteSet<string>;
    queued: Set<string>;
    onOpenFolder: (url: string) => void;
    onDownload: (e: Entry) => void;
    /** Show the details modal for a game. */
    onOpen: (e: Entry) => void;
    /** Keyboard-highlighted index into `entries` (-1 for none). */
    cursor?: number;
    /** Columns currently shown, reported back for up/down keys. */
    cols?: number;
    /** URLs of games already in the download folder. */
    downloaded?: Set<string>;
  }
  let {
    entries,
    slug,
    selected,
    queued,
    onOpenFolder,
    onDownload,
    onOpen,
    cursor = -1,
    cols = $bindable(1),
    downloaded = new Set(),
  }: Props = $props();

  const PAD = 14;
  const GAP = 14;
  const MIN_CARD = 150;
  const CAPTION = 50;

  let scrollTop = $state(0);
  let height = $state(400);
  let width = $state(800);

  let colCount = $derived(Math.max(1, Math.floor((width - PAD * 2 + GAP) / (MIN_CARD + GAP))));
  $effect(() => {
    cols = colCount;
  });

  let viewport = $state<HTMLDivElement>();
  // Keep the keyboard highlight on screen.
  $effect(() => {
    if (cursor < 0 || !viewport) return;
    const top = PAD + Math.floor(cursor / colCount) * rowH;
    if (top < viewport.scrollTop) viewport.scrollTop = Math.max(0, top - PAD);
    else if (top + rowH > viewport.scrollTop + viewport.clientHeight)
      viewport.scrollTop = top + rowH - viewport.clientHeight;
  });
  let cardW = $derived((width - PAD * 2 - GAP * (colCount - 1)) / colCount);
  let coverH = $derived(Math.round(cardW * 1.5)); // 2:3, SteamGridDB's portrait shape
  let rowH = $derived(coverH + CAPTION + GAP);
  let rows = $derived(Math.ceil(entries.length / colCount));

  let firstRow = $derived(Math.max(0, Math.floor(scrollTop / rowH) - 2));
  let lastRow = $derived(Math.min(rows, Math.ceil((scrollTop + height) / rowH) + 2));
  let visible = $derived(
    entries.slice(firstRow * colCount, lastRow * colCount).map((e, k) => ({ e, i: firstRow * colCount + k })),
  );

  // Animate only when the list itself changes (filter/sort), not while scrolling or resizing.
  const MOVE_MS = 240;
  const reduceMotion =
    typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  let animating = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let first = true;
  $effect(() => {
    entries;
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

  function fadeIn(_node: Element) {
    return {
      duration: animating ? 220 : 0,
      delay: animating ? 60 : 0,
      css: (t: number) => `opacity: ${t}; transform: scale(${0.94 + t * 0.06})`,
    };
  }
  function fadeOut(_node: Element) {
    return {
      duration: animating ? 150 : 0,
      css: (t: number) => `opacity: ${t}; transform: scale(${0.96 + t * 0.04})`,
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
  bind:clientWidth={width}
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
>
  <div class="spacer" class:animating style="height: {rows * rowH + PAD}px">
    {#each visible as { e, i } (e.url)}
      {@const x = PAD + (i % colCount) * (cardW + GAP)}
      {@const y = PAD + Math.floor(i / colCount) * rowH}
      {#if e.kind === "folder"}
        <button
          class="card"
          style="left: {x}px; top: {y}px; width: {cardW}px"
          in:fadeIn
          out:fadeOut
          onclick={() => onOpenFolder(e.url)}
        >
          <div class="art" style="height: {coverH}px"><Cover title={e.name} folder /></div>
          <div class="caption">
            <div class="name">{e.name}</div>
            <div class="sub">{e.size}</div>
          </div>
        </button>
      {:else}
        {@const n = parseName(e.name)}
        <div
          class="card"
          class:sel={selected.has(e.url)}
          class:cursor={i === cursor}
          style="left: {x}px; top: {y}px; width: {cardW}px"
          role="button"
          tabindex="-1"
          title={e.name}
          in:fadeIn
          out:fadeOut
          onclick={() => onOpen(e)}
          onkeydown={(k) => (k.key === " " ? toggle(e.url) : k.key === "Enter" && onOpen(e))}
        >
          <div class="art" style="height: {coverH}px">
            <Cover title={n.title} {slug} glow />
            <button
              class="check"
              title={selected.has(e.url) ? "Deselect" : "Select"}
              onclick={(ev) => {
                ev.stopPropagation();
                toggle(e.url);
              }}
            >
              <Icon name="check" size={13} />
            </button>
            {#if n.regions.length}
              <span class="flags">
                {#each n.regions as r}<Flag region={r} height={11} />{/each}
              </span>
            {/if}
            {#if queued.has(e.url)}
              <span class="queued">queued</span>
            {:else if downloaded.has(e.url)}
              <span class="have" title="Already in your download folder"><Icon name="check" size={13} /></span>
            {:else}
              <button
                class="dl"
                title="Download"
                onclick={(ev) => {
                  ev.stopPropagation();
                  onDownload(e);
                }}
              >
                <Icon name="download" size={16} />
              </button>
            {/if}
          </div>
          <div class="caption">
            <div class="name">{n.title}</div>
            <div class="sub">
              {e.size}{#each n.tags.filter((t) => t !== n.regions.join(", ")) as t}<span>· {t}</span>{/each}
            </div>
          </div>
        </div>
      {/if}
    {/each}
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
  .card {
    all: unset;
    position: absolute;
    box-sizing: border-box;
    cursor: pointer;
  }
  .animating .card {
    transition:
      top 240ms cubic-bezier(0.2, 0, 0, 1),
      left 240ms cubic-bezier(0.2, 0, 0, 1);
  }
  .art {
    position: relative;
    border-radius: 8px;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.25);
    isolation: isolate;
    transition: transform 0.15s, box-shadow 0.15s;
  }
  .card:hover .art {
    transform: translateY(-3px);
    box-shadow: none;
  }
  .cursor .art {
    box-shadow: 0 0 0 2px var(--text), 0 6px 16px rgb(0 0 0 / 0.3);
  }
  .sel .art {
    box-shadow: 0 0 0 2px var(--accent), 0 6px 16px rgb(0 0 0 / 0.3);
  }
  .check {
    position: absolute;
    top: 8px;
    left: 8px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.35);
    border: 1.5px solid rgb(255 255 255 / 0.7);
    color: transparent;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .card:hover .check {
    opacity: 1;
  }
  .sel .check {
    opacity: 1;
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .flags {
    position: absolute;
    top: 9px;
    right: 8px;
    display: flex;
    gap: 3px;
  }
  .dl {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.35);
    opacity: 0;
    transform: translateY(4px);
    transition: opacity 0.15s, transform 0.15s;
  }
  .card:hover .dl,
  .dl:focus-visible {
    opacity: 1;
    transform: none;
  }
  .have {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--ok);
    color: #fff;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.35);
  }
  .queued {
    position: absolute;
    right: 8px;
    bottom: 8px;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 11px;
    background: var(--accent);
    color: var(--on-accent);
  }
  .caption {
    height: 50px;
    padding-top: 7px;
    box-sizing: border-box;
  }
  .name {
    font: 600 13px var(--display);
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub span {
    margin-left: 4px;
  }
  .empty {
    color: var(--muted);
    padding: 16px 14px;
    margin: 0;
  }
</style>
