// Box art from the libretro thumbnail collection (the one RetroArch uses),
// for the build scripts. It's sorted by system and named after the same
// No-Intro / Redump file names as Minerva's listings, so a match is the right
// game on the right platform: unlike SteamGridDB, which has one entry per game
// across every platform and can hand back, say, the GBA box for an Xbox game.

import { parseName } from "../src/lib/names.ts";

const BASE = "https://thumbnails.libretro.com/";

/** Titles compared ignoring case and punctuation. libretro file names also
 *  swap characters that aren't allowed in file names (&*:/ etc.) for "_". */
function key(title: string): string {
  return title
    .replace(/[&*/:`<>?\\|"]/g, "_")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

export interface LibretroIndex {
  system: string;
  /** Title key -> box art file names (without .png). */
  files: Map<string, string[]>;
}

/** Lists a system's box art. `system` is a libretro folder, e.g. "Microsoft - Xbox". */
export async function libretroIndex(system: string): Promise<LibretroIndex> {
  const res = await fetch(`${BASE}${encodeURIComponent(system)}/Named_Boxarts/`, {
    headers: { "User-Agent": "owlcart-scripts" },
  });
  if (!res.ok) throw new Error(`libretro ${system}: ${res.status}`);
  const html = await res.text();
  const files = new Map<string, string[]>();
  for (const m of html.matchAll(/href="([^"?/][^"]*)\.png"/g)) {
    const name = decodeURIComponent(m[1]);
    const k = key(parseName(name).title);
    const list = files.get(k);
    if (list) list.push(name);
    else files.set(k, [name]);
  }
  return { system, files };
}

const REGION_ORDER = ["USA", "World", "Europe", "UK", "Australia", "Japan"];

/** Compact cover reference ("l:<system>/<file>") for a title, or null.
 *  With several versions, prefers USA, then World, then Europe box art, and
 *  plain releases over revisions, demos and the like. */
export function libretroCover(index: LibretroIndex, title: string): string | null {
  const names = index.files.get(key(title));
  if (!names?.length) return null;
  const rank = (name: string) => {
    const p = parseName(name);
    const r = Math.min(...p.regions.map((x) => REGION_ORDER.indexOf(x)).filter((i) => i >= 0), REGION_ORDER.length);
    return r * 100 + p.tags.length;
  };
  const best = [...names].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))[0];
  return `l:${index.system}/${best}`;
}
