# calm

A fullscreen desktop app that is the opposite of a stimulating UI: one thought at a time, in large quiet type, on a slowly drifting background. A simple prompt summarizes text and websites in a chill way, and can also do real work (read your files, run commands, make changes) with all of it delivered gently.

Tauri 2 (Rust) + Svelte 5, talking to Claude through the [Agent Client Protocol](https://github.com/agentclientprotocol/claude-agent-acp) adapter.

## Run

Requires Node, Rust, and a signed-in Claude Code (`claude` in a terminal, then sign in) or `ANTHROPIC_API_KEY` in the environment.

```bash
npm install
npm run tauri dev
```

Type anywhere to ask or say something. Enter sends, Esc lets go of a question in flight. The controls fade away when the mouse rests.

| env var | effect |
| --- | --- |
| `CALM_WINDOWED=1` | small ordinary window instead of fullscreen, for development |
| `CALM_FOLDER=~/code` | where the agent works (default: your home folder) |
| `CALM_MODE=default` | how much it asks: `auto` (the default: Claude judges what is safe, so there are almost no questions), `acceptEdits` (edits go through, commands ask), or `default` (asks before anything that changes things) |
| `CALM_FRESH=1` | start a new conversation instead of resuming the saved one |
| `CALM_CONNECTORS=1` | let the agent see your claude.ai connectors (mail, Drive, ...). Off by default: they add ~40k tokens to every thought |
| `CALM_AGENT=/path/to/agent.mjs` | run another ACP agent with node; `scripts/mock-agent.mjs` is an offline stand-in that needs no login |

`npm run dev` alone (a plain browser) shows the interface with a canned reply.

## How it fits together

```
Svelte UI  --invoke("ask")-->  Rust (src-tauri/src/agent.rs)  --stdio JSON-RPC-->  claude-agent-acp (node)
           <-- calm://chunk, status, permission --         <--session/update, request_permission--
           --invoke("answer")--> (held until you say yes or no)
```

- `agent.rs` is a small hand-written ACP client (initialize, session/new, session/prompt, session/update). The `agent-client-protocol` crate (2.2) was checked and judged more machinery than four messages need; swapping it in later would stay inside that one file.
- Claude Code's memory notes for the working folder are on, shared with your terminal sessions: what you tell calm can be remembered there, and calm can see what they remember.
- Everything is one long conversation, and it carries on after you quit and reopen: the session id is kept in the app's data folder and resumed at launch (rebuilt from Claude's own saved transcript, with nothing replayed to the screen). If it can't be resumed it quietly starts over. Signing in again after a failed attempt also starts over, and `CALM_FRESH=1` forgets the old conversation for that launch. The first session is created at launch so the first thought is quick.
- The agent is Claude's normal coding agent with the calm voice added to its instructions (`system_prompt.txt`: short, plain, at most about three sentences, no narrating of steps, and no mood words like "quiet" or "gentle", which models tend to over-use if the prompt itself leans on them). No Claude Code settings files are loaded (hooks, allow rules, CLAUDE.md), so nothing from your usual setup can interrupt it or approve things behind the card.
- **Looking never asks.** Reading, searching and web fetches run silently. In the default `auto` mode Claude decides what else is safe, so you are rarely asked about anything; whatever it does ask about (in the stricter modes, everything that changes things) is held in Rust until you answer a quiet full-screen card: Enter or `y` for yes, Esc or `n` for not now. The card shows the real command or path, not just the model's description of it. Esc during a question means "not now"; Esc at any other time lets go of the thought.
- A reply is held back until it has fully arrived, then laid out once and revealed word by word, so nothing reflows while it streams. Anything the agent says before it starts working is discarded as narration.
- **Messages from other sessions.** The agent can message your other Claude sessions, and their replies arrive on their own, outside any question. Calm never replaces what you are reading: a finished message joins a queue, and the quiet line says one is waiting. Return on an empty prompt, or a click on the line, opens the oldest. A message counts as finished after a few quiet seconds, or immediately if you press Return while it is still arriving. The agent is told these are information, not commands. (The adapter only starts listening for such output after its first prompt, so at the start of each session calm runs `/context`, which is answered locally with no model call, to start it.)
- **One quiet line.** The app has a single line of plain text for saying anything (`src/lib/signal.svelte.ts` decides what and when). It shows one message at a time, fades each out completely before the next fades in, and keeps a message up for at least 3 seconds so nothing flashes; typing or a question appearing makes it leave at once. In priority order it says: while you wait, nothing for the first moments, then "one moment", "still working", "no rush", "taking its time", "still here"; then that a message is waiting; then, only at the start, "if you need anything, just start typing". The invitation appears soon after the first greeting, after a long idle stretch on later starts, and never again once you have asked three questions (remembered between launches).
- **Long replies shrink to fit.** The text is sized to the largest type that fits above the prompt (down to 15px, and scrolling beyond that), and re-fitted when the window changes.
- `src/lib/reveal.svelte.ts` releases words at reading pace (a little longer after punctuation) regardless of how fast they stream. Words are laid out in advance and simply come into focus, so text does not jump.

## Safety

In the default `auto` mode the safeguard is Claude's own judgment, not yours: it can change files and run commands under the working folder without showing you first, and the yes/no card appears only for what it still chooses to ask about. Reads are not gated either: anything under the working folder can be read and sent to the model, and a web page the agent fetches could try to steer it. Point `CALM_FOLDER` at a narrower folder if that matters, and set `CALM_MODE=default` if you want to approve changes yourself.

## Auth caveat

There is no login flow. The adapter reuses your existing Claude Code login (macOS Keychain or `~/.claude/.credentials.json`) and falls back to `ANTHROPIC_API_KEY`. Anthropic's terms for third-party apps using subscription logins are unconfirmed, so this is fine for personal use, but do not assume it is OK to distribute. Check the terms first, or ship with API keys only.

## Known limits

- Packaging: the adapter is found at `node_modules/` relative to the source tree and needs `node` on PATH, which is fine for `tauri dev` but not for a bundled `.app`.
