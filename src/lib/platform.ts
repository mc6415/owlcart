// Platform checks for the bits of the UI that only make sense on desktop.

/** Running inside the Android app. */
export const isAndroid = typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);

/**
 * How much of a scrolling list is actually on screen. On Android the page
 * runs past the bottom of the screen by the header's height while the header
 * shows (see +page.svelte), so the list's own height overstates it.
 */
export function onScreenHeight(el: HTMLElement): number {
  return Math.max(0, Math.min(el.clientHeight, window.innerHeight - el.getBoundingClientRect().top));
}
