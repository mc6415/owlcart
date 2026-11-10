<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { reduceMotion, EASE_OUT } from "./motion";
  import type { Addon } from "./addons";
  import { isAndroid } from "./platform";
  import { api, gameDetails, igdbImage, parseName, type Entry, type GameDetails } from "./api";
  import { coverFor } from "./covers.svelte";
  import Cover from "./Cover.svelte";
  import Flag from "./Flag.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Parsed game title (without region/language tags). */
    title: string;
    /** Every file on this listing with that title. */
    versions: Entry[];
    slug: string;
    queued: Set<string>;
    onDownload: (e: Entry) => void;
    onClose: () => void;
    /** Where a version already is on disk, if it's downloaded. */
    pathFor?: (e: Entry) => string | undefined;
    /** Where the modal grows from: the clicked card's cover, or its list row. */
    origin?: { rect: DOMRect; card: boolean } | null;
    /** Finds that spot again when closing (it may have moved or scrolled away). */
    findOrigin?: () => { rect: DOMRect; card: boolean } | null;
    /** Whether this console has update/DLC collections at all. */
    addonsEnabled?: boolean;
    /** This game's updates and DLC, once loaded. */
    addons?: { updates: Addon[]; dlc: Addon[] };
    addonsLoading?: boolean;
    onDownloadAddon?: (a: Addon) => void;
  }
  let {
    title,
    versions,
    slug,
    queued,
    onDownload,
    onClose,
    pathFor = () => undefined,
    origin = null,
    findOrigin = () => null,
    addonsEnabled = false,
    addons = undefined,
    addonsLoading = false,
    onDownloadAddon = () => {},
  }: Props = $props();

  const DLC_PREVIEW = 8;
  let showFullSummary = $state(false);
  let showAllDlc = $state(false);
  let dlcShown = $derived(addons ? (showAllDlc ? addons.dlc : addons.dlc.slice(0, DLC_PREVIEW)) : []);

  let panelEl = $state<HTMLDivElement>();
  let coverEl = $state<HTMLDivElement>();

  /** Transform that puts the modal's cover exactly over `rect`. */
  function coverTransform(rect: DOMRect) {
    const t = coverEl!.getBoundingClientRect();
    const dx = rect.left + rect.width / 2 - (t.left + t.width / 2);
    const dy = rect.top + rect.height / 2 - (t.top + t.height / 2);
    return `translate(${dx}px, ${dy}px) scale(${rect.width / t.width}, ${rect.height / t.height})`;
  }

  /** Panel transform that starts it small, centred on `rect`. */
  function panelFrom(rect: DOMRect) {
    const p = panelEl!.getBoundingClientRect();
    const dy = rect.top + rect.height / 2 - (p.top + p.height / 2);
    return `translateY(${dy * 0.6}px) scale(0.86)`;
  }

  // Opening: the clicked card's cover grows into the modal's cover, and the
  // panel unfolds around it. From the list, the panel grows out of the row.
  onMount(() => {
    if (!panelEl || reduceMotion) return;
    const opts = { duration: 460, easing: EASE_OUT };
    if (origin?.card && coverEl) {
      coverEl.animate([{ transform: coverTransform(origin.rect), borderRadius: "8px" }, { transform: "none" }], opts);
      panelEl.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 260, easing: "ease-out" });
    } else if (origin) {
      panelEl.animate([{ opacity: 0, transform: panelFrom(origin.rect) }, { opacity: 1, transform: "none" }], opts);
    } else {
      panelEl.animate([{ opacity: 0, transform: "scale(0.96)" }, { opacity: 1, transform: "none" }], {
        duration: 220,
        easing: EASE_OUT,
      });
    }
  });

  // Closing runs the opening in reverse, then tells the page.
  let closing = false;
  function close() {
    if (closing) return;
    closing = true;
    if (!panelEl || reduceMotion) return onClose();
    const back = findOrigin();
    const opts = { duration: 340, easing: "cubic-bezier(0.4, 0, 0.6, 1)", fill: "forwards" as const };
    let anim: Animation;
    if (back?.card && coverEl) {
      anim = coverEl.animate([{ transform: "none" }, { transform: coverTransform(back.rect), borderRadius: "8px" }], opts);
      panelEl.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 240, delay: 60, fill: "forwards" });
    } else if (back) {
      anim = panelEl.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: panelFrom(back.rect) }], opts);
    } else {
      anim = panelEl.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(0.96)" }], {
        ...opts,
        duration: 180,
      });
    }
    anim.onfinish = () => onClose();
  }

  let details = $state<GameDetails | null>(null);
  let loading = $state(true);
  let coverUrl = $state<string | null>(null);
  let hero = $state<string | null>(null);

  $effect(() => {
    const t = title;
    loading = true;
    details = null;
    hero = null;
    gameDetails(t, slug)
      .then((d) => {
        details = d;
        const bg = d?.artworks?.[0] ?? d?.screenshots?.[0];
        hero = bg ? igdbImage(bg, "1080p") : null;
      })
      .catch(() => (details = null))
      .finally(() => (loading = false));
    coverFor(t, slug).then((u) => (coverUrl = u));
  });

  let year = $derived(details?.released?.slice(0, 4));
  let byline = $derived(
    [...new Set([...(details?.developers ?? []), ...(details?.publishers ?? [])])].slice(0, 3).join(" · "),
  );
  let allRegions = $derived([...new Set(versions.flatMap((v) => parseName(v.name).regions))]);

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 200 }} onclick={close}>
  <div
    bind:this={panelEl}
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <div class="hero" class:empty={!hero}>
      {#if hero}
        {#key hero}
          <img class="hero-img" src={hero} alt="" transition:fade={{ duration: 250 }} />
        {/key}
      {/if}
      <button class="icon close" title="Close" onclick={close}><Icon name="x" size={16} /></button>
    </div>

    <div class="head">
      <div class="cover" bind:this={coverEl}>
        {#if coverUrl}
          <img src={coverUrl} alt={title} />
        {:else}
          <Cover {title} />
        {/if}
      </div>
      <div class="titles">
        <h2>{details?.name ?? title}</h2>
        <div class="meta">
          {#if year}<span>{year}</span>{/if}
          {#if byline}<span>{byline}</span>{/if}
          {#if allRegions.length}
            <span class="flags">{#each allRegions as r}<Flag region={r} height={11} />{/each}</span>
          {/if}
        </div>
        {#if details?.genres?.length || details?.rating}
          <div class="chips">
            {#if details?.rating}<span class="rating" title="IGDB rating">{details.rating}</span>{/if}
            {#each details?.genres ?? [] as g}<span class="chip">{g}</span>{/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="body">
      <!-- Downloads: versions, updates and DLC. -->
      <div class="main">
        <section class="versions-block">
        <h3>Versions</h3>
        <div class="versions">
          {#each versions as v (v.url)}
            {@const n = parseName(v.name)}
            {@const other = n.tags.filter((t) => t !== n.regions.join(", "))}
            {@const path = pathFor(v)}
            <div class="version">
              <span class="vflags">
                {#each n.regions as r}<Flag region={r} height={12} />{/each}
              </span>
              <span class="vname" title={v.name}>
                {n.regions.join(", ") || "Unknown region"}
                {#each other as t}<span class="tag">{t}</span>{/each}
              </span>
              <span class="vsize">{v.size}</span>
              {#if queued.has(v.url)}
                <span class="queued">queued</span>
              {:else if path}
                <span class="vhave">
                  <span class="ok"><Icon name="check" size={13} /> Downloaded</span>
                  <button class="icon" class:hidden={isAndroid} title="Show in folder" onclick={() => api.revealFile(path)}>
                    <Icon name="reveal" size={14} />
                  </button>
                  <button class="text again" title="Download again" onclick={() => onDownload(v)}>Again</button>
                </span>
              {:else}
                <button class="primary small" onclick={() => onDownload(v)}>
                  <Icon name="download" size={14} /> Download
                </button>
              {/if}
            </div>
          {/each}
        </div>
        </section>

        {#if addonsEnabled}
          <h3>
            Updates &amp; DLC
            {#if addons}<span class="count">{addons.updates.length + addons.dlc.length}</span>{/if}
          </h3>
          {#if !addons && addonsLoading}
            <p class="muted">Looking for updates and DLC…</p>
          {:else if !addons || (!addons.updates.length && !addons.dlc.length)}
            <p class="muted">None found for this game.</p>
          {:else}
            {#each [{ label: "Updates", items: addons.updates }, { label: "DLC", items: dlcShown }] as group (group.label)}
              {#if group.items.length}
                <div class="group-label">{group.label}{group.label === "DLC" ? ` · ${addons.dlc.length}` : ""}</div>
                <div class="versions">
                  {#each group.items as a (a.entry.url)}
                    {@const n = parseName(a.entry.name)}
                    {@const path = pathFor(a.entry)}
                    <div class="version">
                      <span class="vflags">
                        {#each n.regions as r}<Flag region={r} height={12} />{/each}
                      </span>
                      <span class="vname" title={a.entry.name}>{a.label}</span>
                      <span class="vsize">{a.entry.size}</span>
                      {#if queued.has(a.entry.url)}
                        <span class="queued">queued</span>
                      {:else if path}
                        <span class="vhave">
                          <span class="ok"><Icon name="check" size={13} /> Downloaded</span>
                          <button class="icon" class:hidden={isAndroid} title="Show in folder" onclick={() => api.revealFile(path)}>
                            <Icon name="reveal" size={14} />
                          </button>
                        </span>
                      {:else}
                        <button class="secondary small" onclick={() => onDownloadAddon(a)}>
                          <Icon name="download" size={14} /> Download
                        </button>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            {/each}
            {#if addons.dlc.length > DLC_PREVIEW}
              <button class="text more" onclick={() => (showAllDlc = !showAllDlc)}>
                {showAllDlc ? "Show fewer" : `Show all ${addons.dlc.length} DLC`}
              </button>
            {/if}
            <p class="credit">Saved to the console's Updates and DLC folders.</p>
          {/if}
        {/if}
      </div>
      <!-- About the game: description, screenshots, credit. -->
      <div class="side">
        <div class="about">
          {#if loading}
            <p class="muted">Loading details…</p>
          {:else if details?.summary}
            <p class="summary" class:clamp={!showFullSummary}>{details.summary}</p>
            {#if details.summary.length > 220}
              <button class="text more-summary" onclick={() => (showFullSummary = !showFullSummary)}>
                {showFullSummary ? "Less" : "More"}
              </button>
            {/if}
          {:else}
            <p class="muted">No details found for this game.</p>
          {/if}
        </div>

        {#if details?.screenshots?.length}
          <div class="shots">
            {#each details.screenshots as s}
              {@const big = igdbImage(s, "1080p")}
              <button class="shot" class:on={hero === big} title="Show as background" onclick={() => (hero = big)}>
                <img src={igdbImage(s, "screenshot_med")} alt="" loading="lazy" />
              </button>
            {/each}
          </div>
        {/if}

        {#if details}
          <p class="credit">Game details from IGDB.com</p>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(8 5 16 / 0.55);
    backdrop-filter: blur(3px);
  }
  .panel {
    position: relative;
    width: min(860px, 92vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    /* Not clipped, so the cover can fly in from the grid; the banner clips itself. */
    overflow: visible;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.45);
  }
  .hero {
    position: relative;
    height: 240px;
    flex-shrink: 0;
    overflow: hidden;
    border-radius: 13px 13px 0 0;
    background:
      radial-gradient(100% 120% at 20% 0%, color-mix(in srgb, var(--accent) 45%, transparent), transparent 70%),
      var(--field);
  }
  .hero-img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .hero::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(to bottom, rgb(0 0 0 / 0.15), transparent 40%, var(--panel));
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 2;
    width: 32px !important;
    height: 32px !important;
    border-radius: 50% !important;
    background: rgb(0 0 0 / 0.45) !important;
    color: #fff !important;
  }
  .close:hover {
    background: rgb(0 0 0 / 0.65) !important;
  }
  .head {
    position: relative;
    z-index: 1;
    display: flex;
    gap: 20px;
    align-items: flex-end;
    margin-top: -120px;
    padding: 0 24px;
  }
  .cover {
    width: 150px;
    aspect-ratio: 2 / 3;
    flex-shrink: 0;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.5);
    background: var(--field);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .titles {
    min-width: 0;
    padding-bottom: 4px;
  }
  h2 {
    margin: 0 0 6px;
    font: 700 28px/1.1 var(--display);
    letter-spacing: -0.03em;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    color: var(--muted);
    font-size: 13px;
  }
  .flags,
  .vflags {
    display: inline-flex;
    gap: 3px;
    align-items: center;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }
  .chip {
    padding: 2px 9px;
    border-radius: 10px;
    background: var(--chip);
    color: var(--text);
    font-size: 12px;
  }
  .rating {
    padding: 2px 9px;
    border-radius: 10px;
    background: var(--grad);
    color: var(--on-accent);
    font: 600 12px var(--mono);
  }
  .body {
    min-height: 0; /* scroll inside the panel rather than growing past it */
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 18px 24px 22px;
    user-select: text;
  }
  .summary {
    margin: 0;
    line-height: 1.6;
    max-width: 70ch;
  }
  .muted {
    margin: 0;
    color: var(--muted);
  }
  .shots {
    display: flex;
    gap: 8px;
    margin-top: 16px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .shot {
    flex-shrink: 0;
    width: 150px;
    aspect-ratio: 16 / 9;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 6px;
    overflow: hidden;
    background: var(--field);
  }
  .shot.on {
    border-color: var(--accent);
  }
  .shot img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 20px 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  h3 .count {
    padding: 0 7px;
    border-radius: 9px;
    background: var(--chip);
    font: 600 11px/18px var(--mono);
    color: var(--text);
  }
  .group-label {
    margin: 12px 0 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .more {
    margin-top: 6px;
    font-size: 12px;
  }
  :global(button.secondary.small) {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
  }
  .versions {
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: hidden;
  }
  .version {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
  }
  .version + .version {
    border-top: 1px solid var(--line);
  }
  .vname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    margin-left: 6px;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--muted);
    background: var(--chip);
  }
  .vsize {
    font: 12px var(--mono);
    color: var(--muted);
  }
  .vhave {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .vhave .ok {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ok);
    font-size: 12px;
    font-weight: 600;
    margin-right: 2px;
  }
  .again {
    font-size: 12px;
  }
  .queued {
    font-size: 12px;
    color: var(--accent);
    padding: 0 8px;
  }
  :global(button.primary.small) {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
  }
  .credit {
    margin: 14px 0 0;
    font-size: 11px;
    color: var(--muted);
  }
  :global(html.no-color-mix) .hero {
    background: var(--field);
  }
  .hidden {
    display: none;
  }
  /* Android: the details take over the whole screen, like a native page. */
  :global(html.android) .backdrop {
    place-items: stretch;
    backdrop-filter: none;
  }
  :global(html.android) .panel {
    width: 100vw;
    height: 100vh;
    max-height: none;
    border: 0;
    border-radius: 0;
  }
  :global(html.android) .hero {
    height: 160px;
    border-radius: 0;
  }
  :global(html.android) .head {
    margin-top: -84px;
    padding: 0 16px;
    gap: 14px;
  }
  :global(html.android) .cover {
    width: 104px;
  }
  :global(html.android) h2 {
    font-size: 22px;
  }
  :global(html.android) .body {
    padding: 14px 16px 24px;
  }
  :global(html.android) .close {
    width: 40px !important;
    height: 40px !important;
  }
  :global(html.android) .version {
    padding: 10px 12px;
  }
  /* The summary's "More" link only matters where it's clamped (below). */
  .more-summary {
    display: none;
    align-self: flex-start;
    margin-top: 2px;
    padding: 2px 0;
    font-size: 13px;
    color: var(--accent);
  }
  .versions-block h3:first-child {
    margin-top: 0;
  }
  /* Desktop: the description and screenshots, then the downloads. */
  .side {
    order: 0;
    display: flex;
    flex-direction: column;
  }
  .main {
    order: 1;
  }
  .side :global(.credit) {
    margin-top: 14px;
  }
  /* Short screens (handhelds, phones in landscape, small windows): downloads
     on the left, description and screenshots on the right, both visible
     without scrolling. */
  @media (max-height: 640px) and (min-width: 700px) {
    /* Each column scrolls on its own, so reading a long description
       doesn't push a short list of downloads out of sight. */
    .body,
    :global(html.android) .body {
      flex: 1;
      flex-direction: row;
      align-items: stretch;
      gap: 22px;
      overflow: hidden;
      padding-top: 12px;
      padding-bottom: 0;
    }
    .main,
    .side {
      min-width: 0;
      min-height: 0;
      overflow-y: auto;
      overscroll-behavior: contain;
      padding-bottom: 22px;
    }
    .main {
      order: 0;
      flex: 1.15;
    }
    .side {
      order: 1;
      flex: 1;
    }
    .shots .shot {
      width: 120px;
    }
  }
  /* Narrow screens (phones in portrait): one column, downloads first, with
     the description clamped to a few lines. */
  @media (max-width: 699px) {
    .main {
      order: 0;
      margin-bottom: 14px;
    }
    .side {
      order: 1;
    }
    .summary.clamp {
      display: -webkit-box;
      -webkit-line-clamp: 3;
      line-clamp: 3;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }
    .more-summary {
      display: block;
    }
  }
  /* Short screens also get a shorter banner and cover (the html.android
     selectors outrank the full-screen Android sizes above). */
  @media (max-height: 640px) {
    .hero,
    :global(html.android) .hero {
      height: 120px;
    }
    .head,
    :global(html.android) .head {
      margin-top: -66px;
    }
    .cover,
    :global(html.android) .cover {
      width: 84px;
    }
    h2,
    :global(html.android) h2 {
      font-size: 20px;
    }
  }
</style>
