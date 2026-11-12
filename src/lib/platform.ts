// Platform checks for the bits of the UI that only make sense on desktop.

/** Running inside the Android app. */
export const isAndroid = typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);

/**
 * How much of a scrolling list is actually on screen. On Android the page
 * runs past the bottom of the screen by the header's height while the header
 * shows (see +page.svelte), so the list's own height overstates it.
 */
/**
 * Svelte action: lets go of focus after each tap or click inside the element.
 * A tapped card (or its select / download button) would otherwise keep focus,
 * so a later Enter or controller A would act on it rather than on the game
 * highlighted since.
 */
export function dropFocusOnTap(el: HTMLElement) {
  const up = () => {
    const f = document.activeElement;
    if (f instanceof HTMLElement && f !== el && el.contains(f)) f.blur();
  };
  el.addEventListener("pointerup", up);
  return { destroy: () => el.removeEventListener("pointerup", up) };
}

export function onScreenHeight(el: HTMLElement): number {
  return Math.max(0, Math.min(el.clientHeight, window.innerHeight - el.getBoundingClientRect().top));
}
