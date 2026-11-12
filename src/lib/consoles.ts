import bundled from "../../consoles.json";
import { api } from "./api";
import { listingSlug } from "./names";
import type { AddonCollection } from "./addons-core";

export interface ConsoleInfo {
  group: string;
  name: string;
  /** Download folder name, e.g. "psx" (ES-DE system names). */
  folder?: string;
  listing: string;
  /** IGDB platform slug, for fetching game details. */
  igdb?: string;
  /** Collections holding this console's updates and DLC. */
  addons?: AddonCollection[];
  /** The shape of its box art, height / width: 1 for PlayStation's square
   *  cases, 0.73 for SNES's wide boxes. Measured from the libretro covers. */
  coverRatio?: number;
  /** Present when the game data has covers/details for this console. */
  titles?: number;
  covers?: number;
}

/**
 * Consoles for the picker: the list built into the app (consoles.json), plus
 * anything newer in the hosted game data's index.json.
 */
export async function loadConsoles(): Promise<ConsoleInfo[]> {
  const list: ConsoleInfo[] = bundled.map((c) => ({
    group: c.group,
    name: c.name,
    folder: c.folder,
    listing: c.listing,
    igdb: c.igdb,
    addons: (c as { addons?: AddonCollection[] }).addons,
    coverRatio: (c as { coverRatio?: number }).coverRatio,
  }));
  try {
    const index = await api.consoleIndex();
    for (const [slug, e] of Object.entries(index ?? {})) {
      const known = list.find((c) => listingSlug(c.listing) === slug);
      if (known) Object.assign(known, { titles: e.titles, covers: e.covers });
      else
        list.push({
          group: e.group || "Other",
          name: e.name,
          folder: e.folder,
          igdb: e.igdb,
          listing: e.listing,
          titles: e.titles,
          covers: e.covers,
        });
    }
  } catch (e) {
    console.warn("console index unavailable", e);
  }
  return list;
}

/** The console a listing URL belongs to (the longest matching prefix). */
export function consoleFor(url: string, consoles: ConsoleInfo[]): ConsoleInfo | undefined {
  let best: ConsoleInfo | undefined;
  const norm = (u: string) => decodeURIComponent(u).replace("/browse/./", "/browse/");
  for (const c of consoles) {
    if (norm(url).startsWith(norm(c.listing)) && (!best || c.listing.length > best.listing.length)) best = c;
  }
  return best;
}
