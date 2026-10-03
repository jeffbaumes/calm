//! A tiny Agent Client Protocol client.
//!
//! Spawns the ACP adapter as a child process, speaks newline-delimited JSON-RPC
//! over its stdio, and relays what happens to the webview:
//!   `calm://chunk`      streamed text of the reply
//!   `calm://status`     what the agent is doing right now ("Reading App.svelte")
//!   `calm://permission` a request to change something, answered with the `answer` command
//! Looking around (read, search, fetch) is always allowed. Anything that changes
//! something waits for a quiet yes or no from the person.

use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};

const SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");
/// Tools that never ask: they only look.
const LOOKING: [&str; 5] = ["Read", "Glob", "Grep", "WebFetch", "WebSearch"];

type Reply = oneshot::Sender<Result<Value, String>>;
type Pending = Arc<Mutex<HashMap<u64, Reply>>>;
/// Permission requests waiting for a yes (true) or no (false), by request id.
type Perms = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

/// Where the adapter lives: `CALM_AGENT` (a JS file run with node) or the npm dependency.
fn agent_script() -> PathBuf {
    std::env::var_os("CALM_AGENT").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js")
    })
}

// ---------------------------------------------------------------- JSON-RPC

/// Everything the reader and the callers share.
#[derive(Clone)]
struct Wire {
    app: AppHandle,
    tx: mpsc::UnboundedSender<String>,
    pending: Pending,
    perms: Perms,
    alive: Arc<AtomicBool>,
}

impl Wire {
    fn send(&self, msg: Value) -> Result<(), String> {
        self.tx.send(msg.to_string()).map_err(|_| "the agent has stopped".to_string())
    }
}

struct Rpc {
    wire: Wire,
    next_id: AtomicU64,
}

impl Rpc {
    async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = oneshot::channel();
        self.wire.pending.lock().unwrap().insert(id, reply);
        self.wire.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        answer.await.map_err(|_| "the agent has stopped".to_string())?
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.wire.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }
}

/// Reads everything the agent says: answers to our calls, streamed updates,
/// and requests from the agent.
async fn read_loop(stdout: tokio::process::ChildStdout, wire: Wire) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        match (msg["method"].as_str(), msg.get("id")) {
            (Some(method), Some(id)) => handle_request(&wire, id.clone(), method, &msg["params"]),
            (Some("session/update"), None) => handle_update(&wire.app, &msg["params"]["update"]),
            (None, Some(id)) => {
                let Some(reply) = id.as_u64().and_then(|id| wire.pending.lock().unwrap().remove(&id)) else { continue };
                let _ = reply.send(match msg.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or("something went wrong").to_string()),
                    None => Ok(msg["result"].clone()),
                });
            }
            _ => {}
        }
    }
    // The agent is gone: fail anything still waiting.
    wire.alive.store(false, Ordering::SeqCst);
    wire.pending.lock().unwrap().clear();
    wire.perms.lock().unwrap().clear();
}

fn handle_update(app: &AppHandle, update: &Value) {
    match update["sessionUpdate"].as_str() {
        Some("agent_message_chunk") if update["content"]["type"] == "text" => {
            let _ = app.emit("calm://chunk", update["content"]["text"].as_str().unwrap_or_default());
        }
        Some("tool_call" | "tool_call_update") => {
            if let Some(title) = update["title"].as_str() {
                let _ = app.emit("calm://status", title);
            }
        }
        _ => {}
    }
}

/// The agent asks to do something. Looking is always fine; changing waits for the person.
fn handle_request(wire: &Wire, id: Value, method: &str, params: &Value) {
    if method != "session/request_permission" {
        let _ = wire.send(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "not supported" } }));
        return;
    }
    let option = |kind: &str| {
        let options = params["options"].as_array()?;
        options.iter().find(|o| o["kind"] == kind).map(|o| o["optionId"].clone())
    };
    let (allow, reject) = (option("allow_once"), option("reject_once"));
    let outcome = move |chosen: Option<Value>| match chosen {
        Some(option_id) => json!({ "outcome": "selected", "optionId": option_id }),
        None => json!({ "outcome": "cancelled" }),
    };

    let call = &params["toolCall"];
    if matches!(call["kind"].as_str(), Some("read" | "search" | "fetch" | "think")) {
        let _ = wire.send(json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome(allow) } }));
        return;
    }

    let key = id.to_string();
    let (answer, response) = oneshot::channel();
    wire.perms.lock().unwrap().insert(key.clone(), answer);
    let _ = wire.app.emit(
        "calm://permission",
        json!({ "key": key, "kind": call["kind"], "title": call["title"], "detail": call["rawInput"]["description"] }),
    );
    let wire = wire.clone();
    tauri::async_runtime::spawn(async move {
        let chosen = match response.await {
            Ok(true) => allow,
            Ok(false) => reject,
            Err(_) => None, // cancelled
        };
        let _ = wire.send(json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome(chosen) } }));
    });
}

// ------------------------------------------------------------------- Agent

struct Running {
    rpc: Rpc,
    cwd: PathBuf,
    /// The first session, created at launch so the first thought starts instantly.
    warm: Mutex<Option<JoinHandle<Result<String, String>>>>,
    /// The one long conversation. Every thought continues it, so Claude remembers what came before.
    session: Mutex<Option<String>>,
    /// The session while a thought is in flight (what `cancel` interrupts).
    current: Mutex<Option<String>>,
    _child: Child, // killed when dropped
}

#[derive(Default)]
pub struct Agent {
    running: tokio::sync::Mutex<Option<Arc<Running>>>,
    busy: AtomicBool,
}

impl Agent {
    /// Starts the adapter in the background so the first thought is quick.
    pub async fn wake(&self, app: &AppHandle) {
        let _ = self.running(app).await;
    }

    /// The live adapter, starting it (and warming its first session) if it isn't running or has died.
    async fn running(&self, app: &AppHandle) -> Result<Arc<Running>, String> {
        let mut slot = self.running.lock().await;
        if let Some(r) = slot.as_ref().filter(|r| r.rpc.wire.alive.load(Ordering::SeqCst)) {
            return Ok(r.clone());
        }
        let r = Arc::new(start(app).await?);
        r.warm_session();
        *slot = Some(r.clone());
        Ok(r)
    }

    /// Sends one prompt into the ongoing conversation, streaming text as `calm://chunk` events.
    pub async fn ask(&self, app: &AppHandle, prompt: String) -> Result<String, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err("still thinking about the last one".into());
        }
        let result = async {
            let r = self.running(app).await?;
            let session = r.session().await?;
            *r.current.lock().unwrap() = Some(session.clone());
            let done = r
                .rpc
                .call("session/prompt", json!({ "sessionId": session, "prompt": [{ "type": "text", "text": prompt }] }))
                .await;
            *r.current.lock().unwrap() = None;
            // A session begun while signed out can stay stuck that way: start over after signing in.
            if matches!(&done, Err(e) if e.to_lowercase().contains("authenticat")) {
                *r.session.lock().unwrap() = None;
            }
            Ok(done?["stopReason"].as_str().unwrap_or("end_turn").to_string())
        }
        .await;
        self.busy.store(false, Ordering::SeqCst);
        result
    }

    /// Let go of the current thought, including any question waiting for an answer.
    pub async fn cancel(&self) {
        if let Some(r) = self.running.lock().await.as_ref() {
            r.rpc.wire.perms.lock().unwrap().clear();
            if let Some(session) = r.current.lock().unwrap().clone() {
                r.rpc.notify("session/cancel", json!({ "sessionId": session }));
            }
        }
    }

    /// The person's answer to a `calm://permission` request.
    pub async fn answer(&self, key: &str, yes: bool) {
        if let Some(r) = self.running.lock().await.as_ref() {
            if let Some(waiting) = r.rpc.wire.perms.lock().unwrap().remove(key) {
                let _ = waiting.send(yes);
            }
        }
    }
}

impl Running {
    /// The ongoing conversation, begun on first use.
    async fn session(&self) -> Result<String, String> {
        if let Some(session) = self.session.lock().unwrap().clone() {
            return Ok(session);
        }
        let warm = self.warm.lock().unwrap().take();
        let session = match warm {
            Some(handle) => handle.await.map_err(|e| e.to_string())??,
            None => self.new_session().await?,
        };
        *self.session.lock().unwrap() = Some(session.clone());
        Ok(session)
    }

    async fn new_session(&self) -> Result<String, String> {
        new_session(&self.rpc, &self.cwd).await
    }

    fn warm_session(self: &Arc<Self>) {
        let this = self.clone();
        let handle = tauri::async_runtime::spawn(async move { this.new_session().await });
        *self.warm.lock().unwrap() = Some(handle);
    }
}

async fn new_session(rpc: &Rpc, cwd: &PathBuf) -> Result<String, String> {
    let created = rpc
        .call(
            "session/new",
            json!({
                "cwd": cwd,
                "mcpServers": [],
                "_meta": {
                    // Claude's usual instructions for working, plus the calm voice.
                    "systemPrompt": { "append": SYSTEM_PROMPT },
                    "claudeCode": { "options": {
                        "settingSources": [],                   // no hooks, rules or notifications from any settings file
                        "allowedTools": LOOKING,                // looking never asks
                        "model": "sonnet",
                        "maxTurns": 40,
                    } },
                },
            }),
        )
        .await?;
    let session = created["sessionId"].as_str().ok_or("the agent gave no session")?.to_string();
    // "auto": Claude judges what is safe, so there are almost never any questions. Anything it still
    // asks about reaches the person as the quiet yes/no card. CALM_MODE=acceptEdits asks only about
    // commands; CALM_MODE=default asks before anything that changes things.
    let mode = std::env::var("CALM_MODE").unwrap_or_else(|_| "auto".into());
    let _ = rpc.call("session/set_mode", json!({ "sessionId": session, "modeId": mode })).await;
    Ok(session)
}

async fn start(app: &AppHandle) -> Result<Running, String> {
    let script = agent_script();
    if !script.exists() {
        return Err(format!("agent not found at {} (run `npm install`)", script.display()));
    }
    // Where the agent works: CALM_FOLDER, or the home folder.
    let cwd = match std::env::var_os("CALM_FOLDER") {
        Some(folder) => PathBuf::from(folder),
        None => app.path().home_dir().map_err(|e| e.to_string())?,
    };

    let mut command = Command::new("node");
    command.arg(script).stdin(Stdio::piped()).stdout(Stdio::piped()).kill_on_drop(true);
    if std::env::var_os("CALM_CONNECTORS").is_none() {
        // Otherwise the claude.ai connectors (mail, drive, ...) join every thought: ~40k tokens of context each.
        command.env("ENABLE_CLAUDEAI_MCP_SERVERS", "false");
    }
    let mut child = command.spawn().map_err(|e| format!("could not start node: {e}"))?;

    let mut stdin = child.stdin.take().unwrap();
    let (tx, mut outbox) = mpsc::unbounded_channel::<String>();
    tauri::async_runtime::spawn(async move {
        while let Some(line) = outbox.recv().await {
            if stdin.write_all(format!("{line}\n").as_bytes()).await.is_err() {
                break;
            }
        }
    });

    let wire = Wire {
        app: app.clone(),
        tx,
        pending: Default::default(),
        perms: Default::default(),
        alive: Arc::new(AtomicBool::new(true)),
    };
    tauri::async_runtime::spawn(read_loop(child.stdout.take().unwrap(), wire.clone()));

    let rpc = Rpc { wire, next_id: AtomicU64::new(1) };
    rpc.call(
        "initialize",
        json!({ "protocolVersion": 1, "clientCapabilities": {}, "clientInfo": { "name": "calm", "version": "0.1.0" } }),
    )
    .await?;
    Ok(Running { rpc, cwd, warm: Mutex::new(None), session: Mutex::new(None), current: Mutex::new(None), _child: child })
}
