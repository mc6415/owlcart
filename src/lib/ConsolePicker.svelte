<script lang="ts">
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import { consoleFor, type ConsoleInfo } from "./consoles";
  import Icon from "./Icon.svelte";
  import { isAndroid } from "./platform";

  interface Props {
    consoles: ConsoleInfo[];
    /** URL of the listing currently shown. */
    current: string;
    /** Name to show when the current page isn't one of the consoles. */
    fallbackLabel: string;
    loading: boolean;
    onPick: (url: string) => void;
  }
  let { consoles, current, fallbackLabel, loading, onPick }: Props = $props();

  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  const shortcut = isMac ? "⌘K" : "Ctrl K";

  let open = $state(false);
  let query = $state("");
  let pasting = $state(false);
  let pasted = $state("");
  let active = $state(0);
  let searchEl = $state<HTMLInputElement>();
  let pasteEl = $state<HTMLInputElement>();
  let root = $state<HTMLDivElement>();

  let selected = $derived(consoleFor(current, consoles));

  let filtered = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return consoles.filter((c) => {
      const hay = `${c.group} ${c.name}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
  });

  /** Filtered consoles grouped by maker, keeping the list's order. */
  let groups = $derived.by(() => {
    const out: { group: string; items: { c: ConsoleInfo; i: number }[] }[] = [];
    filtered.forEach((c, i) => {
      let g = out.find((x) => x.group === c.group);
      if (!g) out.push((g = { group: c.group, items: [] }));
      g.items.push({ c, i });
    });
    return out;
  });

  /** Opens the picker (also used by the Ctrl+K shortcut). */
  export async function show() {
    open = true;
    pasting = false;
    query = "";
    active = Math.max(0, selected ? filtered.indexOf(selected) : 0);
    await tick();
    // On Android, focusing would pop up the keyboard over the list.
    if (!isAndroid) searchEl?.focus();
    root?.querySelector(".item.active")?.scrollIntoView({ block: "center" });
  }

  function pick(url: string) {
    open = false;
    onPick(url);
  }

  async function startPaste() {
    pasting = true;
    pasted = "";
    await tick();
    pasteEl?.focus();
  }

  async function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      open = false;
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = filtered.length;
      if (!n) return;
      active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      await tick();
      root?.querySelector(".item.active")?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && filtered[active]) {
      e.preventDefault();
      pick(filtered[active].listing);
    }
  }
</script>

<svelte:window
  onmousedown={(e) => {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }}
/>

<div class="picker" bind:this={root}>
  <button class="trigger" class:open onclick={() => (open ? (open = false) : show())} title="Choose a console">
    {#if selected}
      <span class="group">{selected.group}</span>
      <span class="name">{selected.name}</span>
    {:else}
      <span class="name">{fallbackLabel}</span>
    {/if}
    <span class="grow"></span>
    {#if loading}<span class="spinner"></span>{:else if !isAndroid}<kbd class="hint">{shortcut}</kbd>{/if}
    <svg class="chev" width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>

  {#if open}
    <div class="menu" transition:fly={{ y: -6, duration: 140 }} role="listbox" tabindex="-1">
      {#if pasting}
        <form
          class="paste"
          onsubmit={(e) => {
            e.preventDefault();
            if (pasted.trim()) pick(pasted.trim());
          }}
        >
          <input
            bind:this={pasteEl}
            bind:value={pasted}
            spellcheck="false"
            placeholder="https://minerva-archive.org/browse/…"
            onkeydown={(e) => e.key === "Escape" && (open = false)}
          />
          <button type="submit" class="primary small">Open</button>
        </form>
        <button class="text back" onclick={() => (pasting = false)}>← Back to consoles</button>
      {:else}
        <label class="search">
          <Icon name="search" size={14} />
          <input
            bind:this={searchEl}
            bind:value={query}
            spellcheck="false"
            placeholder="Search consoles"
            oninput={() => (active = 0)}
            onkeydown={onKey}
          />
        </label>
        <div class="list">
          {#each groups as g (g.group)}
            <div class="ghead">{g.group}</div>
            {#each g.items as { c, i } (c.listing)}
              <button
                class="item"
                class:active={i === active}
                class:current={c === selected}
                onmouseenter={() => (active = i)}
                onclick={() => pick(c.listing)}
              >
                <span class="iname">{c.name}</span>
                {#if c.titles}
                  <span class="badge" title="Has covers and game details">{c.titles.toLocaleString()} games</span>
                {/if}
                {#if c === selected}<Icon name="check" size={14} />{/if}
              </button>
            {/each}
          {:else}
            <p class="none">No consoles match.</p>
          {/each}
        </div>
        <button class="text paste-link" onclick={startPaste}>Paste a Minerva link…</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    flex: 0 1 380px;
    min-width: 0;
  }
  .trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 32px;
    padding: 0 10px 0 12px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: var(--field);
    color: var(--text);
    text-align: left;
    transition: border-color 0.15s, background 0.15s;
  }
  .trigger:hover,
  .trigger.open {
    border-color: var(--accent);
  }
  .group {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .name {
    font: 600 13px var(--display);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
  }
  .hint {
    opacity: 0.8;
  }
  .trigger:hover .hint,
  .open .hint {
    opacity: 0;
  }
  .chev {
    color: var(--muted);
    flex-shrink: 0;
    transition: transform 0.15s;
  }
  .open .chev {
    transform: rotate(180deg);
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 40;
    width: max(100%, 320px);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.35);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    border: 0 !important;
    background: none !important;
    padding: 0 !important;
    color: var(--text);
  }
  .list {
    max-height: min(60vh, 460px);
    overflow-y: auto;
    padding: 4px;
  }
  .ghead {
    padding: 8px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text);
    text-align: left;
  }
  .item.active {
    background: var(--hover);
  }
  .item.current {
    color: var(--accent);
    font-weight: 600;
  }
  .iname {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    font: 11px var(--mono);
    color: var(--muted);
    font-weight: 400;
  }
  .none {
    margin: 0;
    padding: 12px 10px;
    color: var(--muted);
  }
  .paste-link,
  .back {
    border-top: 1px solid var(--line) !important;
    border-radius: 0;
    padding: 9px 12px !important;
    text-align: left;
  }
  .paste {
    display: flex;
    gap: 8px;
    padding: 10px;
  }
  .paste input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
