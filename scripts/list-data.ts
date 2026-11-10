// Lists every console in consoles.json and how much published data it has.
//
//   npm run data:list [-- --out data]

import { readFile, access } from "node:fs/promises";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { listingSlug } from "../src/lib/names.ts";
import { arg } from "./common.ts";

interface ConsoleConfig {
  name: string;
  group: string;
  folder?: string;
  listing: string;
}
interface IndexEntry {
  updated?: string | number;
  titles?: number;
  covers?: number;
  details?: number;
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = resolve(root, arg("--out") ?? "data");

const consoles: ConsoleConfig[] = JSON.parse(await readFile(join(root, "consoles.json"), "utf8"));
let index: Record<string, IndexEntry> = {};
try {
  index = JSON.parse(await readFile(join(outDir, "index.json"), "utf8")).consoles ?? {};
} catch {}

const exists = (p: string) => access(p).then(() => true, () => false);
// Older entries store the update time as Unix seconds (as a string), newer ones as an ISO date.
const day = (u?: string | number) => {
  const d = new Date(/^\d+$/.test(String(u)) ? Number(u) * 1000 : (u ?? NaN));
  return isNaN(d.getTime()) ? "" : d.toISOString().slice(0, 10);
};
const num = (n?: number) => (n == null ? "" : String(n)).padStart(7);

const fw = Math.max(6, ...consoles.map((c) => (c.folder ?? "").length)) + 2;
const nw = Math.max(...consoles.map((c) => c.name.length)) + 2;
console.log(`${"Folder".padEnd(fw)}${"Console".padEnd(nw)}${"Titles".padStart(7)}${"Covers".padStart(7)}${"Details".padStart(8)}  Updated`);
let withData = 0;
for (const c of consoles) {
  const slug = listingSlug(c.listing);
  const e = index[slug];
  let note = "";
  if (e) {
    withData++;
    note = day(e.updated);
  } else if (await exists(join(outDir, slug))) {
    note = "partial (not in index)";
  } else {
    note = "-";
  }
  console.log(`${(c.folder ?? "").padEnd(fw)}${c.name.padEnd(nw)}${num(e?.titles)}${num(e?.covers)} ${num(e?.details)}  ${note}`);
}
console.log(`\n${withData} of ${consoles.length} consoles have data.`);
