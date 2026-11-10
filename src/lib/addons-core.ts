// Pure matching logic for updates and DLC (no app imports, so scripts and
// tests can use it too). See addons.ts for loading.

import { parseName } from "./names.ts";

export type AddonKind = "update" | "dlc";

export interface AddonCollection {
  /** "mixed" collections hold both; each file's tags decide. */
  kind: AddonKind | "mixed";
  listing: string;
}

/** The minimum a listing entry needs for matching. */
export interface AddonEntry {
  kind: "file" | "folder";
  name: string;
}

export interface Addon<E extends AddonEntry = AddonEntry> {
  entry: E;
  kind: AddonKind;
  /** What to show: the DLC's own name, or the update's version. */
  label: string;
}

export type AddonsByGame<E extends AddonEntry = AddonEntry> = Map<string, { updates: Addon<E>[]; dlc: Addon<E>[] }>;

/** Which kind a file is, or null if it isn't an update or DLC (e.g. a standalone game). */
export function classify(collection: AddonCollection, tags: string[]): AddonKind | null {
  const has = (re: RegExp) => tags.some((t) => re.test(t));
  const dlc = has(/^(DLC|Addon)$/i);
  const update = has(/^(Update|Embedded Update|Title Update)$/i);
  if (collection.kind === "mixed") return dlc ? "dlc" : update ? "update" : null;
  // Digital collections also hold full games (e.g. Xbox Live Arcade); keep only add-ons.
  if (collection.kind === "dlc") return dlc || collection.listing.includes("DLC") ? "dlc" : null;
  return "update";
}

/**
 * Finds which game an add-on belongs to: the longest game title its name
 * starts with, so "Rock Band 3 - Song Pack" goes to Rock Band 3, not Rock Band.
 */
export function ownerOf(title: string, games: Set<string>): string | null {
  const parts = title.split(" - ");
  for (let k = parts.length; k > 0; k--) {
    const candidate = parts.slice(0, k).join(" - ");
    if (games.has(candidate)) return candidate;
  }
  return null;
}

/** Groups add-on files from several collections by the game they belong to. */
export function groupAddons<E extends AddonEntry>(
  listings: { c: AddonCollection; entries: E[] }[],
  games: Set<string>,
): AddonsByGame<E> {
  const byGame: AddonsByGame<E> = new Map();
  for (const { c, entries } of listings) {
    for (const entry of entries) {
      if (entry.kind !== "file" || entry.name.startsWith("[")) continue; // [BIOS] and tools
      const p = parseName(entry.name);
      const kind = classify(c, p.tags);
      if (!kind) continue;
      const owner = ownerOf(p.title, games);
      if (!owner) continue;
      const rest = p.title.slice(owner.length).replace(/^ - /, "");
      const version = p.tags.find((t) => /^v\d/i.test(t));
      // Generic add-on packages carry only the game's name plus an ID tag.
      const extra = p.tags.find((t) => /^[0-9A-F]{8}$/.test(t));
      const label =
        kind === "dlc" ? rest || `Add-on content${extra ? ` (${extra})` : ""}` : (version ?? (rest || "Update"));
      const slot = byGame.get(owner) ?? { updates: [], dlc: [] };
      (kind === "dlc" ? slot.dlc : slot.updates).push({ entry, kind, label });
      byGame.set(owner, slot);
    }
  }
  // Newest updates first; DLC alphabetically.
  for (const slot of byGame.values()) {
    slot.updates.sort((a, b) => b.label.localeCompare(a.label, undefined, { numeric: true }));
    slot.dlc.sort((a, b) => a.label.localeCompare(b.label));
  }
  return byGame;
}
