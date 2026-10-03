<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Ambient from "./lib/Ambient.svelte";
  import Whisper from "./lib/Whisper.svelte";
  import { Reveal } from "./lib/reveal.svelte";
  import { answer, ask, cancel, onChunk, onPermission, onStatus, type Permission } from "./lib/agent";

  const GREETING =
    "There is nothing you need to do right now. The day can wait a little while, and so can everything in it.";

  const reveal = new Reveal();
  let leaving = $state(false); // the current thought is fading away
  let waiting = $state(false); // asked, but nothing has arrived yet
  let hint = $state("one moment"); // changes gently if the work takes a while
  let patience: ReturnType<typeof setTimeout>[] = [];
  let permission = $state<Permission | null>(null); // the agent is asking to change something
  let awake = $state(true); // controls are showing
  let prompt = $state("");
  let whisper: Whisper;
  let clearing: Promise<void> | null = null; // the current thought is fading out
  let held: string[] = []; // words that arrived while it was
  let busy = false;
  let cancelled = false;
  let rest: ReturnType<typeof setTimeout>;

  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  /** Controls appear when the mouse moves and slip away a few seconds after it stops. */
  function wake() {
    awake = true;
    clearTimeout(rest);
    rest = setTimeout(() => {
      if (!prompt && !whisper.isFocused()) awake = false;
    }, 3500);
  }

  function gentle(error: string) {
    if (/authenticat|login|oauth/i.test(error))
      return "I can't reach Claude just yet, because you're not signed in. In a terminal, run claude auth login, then come back and ask again.";
    if (/not found|node/i.test(error))
      return "The helper that thinks for me isn't here. Running npm install in the project should bring it back.";
    return "Something got tangled just now. Take a breath, and try again whenever you like.";
  }

  /** If the work runs long, the hint softly changes: after these many seconds, to these words. */
  const WAITING: [number, string][] = [
    [8, "still working"],
    [20, "no rush"],
    [40, "taking its time"],
    [75, "still here"],
  ];

  function beginWaiting() {
    endWaiting();
    hint = "one moment";
    patience = WAITING.map(([seconds, words]) => setTimeout(() => (hint = words), seconds * 1000));
  }

  function endWaiting() {
    patience.forEach(clearTimeout);
    patience = [];
  }

  /** Fade the current thought away, then forget it. Nothing on screen ever just vanishes. */
  function clearThought() {
    if (clearing) return clearing;
    if (!reveal.words.length) return Promise.resolve();
    leaving = true;
    clearing = sleep(1100).then(() => {
      reveal.reset();
      leaving = false;
      clearing = null;
      held.splice(0).forEach((text) => reveal.push(text));
    });
    return clearing;
  }

  async function think(text: string) {
    busy = true;
    cancelled = false;
    await clearThought();
    waiting = true;
    beginWaiting();
    try {
      await ask(text);
      await clearing;
      if (cancelled && !reveal.words.length) reveal.set("Alright. We can leave that one for now.");
      else reveal.end();
    } catch (error) {
      await clearThought();
      reveal.set(gentle(String(error)));
    } finally {
      waiting = false;
      busy = false;
      endWaiting();
      permission = null;
    }
  }

  function submit() {
    const text = prompt.trim();
    if (!text || busy) return;
    prompt = "";
    whisper.blur();
    think(text);
  }

  const QUESTIONS: Record<string, string> = {
    edit: "May I change something?",
    delete: "May I remove something?",
    move: "May I move something?",
    execute: "May I run something?",
  };

  function reply(yes: boolean) {
    if (!permission) return;
    answer(permission.key, yes);
    permission = null;
    waiting = true;
  }

  function onKey(event: KeyboardEvent) {
    if (permission) {
      // While a question is open, keys answer it and nothing else happens.
      if (event.key === "Enter" || event.key.toLowerCase() === "y") reply(true);
      else if (event.key === "Escape" || event.key.toLowerCase() === "n") reply(false);
      return;
    }
    if (event.key === "Escape") {
      whisper.blur();
      prompt = "";
      if (busy) {
        cancelled = true;
        cancel();
      }
    } else if (event.key.length === 1 && !event.metaKey && !event.ctrlKey && !whisper.isFocused()) {
      whisper.focus(); // typing anywhere begins a question
    }
    wake();
  }

  onMount(() => {
    reveal.set(GREETING);
    wake();
    const stops = [
      onChunk((text) => {
        waiting = false;
        if (clearing) held.push(text);
        else reveal.push(text);
      }),
      // The agent has started doing something. We do not say what; we only let go of any narration before it.
      onStatus(() => {
        clearThought();
        waiting = true;
      }),
      onPermission((ask) => {
        clearThought();
        waiting = false;
        permission = ask;
      }),
    ];
    return () => {
      stops.forEach((stop) => stop.then((off) => off()));
    };
  });

  $effect(() => {
    document.body.classList.toggle("resting", !awake);
  });
</script>

<svelte:window onmousemove={wake} onkeydown={onKey} />

<Ambient />

<main>
  <p class="thought" class:leaving aria-live="polite">
    {#each reveal.words as word, i}<span class:on={i < reveal.count}>{word}</span>{" "}{/each}
  </p>
<div class="hint" class:on={waiting}>
    {#key hint}
      <span in:fade={{ duration: 1100, easing: cubicOut }} out:fade={{ duration: 900, easing: cubicOut }}>{hint}</span>
    {/key}
  </div>

  {#if permission}
    {#key permission.key}
      <section class="question" transition:fade={{ duration: 1100, easing: cubicOut }}>
        <p class="ask">{QUESTIONS[permission.kind] ?? "May I do this?"}</p>
        <p class="what">{permission.detail ?? permission.title}</p>
        {#if permission.detail}<p class="what small">{permission.title}</p>{/if}
        <div class="choices">
          <button onclick={() => reply(true)}>yes</button>
          <button onclick={() => reply(false)}>not now</button>
        </div>
      </section>
    {/key}
  {/if}
</main>

<Whisper bind:this={whisper} bind:value={prompt} {awake} placeholder="ask, or paste a link" onsubmit={submit} onwake={wake} />

<style>
  main {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 12vh 14vw;
  }

  .thought, .question { grid-area: 1 / 1; }

  .thought {
    max-width: 24em;
    font-size: clamp(28px, 3.4vw, 48px);
    line-height: 1.7;
    text-align: center;
    text-wrap: balance;
    transition: opacity var(--slow) var(--ease);
  }
  .thought.leaving { opacity: 0; }

  /* Words are laid out in advance and simply come into focus. */
  .thought span {
    opacity: 0;
    filter: blur(6px);
    transition: opacity var(--slow) var(--ease), filter var(--slow) var(--ease);
  }
  .thought span.on { opacity: 0.94; filter: none; }

  .hint {
    position: fixed;
    inset: auto 0 24vh;
    display: grid;
    justify-items: center;
    font-size: 20px;
    font-style: italic;
    color: var(--ink-faint);
    opacity: 0;
    transition: opacity var(--slow) var(--ease);
  }
  .hint.on { opacity: 1; transition-delay: 1800ms; }
  /* Old and new status text sit in the same spot and cross-fade. */
  .hint span {
    grid-area: 1 / 1;
    max-width: 70vw;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .question {
    max-width: 24em;
    text-align: center;
  }
  .ask {
    font-size: clamp(28px, 3.4vw, 48px);
    line-height: 1.7;
  }
  .what {
    margin-top: 1.2rem;
    font-size: 22px;
    line-height: 1.6;
    font-style: italic;
    color: var(--ink-faint);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .what.small { margin-top: 0.4rem; font-size: 18px; }
  .choices { margin-top: 3rem; display: flex; gap: 3.5rem; justify-content: center; }
  .choices button {
    padding: 0.4em 0.2em;
    border: none;
    border-bottom: 1px solid var(--ink-faint);
    background: none;
    color: var(--ink);
    font: italic 24px var(--serif);
    cursor: pointer;
    opacity: 0.8;
    transition: opacity var(--slow) var(--ease), border-color var(--slow) var(--ease);
  }
  .choices button:hover { opacity: 1; border-bottom-color: var(--ink); }
</style>
