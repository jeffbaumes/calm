// The one quiet line of text the app uses to say anything ("one moment", "a message is waiting", ...).
// Whatever wants to be said, this decides when. It shows one message at a time, fades each out completely
// before the next fades in, and keeps a message up for a minimum time so nothing flashes.

const MIN_SHOW = 3000; // a message stays at least this long once it is fully visible
const FADE_OUT = 800; // keep in step with SignalLine.svelte
const FADE_IN = 1100;

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export class Signal {
  /** The words in the line (they stay put while it fades out). */
  text = $state("");
  visible = $state(false);

  #want = "";
  #hurry = false; // skip the minimum time (the person did something)
  #running = false;
  #shownAt = 0;
  #wake: (() => void) | null = null;
  #hiddenWaiters: (() => void)[] = [];

  /** Say `want` ("" for nothing). With `hurry`, a message that is up leaves at once instead of waiting out its minimum time. */
  show(want: string, hurry = false) {
    if (want === this.#want) {
      if (hurry) {
        this.#hurry = true;
        this.#wake?.();
      }
      return;
    }
    this.#want = want;
    this.#hurry = hurry;
    this.#wake?.();
    if (!this.#running) void this.#run();
  }

  /** Resolves once nothing is being said. */
  hidden() {
    return new Promise<void>((resolve) => {
      if (this.text === "") resolve();
      else this.#hiddenWaiters.push(resolve);
    });
  }

  async #run() {
    this.#running = true;
    try {
      for (;;) {
        const want = this.#want;
        if (want === this.text && (want === "" || this.visible)) break; // already saying what we want

        if (this.text !== "" && this.visible) {
          const remaining = this.#hurry ? 0 : this.#shownAt + MIN_SHOW - Date.now();
          if (remaining > 0) {
            await this.#nap(remaining); // then look again: what we want may have changed
            continue;
          }
          this.visible = false;
          await sleep(FADE_OUT);
        }

        // Nothing is visible now. Put the next words in place (unseen), then bring them in.
        const next = this.#want;
        this.#hurry = false;
        this.text = next;
        if (next === "") {
          this.#hiddenWaiters.splice(0).forEach((resolve) => resolve());
          continue;
        }
        await sleep(40); // let the words settle at zero opacity so the fade-in has something to fade from
        if (this.#want !== next) continue;
        this.visible = true;
        await sleep(FADE_IN);
        this.#shownAt = Date.now();
      }
    } finally {
      this.#running = false;
    }
  }

  #nap(ms: number) {
    return new Promise<void>((resolve) => {
      const timer = setTimeout(done, ms);
      function done() {
        clearTimeout(timer);
        resolve();
      }
      this.#wake = () => {
        this.#wake = null;
        done();
      };
    });
  }
}
