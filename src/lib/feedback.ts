// Little touches of feedback: rumble (Android, on by default) and soft menu
// sounds (off by default), both switched in Settings. Sounds are made with
// Web Audio, so there are no sound files to ship.
import { api } from "./api";
import { isAndroid } from "./platform";

export type Feel = "move" | "open" | "back" | "download" | "done" | "switch";

let haptics = true;
let sounds = false;

export function setFeedback(o: { haptics: boolean; sounds: boolean }) {
  haptics = o.haptics;
  sounds = o.sounds;
}

/** [milliseconds, strength 1-255] pulses, played one after another. */
const RUMBLE: Record<Feel, [number, number][]> = {
  move: [[8, 40]],
  open: [[14, 90]],
  back: [[10, 60]],
  switch: [[18, 110]],
  download: [[30, 160]],
  done: [
    [35, 200],
    [35, 200],
  ],
};

/** [frequency Hz, start s, length s] notes. */
const NOTES: Record<Feel, [number, number, number][]> = {
  move: [[1400, 0, 0.03]],
  open: [
    [660, 0, 0.06],
    [990, 0.05, 0.08],
  ],
  back: [
    [740, 0, 0.06],
    [500, 0.05, 0.08],
  ],
  switch: [
    [520, 0, 0.05],
    [780, 0.04, 0.05],
    [1040, 0.08, 0.07],
  ],
  download: [
    [880, 0, 0.07],
    [1320, 0.07, 0.12],
  ],
  done: [
    [1046, 0, 0.1],
    [1318, 0.09, 0.1],
    [1568, 0.18, 0.22],
  ],
};

let ctx: AudioContext | null = null;
// Moving quickly through the grid shouldn't buzz or click on every step.
let lastMove = 0;

export function feel(kind: Feel) {
  if (kind === "move") {
    const now = performance.now();
    if (now - lastMove < 60) return;
    lastMove = now;
  }
  if (haptics && isAndroid) rumble(RUMBLE[kind]);
  if (sounds) play(NOTES[kind], kind === "move" ? 0.04 : 0.08);
}

function rumble(pulses: [number, number][]) {
  let at = 0;
  for (const [ms, strength] of pulses) {
    setTimeout(() => api.rumble(ms, strength).catch(() => {}), at);
    at += ms + 60;
  }
}

function play(notes: [number, number, number][], volume: number) {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    const t0 = ctx.currentTime + 0.01;
    for (const [freq, start, len] of notes) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = freq;
      // A quick rise and a soft tail, so notes don't click.
      gain.gain.setValueAtTime(0, t0 + start);
      gain.gain.linearRampToValueAtTime(volume, t0 + start + 0.008);
      gain.gain.exponentialRampToValueAtTime(0.0001, t0 + start + len);
      osc.connect(gain).connect(ctx.destination);
      osc.start(t0 + start);
      osc.stop(t0 + start + len + 0.02);
    }
  } catch {
    // No audio available: stay quiet.
  }
}
