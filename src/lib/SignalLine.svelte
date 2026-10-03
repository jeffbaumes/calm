<!-- Where the app's one quiet line of text is drawn. What it says, and when, is decided by Signal. -->
<script lang="ts">
  let {
    text,
    visible,
    clickable = false,
    onclick,
  }: { text: string; visible: boolean; clickable?: boolean; onclick?: () => void } = $props();
</script>

<div class="signal" class:clickable>
  <button class:on={visible} tabindex={clickable && visible ? 0 : -1} aria-hidden={!visible} onclick={() => clickable && onclick?.()}>{text}</button>
</div>

<style>
  .signal {
    pointer-events: none; /* only an offered message can be clicked */
    position: fixed;
    /* Just above the prompt line (7vh from the bottom, 2.6 lines tall). App.svelte's fit() uses these same numbers
       so that a reply always ends above this line. */
    inset: auto 0 calc(7vh + 2.6 * var(--small) + 1.5vh);
    display: grid;
    justify-items: center;
    font-size: var(--small); /* as large as what you type */
    line-height: 1.4;
    font-style: italic;
    color: var(--ink-faint);
  }
  .signal.clickable { color: rgba(232, 224, 210, 0.6); }

  button {
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
    pointer-events: none;
    opacity: 0;
    /* Out is a little quicker than in. These durations match FADE_OUT and FADE_IN in signal.svelte.ts. */
    transition: opacity 800ms var(--ease), color var(--slow) var(--ease);
  }
  button.on {
    opacity: 1;
    transition: opacity 1100ms var(--ease), color var(--slow) var(--ease);
  }
  .clickable button.on { pointer-events: auto; cursor: pointer; }
  .clickable button.on:hover { color: var(--ink); }
</style>
