/** Colour band for an IGDB rating (0-100): green, amber or red, like review sites. */
export function ratingTier(r: number): "good" | "mid" | "low" {
  return r >= 75 ? "good" : r >= 50 ? "mid" : "low";
}
