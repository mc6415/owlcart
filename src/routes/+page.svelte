<script lang="ts">
  import "@fontsource-variable/space-grotesk";
  import "@fontsource-variable/jetbrains-mono";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { resetCovers } from "$lib/covers.svelte";
  import { api, fileStem, formatBytes, isExtra, listingSlug, parseName, type FetchStatus, type Entry, type Job, type Listing, type Settings } from "$lib/api";
  import GameList from "$lib/GameList.svelte";
  import GameGrid from "$lib/GameGrid.svelte";
  import DownloadsButton from "$lib/DownloadsButton.svelte";
  import Icon from "$lib/Icon.svelte";
  import Flag from "$lib/Flag.svelte";
  import GameModal from "$lib/GameModal.svelte";
  import ConsolePicker from "$lib/ConsolePicker.svelte";
  import Skeleton from "$lib/Skeleton.svelte";
  import HeroCollage from "$lib/HeroCollage.svelte";
  import { flyToDownloads } from "$lib/motion";
  import { loadAddons, type Addon } from "$lib/addons";
  import { coverFor } from "$lib/covers.svelte";
  import { fade } from "svelte/transition";
  import { loadConsoles, consoleFor, type ConsoleInfo } from "$lib/consoles";

  const DEFAULT_URL = "https://minerva-archive.org/browse/Redump/Microsoft%20-%20Xbox%20360/";

  let consoles = $state<ConsoleInfo[]>([]);

  // A soft brand-ish tint per maker for the console header's glow.
  const TINTS: Record<string, string> = {
    Nintendo: "#e4002b",
    Sega: "#1f6fe5",
    Sony: "#2d5be3",
    Microsoft: "#2fa84f",
    NEC: "#f08a00",
    SNK: "#d8a400",
    Atari: "#e0402e",
    Bandai: "#e5007e",
  };

  function lastListing(): string {
    try {
      return localStorage.getItem("lastListing") || DEFAULT_URL;
    } catch {
      return DEFAULT_URL;
    }
  }
  let listing = $state<Listing | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let engineError = $state<string | null>(null);

  let query = $state("");
  let sortBy = $state<"name" | "size">("name");
  let sortDesc = $state(false);
  let selected = new SvelteSet<string>();
  let regionSel = new SvelteSet<string>();

  type View = "list" | "grid";
  function savedView(): View {
    try {
      return localStorage.getItem("view") === "grid" ? "grid" : "list";
    } catch {
      return "list";
    }
  }
  let view = $state<View>(savedView());

  // "Games only": hide demos, betas, prototypes and other extras.
  function savedGamesOnly(): boolean {
    try {
      return localStorage.getItem("gamesOnly") === "1";
    } catch {
      return false;
    }
  }
  let gamesOnly = $state(savedGamesOnly());
  function setGamesOnly(on: boolean) {
    gamesOnly = on;
    try {
      localStorage.setItem("gamesOnly", on ? "1" : "0");
    } catch {}
  }
  function setView(v: View) {
    view = v;
    try {
      localStorage.setItem("view", v);
    } catch {}
  }

  /** Title of the game shown in the details modal. */
  let detailTitle = $state<string | null>(null);
  let detailVersions = $derived(
    detailTitle && listing
      ? listing.entries.filter((e) => e.kind === "file" && parseName(e.name).title === detailTitle)
      : [],
  );
  /** Where the details modal grows from: the clicked card's cover, or its list row. */
  type Origin = { url: string; rect: DOMRect; card: boolean };
  let detailOrigin = $state<Origin | null>(null);
  function originFor(url: string): Origin | null {
    const el = document.querySelector(`[data-url="${CSS.escape(url)}"]`);
    if (!el) return null;
    const art = el.querySelector(".art");
    return { url, rect: (art ?? el).getBoundingClientRect(), card: !!art };
  }
  const openDetails = (e: Entry) => {
    detailOrigin = originFor(e.url);
    detailTitle = parseName(e.name).title;
  };

  // ---- keyboard shortcuts ----
  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? "⌘" : "Ctrl";
  const SHORTCUTS: [string, string][] = [
    ["/", "Filter games"],
    [`${mod} K`, "Choose a console"],
    ["↑ ↓  or  J K", "Move through games (← → too in grid view)"],
    ["Enter", "Open game details"],
    ["Space", "Select or deselect"],
    [`${mod} A`, "Select all shown"],
    [`${mod} Enter`, "Download selected (or the highlighted game)"],
    ["V", "Switch list / grid"],
    ["G", "Games only (hide demos, betas, prototypes)"],
    ["D", "Downloads"],
    [`${mod} ,`, "Settings"],
    ["Esc", "Close, clear the filter, then the selection"],
    ["?", "This list"],
  ];
  let showKeys = $state(false);
  /** Keyboard-highlighted game, as an index into `filtered` (-1 = none). */
  let cursor = $state(-1);
  let gridCols = $state(1);
  let filterEl = $state<HTMLInputElement>();
  let picker = $state<{ show: () => void }>();
  let downloadsBtn = $state<{ toggle: () => void }>();

  function typingIn(t: EventTarget | null) {
    const el = t as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
  }

  function moveCursor(delta: number) {
    const n = filtered.length;
    if (!n) return;
    cursor = cursor < 0 ? 0 : Math.min(n - 1, Math.max(0, cursor + delta));
  }

  function onGlobalKey(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    const typing = typingIn(e.target);
    // Modals, plus the console picker and downloads popovers (they handle their own keys).
    const overlay =
      !!detailTitle || showSettings || showKeys || !!document.querySelector(".picker .menu, .wrap .pop");

    if (ctrl && e.key.toLowerCase() === "k") {
      e.preventDefault();
      picker?.show();
      return;
    }
    if (ctrl && e.key === ",") {
      e.preventDefault();
      if (!showSettings) openSettings();
      return;
    }
    if (e.key === "Escape") {
      if (showKeys) showKeys = false;
      else if (typing && e.target === filterEl) {
        if (query) query = "";
        else filterEl?.blur();
      } else if (!overlay && !typing) {
        if (query) query = "";
        else if (selected.size) selected.clear();
        else cursor = -1;
      }
      return;
    }
    // In-filter keys: arrows and Enter jump into the results.
    if (typing && e.target === filterEl && (e.key === "ArrowDown" || e.key === "Enter")) {
      e.preventDefault();
      filterEl?.blur();
      cursor = 0;
      return;
    }
    if (typing || overlay || e.altKey) return;

    const item = cursor >= 0 ? filtered[cursor] : undefined;
    const down = view === "grid" ? gridCols : 1;
    switch (ctrl ? `ctrl+${e.key.toLowerCase()}` : e.key) {
      case "/":
        e.preventDefault();
        filterEl?.focus();
        filterEl?.select();
        break;
      case "?":
        showKeys = true;
        break;
      case "v":
      case "V":
        setView(view === "grid" ? "list" : "grid");
        break;
      case "g":
      case "G":
        setGamesOnly(!gamesOnly);
        break;
      case "d":
      case "D":
        downloadsBtn?.toggle();
        break;
      case "ArrowDown":
      case "j":
        e.preventDefault();
        moveCursor(down);
        break;
      case "ArrowUp":
      case "k":
        e.preventDefault();
        moveCursor(-down);
        break;
      case "ArrowRight":
        if (view === "grid") (e.preventDefault(), moveCursor(1));
        break;
      case "ArrowLeft":
        if (view === "grid") (e.preventDefault(), moveCursor(-1));
        break;
      case "Home":
        e.preventDefault();
        if (filtered.length) cursor = 0;
        break;
      case "End":
        e.preventDefault();
        cursor = filtered.length - 1;
        break;
      case "Enter":
        if (item) {
          e.preventDefault();
          if (item.kind === "folder") load(item.url);
          else openDetails(item);
        }
        break;
      case " ":
        if (item?.kind === "file") {
          e.preventDefault();
          if (selected.has(item.url)) selected.delete(item.url);
          else selected.add(item.url);
        }
        break;
      case "ctrl+a":
        e.preventDefault();
        visibleFiles.forEach((f) => selected.add(f.url));
        break;
      case "ctrl+enter":
        e.preventDefault();
        if (selected.size) download(selectedEntries);
        else if (item?.kind === "file") download([item]);
        break;
    }
  }

  // A new list (filter, console, sort) starts without a highlight.
  $effect(() => {
    filtered;
    cursor = -1;
  });

  let jobs = $state<Job[]>([]);
  let settings = $state<Settings | null>(null);
  let showSettings = $state(false);
  let fetchState = $state<FetchStatus | null>(null);

  // "Hide games without artwork": the titles with art on this console, or null
  // when the console has no cover data (then nothing is hidden).
  let hideNoArt = $state(false);
  let artTitles = $state<Set<string> | null>(null);
  async function refreshArtTitles() {
    const slug = listing ? listingSlug(listing.url) : "";
    const titles = slug ? await api.coverTitles(slug).catch(() => null) : null;
    artTitles = titles ? new Set(titles) : null;
  }
  $effect(() => {
    listing; // reload when the console changes
    refreshArtTitles();
  });

  /** Saves the keys, then fetches covers and details for the current console. */
  async function fetchArt() {
    if (!settings || !currentConsole) return;
    try {
      await api.setSettings($state.snapshot(settings));
      const c = currentConsole;
      await api.startFetch([{ name: c.name, group: c.group, folder: c.folder, listing: c.listing, igdb: c.igdb }]);
    } catch (e) {
      error = String(e);
    }
  }

  const win = getCurrentWindow();
  let maximized = $state(false);

  let queued = $derived(
    new Set(jobs.filter((j) => !["done", "failed", "cancelled"].includes(j.state)).map((j) => j.romUrl)),
  );

  /** Regions of each file, keyed by URL. */
  let regionsOf = $derived(
    new Map((listing?.entries ?? []).map((e) => [e.url, e.kind === "file" ? parseName(e.name).regions : []])),
  );

  /** Every region in this listing, most common first. */
  let allRegions = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const rs of regionsOf.values()) for (const r of rs) counts.set(r, (counts.get(r) ?? 0) + 1);
    return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([r]) => r);
  });

  /** Entries matching the text filter (before the region filter). */
  /** Listing entries, minus games without art when that setting is on. */
  let artFiltered = $derived.by(() => {
    if (!listing) return [];
    const art = hideNoArt ? artTitles : null;
    if (!art) return listing.entries;
    return listing.entries.filter((e) => e.kind === "folder" || art.has(parseName(e.name).title));
  });
  let hiddenNoArt = $derived((listing?.entries.length ?? 0) - artFiltered.length);

  /** Minus demos, betas and other extras when "Games only" is on. */
  let gameFiltered = $derived(
    gamesOnly ? artFiltered.filter((e) => e.kind === "folder" || !isExtra(parseName(e.name))) : artFiltered,
  );
  let hiddenExtras = $derived(artFiltered.length - gameFiltered.length);

  let textMatched = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return gameFiltered.filter((e) => {
      const n = e.name.toLowerCase();
      return words.every((w) => n.includes(w));
    });
  });

  /** Region counts within the current text filter. */
  let regionCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of textMatched) for (const r of regionsOf.get(e.url) ?? []) counts.set(r, (counts.get(r) ?? 0) + 1);
    return counts;
  });

  function toggleRegion(r: string) {
    if (regionSel.has(r)) regionSel.delete(r);
    else regionSel.add(r);
  }

  let filtered = $derived.by(() => {
    const list = regionSel.size
      ? textMatched.filter((e) => e.kind === "folder" || (regionsOf.get(e.url) ?? []).some((r) => regionSel.has(r)))
      : [...textMatched];
    const dir = sortDesc ? -1 : 1;
    return list.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
      if (sortBy === "size") return (a.sizeBytes - b.sizeBytes) * dir;
      return a.name.localeCompare(b.name) * dir;
    });
  });

  let visibleFiles = $derived(filtered.filter((e) => e.kind === "file"));
  let folderCount = $derived(filtered.length - visibleFiles.length);
  let allVisibleSelected = $derived(
    visibleFiles.length > 0 && visibleFiles.every((e) => selected.has(e.url)),
  );
  let selectedEntries = $derived(listing?.entries.filter((e) => selected.has(e.url)) ?? []);
  let selectedBytes = $derived(selectedEntries.reduce((s, e) => s + e.sizeBytes, 0));

  let currentConsole = $derived(listing ? consoleFor(listing.url, consoles) : undefined);

  // Games already on disk for this console (file name without extension -> path).
  let have = $state<Record<string, string>>({});
  async function refreshHave() {
    if (!listing) return;
    const archiveFolder = listing.crumbs.at(-1)?.name;
    have = await api.downloaded(currentConsole?.folder, archiveFolder).catch(() => ({}));
  }
  $effect(() => {
    listing;
    currentConsole?.folder;
    refreshHave();
  });
  // Re-check whenever another download finishes.
  let doneCount = $derived(jobs.filter((j) => j.state === "done").length);
  $effect(() => {
    if (doneCount) refreshHave();
  });
  let downloadedUrls = $derived(
    new Set((listing?.entries ?? []).filter((e) => e.kind === "file" && have[fileStem(e.name)]).map((e) => e.url)),
  );
  let tint = $derived(TINTS[currentConsole?.group ?? ""] ?? "var(--accent)");

  // Updates and DLC, loaded in the background for consoles that have them.
  let addonsByGame = $state<Map<string, { updates: Addon[]; dlc: Addon[] }> | null>(null);
  let addonsLoading = $state(false);
  $effect(() => {
    const c = currentConsole;
    const l = listing;
    addonsByGame = null;
    if (!l || !c?.addons?.length) return;
    const games = new Set(l.entries.filter((e) => e.kind === "file").map((e) => parseName(e.name).title));
    let alive = true;
    addonsLoading = true;
    loadAddons(l.url, c.addons, games)
      .then((m) => alive && (addonsByGame = m))
      .catch((e) => console.warn("updates/DLC unavailable", e))
      .finally(() => alive && (addonsLoading = false));
    return () => {
      alive = false;
    };
  });

  // ---- ambient light and spotlight ----

  /** Cover whose colours wash softly over the window (the hovered game). */
  let ambient = $state<string | null>(null);
  let ambientTimer: ReturnType<typeof setTimeout> | undefined;
  function onPointerOver(ev: PointerEvent) {
    const card = (ev.target as Element | null)?.closest?.("[data-url]");
    const img = card?.querySelector<HTMLImageElement>(".art img:not(.glow)");
    if (!img?.complete || !img.src || img.src === ambient) return;
    clearTimeout(ambientTimer);
    ambientTimer = setTimeout(() => (ambient = img.src), 140);
  }

  /** A featured game in the header, picked when a console opens. */
  let spotlight = $state<{ title: string; cover: string; entry: Entry } | null>(null);
  let spotImg = $state<HTMLImageElement>();
  async function pickSpotlight() {
    const l = listing;
    if (!l || !artTitles?.size) return (spotlight = null);
    const files = l.entries.filter((e) => e.kind === "file" && !isExtra(parseName(e.name)));
    const titles = [...new Set(files.map((e) => parseName(e.name).title))].filter((t) => artTitles!.has(t));
    for (let tries = 0; tries < 5 && titles.length; tries++) {
      const title = titles[Math.floor(Math.random() * titles.length)];
      const cover = await coverFor(title, listingSlug(l.url));
      if (l !== listing) return; // console changed meanwhile
      if (cover) {
        const entry = files.find((e) => parseName(e.name).title === title)!;
        spotlight = { title, cover, entry };
        ambient = cover;
        return;
      }
    }
    spotlight = null;
  }
  $effect(() => {
    listing;
    artTitles;
    pickSpotlight();
  });
  function openSpotlight() {
    if (!spotlight) return;
    detailOrigin = spotImg ? { url: spotlight.entry.url, rect: spotImg.getBoundingClientRect(), card: true } : null;
    detailTitle = spotlight.title;
  }

  /** Progress (0..1) of games downloading or unpacking right now, by URL. */
  let progressByUrl = $derived(
    new Map(
      jobs
        .filter((j) => ["downloading", "paused", "moving", "extracting"].includes(j.state) && j.total > 0)
        .map((j) => [j.romUrl, j.completed / j.total]),
    ),
  );

  // Games that just finished get a brief celebration on their card.
  let justDone = new SvelteSet<string>();
  let seenDone: Set<number> | null = null;
  $effect(() => {
    const done = jobs.filter((j) => j.state === "done");
    if (!seenDone) {
      seenDone = new Set(done.map((j) => j.id)); // don't celebrate old ones on startup
      return;
    }
    for (const j of done) {
      if (seenDone.has(j.id)) continue;
      seenDone.add(j.id);
      justDone.add(j.romUrl);
      setTimeout(() => justDone.delete(j.romUrl), 1600);
    }
  });
  let totalBytes = $derived(listing?.entries.reduce((s, e) => s + (e.kind === "file" ? e.sizeBytes : 0), 0) ?? 0);
  let fileCount = $derived(listing?.entries.filter((e) => e.kind === "file").length ?? 0);

  async function load(url: string) {
    loading = true;
    error = null;
    try {
      listing = await api.loadListing(url);
      try {
        localStorage.setItem("lastListing", listing.url);
      } catch {}
      query = "";
      selected.clear();
      regionSel.clear();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function toggleAllVisible() {
    if (allVisibleSelected) visibleFiles.forEach((e) => selected.delete(e.url));
    else visibleFiles.forEach((e) => selected.add(e.url));
  }

  /** Sends covers (or dots, in list view) flying into the downloads button. */
  function flyFrom(entries: Entry[]) {
    entries.slice(0, 5).forEach((e, i) => {
      const modalCover = detailTitle ? document.querySelector(".panel .cover") : null;
      const card = document.querySelector(`[data-url="${CSS.escape(e.url)}"]`);
      const src = modalCover ?? card?.querySelector(".art") ?? card?.querySelector(".act") ?? card;
      if (!src) return;
      const img = src.querySelector<HTMLImageElement>("img:not(.glow)");
      flyToDownloads(src.getBoundingClientRect(), img?.complete ? img.src : null, i * 70);
    });
  }

  async function download(entries: Entry[], subfolder?: string) {
    flyFrom(entries);
    try {
      const folder = listing ? consoleFor(listing.url, consoles)?.folder : undefined;
      await api.enqueue(entries.map((e) => ({ name: e.name, url: e.url, folder, subfolder })));
      entries.forEach((e) => selected.delete(e.url));
    } catch (e) {
      error = String(e);
    }
  }

  function setSort(key: "name" | "size") {
    if (sortBy === key) sortDesc = !sortDesc;
    else {
      sortBy = key;
      sortDesc = key === "size";
    }
  }

  async function browseFetchDir() {
    if (!settings) return;
    const dir = await openDialog({ directory: true, defaultPath: settings.fetchDir || undefined });
    if (typeof dir === "string") settings.fetchDir = dir;
  }

  async function browseDir() {
    if (!settings) return;
    const dir = await openDialog({ directory: true, defaultPath: settings.downloadDir });
    if (typeof dir === "string") settings.downloadDir = dir;
  }

  async function saveSettings() {
    if (!settings) return;
    try {
      const old = await api.getSettings();
      await api.setSettings($state.snapshot(settings));
      if (
        old.steamgriddbKey !== settings.steamgriddbKey.trim() ||
        old.dataUrl !== settings.dataUrl.trim() ||
        old.fetchDir !== settings.fetchDir.trim()
      ) {
        resetCovers();
        refreshArtTitles();
      }
      hideNoArt = settings.hideWithoutArt;
      refreshHave();
      showSettings = false;
    } catch (e) {
      error = String(e);
    }
  }

  async function openSettings() {
    settings = await api.getSettings();
    showSettings = true;
  }

  onMount(() => {
    const unlisten = listen<Job[]>("jobs", (e) => (jobs = e.payload));
    let wasRunning = false;
    const unFetch = listen<FetchStatus>("fetch", (e) => {
      fetchState = e.payload;
      // When a fetch ends, show the new covers and counts.
      if (wasRunning && !e.payload.running) {
        resetCovers();
        refreshArtTitles();
        loadConsoles().then((c) => (consoles = c));
      }
      wasRunning = e.payload.running;
    });
    api.fetchStatus().then((s) => (fetchState = s));
    api.getSettings().then((s) => (hideNoArt = s.hideWithoutArt));
    api.listJobs().then((j) => (jobs = j));
    api.engineStatus().then((s) => (engineError = s.ok ? null : s.error));
    loadConsoles().then((c) => (consoles = c));
    load(lastListing());
    const syncMax = () => win.isMaximized().then((m) => (maximized = m));
    syncMax();
    const unResize = win.onResized(syncMax);
    return () => {
      unlisten.then((f) => f());
      unFetch.then((f) => f());
      unResize.then((f) => f());
    };
  });
</script>

<div class="app" class:maximized>
  <!-- The hovered game's colours, as a soft wash behind everything. -->
  <div class="ambient" aria-hidden="true">
    {#key ambient}
      {#if ambient}<img src={ambient} alt="" transition:fade={{ duration: 900 }} />{/if}
    {/key}
  </div>
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <svg width="22" height="22" viewBox="0 0 64 64" aria-hidden="true">
        <rect x="2" y="2" width="60" height="60" rx="16" fill="var(--accent)" />
        <path d="M14 25 L17 12 L26 19 Q32 17 38 19 L47 12 L50 25 Q53 31 51 39 Q48 52 32 53 Q16 52 13 39 Q11 31 14 25 Z" fill="#2b1660" fill-opacity="0.5" />
        <circle cx="24" cy="32" r="8" fill="#fff" />
        <circle cx="40" cy="32" r="8" fill="#fff" />
        <circle cx="25" cy="33" r="3.4" fill="#2b1660" />
        <circle cx="39" cy="33" r="3.4" fill="#2b1660" />
        <path d="M30 40 L34 40 L32 44 Z" fill="#ffb547" stroke="#ffb547" stroke-width="1.5" stroke-linejoin="round" />
      </svg>
      <span data-tauri-drag-region>Owlcart</span>
    </div>
    <ConsolePicker
      bind:this={picker}
      {consoles}
      current={listing?.url ?? ""}
      fallbackLabel={listing?.crumbs.at(-1)?.name ?? "Choose a console"}
      {loading}
      onPick={load}
    />
    <div class="drag" data-tauri-drag-region></div>
    <DownloadsButton bind:this={downloadsBtn} {jobs} onError={(m) => (error = m)} />
    <button type="button" class="icon" onclick={openSettings} title="Settings ({mod} ,)">
      <Icon name="gear" size={16} />
    </button>
    <div class="winctl">
      <button class="icon" title="Minimise" onclick={() => win.minimize()}><Icon name="min" size={14} /></button>
      <button class="icon" title={maximized ? "Restore" : "Maximise"} onclick={() => win.toggleMaximize()}>
        <Icon name={maximized ? "restore" : "max"} size={13} />
      </button>
      <button class="icon close" title="Close" onclick={() => win.close()}><Icon name="x" size={14} /></button>
    </div>
  </header>

  {#if engineError}
    <div class="banner">
      aria2c didn't start: {engineError}. Install aria2, or put an aria2c binary next to the app.
    </div>
  {/if}
  {#if error}
    <div class="banner">
      <span>{error}</span>
      <button class="icon" title="Dismiss" onclick={() => (error = null)}><Icon name="x" size={14} /></button>
    </div>
  {/if}

  <div class="body">
    <main onpointerover={onPointerOver}>
      {#if listing}
        <section class="hero" style="--tint: {tint}">
          <HeroCollage slug={listingSlug(listing.url)} titles={artTitles ? [...artTitles] : []} />
          <div class="hero-text">
            <div class="eyebrow">{currentConsole?.group ?? listing.crumbs.at(-2)?.name ?? "Minerva"}</div>
            <h1>{currentConsole?.name ?? listing.crumbs.at(-1)?.name ?? "Browse"}</h1>
            <div class="stats">
              {#if fileCount}
                <div><b>{fileCount.toLocaleString()}</b><span>files</span></div>
                <div><b>{formatBytes(totalBytes)}</b><span>total</span></div>
              {/if}
              {#if folderCount && !fileCount}
                <div><b>{folderCount}</b><span>folders</span></div>
              {/if}
            </div>
          </div>
          {#if spotlight}
            {#key spotlight.title}
              <div class="spot" in:fade={{ duration: 400 }}>
                <button class="spot-main" onclick={openSpotlight} title="View details">
                  <img class="spot-cover" bind:this={spotImg} src={spotlight.cover} alt={spotlight.title} />
                  <span class="spot-text">
                    <span class="spot-eyebrow">Spotlight</span>
                    <span class="spot-title">{spotlight.title}</span>
                    <span class="spot-cta">View details →</span>
                  </span>
                </button>
                <button class="icon spot-shuffle" title="Show another" onclick={pickSpotlight}>
                  <Icon name="retry" size={14} />
                </button>
              </div>
            {/key}
          {/if}
        </section>
        <div class="subbar">
          <nav class="crumbs">
            {#if listing.parent}
              <button class="icon" title="Up one level" onclick={() => load(listing!.parent!)}>
                <Icon name="up" size={14} />
              </button>
            {/if}
            {#each listing.crumbs as c, i}
              {#if i > 0}<span class="sep">/</span>{/if}
              {#if i === listing.crumbs.length - 1}
                <span class="here">{c.name}</span>
              {:else}
                <button class="crumb" onclick={() => load(c.url)}>{c.name}</button>
              {/if}
            {/each}
          </nav>
          <button
            class="toggle"
            class:on={gamesOnly}
            aria-pressed={gamesOnly}
            title="Hide demos, betas, prototypes and other extras (G)"
            onclick={() => setGamesOnly(!gamesOnly)}
          >
            {#if gamesOnly}<Icon name="check" size={13} />{/if}
            Games only
          </button>
          <div class="seg" role="group" aria-label="View">
            <button class:on={view === "list"} title="List view (V)" onclick={() => setView("list")}>
              <Icon name="list" size={15} />
            </button>
            <button class:on={view === "grid"} title="Grid view (V)" onclick={() => setView("grid")}>
              <Icon name="grid" size={14} />
            </button>
          </div>
          <label class="filter">
            <Icon name="search" size={14} />
            <input bind:this={filterEl} bind:value={query} spellcheck="false" placeholder="Filter" />
            {#if !query}<kbd class="in-field">/</kbd>{/if}
          </label>
          <button class="icon keys" title="Keyboard shortcuts (?)" onclick={() => (showKeys = true)}>
            <kbd>?</kbd>
          </button>
        </div>

        {#if allRegions.length > 1}
          <div class="regions">
            {#each allRegions as r (r)}
              {@const n = regionCounts.get(r) ?? 0}
              <button
                class="chip"
                class:on={regionSel.has(r)}
                class:empty={!n && !regionSel.has(r)}
                title={r}
                onclick={() => toggleRegion(r)}
              >
                <Flag region={r} height={13} /><span>{n.toLocaleString()}</span>
              </button>
            {/each}
            {#if regionSel.size}
              <button class="text clear" onclick={() => regionSel.clear()}>Clear</button>
            {/if}
          </div>
        {/if}

        <div class="colhead">
          <span class="lead">
            <input
              type="checkbox"
              checked={allVisibleSelected}
              onchange={toggleAllVisible}
              disabled={!visibleFiles.length}
              title="Select all shown"
            />
          </span>
          <button class="sort" onclick={() => setSort("name")}>
            Name {sortBy === "name" ? (sortDesc ? "↓" : "↑") : ""}
            <span class="count">
              {visibleFiles.length.toLocaleString()} files{folderCount ? `, ${folderCount} folders` : ""}{hiddenNoArt
                ? ` · ${hiddenNoArt.toLocaleString()} without art hidden`
                : ""}{hiddenExtras
                ? ` · ${hiddenExtras.toLocaleString()} demos & betas hidden`
                : ""}
            </span>
          </button>
          <button class="sort right" onclick={() => setSort("size")}>
            Size {sortBy === "size" ? (sortDesc ? "↓" : "↑") : ""}
          </button>
          <span></span>
        </div>

        {#if loading}
          <Skeleton {view} />
        {:else if view === "grid"}
          <GameGrid
            progress={progressByUrl}
            {justDone}
            introKey={listing.url}
            downloaded={downloadedUrls}
            bind:cols={gridCols}
            {cursor}
            entries={filtered}
            slug={listingSlug(listing.url)}
            {selected}
            {queued}
            onOpenFolder={load}
            onDownload={(e) => download([e])}
            onOpen={(e) => {
              cursor = filtered.indexOf(e);
              openDetails(e);
            }}
          />
        {:else}
          <GameList
            progress={progressByUrl}
            {justDone}
            introKey={listing.url}
            downloaded={downloadedUrls}
            {cursor}
            entries={filtered}
            {selected}
            {queued}
            onOpenFolder={load}
            onDownload={(e) => download([e])}
            onOpen={(e) => {
              cursor = filtered.indexOf(e);
              openDetails(e);
            }}
          />
        {/if}

        {#if selected.size}
          <div class="selbar">
            <span><b>{selected.size}</b> selected</span>
            <span class="mono">{formatBytes(selectedBytes)}</span>
            <div class="grow"></div>
            <button class="text" onclick={() => selected.clear()}>Clear</button>
            <button class="primary" onclick={() => download(selectedEntries)} disabled={!!engineError}>
              <Icon name="download" size={15} /> Download <kbd class="on-accent">{mod}↵</kbd>
            </button>
          </div>
        {/if}
      {:else}
        {#if loading}
          <Skeleton {view} />
        {:else}
          <p class="placeholder">Choose a console to start.</p>
        {/if}
      {/if}
    </main>

  </div>
</div>

<svelte:window onkeydown={onGlobalKey} />

{#if showKeys}
  <div class="modal-bg" role="presentation" onclick={() => (showKeys = false)}>
    <div class="modal keys-modal" role="dialog" tabindex="-1" aria-label="Keyboard shortcuts" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>Keyboard shortcuts</h2>
      <dl>
        {#each SHORTCUTS as [k, what]}
          <dt>{#each k.split("  or  ") as part, i}{#if i}<span class="or">or</span>{/if}{#each part.split(" ") as key}<kbd>{key}</kbd>{/each}{/each}</dt>
          <dd>{what}</dd>
        {/each}
      </dl>
    </div>
  </div>
{/if}

{#if detailTitle && listing}
  <GameModal
    title={detailTitle}
    versions={detailVersions}
    slug={listingSlug(listing.url)}
    {queued}
    onDownload={(e) => download([e])}
    onClose={() => (detailTitle = null)}
    pathFor={(e) => have[fileStem(e.name)]}
    origin={detailOrigin}
    addonsEnabled={!!currentConsole?.addons?.length}
    addons={addonsByGame?.get(detailTitle)}
    {addonsLoading}
    onDownloadAddon={(a) => download([a.entry], a.kind === "update" ? "Updates" : "DLC")}
    findOrigin={() => (detailOrigin ? originFor(detailOrigin.url) : null)}
  />
{/if}

{#if showSettings && settings}
  <div class="modal-bg" role="presentation" onclick={() => (showSettings = false)}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (showSettings = false)}>
      <h2>Settings</h2>
      <label>
        <span>Download folder</span>
        <div class="row">
          <input bind:value={settings.downloadDir} spellcheck="false" />
          <button type="button" class="secondary" onclick={browseDir}>Browse</button>
        </div>
      </label>
      <label class="check-row">
        <input type="checkbox" bind:checked={settings.perConsoleFolders} />
        <div>
          Sort into a folder per console
          <small>Uses ES-DE folder names, e.g. <code>{settings.downloadDir}/psx/</code></small>
        </div>
      </label>
      <label class="check-row">
        <input type="checkbox" bind:checked={settings.extractArchives} />
        <div>
          Extract .zip files after downloading
          <small>Multi-file games (like a .cue with .bin tracks) get their own folder.</small>
        </div>
      </label>
      <label class="check-row" class:disabled={!settings.extractArchives}>
        <input type="checkbox" bind:checked={settings.deleteArchives} disabled={!settings.extractArchives} />
        <div>
          Delete the .zip once extracted
          <small>Kept automatically if extraction fails.</small>
        </div>
      </label>
      <label>
        <span>Simultaneous downloads</span>
        <input class="num" type="number" min="1" max="10" bind:value={settings.maxActive} />
        <small>Games from the same console share a torrent, so they download one at a time.</small>
      </label>
      <h3 class="section">Game data</h3>
      <p class="hint">
        Covers and details come from the published game data. You can also fetch them yourself with free
        API keys; results are saved on this computer and used first.
      </p>
      <label>
        <span>SteamGridDB API key</span>
        <input
          type="password"
          bind:value={settings.steamgriddbKey}
          spellcheck="false"
          placeholder="For cover art"
        />
        <small>
          Get a free key from your
          <button
            type="button"
            class="link"
            onclick={() => openUrl("https://www.steamgriddb.com/profile/preferences/api")}
          >SteamGridDB API preferences</button>.
        </small>
      </label>
      <div class="pair">
        <label>
          <span>IGDB client ID</span>
          <input bind:value={settings.igdbClientId} spellcheck="false" placeholder="For game details" />
        </label>
        <label>
          <span>IGDB client secret</span>
          <input type="password" bind:value={settings.igdbClientSecret} spellcheck="false" />
        </label>
      </div>
      <small class="hint">
        Create a <em>Confidential</em> app at
        <button type="button" class="link" onclick={() => openUrl("https://dev.twitch.tv/console/apps")}>dev.twitch.tv</button>
        (redirect URL <code>http://localhost</code>). Keys are stored in this app's settings file.
      </small>

      <label>
        <span>Save fetched data to</span>
        <div class="row">
          <input bind:value={settings.fetchDir} spellcheck="false" placeholder="This app's own data folder" />
          <button type="button" class="secondary" onclick={browseFetchDir}>Browse</button>
        </div>
        <small>To publish what you fetch, choose your project's <code>data/</code> folder and commit it.</small>
      </label>
      <label class="check-row">
        <input type="checkbox" bind:checked={settings.hideWithoutArt} />
        <div>
          Hide games without artwork
          <small>Only on consoles that have cover data; others show everything.</small>
        </div>
      </label>

      <div class="fetch">
        {#if fetchState?.running}
          <div class="fetch-row">
            <span>
              <b>{fetchState.console}</b>
              {#if fetchState.total}· {fetchState.done.toLocaleString()} / {fetchState.total.toLocaleString()}{:else}· starting…{/if}
            </span>
            <button type="button" class="secondary small" onclick={() => api.cancelFetch()}>Stop</button>
          </div>
          <div class="fetch-bar">
            <div style="width: {fetchState.total ? (fetchState.done / fetchState.total) * 100 : 0}%"></div>
          </div>
          <small>
            Found {fetchState.covers.toLocaleString()} covers and {fetchState.details.toLocaleString()} sets of details so far.
            You can close this; it carries on in the background.
          </small>
        {:else}
          <div class="fetch-row">
            <button
              type="button"
              class="primary"
              disabled={!currentConsole || (!settings.steamgriddbKey && !(settings.igdbClientId && settings.igdbClientSecret))}
              onclick={fetchArt}
            >
              Fetch art &amp; details{currentConsole ? ` for ${currentConsole.name}` : ""}
            </button>
          </div>
          {#if !currentConsole}
            <small>Open a console first.</small>
          {:else}
            <small>Only games without data yet are looked up, so running it again is quick.</small>
          {/if}
          {#if fetchState?.message}<small class="note">{fetchState.message}</small>{/if}
        {/if}
      </div>

      <label>
        <span>Game data source</span>
        <input
          bind:value={settings.dataUrl}
          spellcheck="false"
          placeholder="URL or folder with index.json"
        />
        <small>Where the published covers and game details live (the data/ folder).</small>
      </label>

      <div class="row end">
        <button class="secondary" onclick={() => (showSettings = false)}>Cancel</button>
        <button class="primary" onclick={saveSettings}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  :global(:root) {
    --bg: #f7f6fa;
    --panel: #fbfafd;
    --text: #1f1b2b;
    --muted: #716b82;
    --line: #e4e0ec;
    --stripe: #f3f1f7;
    --hover: #ebe6f5;
    --sel: #e3d9fb;
    --chip: #ebe7f2;
    --field: #efecf4;
    --frame: #d8d2e4;
    --accent: #7b4fd6;
    --on-accent: #ffffff;
    --ok: #2f8f57;
    --danger: #c23b4a;
    --mono: "JetBrains Mono Variable", ui-monospace, "Cascadia Mono", monospace;
    --display: "Space Grotesk Variable", system-ui, sans-serif;
    --grad: linear-gradient(90deg, var(--accent), #e46bd0);
    color-scheme: light dark;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #14121a;
      --panel: #19161f;
      --text: #e7e3ef;
      --muted: #8e879e;
      --line: #2a2533;
      --stripe: #17151d;
      --hover: #231f2c;
      --sel: #2e2347;
      --chip: #262130;
      --field: #221e2b;
      --frame: #322c3d;
      --accent: #a585f2;
      --on-accent: #17121f;
      --ok: #6cc28f;
      --danger: #ec7584;
    }
  }
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: transparent;
    color: var(--text);
    font: 13px/1.4 system-ui, "Segoe UI", sans-serif;
    user-select: none;
  }
  :global(button) {
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  :global(button:disabled) {
    opacity: 0.45;
    cursor: default;
  }
  :global(button.icon) {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--muted);
  }
  :global(button.icon:hover) {
    background: var(--hover);
    color: var(--text);
  }
  :global(button.text) {
    border: 0;
    background: none;
    color: var(--muted);
    padding: 4px 6px;
  }
  :global(button.text:hover) {
    color: var(--text);
  }
  :global(button.primary),
  :global(button.secondary) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 14px;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 500;
  }
  :global(button.secondary) {
    background: none;
    border-color: var(--line);
    color: var(--text);
  }
  :global(input:not([type="checkbox"])) {
    font: inherit;
    color: inherit;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 0 10px;
    height: 30px;
    box-sizing: border-box;
    outline: none;
    user-select: text;
  }
  :global(input:not([type="checkbox"]):focus) {
    border-color: var(--accent);
  }
  :global(kbd) {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    box-sizing: border-box;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--panel);
    color: var(--muted);
    font: 600 10.5px/1 var(--mono);
    white-space: nowrap;
  }
  :global(kbd.on-accent) {
    background: transparent;
    border-color: color-mix(in srgb, var(--on-accent) 40%, transparent);
    color: inherit;
    opacity: 0.8;
  }
  :global(input[type="checkbox"]) {
    accent-color: var(--accent);
    margin: 0;
  }
  .app {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
    background:
      radial-gradient(900px 500px at 100% 0%, color-mix(in srgb, var(--accent) 9%, transparent), transparent 70%),
      radial-gradient(700px 500px at 0% 100%, color-mix(in srgb, #e46bd0 6%, transparent), transparent 70%),
      var(--bg);
    border: 1px solid var(--frame);
    border-radius: 10px;
    overflow: hidden;
  }
  /* Fine film grain over everything, so large surfaces aren't flat. */
  .app::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 100;
    pointer-events: none;
    opacity: 0.05;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
    mix-blend-mode: overlay;
  }
  .app.maximized {
    border: 0;
    border-radius: 0;
  }
  .titlebar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    flex-shrink: 0;
    padding: 0 8px 0 14px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 170px;
    font: 700 15px var(--display);
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
  .winctl {
    display: flex;
    gap: 2px;
    margin-left: 6px;
    padding-left: 8px;
    border-left: 1px solid var(--line);
  }
  .winctl .icon {
    width: 30px;
    height: 28px;
    border-radius: 6px;
  }
  .winctl .close:hover {
    background: var(--danger);
    color: #fff;
  }
  .banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 10px 6px 14px;
    background: color-mix(in srgb, var(--danger) 12%, var(--bg));
    color: var(--danger);
    border-bottom: 1px solid var(--line);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  /* The hovered game's colours, washed across the window. A tiny image scaled
     up is far cheaper to blur than a full-size one. */
  .ambient {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    pointer-events: none;
  }
  .ambient img {
    position: absolute;
    left: 50%;
    top: 55%;
    width: 64px;
    height: 96px;
    object-fit: cover;
    transform: translate(-50%, -50%) scale(24, 14);
    filter: blur(3px) saturate(1.7);
    opacity: 0.2;
  }
  .hero {
    position: relative;
    isolation: isolate;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 150px;
    padding: 24px 24px 22px;
    overflow: hidden;
    flex-shrink: 0;
    border-bottom: 1px solid var(--line);
  }
  /* Keeps the console name readable over the collage. */
  .hero::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    background: linear-gradient(90deg, var(--bg) 18%, color-mix(in srgb, var(--bg) 40%, transparent) 55%, color-mix(in srgb, var(--bg) 70%, transparent));
    pointer-events: none;
  }
  .hero::before {
    content: "";
    position: absolute;
    inset: -40% -10% auto auto;
    width: 70%;
    height: 220%;
    z-index: -3;
    background:
      radial-gradient(closest-side, color-mix(in srgb, var(--tint) 34%, transparent), transparent),
      radial-gradient(closest-side at 30% 60%, color-mix(in srgb, var(--accent) 26%, transparent), transparent);
    filter: blur(20px);
    transition: background 0.4s;
  }
  .eyebrow {
    font: 600 11px var(--display);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--tint) 55%, var(--text));
  }
  h1 {
    margin: 4px 0 0;
    font: 700 40px/1.02 var(--display);
    letter-spacing: -0.035em;
    background: linear-gradient(95deg, var(--text) 30%, color-mix(in srgb, var(--tint) 65%, var(--text)));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .stats {
    display: flex;
    gap: 22px;
    margin-top: 12px;
  }
  .stats div {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .stats b {
    font: 600 15px var(--mono);
    letter-spacing: -0.02em;
  }
  .spot {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    gap: 4px;
  }
  .spot-main {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 18px 8px 8px;
    border: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
    border-radius: 14px;
    background: color-mix(in srgb, var(--panel) 55%, transparent);
    backdrop-filter: blur(12px);
    text-align: left;
    transition: border-color 0.2s, background 0.2s;
  }
  .spot-main:hover {
    border-color: color-mix(in srgb, var(--tint) 60%, transparent);
    background: color-mix(in srgb, var(--panel) 75%, transparent);
  }
  .spot-cover {
    width: 72px;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 8px;
    transform: rotate(-4deg);
    box-shadow: 0 12px 28px rgb(0 0 0 / 0.5);
    transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .spot-main:hover .spot-cover {
    transform: rotate(0deg) scale(1.06);
  }
  .spot-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 220px;
  }
  .spot-eyebrow {
    font: 600 10px var(--display);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--tint) 60%, var(--text));
  }
  .spot-title {
    font: 700 16px/1.2 var(--display);
    letter-spacing: -0.01em;
    color: var(--text);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .spot-cta {
    font-size: 12px;
    color: var(--muted);
    transition: color 0.2s;
  }
  .spot-main:hover .spot-cta {
    color: var(--text);
  }
  .spot-shuffle {
    opacity: 0.6;
  }
  .spot:hover .spot-shuffle {
    opacity: 1;
  }
  .stats span {
    font-size: 11px;
    color: var(--muted);
  }
  .subbar {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 41px;
    padding: 0 10px 0 8px;
    border-bottom: 1px solid var(--line);
    box-sizing: border-box;
  }
  .crumbs {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
  }
  .crumb {
    border: 0;
    background: none;
    padding: 2px 4px;
    border-radius: 3px;
    color: var(--muted);
  }
  .crumb:hover {
    color: var(--text);
    background: var(--hover);
  }
  .here {
    padding: 2px 4px;
    font-weight: 600;
  }
  .sep {
    color: var(--line);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 11px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: none;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }
  .toggle:hover {
    color: var(--text);
    border-color: var(--accent);
  }
  .toggle.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .seg {
    display: flex;
    padding: 2px;
    gap: 2px;
    border-radius: 7px;
    background: var(--field);
  }
  .seg button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .seg button:hover {
    color: var(--text);
  }
  .seg button.on {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.2);
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    position: relative;
  }
  .filter .in-field {
    position: absolute;
    right: 7px;
    pointer-events: none;
  }
  .keys kbd {
    cursor: pointer;
  }
  .keys-modal {
    width: min(460px, 90vw);
  }
  .keys-modal dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 9px 18px;
    margin: 0;
    align-items: center;
  }
  .keys-modal dt {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .keys-modal dd {
    margin: 0;
    color: var(--muted);
  }
  .keys-modal .or {
    margin: 0 4px;
    font-size: 11px;
    color: var(--muted);
  }
  .filter input {
    width: 220px;
  }
  .regions {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
    scrollbar-width: none;
    flex-shrink: 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 10px 0 8px;
    border: 1px solid var(--line);
    border-radius: 13px;
    background: none;
    color: var(--text);
    font-size: 12px;
    white-space: nowrap;
    transition: background 0.15s, border-color 0.15s, color 0.15s, opacity 0.15s;
  }
  .chip span {
    font: 11px var(--mono);
    color: var(--muted);
  }
  .chip:hover {
    border-color: var(--accent);
  }
  .chip.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .chip.on span {
    color: inherit;
    opacity: 0.75;
  }
  .chip.empty {
    opacity: 0.4;
  }
  .clear {
    font-size: 12px;
    white-space: nowrap;
  }
  .colhead {
    display: grid;
    grid-template-columns: 30px 1fr 84px 40px;
    gap: 8px;
    align-items: center;
    height: 28px;
    padding: 0 10px 0 12px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
  }
  .lead {
    display: grid;
    place-items: center;
  }
  .sort {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: inherit;
  }
  .sort:hover {
    color: var(--text);
  }
  .sort.right {
    text-align: right;
  }
  .count {
    margin-left: 8px;
    opacity: 0.7;
  }
  .selbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px 8px 14px;
    border-top: 1px solid var(--line);
    background: var(--panel);
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
  }
  .grow {
    flex: 1;
  }
  .placeholder {
    color: var(--muted);
    padding: 16px 14px;
    margin: 0;
  }
  .modal-bg {
    position: fixed;
    inset: 0;
    background: rgb(10 6 20 / 0.5);
    display: grid;
    place-items: center;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 20px 22px;
    width: min(500px, 90vw);
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
  }
  .modal h2 {
    margin: 0;
    font-size: 14px;
  }
  .modal label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .modal label > span {
    color: var(--muted);
    font-size: 12px;
  }
  .modal small {
    color: var(--muted);
  }
  .modal .row {
    display: flex;
    gap: 8px;
  }
  .modal .row input {
    flex: 1;
    font-family: var(--mono);
    font-size: 12px;
  }
  .modal {
    max-height: 86vh;
    overflow-y: auto;
  }
  .modal h3.section {
    margin: 6px 0 -6px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font: 700 13px var(--display);
  }
  .modal .hint {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .modal .hint code {
    font: 11px var(--mono);
  }
  .modal .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .fetch {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: 8px;
    background: var(--field);
  }
  .fetch small {
    color: var(--muted);
  }
  .fetch .note {
    color: var(--text);
  }
  .fetch-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .fetch-bar {
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    overflow: hidden;
  }
  .fetch-bar div {
    height: 100%;
    background: var(--grad);
    transition: width 0.4s;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: inherit;
  }
  .link:hover {
    text-decoration: underline;
  }
  .modal .check-row {
    flex-direction: row;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
  }
  .check-row input {
    margin-top: 3px;
  }
  .check-row.disabled {
    opacity: 0.5;
  }
  .check-row div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .check-row code {
    font: 11px var(--mono);
    word-break: break-all;
  }
  .num {
    width: 80px;
  }
  .row.end {
    justify-content: flex-end;
  }
</style>
