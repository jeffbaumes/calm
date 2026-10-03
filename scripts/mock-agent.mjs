// A stand-in for the real ACP adapter, for developing without a Claude login:
//   CALM_AGENT=scripts/mock-agent.mjs npm run tauri dev
// Prompt "fail" returns an auth error; "sneaky" asks permission to run a command (must be refused).
import readline from "node:readline";

const out = (m) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...m }) + "\n");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const chunk = (sessionId, text) =>
  out({ method: "session/update", params: { sessionId, update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text } } } });
process.stdout.on("error", () => process.exit(0)); // the app went away
let sessions = 0;
const waiting = new Map();

const handlers = {
  initialize: () => ({ protocolVersion: 1, agentCapabilities: {}, authMethods: [] }),
  "session/new": async () => (await sleep(800), { sessionId: `mock-${++sessions}` }),
  "session/prompt": async ({ sessionId, prompt }) => {
    const text = prompt[0].text;
    if (text.includes("fail")) throw { code: -32000, message: "Authentication required" };
    if (text.includes("sneaky")) {
      const id = `perm-${Date.now()}`;
      out({ id, method: "session/request_permission", params: { sessionId, toolCall: { toolCallId: "t1", kind: "execute", title: "rm -rf" },
        options: [{ optionId: "yes", name: "Allow", kind: "allow_once" }, { optionId: "no", name: "Reject", kind: "reject_once" }] } });
      const answer = await new Promise((r) => waiting.set(id, r));
      process.stderr.write(`[mock] permission answered: ${JSON.stringify(answer)}\n`);
    }
    await sleep(1200); // thinking
    const words = `You said "${text.slice(0, 60)}". Nothing here needs hurrying, and the rest can wait. Let it be as quiet as it already is.`.split(" ");
    for (let i = 0; i < words.length; i += 3) { chunk(sessionId, words.slice(i, i + 3).join(" ") + " "); await sleep(250); }
    return { stopReason: "end_turn" };
  },
};

readline.createInterface({ input: process.stdin }).on("line", async (line) => {
  const m = JSON.parse(line);
  if (!m.method) return waiting.get(m.id)?.(m.result);
  if (!(m.method in handlers)) return;
  try { out({ id: m.id, result: await handlers[m.method](m.params) }); }
  catch (e) { out({ id: m.id, error: e }); }
});
