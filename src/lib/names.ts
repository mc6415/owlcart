// Plain TypeScript with no imports: shared by the app and scripts/build-covers.ts.

export interface ParsedName {
  title: string;
  tags: string[];
  regions: string[];
}

/** Region names used in No-Intro / Redump file names. */
const REGIONS = new Set([
  "World", "USA", "Europe", "Japan", "Asia", "Australia", "Brazil", "Canada", "China", "France",
  "Germany", "Hong Kong", "Italy", "Korea", "Netherlands", "Spain", "Sweden", "UK", "Austria",
  "Belgium", "Denmark", "Finland", "Greece", "India", "Ireland", "Latin America", "Mexico",
  "New Zealand", "Norway", "Poland", "Portugal", "Russia", "Scandinavia", "South Africa",
  "Switzerland", "Taiwan", "United Arab Emirates", "Argentina", "Chile", "Croatia", "Czech",
  "Hungary", "Israel", "Turkey", "Ukraine", "Singapore", "Thailand", "Indonesia", "Malaysia",
  "Philippines", "Vietnam", "Saudi Arabia", "Iceland", "Romania", "Bulgaria", "Serbia", "Slovakia",
  "Slovenia", "Estonia", "Latvia", "Lithuania", "Peru", "Colombia", "Unknown",
]);

/**
 * Splits a No-Intro/Redump style name.
 * "Halo 3 (USA, Europe) (En,Fr).zip" -> title "Halo 3", tags ["USA, Europe", "En,Fr"],
 * regions ["USA", "Europe"]. The regions are the first bracketed group made up only of
 * known region names.
 */
export function parseName(name: string): ParsedName {
  const base = name.replace(/\.(zip|7z|rar|chd|iso)$/i, "");
  const i = base.indexOf(" (");
  if (i < 0) return { title: base, tags: [], regions: [] };
  const tags = [...base.slice(i).matchAll(/\(([^)]*)\)/g)].map((m) => m[1]);
  let regions: string[] = [];
  for (const t of tags) {
    const parts = t.split(/,\s*/);
    if (parts.every((p) => REGIONS.has(p))) {
      regions = parts;
      break;
    }
  }
  return { title: base.slice(0, i), tags, regions };
}

/**
 * Tags that mark something other than a finished game: demos, betas and
 * prototypes, test programs, bootlegs and bonus discs. Unlicensed (Unl) and
 * homebrew (Aftermarket) releases are real games, so they're not included.
 */
const EXTRA_TAG =
  /^(demo|trade demo|rolling demo|kiosk|taikenban|sample|trial|preview|promo|beta|alpha|proto|prototype|build|debug|program|test program|music program|pirate|bonus disc)\b/i;

/** True for demos, betas, prototypes, BIOS files and other non-games. */
export function isExtra(p: ParsedName): boolean {
  return p.title.startsWith("[BIOS]") || p.tags.some((t) => EXTRA_TAG.test(t.trim()));
}

/** File name without its last extension: "Halo 3 (USA).zip" -> "Halo 3 (USA)". */
export function fileStem(name: string): string {
  return name.replace(/\.[^./\\]+$/, "");
}

/**
 * File name for a listing's cover manifest, from its /browse/ URL.
 * ".../browse/Redump/Microsoft%20-%20Xbox%20360/" -> "redump-microsoft-xbox-360"
 */
export function listingSlug(listingUrl: string): string {
  const path = decodeURIComponent(new URL(listingUrl).pathname).replace(/^\/browse\/(\.\/)?/, "");
  return path
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/**
 * Turns a Redump title into a search query:
 * "Adventures of Tintin, The - The Game" -> "The Adventures of Tintin: The Game"
 */
export function searchQuery(title: string): string {
  return title
    .split(" - ")
    .map((part) => part.replace(/^(.*), (The|A|An)$/, "$2 $1"))
    .join(": ");
}

function words(s: string): string[] {
  return s
    .toLowerCase()
    .replace(/&/g, " and ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim()
    .split(" ")
    .filter(Boolean);
}

/**
 * How well a search result name matches a query, 0..1 (Dice overlap of words).
 * Returns 0 if the numbers differ, so "Wipeout 2" never matches "Wipeout".
 */
export function matchScore(query: string, candidate: string): number {
  const a = words(query);
  const b = words(candidate);
  if (!a.length || !b.length) return 0;
  const nums = (w: string[]) => w.filter((x) => /^\d+$/.test(x)).sort().join(" ");
  if (nums(a) !== nums(b)) return 0;
  const setB = new Set(b);
  const shared = new Set(a.filter((w) => setB.has(w))).size;
  return (2 * shared) / (new Set(a).size + setB.size);
}

/** Minimum matchScore to accept a search result as the right game. */
export const MATCH_THRESHOLD = 0.85;

/**
 * Which details bucket (00-ff) a title lives in: low byte of the FNV-1a hash
 * of its UTF-8 bytes. Must match `title_bucket` in src-tauri/src/data.rs.
 */
export function titleBucket(title: string): string {
  let h = 0x811c9dc5;
  for (const b of new TextEncoder().encode(title)) {
    h ^= b;
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return (h & 0xff).toString(16).padStart(2, "0");
}

/** Expands a compact cover reference from covers.json into an image URL. */
export function coverUrl(ref: string): string {
  if (ref.startsWith("s:")) return `https://cdn2.steamgriddb.com/${ref.slice(2)}`;
  if (ref.startsWith("i:")) return `https://images.igdb.com/igdb/image/upload/t_cover_big/${ref.slice(2)}.jpg`;
  return ref; // a full URL (e.g. a hand-written override)
}
