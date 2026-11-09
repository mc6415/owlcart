<script lang="ts" module>
  // Region name -> ISO code in flag-icons. Regions not listed here have no flag.
  const CODES: Record<string, string> = {
    USA: "us", Europe: "eu", Japan: "jp", Germany: "de", Australia: "au", Russia: "ru",
    Italy: "it", Korea: "kr", Spain: "es", France: "fr", Brazil: "br", UK: "gb", Canada: "ca",
    Poland: "pl", "New Zealand": "nz", Taiwan: "tw", Austria: "at", Switzerland: "ch",
    "United Arab Emirates": "ae", Netherlands: "nl", Sweden: "se", China: "cn", "Hong Kong": "hk",
    Belgium: "be", Denmark: "dk", Finland: "fi", Greece: "gr", India: "in", Ireland: "ie",
    Mexico: "mx", Norway: "no", Portugal: "pt", "South Africa": "za", Argentina: "ar", Chile: "cl",
    Croatia: "hr", Czech: "cz", Hungary: "hu", Israel: "il", Turkey: "tr", Ukraine: "ua",
    Singapore: "sg", Thailand: "th", Indonesia: "id", Malaysia: "my", Philippines: "ph",
    Vietnam: "vn", "Saudi Arabia": "sa", Iceland: "is", Romania: "ro", Bulgaria: "bg",
    Serbia: "rs", Slovakia: "sk", Slovenia: "si", Estonia: "ee", Latvia: "lv", Lithuania: "lt",
    Peru: "pe", Colombia: "co",
  };

  // Only the flags above get bundled.
  const files = import.meta.glob(
    "/node_modules/flag-icons/flags/4x3/{us,eu,jp,de,au,ru,it,kr,es,fr,br,gb,ca,pl,nz,tw,at,ch,ae,nl,se,cn,hk,be,dk,fi,gr,in,ie,mx,no,pt,za,ar,cl,hr,cz,hu,il,tr,ua,sg,th,id,my,ph,vn,sa,is,ro,bg,rs,sk,si,ee,lv,lt,pe,co}.svg",
    { eager: true, query: "?url", import: "default" },
  ) as Record<string, string>;

  export function flagUrl(region: string): string | undefined {
    const code = CODES[region];
    return code ? files[`/node_modules/flag-icons/flags/4x3/${code}.svg`] : undefined;
  }
</script>

<script lang="ts">
  let { region, height = 12 }: { region: string; height?: number } = $props();
  let url = $derived(flagUrl(region));
</script>

{#if url}
  <img src={url} alt={region} title={region} style="height: {height}px" />
{:else if region === "World"}
  <svg viewBox="0 0 24 24" style="height: {height + 2}px" aria-label="World">
    <title>World</title>
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z" />
  </svg>
{:else}
  <span class="name" title={region}>{region}</span>
{/if}

<style>
  img {
    display: block;
    aspect-ratio: 4 / 3;
    border-radius: 2px;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.15);
    flex-shrink: 0;
  }
  svg {
    display: block;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    color: var(--muted);
    flex-shrink: 0;
  }
  .name {
    font-size: 11px;
  }
</style>
