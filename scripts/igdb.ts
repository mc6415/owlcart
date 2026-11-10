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
      if (s > bestScore) [best, bestScore] = [g, s];
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

/** Current rating and review count of each game, by IGDB id (500 per request). */
export async function igdbRatings(
  auth: Igdb,
  ids: number[],
): Promise<Map<number, { rating?: number; ratingCount?: number }>> {
  const out = new Map<number, { rating?: number; ratingCount?: number }>();
  for (let i = 0; i < ids.length; i += 500) {
    const batch = ids.slice(i, i + 500);
    const games = await igdb<{ id: number; total_rating?: number; total_rating_count?: number }[]>(
      auth,
      "games",
      `fields total_rating,total_rating_count; where id = (${batch.join(",")}); limit 500;`,
    );
    for (const g of games) {
      out.set(g.id, g.total_rating ? { rating: Math.round(g.total_rating), ratingCount: g.total_rating_count ?? 0 } : {});
    }
    await sleep(300);
  }
  return out;
}
