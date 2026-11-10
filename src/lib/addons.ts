// Loads a console's updates and DLC (the "addons" collections in
// consoles.json) and groups them by game. Matching rules: addons-core.ts.

import { api, type Entry } from "./api";
import { groupAddons, type Addon as CoreAddon, type AddonCollection, type AddonsByGame } from "./addons-core";

export type { AddonCollection, AddonKind } from "./addons-core";
/** An update or DLC file, with the full listing entry (size, URL…). */
export type Addon = CoreAddon<Entry>;

const cache = new Map<string, Promise<AddonsByGame<Entry>>>();

/** Loads every add-on collection for a console. Cached per console for the session. */
export function loadAddons(
  consoleKey: string,
  collections: AddonCollection[],
  games: Set<string>,
): Promise<AddonsByGame<Entry>> {
  let p = cache.get(consoleKey);
  if (!p) {
    p = Promise.all(
      collections.map((c) => api.loadListing(c.listing).then((l) => ({ c, entries: l.entries }))),
    ).then((listings) => groupAddons(listings, games));
    p.catch(() => cache.delete(consoleKey)); // let a failed load retry
    cache.set(consoleKey, p);
  }
  return p;
}
