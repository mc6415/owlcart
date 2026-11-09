// Sets the app version everywhere it's recorded, so a push to main builds a
// release (see .github/workflows/release.yml).
//
//   npm run bump -- patch        0.1.0 -> 0.1.1
//   npm run bump -- minor        0.1.0 -> 0.2.0
//   npm run bump -- major        0.1.0 -> 1.0.0
//   npm run bump -- 0.3.0        exact version

import { readFile, writeFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const path = (p: string) => join(root, p);

async function main() {
  const arg = process.argv[2];
  const conf = JSON.parse(await readFile(path("src-tauri/tauri.conf.json"), "utf8"));
  const current: string = conf.version;
  const [maj, min, pat] = current.split(".").map(Number);
  const next =
    arg === "major" ? `${maj + 1}.0.0`
    : arg === "minor" ? `${maj}.${min + 1}.0`
    : arg === "patch" ? `${maj}.${min}.${pat + 1}`
    : arg;
  if (!next || !/^\d+\.\d+\.\d+$/.test(next)) {
    console.error("Usage: npm run bump -- <patch | minor | major | x.y.z>");
    process.exit(1);
  }

  // tauri.conf.json: this is the version the release workflow reads.
  conf.version = next;
  await writeFile(path("src-tauri/tauri.conf.json"), JSON.stringify(conf, null, 2) + "\n");

  // package.json and the root entries in package-lock.json.
  for (const f of ["package.json", "package-lock.json"]) {
    const j = JSON.parse(await readFile(path(f), "utf8"));
    j.version = next;
    if (j.packages?.[""]) j.packages[""].version = next;
    await writeFile(path(f), JSON.stringify(j, null, 2) + "\n");
  }

  // Cargo.toml [package] version, and the matching Cargo.lock entry.
  const toml = await readFile(path("src-tauri/Cargo.toml"), "utf8");
  await writeFile(path("src-tauri/Cargo.toml"), toml.replace(/^version = ".*"$/m, `version = "${next}"`));
  const lock = await readFile(path("src-tauri/Cargo.lock"), "utf8");
  await writeFile(
    path("src-tauri/Cargo.lock"),
    lock.replace(/(name = "owlcart"\nversion = )".*"/, `$1"${next}"`),
  );

  console.log(`${current} -> ${next}. Commit and push to main to build the release.`);
}

main().catch((e) => {
  console.error(String(e));
  process.exit(1);
});
