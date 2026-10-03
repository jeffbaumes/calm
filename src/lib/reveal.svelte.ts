// Releases words onto the screen at a relaxed reading pace, however fast they arrive.

const PACE = 230; // ms per word (~260 words a minute)
const LEAD = 700; // stillness before the first word

export class Reveal {
  /** Every word we know of so far. Unrevealed ones are laid out but invisible, so nothing shifts as they fade in. */
  words = $state<string[]>([]);
  /** How many of them are visible. */
  count = $state(0);

  #partial = ""; // a word that may continue in the next chunk
  #timer: ReturnType<typeof setTimeout> | undefined;

  get text() {
    return this.words.join(" ");
  }

  /** Add streamed text. */
  push(chunk: string) {
    const pieces = (this.#partial + chunk).replace(/[*`#]/g, "").split(/\s+/);
    this.#partial = pieces.pop() ?? "";
    this.words.push(...pieces.filter(Boolean));
    this.#schedule();
  }

  /** The stream is over: whatever was waiting is a whole word now. */
  end() {
    this.push(" ");
  }

  /** Show a complete thought. */
  set(text: string) {
    this.reset();
    this.push(text);
    this.end();
  }

  reset() {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    this.words = [];
    this.count = 0;
    this.#partial = "";
  }

  #schedule() {
    if (this.#timer !== undefined || this.count >= this.words.length) return;
    this.#timer = setTimeout(() => {
      this.#timer = undefined;
      this.count++;
      this.#schedule();
    }, this.count === 0 ? LEAD : PACE * this.#breath(this.words[this.count - 1]));
  }

  /** Linger a little after the end of a sentence or clause. */
  #breath(word: string) {
    if (/[.!?…]["')”’]?$/.test(word)) return 2.2;
    if (/[,;:—–]["')”’]?$/.test(word)) return 1.5;
    return 1;
  }
}
