import { invoke } from "@tauri-apps/api/core";

export type EntryKind = "folder" | "file";

export interface Entry {
  kind: EntryKind;
  name: string;
  size: string;
  sizeBytes: number;
  url: string;
  romId: number | null;
}

export interface Listing {
  url: string;
  crumbs: { name: string; url: string }[];
  parent: string | null;
  entries: Entry[];
}

export type JobState =
  | "resolving"
  | "queued"
  | "starting"
  | "downloading"
  | "paused"
  | "moving"
  | "extracting"
  | "done"
  | "failed"
  | "cancelled";

export interface Job {
  id: number;
  name: string;
  romUrl: string;
  state: JobState;
  total: number;
  completed: number;
  speed: number;
  peers: number;
  error: string | null;
  output: string | null;
}

export interface FetchStatus {
  running: boolean;
  console: string;
  done: number;
  total: number;
  covers: number;
  details: number;
  titles: number;
  queued: string[];
  message: string | null;
}

export interface Settings {
  downloadDir: string;
  maxActive: number;
  steamgriddbKey: string;
  igdbClientId: string;
  igdbClientSecret: string;
  perConsoleFolders: boolean;
  hideWithoutArt: boolean;
  fetchDir: string;
  extractArchives: boolean;
  deleteArchives: boolean;
  dataUrl: string;
}

export const api = {
  engineStatus: () => invoke<{ ok: boolean; error: string | null }>("engine_status"),
  loadListing: (url: string) => invoke<Listing>("load_listing", { url }),
  /** `folder` names the console's download subfolder (e.g. "psx"); omit to use the archive's name. */
  enqueue: (items: { name: string; url: string; folder?: string }[]) => invoke<void>("enqueue", { items }),
  listJobs: () => invoke<Job[]>("list_jobs"),
  pause: (id: number) => invoke<void>("pause_job", { id }),
  resume: (id: number) => invoke<void>("resume_job", { id }),
  cancel: (id: number) => invoke<void>("cancel_job", { id }),
  retry: (id: number) => invoke<void>("retry_job", { id }),
  clearFinished: () => invoke<void>("clear_finished"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  openDownloads: () => invoke<void>("open_downloads"),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  startFetch: (consoles: { name: string; group: string; folder?: string; listing: string; igdb?: string }[]) =>
    invoke<void>("start_fetch", { consoles }),
  cancelFetch: () => invoke<void>("cancel_fetch"),
  fetchStatus: () => invoke<FetchStatus>("fetch_status"),
  /** Games already in the download folder: file name without extension -> path. */
  downloaded: (folder?: string, archiveFolder?: string) =>
    invoke<Record<string, string>>("downloaded", { folder, archiveFolder }),
  /** Titles with cover art on a console, or null if it has no cover data. */
  coverTitles: (slug: string) => invoke<string[] | null>("cover_titles", { slug }),
  coverArt: (title: string, slug: string) => invoke<string | null>("cover_art", { title, slug }),
  consoleIndex: () =>
    invoke<Record<string, { name: string; group?: string; folder?: string; igdb?: string; listing: string; titles: number; covers: number; details: number }> | null>(
      "console_index",
    ),
};

export function formatBytes(n: number): string {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), units.length - 1);
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${units[i]}`;
}

export function formatEta(remaining: number, speed: number): string {
  if (!speed) return "";
  const s = Math.round(remaining / speed);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

export { parseName, listingSlug, fileStem, isExtra, type ParsedName } from "./names.ts";

/** IGDB details from the published <slug>.details.json manifest. */
export interface GameDetails {
  igdb: number;
  name: string;
  summary?: string;
  released?: string;
  genres?: string[];
  developers?: string[];
  publishers?: string[];
  rating?: number;
  cover?: string;
  artworks?: string[];
  screenshots?: string[];
}

export const gameDetails = (title: string, slug: string) =>
  invoke<GameDetails | null>("game_details", { title, slug });

/** IGDB image URL. Sizes: cover_big, screenshot_med, screenshot_big, 720p, 1080p. */
export function igdbImage(id: string, size: "cover_big" | "screenshot_med" | "screenshot_big" | "720p" | "1080p") {
  return `https://images.igdb.com/igdb/image/upload/t_${size}/${id}.jpg`;
}
