// Shared bits for the app's motion: the reduced-motion check and the
// "fly into the downloads button" effect.

export const reduceMotion =
  typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Easing used for the bigger, "physical" movements. */
export const EASE_OUT = "cubic-bezier(0.2, 0.8, 0.2, 1)";

/**
 * Sends a small copy of a cover (or a dot, without an image) from `from` into
 * the downloads button in the title bar, along a gentle arc.
 */
export function flyToDownloads(from: DOMRect, imgSrc?: string | null, delay = 0) {
  if (reduceMotion) return;
  const target = document.querySelector<HTMLElement>("[data-downloads-button]");
  if (!target) return;
  const to = target.getBoundingClientRect();

  const el = document.createElement(imgSrc ? "img" : "div");
  if (imgSrc && el instanceof HTMLImageElement) el.src = imgSrc;
  // Start as a cover-shaped card (or a dot) over the source.
  const w = imgSrc ? Math.min(from.width, 120) : 14;
  const h = imgSrc ? w * 1.5 : 14;
  const x0 = from.left + from.width / 2 - w / 2;
  const y0 = from.top + from.height / 2 - h / 2;
  Object.assign(el.style, {
    position: "fixed",
    left: `${x0}px`,
    top: `${y0}px`,
    width: `${w}px`,
    height: `${h}px`,
    objectFit: "cover",
    borderRadius: imgSrc ? "8px" : "50%",
    background: "var(--grad)",
    boxShadow: "0 10px 30px rgb(0 0 0 / 0.45)",
    zIndex: "1000",
    pointerEvents: "none",
  });
  document.body.appendChild(el);

  const dx = to.left + to.width / 2 - (x0 + w / 2);
  const dy = to.top + to.height / 2 - (y0 + h / 2);
  const end = 18 / w; // shrink to roughly the button's icon size
  const anim = el.animate(
    [
      { transform: "translate(0, 0) scale(1) rotate(0deg)", opacity: 1 },
      // Rise and drift on the way, so it reads as a throw rather than a slide.
      { transform: `translate(${dx * 0.45}px, ${dy * 0.45 - 80}px) scale(${(1 + end) / 2}) rotate(-6deg)`, opacity: 1, offset: 0.5 },
      { transform: `translate(${dx}px, ${dy}px) scale(${end}) rotate(0deg)`, opacity: 0.2 },
    ],
    { duration: 700, delay, easing: "cubic-bezier(0.5, 0, 0.3, 1)", fill: "forwards" },
  );
  anim.onfinish = () => {
    el.remove();
    target.animate(
      [{ transform: "scale(1)" }, { transform: "scale(1.22)" }, { transform: "scale(1)" }],
      { duration: 380, easing: EASE_OUT },
    );
  };
}
