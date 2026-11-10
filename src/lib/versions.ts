// Picks which version of a game to show (and download with one tap) when a
// title has several: different regions, revisions, demos and so on.

import { isExtra, parseName, type ParsedName } from "./names.ts";

/** Region names to prefer for each country, before the general fallbacks. */
const BY_COUNTRY: Record<string, string[]> = {
  US: ["USA"],
  CA: ["Canada", "USA"],
  GB: ["UK", "Europe"],
  IE: ["Ireland", "UK", "Europe"],
  AU: ["Australia", "Europe"],
  NZ: ["New Zealand", "Australia", "Europe"],
  JP: ["Japan"],
  KR: ["Korea", "Asia"],
  CN: ["China", "Asia"],
  TW: ["Taiwan", "Asia"],
  HK: ["Hong Kong", "Asia"],
  BR: ["Brazil", "USA"],
  MX: ["Mexico", "Latin America", "USA"],
  DE: ["Germany", "Europe"],
  AT: ["Austria", "Germany", "Europe"],
  CH: ["Switzerland", "Europe"],
  FR: ["France", "Europe"],
  BE: ["Belgium", "Europe"],
  NL: ["Netherlands", "Europe"],
  ES: ["Spain", "Europe"],
  PT: ["Portugal", "Europe"],
  IT: ["Italy", "Europe"],
  SE: ["Sweden", "Scandinavia", "Europe"],
  NO: ["Norway", "Scandinavia", "Europe"],
  DK: ["Denmark", "Scandinavia", "Europe"],
  FI: ["Finland", "Scandinavia", "Europe"],
  PL: ["Poland", "Europe"],
  RU: ["Russia", "Europe"],
};
const EUROPE = new Set(["GR", "CZ", "HU", "RO", "BG", "SK", "SI", "HR", "RS", "EE", "LV", "LT", "IS", "UA", "TR"]);

/** The device's preferred regions, from its locale. */
function localeRegions(): string[] {
  const langs = typeof navigator === "undefined" ? [] : navigator.languages ?? [navigator.language];
  const out: string[] = [];
  for (const l of langs) {
    const country = l.split("-")[1]?.toUpperCase();
    if (!country) continue;
    out.push(...(BY_COUNTRY[country] ?? (EUROPE.has(country) ? ["Europe"] : [])));
  }
  return out;
}

/**
 * Regions in order of preference: the ones picked in the region filter, then
 * the device's locale, then games released everywhere, then the big three.
 */
export function preferredRegions(picked: Iterable<string>): string[] {
  return [...new Set([...picked, ...localeRegions(), "World", "USA", "Europe", "Japan"])];
}

/** "Rev 2" -> 2, "Rev B" -> 2, "v1.1" -> 1.1, none -> 0. Later revisions fix bugs, so they're preferred. */
function revision(p: ParsedName): number {
  for (const tag of p.tags) {
    const t = tag.trim();
    const num = /^(?:Rev\s*|v)(\d+(?:\.\d+)?)$/i.exec(t);
    if (num) return Number(num[1]);
    const letter = /^Rev\s*([A-Z])$/i.exec(t);
    if (letter) return letter[1].toUpperCase().charCodeAt(0) - 64;
  }
  return 0;
}

/** Sort key for one version: lower is better. */
function rank(name: string, prefs: string[]) {
  const p = parseName(name);
  const region = Math.min(prefs.length, ...p.regions.map((r) => (prefs.includes(r) ? prefs.indexOf(r) : prefs.length)));
  return { extra: isExtra(p) ? 1 : 0, region, rev: -revision(p), tags: p.tags.length };
}

/** Orders a title's versions best first: full games, preferred region, latest revision, fewest extra tags. */
export function compareVersions(a: { name: string }, b: { name: string }, prefs: string[]): number {
  const x = rank(a.name, prefs);
  const y = rank(b.name, prefs);
  return x.extra - y.extra || x.region - y.region || x.rev - y.rev || x.tags - y.tags || a.name.localeCompare(b.name);
}
