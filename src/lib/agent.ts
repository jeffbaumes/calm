// The webview's whole view of the agent: ask a question, receive text as it streams,
// hear what it is doing, and answer when it asks to change something.
// Outside Tauri (plain `npm run dev` in a browser) a canned reply stands in, for previewing the look.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const inTauri = "__TAURI_INTERNALS__" in window;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Calls `onChunk` with each piece of streamed text; returns a function that stops listening. */
export async function onChunk(onChunk: (text: string) => void): Promise<() => void> {
  return inTauri ? listen<string>("calm://chunk", (e) => onChunk(e.payload)) : (window.__calmPreview = onChunk, () => {});
}

export type Permission = { key: string; kind: string; title: string; detail: string | null };

/** What the agent is doing right now, in a few words ("Reading App.svelte"). */
export async function onStatus(onStatus: (text: string) => void): Promise<() => void> {
  return inTauri ? listen<string>("calm://status", (e) => onStatus(e.payload)) : (window.__calmPreviewStatus = onStatus, () => {});
}

/** The agent wants to change something and is waiting to hear yes or no. */
export async function onPermission(onAsk: (ask: Permission) => void): Promise<() => void> {
  return inTauri ? listen<Permission>("calm://permission", (e) => onAsk(e.payload)) : (window.__calmPreviewAsk = onAsk, () => {});
}

export const answer = (key: string, yes: boolean) =>
  inTauri ? invoke("answer", { key, yes }) : Promise.resolve(window.__calmPreviewAnswered?.());

/** Resolves when the reply is complete; rejects with a plain message if it can't be. */
export async function ask(prompt: string): Promise<void> {
  if (inTauri) {
    await invoke("ask", { prompt });
    return;
  }
  if (prompt.includes("slow")) await sleep(12000); // a long wait, to see the waiting line change
  if (prompt.includes("signedout")) {
    // What the real adapter does when not signed in: a raw message, then an error.
    window.__calmPreview?.("Failed to authenticate: OAuth session expired and could not be refreshed");
    throw new Error("Authentication required");
  }
  if (prompt.includes("work")) {
    // Preview of an agent at work: some narration, a status or two, then a question.
    window.__calmPreview?.("Let me take a look at that for you. ");
    await sleep(2500);
    window.__calmPreviewStatus?.("Reading notes.md");
    await sleep(2500);
    window.__calmPreviewStatus?.("Searching the web");
    await sleep(2500);
    window.__calmPreviewAsk?.({ key: "demo", kind: "edit", title: "Write notes.md", detail: "Add a short summary to notes.md" });
    await new Promise<void>((done) => (window.__calmPreviewAnswered = done));
  }
  await sleep(1500);
  for (const piece of ["This is only a ", "preview in the browser. ", "The real words ", "arrive from the agent, ", "when the app is running."]) {
    window.__calmPreview?.(piece);
    await sleep(200);
  }
}

export const cancel = () => (inTauri ? invoke("cancel") : Promise.resolve());

declare global {
  interface Window {
    __calmPreview?: (text: string) => void;
    __calmPreviewStatus?: (text: string) => void;
    __calmPreviewAsk?: (ask: Permission) => void;
    __calmPreviewAnswered?: () => void;
  }
}
