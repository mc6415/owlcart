// Helpers shared by the build-* scripts.

import { parseName } from "../src/lib/names.ts";

export function arg(name: string): string | undefined {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
}

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Distinct game titles on a Minerva listing (same rules as the app). */
export async function listingTitles(url: string): Promise<string[]> {
  const res = await fetch(url, { headers: { "User-Agent": "owlcart-scripts" } });
  if (!res.ok) throw new Error(`Fetching listing failed: ${res.status}`);
  const html = await res.text();
  const titles = new Set<string>();
  // Game entries link to /rom?id= and have a magnet link.
  for (const block of html.split('<div class="entry" data-name=').slice(1)) {
    if (!block.includes("magnet:")) continue;
    const m = block.match(/href="\/rom\?id=\d+"[^>]*>([^<]+)</);
    if (!m) continue;
    const name = m[1]
      .replaceAll("&amp;", "&")
      .replaceAll("&#39;", "'")
      .replaceAll("&quot;", '"')
      .replaceAll("&lt;", "<")
      .replaceAll("&gt;", ">");
    titles.add(parseName(name).title);
  }
  return [...titles];
}
