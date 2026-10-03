<!-- A line of text whose changes dissolve: the old words fade out while the new ones fade in, in the same spot.
     Two layers swap places, and only opacity ever changes, so it reads the same in any browser. -->
<script lang="ts">
  import { untrack } from "svelte";

  let { text, onclick }: { text: string; onclick?: () => void } = $props();

  let layers = $state([
    { text: untrack(() => text), on: true }, // the starting text only; later changes are handled below
    { text: "", on: false },
  ]);
  let front = 0;

  $effect(() => {
    const next = text;
    untrack(() => {
      if (layers[front].text === next) return;
      const back = 1 - front;
      layers[back].text = next; // laid out at opacity 0, then both layers change together
      layers[back].on = true;
      layers[front].on = false;
      front = back;
    });
  });
</script>

<span class="stack">
  {#each layers as layer, i}
    <button class:on={layer.on} tabindex={onclick && layer.on ? 0 : -1} aria-hidden={!layer.on} onclick={() => onclick?.()}>{layer.text}</button>
  {/each}
</span>

<style>
  .stack {
    display: grid;
    justify-items: center;
  }
  button {
    grid-area: 1 / 1;
    max-width: 70vw;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: inherit;
    pointer-events: none; /* clickable only when something is on offer (see the parent) */
    opacity: 0;
    transition: opacity var(--slow) var(--ease), color var(--slow) var(--ease);
  }
  button.on { opacity: 1; }
  :global(.offering) button.on { pointer-events: auto; cursor: pointer; }
  :global(.offering) button.on:hover { color: var(--ink); }
</style>
