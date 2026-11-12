<script lang="ts">
  import "@fontsource-variable/space-grotesk";
  import "@fontsource-variable/jetbrains-mono";
  import { onMount, untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { resetCovers } from "$lib/covers.svelte";
  import { api, fileStem, formatBytes, gameDetails, igdbImage, isExtra, listingSlug, parseName, type FetchStatus, type GameDetails, type Entry, type Job, type Listing, type Settings } from "$lib/api";
  import GameList from "$lib/GameList.svelte";
  import GameGrid from "$lib/GameGrid.svelte";
  import DownloadsButton from "$lib/DownloadsButton.svelte";
  import Icon from "$lib/Icon.svelte";
  import Flag from "$lib/Flag.svelte";
  import PadButton from "$lib/PadButton.svelte";
  import GameModal from "$lib/GameModal.svelte";
  import ConsolePicker from "$lib/ConsolePicker.svelte";
  import Skeleton from "$lib/Skeleton.svelte";
  import HeroCollage from "$lib/HeroCollage.svelte";
  import { ratingTier } from "$lib/rating";
  import { feel, setFeedback } from "$lib/feedback";
  import { flyToDownloads } from "$lib/motion";
  import { isAndroid } from "$lib/platform";
  import { loadAddons, type Addon } from "$lib/addons";
  import { coverFor } from "$lib/covers.svelte";
  import { fade, fly, slide } from "svelte/transition";
  import { loadConsoles, consoleFor, type ConsoleInfo } from "$lib/consoles";
  import { compareVersions, preferredRegions } from "$lib/versions";

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
  /** Consoles with a colour of their own, by download folder (else the maker's). */
  const CONSOLE_TINTS: Record<string, string> = {
    gb: "#6f9a2c",
    gbc: "#8b47d6",
    gba: "#4b3fc4",
    nds: "#7d8796",
    virtualboy: "#d11f2f",
    snes: "#6c4fd0",
    n64: "#1f9a4e",
    gc: "#5c46b8",
    wii: "#3aa6dd",
    dreamcast: "#f0641e",
    saturn: "#4a5bd4",
    psp: "#3d4250",
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
  type SortKey = "name" | "rating" | "size";
  let sortBy = $state<SortKey>("name");
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
  /** Every version of the game in the details modal, best first. */
  let detailVersions = $derived(detailTitle ? versionsOf(detailTitle) : []);
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

  // Android: scrolling down a list slides everything above the Name/Size header
  // (top bar, console banner, filters) out of the way; it returns at the top.
  let appEl = $state<HTMLElement>();
  let colheadEl = $state<HTMLElement>();
  let chromeHidden = $state(false);
  let chromeHeight = $state(0);
  let lastScroll = { el: null as EventTarget | null, top: 0 };
  let chromeLockUntil = 0;
  let chromeTall = $state(false);
  // Dual-screen devices and short windows (the AYN Thor's top screen is about
  // 830x470): a slim header, regions and sort in the toolbar, and title-only
  // captions, so more of the screen is games. The second screen shows the rest.
  let dualScreen = $state(false);
  let winH = $state(800);
  let compact = $derived(dualScreen || winH < 560);
  /** The toolbar panel shown under it in compact mode. */
  let panel = $state<"" | "regions" | "sort">("");
  const togglePanel = (p: "regions" | "sort") => (panel = panel === p ? "" : p);
  const SORTS: [string, string][] = [
    ["name:0", "Name A–Z"],
    ["name:1", "Name Z–A"],
    ["rating:1", "Top rated"],
    ["size:1", "Largest"],
    ["size:0", "Smallest"],
  ];
  let sortValue = $derived(`${sortBy}:${sortDesc ? 1 : 0}`);
  function pickSort(v: string) {
    const [key, desc] = v.split(":");
    sortBy = key as SortKey;
    sortDesc = desc === "1";
  }
  function onListScroll(e: Event) {
    const el = e.target as HTMLElement;
    if (!isAndroid || !el.classList?.contains("viewport")) return;
    const top = el.scrollTop;
    const prev = lastScroll.el === el ? lastScroll.top : top;
    lastScroll = { el, top };
    // Ignore the scroll that the layout change itself causes.
    if (performance.now() < chromeLockUntil) return;
    // Hides on the way down, and only comes back at the very top: bringing it
    // back on any upward scroll felt jumpy, especially when moving up through
    // the list with a controller.
    // Only hide when there's still list left to scroll once it's gone, so a
    // short list that barely overflows just scrolls with the header in place.
    // (The list's end has extra space only while the header shows; see below.)
    const pad = chromeTall && !chromeHidden ? chromeHeight : 0;
    const room = el.scrollHeight - pad - el.clientHeight;
    let hide = chromeHidden;
    if (top < 24) hide = false;
    else if (top - prev > 6 && top + 24 < room) hide = true;
    if (hide === chromeHidden) return;
    if (hide && appEl && colheadEl) {
      chromeHeight = colheadEl.getBoundingClientRect().top - appEl.getBoundingClientRect().top;
    }
    chromeHidden = hide;
    chromeLockUntil = performance.now() + 300;
    // The page slides with a transform, so nothing is laid out again while
    // it moves. It stays taller than the screen by the hidden amount, with the
    // extra part below the screen edge, so hiding and showing never resize the
    // list. While the header shows, the list gets that much space at its end
    // so its last games can still scroll into view.
    chromeTall = true;
  }
  // A new console starts with everything shown.
  $effect(() => {
    void listing;
    chromeHidden = false;
    panel = "";
  });
  // Measure the header as soon as a list is shown (and again if a banner
  // appears), so even the first slide doesn't resize anything.
  $effect(() => {
    void [listing, error, engineError, compact, panel];
    const id = requestAnimationFrame(measureChrome);
    return () => cancelAnimationFrame(id);
  });
  // Also called once a toolbar panel has finished sliding open or shut.
  function measureChrome() {
    if (!isAndroid || !colheadEl || !appEl || chromeHidden) return;
    chromeHeight = colheadEl.getBoundingClientRect().top - appEl.getBoundingClientRect().top;
    chromeTall = true;
  }
  let downloadsBtn = $state<{ toggle: () => void }>();

  function typingIn(t: EventTarget | null) {
    const el = t as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
  }

  function moveCursor(delta: number) {
    const n = filtered.length;
    if (!n) return;
    const next = cursor < 0 ? 0 : Math.min(n - 1, Math.max(0, cursor + delta));
    if (next !== cursor) feel("move");
    cursor = next;
  }

  // ---- switching consoles with L2 / R2 ----
  /** Consoles in the picker's order (grouped by maker). */
  let consoleOrder = $derived(
    [...new Set(consoles.map((c) => c.group))].flatMap((g) => consoles.filter((c) => c.group === g)),
  );
  /** Which way the last switch went, so the new console slides in from that side. */
  let slideDir = $state(0);
  let slideTimer: ReturnType<typeof setTimeout> | undefined;
  function switchConsole(dir: 1 | -1) {
    const list = consoleOrder;
    if (!list.length || loading) return;
    const i = currentConsole ? list.indexOf(currentConsole) : -1;
    const next = list[(i + dir + list.length) % list.length];
    feel("switch");
    slideDir = dir;
    clearTimeout(slideTimer);
    slideTimer = setTimeout(() => (slideDir = 0), 1500);
    load(next.listing);
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

  // ---- controller buttons (Android; sent by MainActivity.kt) ----
  const PAD_BUTTONS: [string, string][] = [
    ["D-pad", "Move through games"],
    ["A", "Open game details"],
    ["B", "Back"],
    ["X", "Download the highlighted game (or the selected ones)"],
    ["Y", "Search"],
    ["L1 R1", "Page up / down"],
    ["L2 R2", "Previous / next console"],
    ["Start", "Downloads"],
    ["Select", "This list"],
    ["L3", "Switch list / grid"],
    ["R3", "Games only (hide demos, betas, prototypes)"],
  ];
  /** A newer Owlcart release, shown in the title bar (see check_update). */
  let update = $state<{ version: string; url: string } | null>(null);
  /** A controller is connected (Android): show its buttons where keyboard hints would be. */
  let padConnected = $state(false);
  function onPad(button: string) {
    padConnected = true;
    if (button === "select") {
      showKeys = !showKeys;
      return;
    }
    if (button === "start") {
      downloadsBtn?.toggle();
      return;
    }
    // Other buttons only act on the game list, not over windows and menus.
    const overlay =
      !!detailTitle || showSettings || showKeys || !!document.querySelector(".picker .menu, .wrap .pop");
    if (overlay || typingIn(document.activeElement)) return;
    const item = cursor >= 0 ? filtered[cursor] : undefined;
    const page = view === "grid" ? gridCols * 2 : 8;
    switch (button) {
      case "x":
        if (selected.size) download(selectedEntries);
        else if (item?.kind === "file") download([item]);
        break;
      case "y":
        chromeHidden = false;
        filterEl?.focus();
        filterEl?.select();
        break;
      case "l1":
        moveCursor(-page);
        break;
      case "r1":
        moveCursor(page);
        break;
      case "l2":
        switchConsole(-1);
        break;
      case "r2":
        switchConsole(1);
        break;
      case "l3":
        setView(view === "grid" ? "list" : "grid");
        break;
      case "r3":
        setGamesOnly(!gamesOnly);
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
  /** Each matched title on this console: [IGDB id] or [IGDB id, rating, review count]. */
  let igdbGames = $state<Record<string, number[]>>({});
  /** IGDB [rating, review count] of each rated title. */
  let ratings = $derived(
    Object.fromEntries(
      Object.entries(igdbGames).flatMap(([t, g]) => (g.length >= 3 ? [[t, [g[1], g[2]] as [number, number]]] : [])),
    ),
  );
  /**
   * Rating used for sorting: blended with the console's average as if every
   * game had 50 more average reviews, so a game rated 98 from a handful of
   * reviews doesn't outrank one rated 95 from hundreds.
   */
  let sortScores = $derived.by(() => {
    const all = Object.values(ratings);
    const mean = all.length ? all.reduce((s, [r]) => s + r, 0) / all.length : 0;
    const PRIOR = 50;
    return new Map(Object.entries(ratings).map(([t, [r, n]]) => [t, (r * n + mean * PRIOR) / (n + PRIOR)]));
  });
  // Kept for the current console and the ones either side (see prefetchNeighbours).
  type ArtData = { titles: string[] | null; games: Record<string, number[]> };
  const artCache = new Map<string, ArtData>();
  async function artData(slug: string): Promise<ArtData> {
    const [titles, games] = await Promise.all([
      api.coverTitles(slug).catch(() => null),
      api.games(slug).catch(() => ({})),
    ]);
    const d = { titles, games };
    artCache.set(slug, d);
    return d;
  }
  /** Loads which games have art, and their IGDB ids and ratings, for the
   *  current console. `cached`: use what's already in memory, if anything. */
  async function refreshArtTitles(cached = false) {
    const slug = listing ? listingSlug(listing.url) : "";
    const d = !slug ? { titles: null, games: {} } : (cached && artCache.get(slug)) || (await artData(slug));
    if (slug !== (listing ? listingSlug(listing.url) : "")) return; // moved on meanwhile
    artTitles = d.titles ? new Set(d.titles) : null;
    igdbGames = d.games;
  }
  $effect(() => {
    listing; // reload when the console changes
    refreshArtTitles(true);
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

  /** Each file's title and regions, keyed by URL. */
  let titleOf = $derived(
    new Map((listing?.entries ?? []).filter((e) => e.kind === "file").map((e) => [e.url, parseName(e.name).title])),
  );
  let regionsOf = $derived(
    new Map((listing?.entries ?? []).map((e) => [e.url, e.kind === "file" ? parseName(e.name).regions : []])),
  );

  // ---- one card per game ----
  // Files with the same title (regions, revisions, demos) show as one game,
  // and so do titles matched to the same IGDB game ("Biohazard 2" and
  // "Resident Evil 2"). Its card stands for the best version (see
  // versions.ts); the details window lists them all.

  /** The game a title belongs to: its IGDB id if matched, else the title itself. */
  function gameKey(title: string): string {
    const id = igdbGames[title]?.[0];
    return id ? `#${id}` : `t:${title}`;
  }
  /** Each file's game, keyed by URL. */
  let gameOf = $derived(new Map([...titleOf].map(([url, t]) => [url, gameKey(t)])));
  /** Number of different games among some entries. */
  const gamesIn = (entries: Entry[]) => new Set(entries.filter((e) => e.kind === "file").map((e) => gameOf.get(e.url))).size;

  /** Regions to prefer: the ones picked in the region filter, then the device's. */
  let prefs = $derived(preferredRegions(regionSel));
  /** Every version of each game on this listing, best first. */
  let versionsByGame = $derived.by(() => {
    const m = new Map<string, Entry[]>();
    for (const e of listing?.entries ?? []) {
      if (e.kind !== "file") continue;
      const g = gameOf.get(e.url)!;
      const list = m.get(g);
      if (list) list.push(e);
      else m.set(g, [e]);
    }
    for (const list of m.values()) if (list.length > 1) list.sort((a, b) => compareVersions(a, b, prefs));
    return m;
  });
  /** Every version of the game a title belongs to, best first. */
  function versionsOf(title: string): Entry[] {
    return versionsByGame.get(gameKey(title)) ?? [];
  }

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
  let hiddenNoArt = $derived(gamesIn(listing?.entries ?? []) - gamesIn(artFiltered));

  /** Minus demos, betas and other extras when "Games only" is on. */
  let gameFiltered = $derived(
    gamesOnly ? artFiltered.filter((e) => e.kind === "folder" || !isExtra(parseName(e.name))) : artFiltered,
  );
  let hiddenExtras = $derived(gamesIn(artFiltered) - gamesIn(gameFiltered));

  let textMatched = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return gameFiltered.filter((e) => {
      const n = e.name.toLowerCase();
      return words.every((w) => n.includes(w));
    });
  });

  /** Games in each region within the current text filter. */
  let regionCounts = $derived.by(() => {
    const games = new Map<string, Set<string>>();
    for (const e of textMatched) {
      for (const r of regionsOf.get(e.url) ?? []) {
        const set = games.get(r) ?? new Set<string>();
        set.add(gameOf.get(e.url)!);
        games.set(r, set);
      }
    }
    return new Map([...games].map(([r, set]) => [r, set.size]));
  });

  function toggleRegion(r: string) {
    if (regionSel.has(r)) regionSel.delete(r);
    else regionSel.add(r);
    saveRegions();
  }
  function clearRegions() {
    regionSel.clear();
    saveRegions();
  }

  // The region filter starts on the major regions, so a console isn't full of
  // releases from one country only. Changing it (or clearing it to show
  // everything) is remembered for every console.
  const DEFAULT_REGIONS = ["USA", "Europe", "Japan", "World"];
  function savedRegions(): string[] {
    try {
      const v = localStorage.getItem("regions");
      if (v) return JSON.parse(v);
    } catch {}
    return DEFAULT_REGIONS;
  }
  function saveRegions() {
    try {
      localStorage.setItem("regions", JSON.stringify([...regionSel]));
    } catch {}
  }
  /** Selects the saved regions this console has (none: show everything). */
  function applyRegions() {
    regionSel.clear();
    const have = new Set(allRegions);
    // In the order the device prefers, which picks each game's shown version.
    const order = preferredRegions([]);
    const at = (r: string) => (order.includes(r) ? order.indexOf(r) : order.length);
    for (const r of savedRegions().sort((a, b) => at(a) - at(b))) if (have.has(r)) regionSel.add(r);
  }

  let filtered = $derived.by(() => {
    const matched = regionSel.size
      ? textMatched.filter((e) => e.kind === "folder" || (regionsOf.get(e.url) ?? []).some((r) => regionSel.has(r)))
      : textMatched;
    // One entry per game: its best version among the ones that match.
    const best = new Map<string, Entry>();
    const list: Entry[] = [];
    for (const e of matched) {
      if (e.kind === "folder") {
        list.push(e);
        continue;
      }
      const g = gameOf.get(e.url)!;
      const cur = best.get(g);
      if (!cur || compareVersions(e, cur, prefs) < 0) best.set(g, e);
    }
    list.push(...best.values());
    const dir = sortDesc ? -1 : 1;
    const rating = (e: Entry) => sortScores.get(titleOf.get(e.url)!);
    return list.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
      if (sortBy === "size") return (a.sizeBytes - b.sizeBytes) * dir;
      if (sortBy === "rating") {
        // Unrated games go last either way, in name order.
        const ra = rating(a), rb = rating(b);
        if (ra == null || rb == null) return ra != null ? -1 : rb != null ? 1 : a.name.localeCompare(b.name);
        if (ra !== rb) return (ra - rb) * dir;
      }
      return a.name.localeCompare(b.name) * (sortBy === "name" ? dir : 1);
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
  let tintHex = $derived(CONSOLE_TINTS[currentConsole?.folder ?? ""] ?? TINTS[currentConsole?.group ?? ""] ?? null);
  let tint = $derived(tintHex ?? "var(--accent)");
  /** The console's box shape (see ConsoleInfo.coverRatio), for covers everywhere. */
  let coverRatio = $derived(currentConsole?.coverRatio ?? 1.5);
  $effect(() => {
    document.documentElement.style.setProperty("--cover-ratio", String(coverRatio));
  });

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

  // ---- second screen (dual-screen devices like the AYN Thor) ----
  // Shows the game in the details window, or else the one highlighted with
  // the controller, with its versions and download buttons, plus downloads.
  let screenTitle = $state<string | null>(null);
  let screenInfo = $state<{ title: string; cover: string | null; details: GameDetails | null } | null>(null);
  $effect(() => {
    void listing;
    screenTitle = null;
  });
  $effect(() => {
    const item = cursor >= 0 ? filtered[cursor] : undefined;
    // Nothing highlighted: the second screen shows the console overview.
    screenTitle = detailTitle ?? (item?.kind === "file" ? parseName(item.name).title : null);
  });
  // The overview: the console's best-rated games (within the current
  // filters) and what finished downloading this session, with covers.
  type Pick = { title: string; url: string; cover: string | null; rating: { score: number; tier: string } | null };
  /** A game for the second screen's shelves: its cover, rating and card. */
  async function makePick(title: string, url: string, slug: string): Promise<Pick> {
    const r = ratings[titleOf.get(url) ?? ""];
    return {
      title,
      url: cardOf.get(url) ?? url,
      cover: await coverFor(title, slug).catch(() => null),
      rating: r ? { score: Math.round(r[0]), tier: ratingTier(r[0]) } : null,
    };
  }
  let overviewTop = $state<Pick[]>([]);
  let overviewRecent = $state<Pick[]>([]);
  $effect(() => {
    if (!dualScreen || !listing) return;
    const slug = listingSlug(listing.url);
    const score = (e: Entry) => sortScores.get(titleOf.get(e.url) ?? "") ?? -1;
    const top = filtered
      .filter((e) => e.kind === "file" && score(e) >= 0)
      .sort((a, b) => score(b) - score(a))
      .slice(0, 6);
    const urls = new Set(listing.entries.map((e) => e.url));
    const recent = jobs
      .filter((j) => j.state === "done" && urls.has(j.romUrl))
      .slice(-4)
      .reverse();
    const pick = (title: string, url: string) => makePick(title, url, slug);
    let stale = false;
    Promise.all([
      Promise.all(top.map((e) => pick(parseName(e.name).title, e.url))),
      Promise.all(recent.map((j) => pick(parseName(j.name).title, j.romUrl))),
    ]).then(([t, r]) => {
      if (stale) return;
      overviewTop = t;
      overviewRecent = r;
    });
    return () => (stale = true);
  });
  // With a game's details open on the top screen, the second screen shows
  // its screenshots and these: the console's best-rated games sharing a
  // genre with it.
  let similar = $state<Pick[]>([]);
  $effect(() => {
    const title = detailTitle;
    const l = listing;
    similar = [];
    if (!dualScreen || !title || !l) return;
    const slug = listingSlug(l.url);
    const score = (e: Entry) => sortScores.get(titleOf.get(e.url) ?? "") ?? -1;
    const pool = untrack(() =>
      filtered
        .filter((e) => e.kind === "file" && score(e) >= 0)
        .sort((a, b) => score(b) - score(a))
        .slice(0, 200),
    );
    // Genres so broad they'd make almost anything "similar" (GTA and Final
    // Fantasy X are both "Adventure"), unless a game has nothing else.
    const BROAD = new Set(["Adventure", "Arcade", "Indie"]);
    const keyGenres = (gs: string[] = []) => {
      const narrow = gs.filter((g) => !BROAD.has(g));
      return new Set(narrow.length ? narrow : gs);
    };
    let stale = false;
    (async () => {
      const me = await gameDetails(title, slug).catch(() => null);
      const mine = keyGenres(me?.genres);
      if (!me || !mine.size) return;
      // The best-rated games sharing the most of its genres.
      const found: { e: Entry; t: string; shared: number; rank: number }[] = [];
      for (const [rank, e] of pool.entries()) {
        if (stale) return;
        const t = parseName(e.name).title;
        const d = await gameDetails(t, slug).catch(() => null);
        if (!d || d.igdb === me.igdb) continue;
        const shared = [...keyGenres(d.genres)].filter((g) => mine.has(g)).length;
        if (shared) found.push({ e, t, shared, rank });
        // Enough that share everything: no need to look further.
        if (found.filter((f) => f.shared === mine.size).length >= 6) break;
      }
      found.sort((a, b) => b.shared - a.shared || a.rank - b.rank);
      const out = await Promise.all(found.slice(0, 6).map((f) => makePick(f.t, f.e.url, slug)));
      if (!stale) similar = out;
    })();
    return () => (stale = true);
  });
  $effect(() => {
    const title = screenTitle;
    const l = listing;
    if (!dualScreen || !title || !l) return;
    const slug = listingSlug(l.url);
    let stale = false;
    Promise.all([coverFor(title, slug).catch(() => null), gameDetails(title, slug).catch(() => null)]).then(
      ([cover, details]) => {
        if (!stale) screenInfo = { title, cover, details };
      },
    );
    return () => (stale = true);
  });
  function jobProgress(j: Job | undefined) {
    return j && j.total > 0 && j.state !== "queued" && j.state !== "resolving" ? j.completed / j.total : null;
  }
  let screenTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!dualScreen) return;
    const info = screenInfo && screenInfo.title === screenTitle ? screenInfo : null;
    const d = info?.details;
    const game = info &&
      listing && {
        title: info.title,
        console: listing.crumbs.at(-1)?.name ?? "",
        cover: info.cover ?? (d?.cover ? igdbImage(d.cover, "cover_big") : null),
        // Artwork and screenshots, shown one after another behind the details.
        heroes: [...(d?.artworks ?? []), ...(d?.screenshots ?? [])].slice(0, 8).map((id) => igdbImage(id, "720p")),
        meta: [d?.released?.slice(0, 4), d?.genres?.slice(0, 2).join(", "), d?.developers?.[0]].filter(Boolean),
        summary: d?.summary ?? "",
        // Details open on the top screen: show screenshots and similar games instead.
        gallery: !!detailTitle,
        similar,
        rating: d?.rating != null ? { score: Math.round(d.rating), tier: ratingTier(d.rating), count: d.ratingCount ?? 0 } : null,
        versions: versionsOf(info.title).map((e) => {
          const p = parseName(e.name);
          const job = jobs.find((j) => j.romUrl === e.url && !["failed", "cancelled"].includes(j.state));
          const status = have[fileStem(e.name)] || job?.state === "done" ? "have" : queued.has(e.url) ? "queued" : "none";
          return {
            url: e.url,
            label: p.regions.join(", ") || "Version",
            // Other titles the game is known by, e.g. "Biohazard 2" under "Resident Evil 2".
            sub: [
              ...(p.title !== info.title ? [p.title] : []),
              ...p.tags.filter((t) => t !== p.regions.join(", ")),
              e.size,
            ].join(" · "),
            status,
            progress: status === "queued" ? jobProgress(job) : null,
          };
        }),
      };
    const active = jobs
      .filter((j) => !["done", "failed", "cancelled"].includes(j.state))
      .map((j) => {
        const p = jobProgress(j);
        return {
          name: parseName(j.name).title,
          progress: p,
          status:
            j.state === "downloading" && p != null
              ? `${Math.round(p * 100)}% · ${formatBytes(j.speed)}/s`
              : j.state,
        };
      });
    const theme = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    const overview = listing && {
      console: currentConsole?.name ?? listing.crumbs.at(-1)?.name ?? "",
      group: currentConsole?.group ?? "",
      stats: gameCount ? [`${gameCount.toLocaleString()} games`, formatBytes(totalBytes)] : [],
      top: overviewTop,
      recent: overviewRecent,
    };
    const state = { game: game || null, overview: overview || null, jobs: active, theme, tint: tintHex, coverRatio };
    // Batch quick changes (moving through the grid, progress ticks).
    clearTimeout(screenTimer);
    screenTimer = setTimeout(() => api.secondScreenUpdate(state).catch(() => {}), 80);
  });
  // A tap on a game highlights it. With a second screen that's where its
  // details show, so the details window only opens on single-screen devices
  // (or with Enter / the controller's A button).
  function tapGame(e: Entry) {
    cursor = filtered.indexOf(e);
    if (!dualScreen) openDetails(e);
  }
  // Taps on the second screen.
  function onSecondScreen(msg: { action?: string; url?: string; title?: string }) {
    if (msg.action === "download" && msg.url) {
      const entry = listing?.entries.find((e) => e.url === msg.url);
      if (entry) download([entry]);
    } else if (msg.action === "details" && msg.title) {
      detailOrigin = null;
      detailTitle = msg.title;
    } else if (msg.action === "highlight" && msg.url) {
      // A game picked from the overview: highlight it on the top screen.
      const i = filtered.findIndex((e) => e.url === msg.url);
      if (i >= 0) {
        cursor = i;
        feel("move");
      }
    }
  }

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
      feel("done");
      setTimeout(() => justDone.delete(j.romUrl), 1600);
    }
  });
  // A card shows its game's state across all versions: downloaded, queued or
  // downloading if any version is.
  /** Each version's URL -> the URL of the card it's shown under. */
  let cardOf = $derived.by(() => {
    const m = new Map<string, string>();
    for (const e of filtered) {
      if (e.kind !== "file") continue;
      for (const v of versionsOf(titleOf.get(e.url)!)) m.set(v.url, e.url);
    }
    return m;
  });
  function toCards(urls: Iterable<string>): Set<string> {
    const out = new Set<string>();
    for (const u of urls) {
      const card = cardOf.get(u);
      if (card) out.add(card);
    }
    return out;
  }
  let cardDownloaded = $derived(toCards(downloadedUrls));
  let cardQueued = $derived(toCards(queued));
  let cardJustDone = $derived(toCards(justDone));
  let cardProgress = $derived(
    new Map([...progressByUrl].flatMap(([u, p]) => (cardOf.has(u) ? [[cardOf.get(u)!, p] as const] : []))),
  );
  /** Each card's IGDB [rating, review count]. */
  let cardRatings = $derived(
    new Map(
      filtered.flatMap((e) => {
        const r = e.kind === "file" ? ratings[titleOf.get(e.url)!] : undefined;
        return r ? [[e.url, r] as const] : [];
      }),
    ),
  );
  /** How many versions each card's game has. */
  let versionCounts = $derived(
    new Map(filtered.filter((e) => e.kind === "file").map((e) => [e.url, versionsOf(titleOf.get(e.url)!).length])),
  );

  let totalBytes = $derived(listing?.entries.reduce((s, e) => s + (e.kind === "file" ? e.sizeBytes : 0), 0) ?? 0);
  let gameCount = $derived(versionsByGame.size);

  // A console opened before shows straight away from its saved copy (see
  // Manager::fetch_listing), then quietly picks up any changes. Only the
  // latest load counts, so a slow one can't replace a console picked since.
  let loadToken = 0;
  /** Listings ready to show without asking the backend: the current console
   *  and the ones either side (see prefetchNeighbours), by requested URL. */
  const listingCache = new Map<string, Listing>();
  function remember(url: string, l: Listing) {
    listingCache.delete(url);
    listingCache.set(url, l);
    // Keep the few most recent; big consoles hold thousands of entries.
    while (listingCache.size > 6) listingCache.delete(listingCache.keys().next().value!);
  }
  async function load(url: string) {
    const token = ++loadToken;
    error = null;
    const cached = listingCache.get(url) ?? (await api.cachedListing(url).catch(() => null));
    if (token !== loadToken) return;
    if (cached) {
      remember(url, cached);
      loading = false;
      show(cached);
      api
        .refreshListing(url)
        .then((fresh) => {
          if (!fresh) return;
          remember(url, fresh);
          if (token === loadToken) listing = fresh;
        })
        .catch(() => {});
    } else {
      loading = true;
      try {
        const l = await api.loadListing(url);
        remember(url, l);
        if (token === loadToken) show(l);
      } catch (e) {
        if (token === loadToken) error = String(e);
      } finally {
        if (token === loadToken) loading = false;
      }
    }
    prefetchNeighbours();
  }
  function show(l: Listing) {
    listing = l;
    try {
      localStorage.setItem("lastListing", l.url);
    } catch {}
    query = "";
    selected.clear();
    applyRegions();
  }
  // On Android (controllers, L2 / R2), have the consoles either side ready
  // in memory, game data and all, so switching to them is instant.
  let prefetchTimer: ReturnType<typeof setTimeout> | undefined;
  function prefetchNeighbours() {
    clearTimeout(prefetchTimer);
    if (!isAndroid && !padConnected) return;
    // A moment's grace, so the console just opened gets the network first.
    prefetchTimer = setTimeout(() => {
      const list = consoleOrder;
      const i = currentConsole ? list.indexOf(currentConsole) : -1;
      if (i < 0 || list.length < 2) return;
      const near = new Set([list[(i + 1) % list.length], list[(i - 1 + list.length) % list.length]]);
      for (const c of near) {
        if (!listingCache.has(c.listing)) {
          api
            .prefetchListing(c.listing)
            .then((l) => remember(c.listing, l))
            .catch(() => {});
        }
        const slug = listingSlug(c.listing);
        if (!artCache.has(slug)) artData(slug).catch(() => {});
      }
    }, 300);
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
    feel("download");
    try {
      const folder = listing ? consoleFor(listing.url, consoles)?.folder : undefined;
      await api.enqueue(entries.map((e) => ({ name: e.name, url: e.url, folder, subfolder })));
      entries.forEach((e) => selected.delete(e.url));
    } catch (e) {
      error = String(e);
    }
  }

  function setSort(key: SortKey) {
    if (sortBy === key) sortDesc = !sortDesc;
    else {
      sortBy = key;
      sortDesc = key !== "name"; // biggest and best-rated first
    }
  }

  async function browseFetchDir() {
    if (!settings) return;
    try {
      // Android has no folder dialog in the dialog plugin; use our own picker.
      const dir = isAndroid
        ? await api.pickFolder()
        : await openDialog({ directory: true, defaultPath: settings.fetchDir || undefined });
      if (typeof dir === "string") settings.fetchDir = dir;
    } catch (e) {
      error = String(e);
    }
  }

  async function browseDir() {
    if (!settings) return;
    try {
      // Android has no folder dialog in the dialog plugin; use our own picker.
      const dir = isAndroid
        ? await api.pickFolder()
        : await openDialog({ directory: true, defaultPath: settings.downloadDir || undefined });
      if (typeof dir === "string") settings.downloadDir = dir;
    } catch (e) {
      error = String(e);
    }
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
        artCache.clear();
        refreshArtTitles();
      }
      hideNoArt = settings.hideWithoutArt;
      setFeedback(settings);
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

  // A sound / buzz as the game details or Settings open and close.
  let wasOpen = false;
  $effect(() => {
    const open = !!detailTitle || showSettings;
    if (open !== wasOpen) feel(open ? "open" : "back");
    wasOpen = open;
  });

  // Android's back gesture should close the game details or Settings rather
  // than leave the app: an extra history entry is added while one is open.
  let backEntry = false;
  $effect(() => {
    const open = !!detailTitle || showSettings;
    if (!isAndroid) return;
    if (open && !backEntry) {
      history.pushState({ overlay: true }, "");
      backEntry = true;
    } else if (!open && backEntry) {
      backEntry = false;
      history.back(); // closed from the UI: drop the extra entry
    }
  });
  function onBack() {
    if (!backEntry) return;
    backEntry = false;
    detailTitle = null;
    showSettings = false;
  }

  onMount(() => {
    window.addEventListener("popstate", onBack);
    if (isAndroid) {
      const w = window as unknown as {
        __owlSecondScreen: typeof onSecondScreen;
        __owlPad: typeof onPad;
        __owlPadConnected: (v: boolean) => void;
      };
      w.__owlSecondScreen = onSecondScreen;
      w.__owlPad = onPad;
      w.__owlPadConnected = (v) => (padConnected = v);
      api.gamepadConnected().then((v) => (padConnected = v)).catch(() => {});
      api.secondScreenAvailable().then((v) => (dualScreen = v)).catch(() => {});
    }
    const unlisten = listen<Job[]>("jobs", (e) => (jobs = e.payload));
    // The game data bundled with the app has been unpacked: show its covers,
    // details and console counts straight away.
    const unData = listen("data-ready", () => {
      resetCovers();
      artCache.clear();
      refreshArtTitles();
      loadConsoles().then((c) => (consoles = c));
    });
    let wasRunning = false;
    const unFetch = listen<FetchStatus>("fetch", (e) => {
      fetchState = e.payload;
      // When a fetch ends, show the new covers and counts.
      if (wasRunning && !e.payload.running) {
        resetCovers();
        artCache.clear();
        refreshArtTitles();
        loadConsoles().then((c) => (consoles = c));
      }
      wasRunning = e.payload.running;
    });
    api.fetchStatus().then((s) => (fetchState = s));
    api.getSettings().then((s) => {
      hideNoArt = s.hideWithoutArt;
      setFeedback(s);
    });
    api.listJobs().then((j) => (jobs = j));
    // Once per launch; quietly gives up offline.
    api
      .checkUpdate()
      .then((u) => (update = u))
      .catch(() => {});
    api.engineStatus().then((s) => (engineError = s.ok ? null : s.error));
    loadConsoles().then((c) => (consoles = c));
    load(lastListing());
    // Window size/state only exists on desktop; on Android the app is always full screen.
    if (isAndroid) document.documentElement.classList.add("android");
    if (!CSS.supports("color", "color-mix(in srgb, red, blue)")) document.documentElement.classList.add("no-color-mix");
    const syncMax = () => win.isMaximized().then((m) => (maximized = m)).catch(() => {});
    if (!isAndroid) syncMax();
    const unResize = isAndroid ? Promise.resolve(() => {}) : win.onResized(syncMax);
    return () => {
      unlisten.then((f) => f());
      unFetch.then((f) => f());
      unData.then((f) => f());
      unResize.then((f) => f());
    };
  });
</script>

<div
  class="app"
  class:maximized
  class:android={isAndroid}
  class:compact
  style:--tint={tint}
  bind:this={appEl}
  style:transform={chromeHidden && chromeHeight ? `translateY(-${chromeHeight}px)` : null}
  style:height={chromeTall && chromeHeight ? `calc(100vh + ${chromeHeight}px)` : null}
  style:--chrome-pad={chromeTall && chromeHeight && !chromeHidden ? `${chromeHeight}px` : null}
>
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
    {#if update}
      <button
        type="button"
        class="update"
        title={isAndroid ? `Download Owlcart ${update.version}` : `Owlcart ${update.version} is out: open the release`}
        onclick={() => update && api.openUpdate(update.url).catch((e) => (error = String(e)))}
        transition:fade
      >
        <Icon name="download" size={13} />
        {update.version} available
      </button>
    {/if}
    <button
      type="button"
      class="icon keys"
      class:hidden={isAndroid && !padConnected}
      title={padConnected ? "Controller buttons (Select)" : "Keyboard shortcuts (?)"}
      onclick={() => (showKeys = true)}
    >
      ?
    </button>
    <DownloadsButton bind:this={downloadsBtn} {jobs} onError={(m) => (error = m)} />
    <button type="button" class="icon" onclick={openSettings} title="Settings ({mod} ,)">
      <Icon name="gear" size={16} />
    </button>
    <div class="winctl" class:hidden={isAndroid}>
      <button class="icon" title="Minimise" onclick={() => win.minimize()}><Icon name="min" size={14} /></button>
      <button class="icon" title={maximized ? "Restore" : "Maximise"} onclick={() => win.toggleMaximize()}>
        <Icon name={maximized ? "restore" : "max"} size={13} />
      </button>
      <button class="icon close" title="Close" onclick={() => win.close()}><Icon name="x" size={14} /></button>
    </div>
  </header>

  {#if engineError}
    <div class="banner">
      The download engine didn't start: {engineError}
    </div>
  {/if}
  {#if error}
    <div class="banner">
      <span>{error}</span>
      <button class="icon" title="Dismiss" onclick={() => (error = null)}><Icon name="x" size={14} /></button>
    </div>
  {/if}

  <div class="body">
    <main onpointerover={onPointerOver} onscrollcapture={onListScroll}>
      {#if listing}
        <section class="hero" style="--tint: {tint}">
          <HeroCollage slug={listingSlug(listing.url)} titles={artTitles ? [...artTitles] : []} slim={compact} />
          {#key listing.url}
          <div class="hero-text" in:fly={{ x: slideDir * 40, duration: slideDir ? 260 : 0 }}>
            <div class="eyebrow">{currentConsole?.group ?? listing.crumbs.at(-2)?.name ?? "Minerva"}</div>
            <h1>{currentConsole?.name ?? listing.crumbs.at(-1)?.name ?? "Browse"}</h1>
            <div class="stats">
              {#if gameCount}
                <div><b>{gameCount.toLocaleString()}</b><span>games</span></div>
                <div><b>{formatBytes(totalBytes)}</b><span>total</span></div>
                {#if compact && visibleFiles.length !== gameCount}
                  <div><b>{visibleFiles.length.toLocaleString()}</b><span>shown</span></div>
                {/if}
              {/if}
              {#if folderCount && !gameCount}
                <div><b>{folderCount}</b><span>folders</span></div>
              {/if}
            </div>
          </div>
          {/key}
          {#if spotlight && !compact}
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
          {#if compact}
            {#if allRegions.length > 1}
              <button
                class="toggle"
                class:on={regionSel.size > 0}
                aria-expanded={panel === "regions"}
                title="Filter by region"
                onclick={() => togglePanel("regions")}
              >
                {#each [...regionSel].slice(0, 3) as r (r)}<Flag region={r} height={11} />{:else}<Icon
                    name="globe"
                    size={14}
                  />{/each}
                Region{regionSel.size > 3 ? ` +${regionSel.size - 3}` : ""}
              </button>
            {/if}
            <button class="toggle" aria-expanded={panel === "sort"} title="Sort" onclick={() => togglePanel("sort")}>
              <Icon name={sortBy === "rating" ? "star" : "sort"} size={14} />
              {SORTS.find(([v]) => v === sortValue)?.[1] ?? "Sort"}
            </button>
          {/if}
          <label class="filter">
            <Icon name="search" size={14} />
            <input bind:this={filterEl} bind:value={query} spellcheck="false" placeholder="Filter" />
            {#if !query && padConnected}<PadButton name="Y" class="in-field" />{:else if !query && !isAndroid}<kbd
                class="in-field">/</kbd
              >{/if}
          </label>
        </div>

        {#if compact && panel === "sort"}
          <div class="regions wrap" transition:slide={{ duration: 180 }} onintroend={measureChrome} onoutroend={measureChrome}>
            {#each SORTS as [v, label] (v)}
              <button class="chip" class:on={sortValue === v} onclick={() => ((panel = ""), pickSort(v))}>
                {#if v === "rating:1"}<Icon name="star" size={12} />{/if}{label}
              </button>
            {/each}
          </div>
        {/if}
        {#if allRegions.length > 1 && (!compact || panel === "regions")}
          <div
            class="regions"
            class:wrap={compact}
            transition:slide={{ duration: compact ? 180 : 0 }}
            onintroend={measureChrome}
            onoutroend={measureChrome}
          >
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
              <button class="text clear" onclick={clearRegions}>Clear</button>
            {/if}
          </div>
        {/if}

        <div class="colhead" class:gone={compact} bind:this={colheadEl}>
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
              {visibleFiles.length.toLocaleString()} {visibleFiles.length === 1 ? "game" : "games"}{folderCount ? `, ${folderCount} folders` : ""}{hiddenNoArt
                ? ` · ${hiddenNoArt.toLocaleString()} without art hidden`
                : ""}{hiddenExtras
                ? ` · ${hiddenExtras.toLocaleString()} demos & betas hidden`
                : ""}
            </span>
          </button>
          <button class="sort right" title="IGDB rating" onclick={() => setSort("rating")}>
            Rating {sortBy === "rating" ? (sortDesc ? "↓" : "↑") : ""}
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
            progress={cardProgress}
            justDone={cardJustDone}
            {versionCounts}
            ratings={cardRatings}
            introKey={listing.url}
            introDir={slideDir}
            downloaded={cardDownloaded}
            brief={compact}
            quiet={isAndroid && padConnected}
            {coverRatio}
            bind:cols={gridCols}
            {cursor}
            entries={filtered}
            slug={listingSlug(listing.url)}
            {selected}
            queued={cardQueued}
            onOpenFolder={load}
            onDownload={(e) => download([e])}
            onOpen={tapGame}
          />
        {:else}
          <GameList
            progress={cardProgress}
            justDone={cardJustDone}
            {versionCounts}
            ratings={cardRatings}
            introKey={listing.url}
            downloaded={cardDownloaded}
            {cursor}
            entries={filtered}
            {selected}
            queued={cardQueued}
            onOpenFolder={load}
            onDownload={(e) => download([e])}
            onOpen={tapGame}
          />
        {/if}

        {#if selected.size}
          <div class="selbar">
            <span><b>{selected.size}</b> selected</span>
            <span class="mono">{formatBytes(selectedBytes)}</span>
            <div class="grow"></div>
            <button class="text" onclick={() => selected.clear()}>Clear</button>
            <button class="primary" onclick={() => download(selectedEntries)} disabled={!!engineError}>
              <Icon name="download" size={15} /> Download {#if padConnected}<PadButton name="X" onAccent />{:else if !isAndroid}<kbd
                  class="on-accent">{mod}↵</kbd
                >{/if}
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

<svelte:window onkeydown={onGlobalKey} bind:innerHeight={winH} />

{#if showKeys}
  <div class="modal-bg" role="presentation" onclick={() => (showKeys = false)}>
    <div class="modal keys-modal" role="dialog" tabindex="-1" aria-label={isAndroid ? "Controller buttons" : "Keyboard shortcuts"} onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>{isAndroid ? "Controller buttons" : "Keyboard shortcuts"}</h2>
      <dl>
        {#each isAndroid ? PAD_BUTTONS : SHORTCUTS as [k, what]}
          <dt>
            {#each k.split("  or  ") as part, i}{#if i}<span class="or">or</span>{/if}{#each part.split(" ") as key}{#if isAndroid}<PadButton
                    name={key}
                  />{:else}<kbd>{key}</kbd>{/if}{/each}{/each}
          </dt>
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
      {#if isAndroid}
        <label class="check-row">
          <input type="checkbox" bind:checked={settings.haptics} />
          <div>
            Rumble
            <small>A light buzz when moving through games, switching consoles and downloading.</small>
          </div>
        </label>
      {/if}
      <label class="check-row">
        <input type="checkbox" bind:checked={settings.checkUpdates} />
        <div>
          Tell me about updates
          <small>Checks GitHub for a newer Owlcart when the app starts. Nothing is installed for you.</small>
        </div>
      </label>
      <label class="check-row">
        <input type="checkbox" bind:checked={settings.sounds} />
        <div>
          Menu sounds
          <small>Soft clicks and chimes as you move around and download.</small>
        </div>
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
  /* Android: always full screen, solid background, no window chrome. */
  :global(html.android),
  :global(html.android body) {
    background: var(--bg);
  }
  .app.android {
    border: 0;
    border-radius: 0;
  }
  .app.android {
    transition: transform 0.24s cubic-bezier(0.2, 0.7, 0.2, 1);
    will-change: transform;
  }
  .app.android :global(.viewport) {
    padding-bottom: var(--chrome-pad, 0px);
  }
  .hidden {
    display: none !important;
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
  .filter :global(.in-field) {
    position: absolute;
    right: 7px;
    pointer-events: none;
  }
  .keys {
    font: 700 15px var(--display);
  }
  /* A newer version is out (see check_update). */
  .update {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 11px;
    border: 0;
    border-radius: 13px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }
  .update:active {
    transform: scale(0.95);
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
  /* In the console's colour (see `tint`). */
  .chip.on {
    background: var(--tint);
    border-color: var(--tint);
    color: #fff;
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
    grid-template-columns: 30px 1fr 62px 84px 40px;
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
  /* Short screens (handhelds, small windows): a slim header so the games
     start near the top. */
  @media (max-height: 640px) {
    .hero {
      min-height: 0;
      padding: 10px 16px;
    }
    .hero h1 {
      font-size: 24px;
    }
    .hero .stats {
      margin-top: 2px;
    }
    .spot {
      display: none;
    }
    .titlebar {
      height: 44px;
    }
  }
  /* Older WebViews (e.g. Android System WebView before v111) lack color-mix(),
     and declarations using it with var() silently become blank. Give the
     parts that would vanish a plain look instead. */
  :global(html.no-color-mix) .app {
    background: var(--bg);
  }
  :global(html.no-color-mix) h1 {
    background: none;
    color: var(--text);
  }
  :global(html.no-color-mix) .eyebrow,
  :global(html.no-color-mix) .spot-eyebrow {
    color: var(--muted);
  }
  :global(html.no-color-mix) .hero::after {
    background: linear-gradient(90deg, var(--bg) 18%, transparent 60%);
  }
  :global(html.no-color-mix) .banner {
    background: var(--field);
  }
  :global(html.no-color-mix) .spot-main {
    background: var(--panel);
    border-color: var(--line);
  }
  /* ---- Android and small screens ---- */
  :global(html.android) {
    -webkit-tap-highlight-color: transparent;
    overscroll-behavior: none;
  }
  /* Settings fill the screen on Android. */
  :global(html.android) .modal-bg {
    place-items: stretch;
  }
  :global(html.android) .modal {
    width: 100vw;
    max-height: none;
    height: 100vh;
    box-sizing: border-box;
    border: 0;
    border-radius: 0;
    padding: 18px 16px 28px;
  }
  /* Bigger touch targets for the toolbar and selection bar. */
  :global(html.android) .seg button {
    width: 38px;
    height: 32px;
  }
  :global(html.android) .toggle {
    height: 34px;
  }
  :global(html.android) .selbar {
    padding: 10px 14px;
  }
  /* Compact (see `compact`): the banner shrinks to one line, the path and up
     button go (the console picker moves between consoles), and the column
     header goes (sorting is in the toolbar). The header stays in the page at
     no height: the slide-away measures to it. */
  .compact .hero {
    min-height: 0;
    padding: 8px 14px;
  }
  .compact .hero-text {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .compact h1 {
    margin: 0;
    font-size: 20px;
    white-space: nowrap;
  }
  .compact .stats {
    margin-top: 0;
    gap: 14px;
  }
  .compact .crumbs {
    display: none;
  }
  .compact .filter {
    flex: 1;
  }
  .compact .filter input {
    width: 100%;
    min-width: 0;
  }
  .regions.wrap {
    flex-wrap: wrap;
  }
  /* A little squash when pressed. */
  .toggle,
  .chip,
  .seg button,
  .titlebar .icon {
    transition-property: background, border-color, color, opacity, transform;
    transition-duration: 0.15s;
  }
  .toggle:active,
  .chip:active,
  .seg button:active,
  .titlebar .icon:active {
    transform: scale(0.93);
  }
  .colhead.gone {
    height: 0;
    padding: 0;
    border: 0;
    overflow: hidden;
    visibility: hidden;
  }
  /* Narrow screens (phones in portrait): the console picker already says
     where you are, so drop the breadcrumbs and let the filter stretch. */
  @media (max-width: 700px) {
    .crumbs {
      display: none;
    }
    .subbar {
      gap: 8px;
    }
    .filter {
      flex: 1;
    }
    .filter input {
      width: 100%;
      min-width: 0;
    }
    .brand span {
      display: none;
    }
    .brand {
      width: auto;
    }
  }
</style>
