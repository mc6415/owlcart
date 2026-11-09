<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
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
  }
  let { title, versions, slug, queued, onDownload, onClose, pathFor = () => undefined }: Props = $props();

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
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 150 }} onclick={onClose}>
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    transition:scale={{ duration: 200, start: 0.96, easing: cubicOut }}
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <div class="hero" class:empty={!hero}>
      {#if hero}
        {#key hero}
          <img class="hero-img" src={hero} alt="" transition:fade={{ duration: 250 }} />
        {/key}
      {/if}
      <button class="icon close" title="Close" onclick={onClose}><Icon name="x" size={16} /></button>
    </div>

    <div class="head">
      <div class="cover">
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
      {#if loading}
        <p class="muted">Loading details…</p>
      {:else if details?.summary}
        <p class="summary">{details.summary}</p>
      {:else}
        <p class="muted">No details found for this game.</p>
      {/if}

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
                <button class="icon" title="Show in folder" onclick={() => api.revealFile(path)}>
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

      {#if details}
        <p class="credit">Game details from IGDB.com</p>
      {/if}
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
    overflow: hidden;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.45);
  }
  .hero {
    position: relative;
    height: 240px;
    flex-shrink: 0;
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
    overflow-y: auto;
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
    margin: 20px 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
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
</style>
