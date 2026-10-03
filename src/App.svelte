<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Ambient from "./lib/Ambient.svelte";
  import Whisper from "./lib/Whisper.svelte";
  import SignalLine from "./lib/SignalLine.svelte";
  import { Signal } from "./lib/signal.svelte";
  import { Reveal } from "./lib/reveal.svelte";
  import { answer, ask, cancel, onChunk, onPermission, onSession, onStatus, openSession, type Peer, type Permission } from "./lib/agent";

  const GREETING =
    "There is nothing you need to do right now. The day can wait a little while, and so can everything in it.";

  const reveal = new Reveal();
  let leaving = $state(false); // the current thought is fading away
  let waiting = $state(false); // asked, but nothing has arrived yet
  let hint = $state(""); // what to say while waiting: nothing at first, then it changes gently if the work takes a while
  let patience: ReturnType<typeof setTimeout>[] = [];
  let permission = $state<Permission | null>(null); // the agent is asking to change something
  let awake = $state(true); // controls are showing
  let prompt = $state("");
  let whisper: Whisper;
  let thoughtEl: HTMLParagraphElement;
  let vw = $state(0); // window size, so the text can be fitted to it
  let vh = $state(0);
  let clearing: Promise<void> | null = null; // the current thought is fading out
  let arriving = ""; // the answer so far; shown only once it is whole, so nothing reflows while it arrives
  let busy = false;
  let working = $state(false); // a thought is being made: the prompt stays out of the way
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

  /** While waiting, the line says nothing at first (a quick reply never shows it), then, after these many seconds, these words. */
  const WAITING: [number, string][] = [
    [1.8, "one moment"],
    [8, "still working"],
    [20, "no rush"],
    [40, "taking its time"],
    [75, "still here"],
  ];

  function beginWaiting() {
    endWaiting();
    hint = "";
    patience = WAITING.map(([seconds, words]) => setTimeout(() => (hint = words), seconds * 1000));
  }

  function endWaiting() {
    patience.forEach(clearTimeout);
    patience = [];
  }

  // ---- Messages that arrive on their own: a reply from another session, finished background work.
  // The agent speaks without being asked, and nothing marks where its message ends, so a few quiet
  // seconds mean it is whole. It never replaces what you are reading: it joins a queue, and a quiet
  // line says a message is waiting. Return (on an empty prompt) or a click opens the oldest one.
  const SETTLE = 4000; // quiet that means the message is complete
  let incoming = ""; // the unprompted message so far
  let incomingTimer: ReturnType<typeof setTimeout>;
  const later: { text: string; peer: Peer | null }[] = []; // whole messages, oldest first, with the session each came from
  let waitingCount = $state(0); // how many are queued (shown only as "a message" or "messages")
  let opening = false;

  function hear(text = "") {
    incoming += text;
    clearTimeout(incomingTimer);
    incomingTimer = setTimeout(() => {
      const message = incoming.trim();
      incoming = "";
      if (message) {
        later.push({ text: message, peer: recentPeer(90000) });
        waitingCount = later.length;
      }
    }, SETTLE);
  }

  async function open() {
    if (!later.length && incoming.trim()) {
      // One is still arriving: you are ready for it, so don't make you wait for the quiet.
      clearTimeout(incomingTimer);
      later.push({ text: incoming.trim(), peer: recentPeer(90000) });
      incoming = "";
      waitingCount = later.length;
    }
    if (opening || !later.length || working || permission) return;
    opening = true;
    const message = later.shift()!;
    waitingCount = later.length;
    related = null;
    await clearThought();
    reveal.set(message.text);
    related = message.peer;
    answeredAt = 0;
    opening = false;
  }

  // ---- A link to the session involved. When the agent messages another Claude session, or hears from one, the
  // desktop app can show that session. The link is offered under the answer (or message) it belongs to,
  // whether or not anything needs doing there: it is only there if you want more detail.
  let related = $state<Peer | null>(null); // the session behind what is on screen
  let lastPeer: (Peer & { at: number }) | null = null;
  let answeredAt = 0; // when the current answer appeared (the saved conversation is read a moment late)
  let turnStartedAt = 0;

  function recentPeer(withinMs: number): Peer | null {
    return lastPeer && Date.now() - lastPeer.at < withinMs ? { id: lastPeer.id, title: lastPeer.title } : null;
  }

  // ---- The one quiet line. Every message the app might say goes through a single Signal (see signal.svelte.ts),
  // so only one thing is ever fading in or out. This decides what, if anything, it should be saying now.
  const OFFER_ONE = "a message is waiting, whenever you're ready";
  const OFFER_MANY = "messages are waiting, whenever you're ready";
  const INVITATION = "if you need anything, just start typing";
  const INVITE_UNTIL = 3; // after this many questions you know what to do: it never appears again
  const INVITE_FIRST = 4000; // on the very first start: soon after the greeting
  const INVITE_LATER = 90000; // afterwards: only after a long, quiet while

  const signal = new Signal();

  let appearing = $derived(reveal.words.length > 0 && reveal.count < reveal.words.length);
  let offering = $derived(waitingCount > 0 && !waiting && !working && !permission && !appearing && !leaving);
  let linkLine = $derived(related ? `see ${related.title} in Claude` : "");
  let linking = $derived(!!related && !waiting && !working && !permission && !appearing && !leaving);

  const readSent = () => {
    try {
      return Number(localStorage.getItem("calm.sent")) || 0;
    } catch {
      return 0;
    }
  };
  let sent = $state(readSent()); // questions asked so far, remembered between launches
  let inviting = $state(false);

  $effect(() => {
    inviting = false;
    const quiet =
      sent < INVITE_UNTIL && !working && !waiting && !permission && !appearing && !leaving && prompt === "" && waitingCount === 0;
    if (!quiet) return;
    const timer = setTimeout(() => (inviting = true), sent === 0 ? INVITE_FIRST : INVITE_LATER);
    return () => clearTimeout(timer);
  });

  // Highest priority first: waiting, then an offered message, then the invitation.
  let wanted = $derived(
    permission ? "" : waiting ? hint : working ? "" : offering ? (waitingCount > 1 ? OFFER_MANY : OFFER_ONE) : linking ? linkLine : inviting ? INVITATION : "",
  );
  /** A click on the line: open the waiting message, or the session it is offering to show. */
  function pressed() {
    if (signal.text === OFFER_ONE || signal.text === OFFER_MANY) open();
    else if (related && signal.text === linkLine) openSession(related.id);
  }

  // Starting to type, or a question appearing, takes priority over everything: the line leaves at once.
  $effect(() => {
    signal.show(wanted, prompt !== "" || !!permission);
  });

  // ---- A long reply must never run into the prompt or off the screen: use the largest type that fits.
  const FONT_MIN = 15;

  function fit() {
    const el = thoughtEl;
    if (!el || !vw || !vh) return;
    const base = Math.min(48, Math.max(28, vw * 0.034)); // the usual size (matches the CSS)
    // Room for the reply: above the signal line, which sits just above the prompt (see SignalLine.svelte).
    // The reply is centred on the screen like a short one would be, so the room is twice the smaller half.
    const small = Math.min(32, Math.max(22, vw * 0.021));
    const promptTop = vh - vh * 0.07 - 2.6 * small;
    const signalTop = promptTop - vh * 0.015 - 1.4 * small;
    const room = 2 * Math.min(vh / 2 - vh * 0.08, signalTop - vh * 0.03 - vh / 2);

    el.style.maxWidth = `${Math.min(24 * base, vw * 0.72)}px`; // fixed, so smaller type fits more per line
    el.style.maxHeight = "none";
    el.style.overflowY = "visible";
    const fits = (px: number) => {
      el.style.fontSize = `${px}px`;
      return el.scrollHeight <= room;
    };
    if (!fits(base)) {
      let small = FONT_MIN, large = base;
      while (large - small > 0.5) {
        const middle = (small + large) / 2;
        if (fits(middle)) small = middle;
        else large = middle;
      }
      el.style.fontSize = `${small}px`;
      if (el.scrollHeight > room) {
        // Even the smallest type does not fit: let it scroll rather than spill.
        el.style.maxHeight = `${room}px`;
        el.style.overflowY = "auto";
      }
    }
  }

  $effect(() => {
    void [reveal.words.length, vw, vh]; // fit again when the text or the window changes
    tick().then(fit);
  });

  /** Fade the current thought away, then forget it. Nothing on screen ever just vanishes. */
  function clearThought() {
    if (clearing) return clearing;
    if (!reveal.words.length) return Promise.resolve();
    leaving = true;
    clearing = sleep(1100).then(() => {
      reveal.reset();
      leaving = false;
      clearing = null;
    });
    return clearing;
  }

  async function think(text: string) {
    busy = true;
    working = true;
    cancelled = false;
    related = null;
    turnStartedAt = Date.now();
    await clearThought();
    waiting = true;
    beginWaiting();
    arriving = "";
    try {
      await ask(text);
      await sleep(150); // let the last streamed words land
      await clearing;
      const answer = arriving.trim();
      waiting = false;
      await Promise.race([signal.hidden(), sleep(4500)]); // the line leaves before the words arrive
      if (answer) reveal.set(answer);
      else reveal.set(cancelled ? "Alright. We can leave that one for now." : "All done.");
      answeredAt = Date.now();
      related = lastPeer && lastPeer.at >= turnStartedAt ? { id: lastPeer.id, title: lastPeer.title } : null;
    } catch (error) {
      arriving = "";
      waiting = false;
      await Promise.race([signal.hidden(), sleep(4500)]);
      await clearThought();
      reveal.set(gentle(String(error)));
    } finally {
      waiting = false;
      busy = false;
      working = false;
      endWaiting();
      permission = null;
    }
  }

  function submit() {
    const text = prompt.trim();
    if (!text || busy) return;
    prompt = "";
    whisper.blur();
    sent++;
    try {
      localStorage.setItem("calm.sent", String(sent));
    } catch {
      // remembering is a nicety
    }
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
      event.preventDefault();
      if (event.key === "Enter" || event.key.toLowerCase() === "y") reply(true);
      else if (event.key === "Escape" || event.key.toLowerCase() === "n") reply(false);
      return;
    }
    if (event.key === "Enter" && prompt.trim() === "") {
      event.preventDefault(); // nothing to send: Return means "show me the next message"
      open();
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
    whisper.focus(); // ready to type, no click needed
    reveal.set(GREETING);
    wake();
    const stops = [
      onChunk((text) => {
        if (busy) arriving += text;
        else hear(text);
      }),
      // The agent has started doing something. Anything it said before this was only narration.
      onStatus(() => {
        if (busy) {
          arriving = "";
          waiting = true;
        } else {
          incoming = "";
          hear();
        }
      }),
      onSession((peer) => {
        lastPeer = { ...peer, at: Date.now() };
        // The saved conversation is read a moment late: a session mentioned just after an answer appeared still belongs to it.
        if (!busy && Date.now() - answeredAt < 4000) related = peer;
      }),
      onPermission((ask) => {
        arriving = "";
        incoming = "";
        waiting = false;
        whisper.blur();
        clearThought();
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

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} onmousemove={wake} onkeydown={onKey} onfocus={() => !permission && whisper.focus()} />

<Ambient />

<main>
  <p class="thought" class:leaving bind:this={thoughtEl} aria-live="polite">
    {#each reveal.words as word, i}<span class:on={i < reveal.count}>{word}</span>{" "}{/each}
  </p>
<SignalLine text={signal.text} visible={signal.visible} clickable={signal.text === OFFER_ONE || signal.text === OFFER_MANY || (!!related && signal.text === linkLine)} onclick={pressed} />

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

<Whisper bind:this={whisper} bind:value={prompt} {awake} onsubmit={submit} onwake={wake} />

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
  .thought { scrollbar-width: none; }
  .thought::-webkit-scrollbar { display: none; }

  /* Words are laid out in advance and simply come into focus. */
  .thought span {
    opacity: 0;
    filter: blur(6px);
    transition: opacity var(--slow) var(--ease), filter var(--slow) var(--ease);
  }
  .thought span.on { opacity: 0.94; filter: none; }

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
