// IGDB game details for the build scripts.

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { searchQuery, matchScore, MATCH_THRESHOLD } from "../src/lib/names.ts";
import { sleep } from "./common.ts";

export interface GameDetails {
  igdb: number;
  name: string;
  summary?: string;
  /** YYYY-MM-DD */
  released?: string;
  genres?: string[];
  developers?: string[];
  publishers?: string[];
  /** 0-100 */
  rating?: number;
  /** How many critic and user reviews `rating` is based on. */
  ratingCount?: number;
  /** IGDB image ids; the app builds URLs from these. */
  cover?: string;
  artworks?: string[];
  screenshots?: string[];
}

export interface Igdb {
  clientId: string;
  token: string;
}

interface IgdbGame {
  id: number;
  name: string;
  alternative_names?: { name: string }[];
  summary?: string;
  first_release_date?: number;
  genres?: { name: string }[];
  involved_companies?: { company: { name: string }; developer: boolean; publisher: boolean }[];
  total_rating?: number;
  total_rating_count?: number;
  cover?: { image_id: string };
  artworks?: { image_id: string }[];
  screenshots?: { image_id: string }[];
}

/** Logs in with client_id/client_secret from .igdb-credentials (git-ignored). */
export async function igdbLogin(root: string): Promise<Igdb> {
  let text: string;
  try {
    text = await readFile(join(root, ".igdb-credentials"), "utf8");
  } catch {
    throw new Error("Missing .igdb-credentials (client_id=... and client_secret=...)");
  }
  const cfg = Object.fromEntries(
    text
      .split("\n")
      .filter((l) => l.includes("="))
      .map((l) => [l.slice(0, l.indexOf("=")).trim(), l.slice(l.indexOf("=") + 1).trim()]),
  );
  if (!cfg.client_id || !cfg.client_secret) throw new Error(".igdb-credentials needs client_id and client_secret");
  const res = await fetch(
    `https://id.twitch.tv/oauth2/token?${new URLSearchParams({
      client_id: cfg.client_id,
      client_secret: cfg.client_secret,
      grant_type: "client_credentials",
    })}`,
    { method: "POST" },
  );
  const tok = (await res.json()) as { access_token?: string; message?: string };
  if (!tok.access_token) throw new Error(`Twitch login failed: ${tok.message ?? res.status}`);
  return { clientId: cfg.client_id, token: tok.access_token };
}

export async function igdb<T>(auth: Igdb, endpoint: string, body: string): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(`https://api.igdb.com/v4/${endpoint}`, {
      method: "POST",
      headers: { "Client-ID": auth.clientId, Authorization: `Bearer ${auth.token}` },
      body,
    });
    if (res.status === 429 || res.status >= 500) {
      if (attempt >= 5) throw new Error(`IGDB ${res.status} after retries`);
      await sleep(1000 * 2 ** attempt);
      continue;
    }
    if (!res.ok) throw new Error(`IGDB ${res.status}: ${await res.text()}`);
    return (await res.json()) as T;
  }
}

export async function igdbPlatform(auth: Igdb, slug: string): Promise<{ id: number; name: string }> {
  const [p] = await igdb<{ id: number; name: string }[]>(
    auth,
    "platforms",
    `fields id,name; where slug = "${slug.replace(/"/g, "")}";`,
  );
  if (!p) throw new Error(`Unknown IGDB platform slug: ${slug}`);
  return p;
}

const FIELDS = [
  "name",
  "alternative_names.name",
  "summary",
  "first_release_date",
  "genres.name",
  "involved_companies.company.name",
  "involved_companies.developer",
  "involved_companies.publisher",
  "total_rating",
  "total_rating_count",
  "cover.image_id",
  "artworks.image_id",
  "screenshots.image_id",
].join(",");

export async function igdbDetails(
  auth: Igdb,
  platformId: number,
  title: string,
  verbose = false,
): Promise<GameDetails | null> {
  const query = searchQuery(title);
  const games = await igdb<IgdbGame[]>(
    auth,
    "games",
    `search "${query.replace(/["\\]/g, " ")}"; fields ${FIELDS}; where platforms = (${platformId}); limit 15;`,
  );
  // Score against the main name and every alternative name; keep the best.
  let best: IgdbGame | undefined;
  let bestScore = 0;
  for (const g of games) {
    for (const n of [g.name, ...(g.alternative_names ?? []).map((a) => a.name)]) {
      const s = matchScore(query, n);
      // On a tie (IGDB sometimes has two entries with the same name), take the
      // one more people have reviewed: it's the one people mean.
      const more = (g.total_rating_count ?? 0) > (best?.total_rating_count ?? 0);
      if (s > bestScore || (s === bestScore && more)) [best, bestScore] = [g, s];
    }
  }
  if (!best || bestScore < MATCH_THRESHOLD) return null;
  if (verbose) console.log(`  [igdb] ${title}  ->  ${best.name} (${bestScore.toFixed(2)})`);

  const companies = best.involved_companies ?? [];
  const d: GameDetails = { igdb: best.id, name: best.name };
  if (best.summary) d.summary = best.summary;
  if (best.first_release_date) d.released = new Date(best.first_release_date * 1000).toISOString().slice(0, 10);
  if (best.genres?.length) d.genres = best.genres.map((g) => g.name);
  const devs = companies.filter((c) => c.developer).map((c) => c.company.name);
  const pubs = companies.filter((c) => c.publisher).map((c) => c.company.name);
  if (devs.length) d.developers = devs;
  if (pubs.length) d.publishers = pubs;
  if (best.total_rating) {
    d.rating = Math.round(best.total_rating);
    d.ratingCount = best.total_rating_count ?? 0;
  }
  if (best.cover) d.cover = best.cover.image_id;
  if (best.artworks?.length) d.artworks = best.artworks.slice(0, 4).map((a) => a.image_id);
  if (best.screenshots?.length) d.screenshots = best.screenshots.slice(0, 8).map((s) => s.image_id);
  return d;
}

interface RatedGame {
  id: number;
  name: string;
  total_rating?: number;
  total_rating_count?: number;
  parent_game?: number;
  version_parent?: number;
}
const RATED_FIELDS = "fields name,total_rating,total_rating_count,parent_game,version_parent;";

/** Every game matching `where`, 500 per request. */
async function igdbAll(auth: Igdb, where: string): Promise<RatedGame[]> {
  const out: RatedGame[] = [];
  for (let offset = 0; ; offset += 500) {
    const page = await igdb<RatedGame[]>(auth, "games", `${RATED_FIELDS} where ${where}; limit 500; offset ${offset};`);
    out.push(...page);
    await sleep(300);
    if (page.length < 500) return out;
  }
}

/**
 * Current rating and review count of each game, by IGDB id. IGDB sometimes
 * splits one game into entries with the same name (PS1 Final Fantasy VII is
 * a "main game" with 10 reviews and an "expanded game" with 1,600), so this
 * uses whichever same-named relative (parent, or version of it) has the most.
 */
export async function igdbRatings(
  auth: Igdb,
  ids: number[],
): Promise<Map<number, { rating?: number; ratingCount?: number }>> {
  const out = new Map<number, { rating?: number; ratingCount?: number }>();
  const same = (a: RatedGame, b: RatedGame) => a.name.trim().toLowerCase() === b.name.trim().toLowerCase();
  for (let i = 0; i < ids.length; i += 500) {
    const batch = ids.slice(i, i + 500);
    const games = await igdbAll(auth, `id = (${batch.join(",")})`);
    const children = await igdbAll(auth, `parent_game = (${batch.join(",")}) | version_parent = (${batch.join(",")})`);
    const parentIds = [...new Set(games.flatMap((g) => [g.parent_game, g.version_parent]).filter((p): p is number => !!p))];
    const parents = new Map<number, RatedGame>();
    for (let j = 0; j < parentIds.length; j += 500) {
      for (const p of await igdbAll(auth, `id = (${parentIds.slice(j, j + 500).join(",")})`)) parents.set(p.id, p);
    }
    for (const g of games) {
      const family = [
        g,
        ...children.filter((c) => c.parent_game === g.id || c.version_parent === g.id),
        ...[g.parent_game, g.version_parent].map((p) => (p ? parents.get(p) : undefined)).filter((p) => !!p),
      ].filter((r) => same(r, g) && r.total_rating);
      const best = family.sort((a, b) => (b.total_rating_count ?? 0) - (a.total_rating_count ?? 0))[0];
      out.set(g.id, best ? { rating: Math.round(best.total_rating!), ratingCount: best.total_rating_count ?? 0 } : {});
    }
  }
  return out;
}
