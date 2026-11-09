// SteamGridDB cover lookups for the build scripts.

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { searchQuery, matchScore, MATCH_THRESHOLD } from "../src/lib/names.ts";
import { sleep } from "./common.ts";

const API = "https://www.steamgriddb.com/api/v2";
const CDN = "https://cdn2.steamgriddb.com/";

/** Reads the key from $STEAMGRIDDB_KEY or .steamgriddb-key (git-ignored). */
export async function sgdbKey(root: string): Promise<string> {
  const env = process.env.STEAMGRIDDB_KEY?.trim();
  if (env) return env;
  try {
    return (await readFile(join(root, ".steamgriddb-key"), "utf8")).trim();
  } catch {
    throw new Error("No SteamGridDB key: set STEAMGRIDDB_KEY or create .steamgriddb-key");
  }
}

async function api<T>(path: string, key: string): Promise<T | null> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(`${API}${path}`, { headers: { Authorization: `Bearer ${key}` } });
    if (res.status === 404) return null;
    if (res.status === 429 || res.status >= 500) {
      if (attempt >= 5) throw new Error(`SteamGridDB ${res.status} after retries`);
      const wait = Number(res.headers.get("retry-after")) * 1000 || 2000 * 2 ** attempt;
      console.warn(`  SteamGridDB ${res.status}, waiting ${Math.round(wait / 1000)}s`);
      await sleep(wait);
      continue;
    }
    if (res.status === 401) throw new Error("SteamGridDB rejected the API key (401)");
    const body = (await res.json()) as { success: boolean; data?: T; errors?: string[] };
    if (!body.success) throw new Error(`SteamGridDB ${res.status}: ${body.errors?.join(", ")}`);
    return body.data ?? null;
  }
}

/** Compact cover reference ("s:thumb/abc.jpg") for a title, or null. */
export async function sgdbCover(title: string, key: string, verbose = false): Promise<string | null> {
  const query = searchQuery(title);
  const games =
    (await api<{ id: number; name: string }[]>(`/search/autocomplete/${encodeURIComponent(query)}`, key)) ?? [];
  let game: { id: number; name: string } | undefined;
  let best = 0;
  for (const g of games) {
    const score = matchScore(query, g.name);
    if (score > best) [game, best] = [g, score];
  }
  if (!game || best < MATCH_THRESHOLD) return null;
  if (verbose) console.log(`  [sgdb] ${title}  ->  ${game.name} (${best.toFixed(2)})`);
  const params = new URLSearchParams({
    dimensions: "600x900,342x482,660x930",
    types: "static",
    nsfw: "false",
    humor: "false",
    epilepsy: "false",
  });
  const grids = (await api<{ url: string; thumb?: string }[]>(`/grids/game/${game.id}?${params}`, key)) ?? [];
  const url = grids[0] ? (grids[0].thumb ?? grids[0].url) : null;
  if (!url) return null;
  return url.startsWith(CDN) ? `s:${url.slice(CDN.length)}` : url;
}
