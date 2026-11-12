// Builds the app's published data (covers + game details) for every console
// in consoles.json. Run it again any time: only titles not looked up yet are
// fetched, and hand edits to the JSON are kept.
//
//   npm run data -- [console...] [--retry-missing] [--limit N] [--verbose]
//                   [--no-covers] [--no-details] [--out data]
//   npm run data -- [console...] --refresh-ratings
//   npm run data -- [console...] --libretro
//
// With no console named, updates every console that already has data. Name
// consoles (their ES-DE folder, or part of the slug or name, e.g.
// `npm run data -- gba saturn`) to
// add them, or `all` for everything in consoles.json.
// --refresh-ratings only re-reads the IGDB rating and review count of games
// already matched (500 per request), without looking anything else up.
// Covers come from libretro's thumbnails first (right platform, matched by
// name; see libretro.ts), then SteamGridDB, then IGDB. --libretro also
// switches existing covers to libretro's wherever it has one.
// Keys: .steamgriddb-key and .igdb-credentials in the project root (git-ignored).
//
// Output layout (host the folder as-is):
//   data/index.json                      consoles, update times, counts
//   data/<slug>/covers.json              title -> cover ref ("l:..." libretro, "s:..." SteamGridDB, "i:..." IGDB) or null
//   data/<slug>/games.json               title -> [IGDB id] or [IGDB id, rating 0-100, review count], matched
//                                        titles only: lets the app group and sort a whole console at once
//   data/<slug>/details/<00-ff>.json     title -> IGDB details or null, bucketed by titleBucket()

import { readFile, writeFile, mkdir, readdir } from "node:fs/promises";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isExtra, listingSlug, parseName, titleBucket } from "../src/lib/names.ts";
import { arg, sleep, listingNames } from "./common.ts";
import { sgdbKey, sgdbCover } from "./sgdb.ts";
import { libretroIndex, libretroCover } from "./libretro.ts";
import {
  igdbLogin,
  igdbPlatform,
  igdbDetails,
  igdbDetailsMany,
  igdbRatings,
  IGDB_BATCH,
  type GameDetails,
  type Igdb,
} from "./igdb.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const flag = (name: string) => process.argv.includes(name);
const verbose = flag("--verbose");

interface ConsoleConfig {
  /** Shown in the app's console picker, grouped by `group`. */
  name: string;
  group: string;
  /** Download folder name (ES-DE system name, e.g. "psx"). */
  folder?: string;
  listing: string;
  /** IGDB platform slug, needed to build game details. */
  igdb?: string;
  /** libretro thumbnails folder, e.g. "Microsoft - Xbox", for box art. */
  libretro?: string;
}

interface IndexEntry {
  name: string;
  group: string;
  folder?: string;
  listing: string;
  igdb: string;
  updated: string;
  titles: number;
  covers: number;
  details: number;
}

interface Index {
  version: 1;
  consoles: Record<string, IndexEntry>;
}

async function readJson<T>(file: string, fallback: T): Promise<T> {
  try {
    return JSON.parse(await readFile(file, "utf8")) as T;
  } catch {
    return fallback;
  }
}

const sortKeys = <T>(o: Record<string, T>) =>
  Object.fromEntries(Object.entries(o).sort(([a], [b]) => a.localeCompare(b)));

/** Runs `fn` over `items` with `workers` in parallel, pausing `pauseMs` between calls. */
async function pool<T>(items: T[], workers: number, pauseMs: number, fn: (item: T) => Promise<void>) {
  let next = 0;
  await Promise.all(
    Array.from({ length: workers }, async () => {
      while (next < items.length) {
        await fn(items[next++]);
        await sleep(pauseMs);
      }
    }),
  );
}

async function buildConsole(
  cfg: ConsoleConfig,
  outDir: string,
  index: Index,
  keys: { sgdb?: string; igdb?: Igdb },
) {
  const slug = listingSlug(cfg.listing);
  const name = cfg.name;
  const dir = join(outDir, slug);
  const detailsDir = join(dir, "details");
  await mkdir(detailsDir, { recursive: true });

  // Load what's already there.
  const covers = await readJson<Record<string, string | null>>(join(dir, "covers.json"), {});
  const details: Record<string, GameDetails | null> = {};
  for (const f of (await readdir(detailsDir)).filter((f) => f.endsWith(".json"))) {
    Object.assign(details, await readJson(join(detailsDir, f), {}));
  }

  const names = await listingNames(cfg.listing);
  const titles = [...new Set(names.map((n) => parseName(n).title))];
  // Titles that are only demos, betas, prototypes, BIOS files and the like:
  // IGDB almost never has them, so retries skip them.
  const real = new Set(names.map(parseName).filter((p) => !isExtra(p)).map((p) => p.title));
  const limit = Number(arg("--limit") ?? Infinity);
  const retry = flag("--retry-missing");
  const detailsTodo = keys.igdb && cfg.igdb ? titles.filter((t) => !(t in details)).slice(0, limit) : [];
  // Retries go in batches (igdbDetailsMany), which is much faster.
  const retryTodo =
    keys.igdb && cfg.igdb && retry
      ? titles.filter((t) => details[t] === null && real.has(t)).slice(0, limit)
      : [];
  // libretro first: no key needed, one request for the whole console.
  let fromLibretro = 0;
  if (cfg.libretro && !flag("--no-covers")) {
    try {
      const index = await libretroIndex(cfg.libretro);
      const all = flag("--libretro");
      for (const t of titles) {
        const c = covers[t];
        // New titles and ones with no cover or only IGDB's; with --libretro, any.
        if (t in covers && c && !c.startsWith("i:") && !(all && !c.startsWith("l:"))) continue;
        const ref = libretroCover(index, t);
        if (ref && ref !== c) {
          covers[t] = ref;
          fromLibretro++;
        }
      }
    } catch (e) {
      console.warn(`  [libretro] ${e}`);
    }
  }
  const coversTodo = keys.sgdb
    ? titles
        .filter((t) => !(t in covers) || (retry && (covers[t] === null || covers[t]?.startsWith("i:"))))
        .slice(0, limit)
    : [];
  console.log(
    `\n${name} (${slug}): ${titles.length} titles; ${fromLibretro} covers from libretro; ${detailsTodo.length} details, ${retryTodo.length} retries and ${coversTodo.length} covers to look up`,
  );

  let changed = fromLibretro > 0;
  const save = async () => {
    // Use IGDB's cover where SteamGridDB has none.
    for (const t of titles) {
      if ((covers[t] === null || covers[t] === undefined) && t in covers && details[t]?.cover) {
        covers[t] = `i:${details[t]!.cover}`;
      }
    }
    await writeFile(join(dir, "covers.json"), JSON.stringify(sortKeys(covers)) + "\n");
    await writeFile(join(dir, "games.json"), JSON.stringify(gamesOf(details)) + "\n");
    const buckets: Record<string, Record<string, GameDetails | null>> = {};
    for (const [t, d] of Object.entries(details)) (buckets[titleBucket(t)] ??= {})[t] = d;
    for (const [b, entries] of Object.entries(buckets)) {
      await writeFile(join(detailsDir, `${b}.json`), JSON.stringify(sortKeys(entries)) + "\n");
    }
    const prev = index.consoles[slug];
    index.consoles[slug] = {
      name,
      group: cfg.group,
      folder: cfg.folder,
      listing: cfg.listing,
      igdb: cfg.igdb ?? "",
      // Bumping this tells the app to re-download this console's files.
      updated: changed || !prev ? new Date().toISOString() : prev.updated,
      titles: titles.length,
      covers: titles.filter((t) => covers[t]).length,
      details: titles.filter((t) => details[t]).length,
    };
    await writeFile(join(outDir, "index.json"), JSON.stringify({ ...index, consoles: sortKeys(index.consoles) }, null, 1) + "\n");
  };

  let done = 0;
  const total = detailsTodo.length + retryTodo.length + coversTodo.length;
  const tick = async (n = 1) => {
    changed = true;
    const before = done;
    done += n;
    if (Math.floor(done / 50) > Math.floor(before / 50)) {
      console.log(`  ${done}/${total}`);
      await save();
    }
  };

  // IGDB and SteamGridDB are separate services, so run both at once.
  const tasks: Promise<void>[] = [];
  if (keys.igdb && cfg.igdb && (detailsTodo.length || retryTodo.length)) {
    const plat = await igdbPlatform(keys.igdb, cfg.igdb);
    const batches: string[][] = [];
    for (let i = 0; i < retryTodo.length; i += IGDB_BATCH) batches.push(retryTodo.slice(i, i + IGDB_BATCH));
    tasks.push(
      (async () => {
        // One batch at a time: a batch is already 10 queries, IGDB's limit
        // for queries in flight at once.
        await pool(batches, 1, 250, async (batch) => {
          try {
            for (const [t, d] of await igdbDetailsMany(keys.igdb!, plat.id, batch, verbose)) details[t] = d;
          } catch (e) {
            console.warn(`  [igdb] batch failed: ${batch.join(", ")}: ${e}`);
          }
          await tick(batch.length);
        });
        // IGDB allows 4 requests/second; stay well under.
        await pool(detailsTodo, 2, 600, async (t) => {
          try {
            details[t] = await igdbDetails(keys.igdb!, plat.id, t, verbose);
          } catch (e) {
            console.warn(`  [igdb] failed: ${t}: ${e}`);
          }
          await tick();
        });
      })(),
    );
  }
  if (keys.sgdb && coversTodo.length) {
    tasks.push(
      pool(coversTodo, 2, 250, async (t) => {
        try {
          covers[t] = await sgdbCover(t, keys.sgdb!, verbose);
        } catch (e) {
          if (String(e).includes("401")) throw e;
          console.warn(`  [sgdb] failed: ${t}: ${e}`);
        }
        await tick();
      }),
    );
  }
  await Promise.all(tasks);
  await save();

  const e = index.consoles[slug];
  console.log(`  ${name}: ${e.covers}/${e.titles} covers, ${e.details}/${e.titles} details`);
}

async function main() {
  const outDir = resolve(root, arg("--out") ?? "data");
  const configs = await readJson<ConsoleConfig[]>(join(root, "consoles.json"), []);
  const valueFlags = new Set(["--limit", "--out"]);
  const filters = process.argv
    .slice(2)
    .filter((a, i, all) => !a.startsWith("--") && !valueFlags.has(all[i - 1]))
    .map((f) => f.toLowerCase());
  const index = await readJson<Index>(join(outDir, "index.json"), { version: 1, consoles: {} });
  // With no filter, update the consoles that already have data. Name a console
  // (e.g. `npm run data -- ps2`) to add it; `all` builds every console listed.
  const selected = configs.filter((c) => {
    const slug = listingSlug(c.listing);
    if (!filters.length) return slug in index.consoles;
    return (
      filters.includes("all") ||
      filters.some((f) => f === c.folder?.toLowerCase() || slug.includes(f) || c.name.toLowerCase().includes(f))
    );
  });
  if (!selected.length) {
    throw new Error(filters.length ? "No consoles in consoles.json match that filter" : "No consoles built yet; name one, e.g. npm run data -- ps2");
  }

  const keys = {
    sgdb: flag("--no-covers") ? undefined : await sgdbKey(root),
    igdb: flag("--no-details") ? undefined : await igdbLogin(root),
  };
  await mkdir(outDir, { recursive: true });

  if (flag("--refresh-ratings")) {
    if (!keys.igdb) throw new Error("--refresh-ratings needs IGDB (drop --no-details)");
    for (const c of selected) await refreshRatings(c, outDir, index, keys.igdb);
    return;
  }
  for (const c of selected) await buildConsole(c, outDir, index, keys);
}

main().catch((e) => {
  console.error(String(e));
  process.exit(1);
});

/**
 * Title -> [IGDB id] or [IGDB id, rating, review count] for every matched
 * title: the whole console in one small file, so the app can merge titles
 * that are the same game (e.g. "Biohazard 2" and "Resident Evil 2") and sort
 * by rating without loading every game's details.
 */
export function gamesOf(details: Record<string, GameDetails | null>): Record<string, number[]> {
  const out: Record<string, number[]> = {};
  for (const t of Object.keys(details).sort()) {
    const d = details[t];
    if (!d) continue;
    out[t] = d.rating != null ? [d.igdb, d.rating, d.ratingCount ?? 0] : [d.igdb];
  }
  return out;
}

/** Updates the ratings and review counts of a console's matched games. */
async function refreshRatings(cfg: ConsoleConfig, outDir: string, index: Index, igdb: Igdb) {
  const slug = listingSlug(cfg.listing);
  const detailsDir = join(outDir, slug, "details");
  const files = (await readdir(detailsDir).catch(() => [])).filter((f) => f.endsWith(".json"));
  if (!files.length) return console.log(`${cfg.name}: no details yet, skipped`);

  const buckets: Record<string, Record<string, GameDetails | null>> = {};
  for (const f of files) buckets[f] = await readJson(join(detailsDir, f), {});
  const all = Object.values(buckets).flatMap((b) => Object.values(b)).filter((d): d is GameDetails => !!d);
  const latest = await igdbRatings(igdb, [...new Set(all.map((d) => d.igdb))]);

  let changed = 0;
  for (const d of all) {
    const r = latest.get(d.igdb);
    if (!r) continue; // not returned (removed from IGDB?): keep what we had
    if (d.rating !== r.rating || d.ratingCount !== r.ratingCount) changed++;
    if (r.rating == null) {
      delete d.rating;
      delete d.ratingCount;
    } else {
      d.rating = r.rating;
      d.ratingCount = r.ratingCount;
    }
  }
  for (const [f, entries] of Object.entries(buckets)) {
    await writeFile(join(detailsDir, f), JSON.stringify(sortKeys(entries)) + "\n");
  }
  const details = Object.assign({}, ...Object.values(buckets)) as Record<string, GameDetails | null>;
  await writeFile(join(outDir, slug, "games.json"), JSON.stringify(gamesOf(details)) + "\n");
  // Bumping this tells the app to re-download this console's files.
  if (changed && index.consoles[slug]) {
    index.consoles[slug].updated = new Date().toISOString();
    await writeFile(join(outDir, "index.json"), JSON.stringify({ ...index, consoles: sortKeys(index.consoles) }, null, 1) + "\n");
  }
  console.log(`${cfg.name}: ${all.length} games, ${changed} ratings updated`);
}
