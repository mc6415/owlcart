// Platform checks for the bits of the UI that only make sense on desktop.

/** Running inside the Android app. */
export const isAndroid = typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
