import { api } from "./api";

/** Bumped when the API key changes so visible covers look themselves up again. */
export const coverState = $state({ version: 0 });

const cache = new Map<string, Promise<string | null>>();
/** Answers already received, so remounted cards can show them instantly. */
const resolved = new Map<string, string | null>();
/** Image URLs that have finished loading at least once this session. */
export const loadedImages = new Set<string>();

/** The cover if it's already known (undefined = not looked up yet). */
export function knownCover(title: string, slug: string): string | null | undefined {
  return resolved.get(`${slug}\n${title}`);
}

/** Cover image URL for a game title, or null for the placeholder. */
export function coverFor(title: string, slug: string): Promise<string | null> {
  const k = `${slug}\n${title}`;
  let p = cache.get(k);
  if (!p) {
    p = api
      .coverArt(title, slug)
      .then((u) => {
        resolved.set(k, u);
        return u;
      })
      .catch((e) => {
        console.warn("cover lookup failed", title, e);
        cache.delete(k); // not remembered, so it's retried later
        return null;
      });
    cache.set(k, p);
  }
  return p;
}

export function resetCovers() {
  cache.clear();
  resolved.clear();
  coverState.version++;
}
