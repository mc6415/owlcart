<script lang="ts">
  // A controller button hint, shown where a keyboard hint would be: round for
  // the face buttons (A B X Y), a pill for the rest (L1, Start, Select...).
  let { name, onAccent = false, class: cls = "" }: { name: string; onAccent?: boolean; class?: string } = $props();
  let face = $derived(/^[ABXY]$/i.test(name));
</script>

<span class="pad {cls}" class:face class:on-accent={onAccent} aria-label="{name} button">{face ? name.toUpperCase() : name}</span>

<style>
  .pad {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 6px;
    box-sizing: border-box;
    border: 1.5px solid var(--line);
    border-radius: 9px;
    background: var(--panel);
    color: var(--muted);
    font: 700 10px/1 var(--mono);
    white-space: nowrap;
  }
  .face {
    width: 18px;
    padding: 0;
    border-radius: 50%;
  }
  .on-accent {
    background: transparent;
    border-color: color-mix(in srgb, var(--on-accent) 45%, transparent);
    color: inherit;
    opacity: 0.85;
  }
</style>
