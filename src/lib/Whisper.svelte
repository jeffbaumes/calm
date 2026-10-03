<!-- The prompt line. A real (invisible) input handles typing, paste and IME;
     what you see is a mirror where each letter fades in as it is typed and fades out when deleted. -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import { cubicOut } from "svelte/easing";

  let {
    value = $bindable(""),
    awake,
    onsubmit,
    onwake,
  }: { value: string; awake: boolean; onsubmit: () => void; onwake: () => void } = $props();

  type Glyph = { id: number; ch: string };
  let glyphs = $state<Glyph[]>([]);
  let caret = $state(0); // index into value
  let focused = $state(false);
  let input: HTMLInputElement;
  let box: HTMLDivElement;
  let line: HTMLSpanElement;
  let nextId = 0;
  let before = "";
  // True while the person is deleting with the keyboard: that is immediate (no fade, no glide).
  let editing = $state(false);
  let editTimer: ReturnType<typeof setTimeout>;
  const startEditing = (e: InputEvent) => {
    if (!e.inputType.startsWith("delete")) return; // typing keeps its fade and glide
    editing = true;
    clearTimeout(editTimer);
    editTimer = setTimeout(() => (editing = false), 150);
  };

  export const focus = () => input.focus();
  export const blur = () => input.blur();
  export const isFocused = () => document.activeElement === input;

  /** A letter arrives, or leaves, by fading alone: nothing about its size or position changes. */
  function glyph(_: HTMLElement, { duration = 220 } = {}) {
    return { duration, easing: cubicOut, css: (t: number) => `opacity:${t}` };
  }

  // Whatever changed the text (typing, paste, backspace, or the app clearing it), work out which
  // letters were removed and which were added, so everything else stays put.
  $effect(() => {
    const now = value;
    untrack(() => {
      const old = [...before]; // by whole characters, so emoji stay in one piece
      const now_ = [...now];
      before = now;
      let start = 0;
      while (start < old.length && start < now_.length && old[start] === now_[start]) start++;
      let end = 0;
      while (end < old.length - start && end < now_.length - start && old[old.length - 1 - end] === now_[now_.length - 1 - end]) end++;
      const added = now_.slice(start, now_.length - end).map((ch) => ({ id: nextId++, ch }));
      glyphs.splice(start, old.length - start - end, ...added);
      tick().then(place); // centre in the same update, not a frame later
    });
  });

  const track = () => (caret = [...value.slice(0, input.selectionStart ?? value.length)].length);

  // The line is centred by one transform that glides (see CSS), so letters never jump as it grows.
  // When it is longer than the field, its end stays in view instead.
  function place() {
    if (!line) return;
    const width = line.offsetWidth, room = box.clientWidth;
    line.style.setProperty("--shift", `${width <= room ? -width / 2 : room / 2 - width}px`);
  }

  $effect(() => {
    const watch = new ResizeObserver(place);
    watch.observe(line);
    place();
    return () => watch.disconnect();
  });
</script>

<form class:awake onsubmit={(e) => { e.preventDefault(); onsubmit(); }}>
  <div class="mirror" bind:this={box} aria-hidden="true">
    <span class="line" class:editing bind:this={line}>{#each glyphs as g, i (g.id)}{#if i === caret}<i class="caret" class:on={focused && value !== ""}></i>{/if}<span transition:glyph={{ duration: editing ? 0 : 220 }}>{g.ch}</span>{/each}{#if caret >= glyphs.length}<i class="caret" class:on={focused && value !== ""}></i>{/if}</span>
  </div>
  <input
    bind:this={input}
    bind:value
    onbeforeinput={startEditing}
    oninput={track}
    onkeyup={track}
    onclick={track}
    onselect={track}
    onfocus={() => { focused = true; track(); onwake(); }}
    onblur={() => { focused = false; onwake(); }}
    aria-label="ask"
    spellcheck="false"
    autocomplete="off"
    autocapitalize="off"
  />
</form>

<style>
  form {
    position: fixed;
    left: 50%;
    bottom: 7vh;
    width: min(24em, 84vw);
    transform: translateX(-50%);
    opacity: 0;
    transition: opacity var(--slow) var(--ease);
  }
  form.awake { opacity: 1; }

  /* Smaller than the replies, but easy to read. */
  form, input, .mirror { font: italic var(--small) / 1.4 var(--serif); }

  .mirror, input {
    display: block;
    width: 100%;
    padding: 0.6em 0;
    text-align: center;
  }
  .mirror {
    position: relative;
    overflow: hidden;
    height: 2.6em;
    white-space: pre;
    color: var(--ink);
  }

  /* The real input sits on top, unseen, to catch the typing. */
  input {
    position: absolute;
    inset: 0;
    border: none;
    background: transparent;
    color: transparent;
    caret-color: transparent;
    outline: none;
    user-select: text;
  }
  input::selection { background: rgba(232, 224, 210, 0.16); color: transparent; }

  .line {
    position: absolute;
    top: 0.6em;
    left: 50%;
    white-space: pre;
    transform: translateX(var(--shift, 0px));
    transition: transform 240ms var(--ease);
  }
  .line.editing { transition: none; }

  /* A still caret: no blinking. Zero width, so it moves nothing. */
  .caret { position: relative; }
  .caret::after {
    content: "";
    position: absolute;
    left: 0;
    top: 0.15em;
    bottom: 0.1em;
    border-left: 1px solid var(--ink-faint);
    opacity: 0;
    transition: opacity var(--slow) var(--ease);
  }
  .caret.on::after { opacity: 1; }
</style>
