<script lang="ts">
  import { parseName, type Entry } from "./api";
  import type { SvelteSet } from "svelte/reactivity";
  import { untrack } from "svelte";
  import Cover from "./Cover.svelte";
  import Flag from "./Flag.svelte";
  import Icon from "./Icon.svelte";
  import ProgressRing from "./ProgressRing.svelte";
  import { reduceMotion } from "./motion";
  import { isAndroid, onScreenHeight } from "./platform";
  import { ratingTier } from "./rating";

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
    /** Download progress (0..1) of games being fetched right now. */
    progress?: Map<string, number>;
    /** Number of versions of each card's game (regions, revisions), by URL. */
    versionCounts?: Map<string, number>;
    /** IGDB [rating 0-100, review count] of each rated card's game, by URL. */
    ratings?: Map<string, readonly [number, number]>;
    /** Games that have just finished, for a brief celebration. */
    justDone?: Set<string>;
    /** Changes when a new console opens, to play the entrance animation. */
    introKey?: string;
    /** -1 / 1: the new console's cards slide in from the left / right. */
    introDir?: number;
    /** Title-only captions, when another screen shows the rest. */
    brief?: boolean;
    /** The console's box shape, height / width (2:3 by default). */
    coverRatio?: number;
    /** Controller in hand: only the highlighted card shows its buttons and
     *  flags (select circles show on every card while some are selected). */
    quiet?: boolean;
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
    progress = new Map(),
    justDone = new Set(),
    versionCounts = new Map(),
    ratings = new Map(),
    introKey = "",
    introDir = 0,
    brief = false,
    quiet = false,
    coverRatio = 1.5,
  }: Props = $props();

  // Smaller cards and gaps on Android, so phones in portrait get 2+ columns.
  const PAD = isAndroid ? 10 : 14;
  const GAP = isAndroid ? 10 : 14;
  const MIN_CARD = isAndroid ? 104 : 150;
  let caption = $derived(brief ? 30 : 50);

  let scrollTop = $state(0);
  let height = $state(400);
  let width = $state(800);

  // Squarer and wider boxes get wider cards, so every console's covers are
  // about the same size on screen.
  let minCard = $derived(MIN_CARD * Math.sqrt(1.5 / coverRatio));
  let colCount = $derived(Math.max(1, Math.floor((width - PAD * 2 + GAP) / (minCard + GAP))));
  $effect(() => {
    cols = colCount;
  });

  let viewport = $state<HTMLDivElement>();
  // Keep the keyboard highlight on screen.
  $effect(() => {
    if (cursor < 0 || !viewport) return;
    const top = PAD + Math.floor(cursor / colCount) * rowH;
    const shown = onScreenHeight(viewport);
    if (top < viewport.scrollTop) viewport.scrollTop = Math.max(0, top - PAD);
    else if (top + rowH > viewport.scrollTop + shown) viewport.scrollTop = top + rowH - shown;
  });
  let cardW = $derived((width - PAD * 2 - GAP * (colCount - 1)) / colCount);
  let coverH = $derived(Math.round(cardW * coverRatio));
  let rowH = $derived(coverH + caption + GAP);
  let rows = $derived(Math.ceil(entries.length / colCount));

  let firstRow = $derived(Math.max(0, Math.floor(scrollTop / rowH) - 2));
  let lastRow = $derived(Math.min(rows, Math.ceil((scrollTop + height) / rowH) + 2));
  let visible = $derived(
    entries.slice(firstRow * colCount, lastRow * colCount).map((e, k) => ({ e, i: firstRow * colCount + k })),
  );

  // Animate only when the list itself changes (filter/sort), not while scrolling or resizing.
  const MOVE_MS = 240;
  let animating = $state(false);

  // A staggered entrance when a new console's games appear.
  let intro = $state(false);
  let introTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    introKey;
    if (reduceMotion || !introKey) return;
    untrack(() => {
      intro = true;
      clearTimeout(introTimer);
      introTimer = setTimeout(() => (intro = false), 1000);
    });
  });

  // 3D tilt toward the pointer, with a soft glare. Written straight to the
  // element's style so moving the mouse doesn't re-render anything.
  function tilt(el: HTMLElement) {
    if (reduceMotion) return;
    let frame = 0;
    const move = (ev: PointerEvent) => {
      // Touch would leave the last-touched cover tilted; tilt for the mouse only.
      if (ev.pointerType !== "mouse") return;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        const px = (ev.clientX - r.left) / r.width;
        const py = (ev.clientY - r.top) / r.height;
        el.style.setProperty("--ry", `${(px - 0.5) * 14}deg`);
        el.style.setProperty("--rx", `${(0.5 - py) * 12}deg`);
        el.style.setProperty("--gx", `${px * 100}%`);
        el.style.setProperty("--gy", `${py * 100}%`);
      });
    };
    const leave = () => {
      cancelAnimationFrame(frame);
      el.style.setProperty("--rx", "0deg");
      el.style.setProperty("--ry", "0deg");
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerleave", leave);
    return {
      destroy() {
        el.removeEventListener("pointermove", move);
        el.removeEventListener("pointerleave", leave);
      },
    };
  }
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

  function fadeIn(_node: Element, k: number) {
    if (intro) {
      // New console: cards rise in one after another.
      return {
        duration: 380,
        delay: Math.min(k * 28, 520),
        easing: (t: number) => 1 - Math.pow(1 - t, 3),
        css: (t: number) =>
          `opacity: ${t}; transform: translate(${(1 - t) * introDir * 60}px, ${introDir ? 0 : (1 - t) * 18}px) scale(${0.96 + t * 0.04})`,
      };
    }
    // Sorted or filtered: the newly shown cards ripple in.
    return {
      duration: animating ? 220 : 0,
      delay: animating ? 60 + Math.min(k * 16, 240) : 0,
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
  <div class="spacer" class:animating class:quiet class:selecting={selected.size > 0} style="height: {rows * rowH + PAD}px">
    {#each visible as { e, i }, k (e.url)}
      {@const x = PAD + (i % colCount) * (cardW + GAP)}
      {@const y = PAD + Math.floor(i / colCount) * rowH}
      {#if e.kind === "folder"}
        <button
          class="card"
          style="left: {x}px; top: {y}px; width: {cardW}px"
          in:fadeIn={k}
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
          class:just-done={justDone.has(e.url)}
          data-url={e.url}
          style="left: {x}px; top: {y}px; width: {cardW}px"
          role="button"
          tabindex="-1"
          title={e.name}
          in:fadeIn={k}
          out:fadeOut
          onclick={() => onOpen(e)}
          onkeydown={(k) => (k.key === " " ? toggle(e.url) : k.key === "Enter" && onOpen(e))}
        >
          <div class="art" style="height: {coverH}px" use:tilt>
            <Cover title={n.title} {slug} glow />
            <span class="glare"></span>
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
            {#if ratings.get(e.url)}
              {@const [r, count] = ratings.get(e.url)!}
              <span class="score {ratingTier(r)}" title="IGDB rating, from {count} {count === 1 ? 'review' : 'reviews'}">{r}</span>
            {/if}
            {#if n.regions.length}
              <span class="flags">
                {#each n.regions as r}<Flag region={r} height={11} />{/each}
              </span>
            {/if}
            {#if progress.has(e.url)}
              <span class="ring" title="Downloading">
                <ProgressRing value={progress.get(e.url) ?? 0} size={30} />
              </span>
            {:else if queued.has(e.url)}
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
            {#if !brief}
              <div class="sub">
                {#if (versionCounts.get(e.url) ?? 1) > 1}
                  {versionCounts.get(e.url)} versions<span>· {e.size}</span>
                {:else}
                  {e.size}{#each n.tags.filter((t) => t !== n.regions.join(", ")) as t}<span>· {t}</span>{/each}
                {/if}
              </div>
            {/if}
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
    --rx: 0deg;
    --ry: 0deg;
    position: relative;
    border-radius: 10px;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.3);
    isolation: isolate;
    transform: perspective(700px) rotateX(var(--rx)) rotateY(var(--ry));
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.2s;
  }
  /* Lifted under the mouse or the keyboard / D-pad highlight. Android only
     lifts the highlight: there a tap counts as hovering until the next one,
     which left the last-touched cover lifted. */
  :global(html:not(.android)) .card:hover .art,
  .card.cursor .art {
    transform: perspective(700px) rotateX(var(--rx)) rotateY(var(--ry)) translateY(-4px) scale(1.02);
    transition: transform 0.12s ease-out, box-shadow 0.2s;
  }
  :global(html:not(.android)) .card:not(.cursor):hover .art {
    box-shadow: none;
  }
  /* A soft highlight that follows the pointer across the cover. */
  .glare {
    position: absolute;
    inset: 0;
    z-index: 2;
    border-radius: 10px;
    pointer-events: none;
    background: radial-gradient(circle at var(--gx, 50%) var(--gy, 0%), rgb(255 255 255 / 0.32), transparent 55%);
    mix-blend-mode: soft-light;
    opacity: 0;
    transition: opacity 0.25s;
  }
  :global(html:not(.android)) .card:hover .glare,
  .card.cursor .glare {
    opacity: 1;
  }
  .ring {
    position: absolute;
    right: 7px;
    bottom: 7px;
    z-index: 3;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgb(10 6 20 / 0.7);
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.4);
  }
  /* Just finished: a light sweep across the cover and the tick pops in. */
  .just-done .art::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 2;
    border-radius: 8px;
    pointer-events: none;
    background: linear-gradient(105deg, transparent 30%, rgb(255 255 255 / 0.55) 50%, transparent 70%) -150% 0 / 60% 100% no-repeat;
    animation: sweep 1s ease-out forwards;
  }
  .just-done .have {
    animation: pop 0.55s cubic-bezier(0.3, 1.6, 0.5, 1);
  }
  @keyframes sweep {
    to {
      background-position: 250% 0;
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
    .art,
    :global(html:not(.android)) .card:hover .art,
    .card.cursor .art {
      transform: none;
    }
    .just-done .art::after,
    .just-done .have {
      animation: none;
    }
  }
  /* The highlight: a ring in the console's colour (--tint, set by the page)
     with a light sweeping slowly round. */
  .cursor .art::before {
    content: "";
    position: absolute;
    inset: -3px;
    z-index: -1;
    border-radius: 13px;
    background: linear-gradient(120deg, var(--tint, var(--accent)), #fff, var(--accent), var(--tint, var(--accent))) 0 0 /
      300% 100%;
    animation: ring 4s linear infinite;
  }
  @keyframes ring {
    to {
      background-position: 300% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .cursor .art::before {
      animation: none;
    }
  }
  .sel .art {
    box-shadow: 0 0 0 2px var(--accent), 0 0 30px color-mix(in srgb, var(--accent) 55%, transparent);
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
  :global(html:not(.android)) .card:hover .check {
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
  /* Quiet: buttons and flags on the highlighted card only. Beats the Android
     rule above that keeps the buttons showing. */
  .quiet .card:not(.cursor) .dl,
  .quiet:not(.selecting) .card:not(.cursor):not(.sel) .check,
  .quiet .card:not(.cursor) .flags {
    opacity: 0 !important;
    pointer-events: none;
  }
  .quiet .flags {
    transition: opacity 0.15s;
  }
  .score {
    position: absolute;
    left: 8px;
    bottom: 8px;
    padding: 2px 7px;
    border-radius: 9px;
    background: rgb(0 0 0 / 0.62);
    color: #fff;
    font: 600 11px var(--mono);
    backdrop-filter: blur(4px);
  }
  .score.good {
    background: rgb(28 150 84 / 0.88);
  }
  .score.mid {
    background: rgb(196 132 16 / 0.88);
  }
  .score.low {
    background: rgb(184 52 52 / 0.88);
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
  /* Touch screens have no hover, so keep the buttons visible. (Android
     handhelds with gamepads can report hover support, so check both.) */
  @media (hover: none) {
    .dl,
    .check {
      opacity: 1 !important;
      transform: none !important;
    }
  }
  :global(html.android) .dl,
  :global(html.android) .check {
    opacity: 1 !important;
    transform: none !important;
  }
  :global(html:not(.android)) .card:hover .dl,
  .card.cursor .dl,
  .dl:focus-visible {
    opacity: 1;
    transform: none;
  }
  .have {
    position: absolute;
    z-index: 3;
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
  :global(html.no-color-mix) .sel .art {
    box-shadow: 0 0 0 2px var(--accent);
  }
</style>
