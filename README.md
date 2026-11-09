# Owlcart

A desktop app (Tauri 2 + Svelte) for browsing [minerva-archive.org](https://minerva-archive.org)
directory listings and downloading individual files from their torrents.

## How it works

- Paste a `/browse/...` link. The app scrapes the listing (folders are clickable).
- Every game in a console folder lives in one big torrent. Each game's `/rom?id=` page
  holds `so_id` (its 0-based file index) and the path of the `.torrent` file.
- The app downloads that `.torrent` once (cached), then asks a private `aria2c` instance
  over JSON-RPC to fetch only that file (`select-file = so_id + 1`).
- Each game downloads into `<download folder>/.owlcart-partial/<id>/`. When it
  finishes, the game is moved into the download folder and the temp folder is deleted.
  (Torrent pieces overlap neighbouring files, so aria2 writes small fragments of those
  too. They get thrown away.)
- aria2 only allows one download per torrent at a time, so games from the same console
  queue one after another. Different consoles download in parallel.

## Running

Needs Rust, Node.js, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev      # development
npm run tauri build    # installers in src-tauri/target/release/bundle/
```

### aria2c

- **Windows:** `scripts/fetch-aria2c.ps1` runs automatically before dev/build. It downloads
  the official static `aria2c.exe` and bundles it with the app as a sidecar.
- **Linux:** install your distro's `aria2` package. The .deb/.rpm builds declare it as a
  dependency.
- Set `OWLCART_ARIA2C=/path/to/aria2c` to use a specific binary.

## Game data (covers and details)

Grid covers and the game details modal come from a folder of static JSON built
once with your own API keys and then hosted, so the app never contains a key.

```
data/index.json                     consoles, versions, counts
data/<console>/covers.json          title -> cover (SteamGridDB, or IGDB as fallback)
data/<console>/details/<00-ff>.json IGDB details, split into 256 small files by title hash
```

The app downloads `covers.json` when you open a console, and only the one small
details file a game needs when you open its modal. Downloads are cached on disk and
only re-fetched when that console's `updated` value in `index.json` changes.

**From the app:** Settings → Game data takes a SteamGridDB key and IGDB client
ID/secret, and "Fetch art & details" looks up the console you're viewing. Results go
to the app's own data folder (same layout as below) and are used before the
published data. This is the same lookup as the scripts, ported to Rust (`fetch.rs`).

**Building it**

1. Keys, both git-ignored, in the project root:
   - `.steamgriddb-key`: from steamgriddb.com/profile/preferences/api
   - `.igdb-credentials`: `client_id=...` and `client_secret=...` from a
     *Confidential* app at dev.twitch.tv/console
2. `consoles.json` lists the consoles shown in the app's console picker (group,
   name, Minerva link, IGDB platform slug). It ships inside the app; consoles added
   to the hosted `index.json` later also appear without an app update.
3. `npm run data -- ps2 saturn` adds data for the named consoles (matched by name
   or slug; `all` for everything). Plain `npm run data` updates every console that
   already has data. Re-runs only look up new titles and keep hand edits.
   `--retry-missing` retries titles that found nothing; `--verbose` prints each match.

**Publishing:** host the `data/` folder (e.g. GitHub Pages) and set `DEFAULT_DATA_URL`
in `src-tauri/src/jobs.rs`. Users can override it under Settings; a local folder
path works too and is re-read whenever files change.

## Releases

Pushing to `main` with a version that hasn't been released yet builds the app on
GitHub Actions (`.github/workflows/release.yml`) and publishes a release with a
Windows `-setup.exe` and Linux `.AppImage`, `.deb` and `.rpm`.

```sh
npm run bump -- patch     # or minor / major / 1.2.3: updates every version field
git commit -am "Release v0.1.1" && git push
```

The release is created as a draft and only published once both builds succeed.
The Windows installer isn't code-signed, so SmartScreen will warn on first run.

## Tests

```sh
cd src-tauri
cargo test
# Hosted data path (serve data/ first, e.g. python3 -m http.server 8765 -d data):
DATA_URL=http://127.0.0.1:8765 cargo test remote -- --ignored --nocapture
# Real download of a 3.6 MB file through aria2 (needs network, takes a few minutes):
TEST_TORRENT=/path/to/"Minerva_Myrient - Redump - Microsoft - Xbox 360.torrent" \
  cargo test e2e -- --ignored --nocapture
```
