//! Finding the Claude desktop session behind a name, so calm can offer a link to it.
//!
//! The desktop app keeps one small record per session (its `local_<id>`, its title). When calm's agent
//! messages another session, or hears from one, the id or name shows up in calm's own saved conversation;
//! `mention` pulls it out of one line of that file and `resolve` finds the session's record.
//! Opening the link only ever navigates the desktop app to a session that already exists: no copy is made.

use serde_json::Value;
use std::{path::PathBuf, process::Command};

#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub id: String,
    pub title: String,
}

/// What a line of the saved conversation says about another session.
#[derive(Debug, PartialEq)]
pub enum Mention {
    Id(String),
    Name(String),
}

/// `local_` followed by a UUID: the only thing ever put into a link.
pub fn is_session_id(id: &str) -> bool {
    id.len() == 42 && id.starts_with("local_") && id[6..].chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    Some(&tag[start..start + tag[start..].find('"')?])
}

/// A message arriving from another session is wrapped as `<cross-session-message from-session="local_…" from-name="…">`;
/// a message calm's agent sends is a `SendMessage` tool call whose `to` is the other session's name.
pub fn mention(line: &str) -> Option<Mention> {
    let entry: Value = serde_json::from_str(line).ok()?;
    let content = &entry["message"]["content"];
    match entry["type"].as_str()? {
        "user" => {
            let text = content.as_str()?;
            let start = text.find("<cross-session-message")?;
            let tag = &text[start..start + text[start..].find('>')?];
            match attribute(tag, "from-session") {
                Some(id) if is_session_id(id) => Some(Mention::Id(id.to_string())),
                _ => attribute(tag, "from-name").map(|name| Mention::Name(name.to_string())),
            }
        }
        "assistant" => content.as_array()?.iter().find_map(|part| {
            if part["type"] != "tool_use" || part["name"] != "SendMessage" {
                return None;
            }
            let to = part["input"]["to"].as_str()?;
            Some(if is_session_id(to) { Mention::Id(to.to_string()) } else { Mention::Name(to.to_string()) })
        }),
        _ => None,
    }
}

fn records() -> Vec<(Peer, bool, u64)> {
    let Some(home) = std::env::var_os("HOME") else { return vec![] };
    let root = PathBuf::from(home).join("Library/Application Support/Claude/claude-code-sessions");
    let mut found = vec![];
    for org in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        for user in std::fs::read_dir(org.path()).into_iter().flatten().flatten() {
            for file in std::fs::read_dir(user.path()).into_iter().flatten().flatten() {
                let name = file.file_name().to_string_lossy().to_string();
                if !name.starts_with("local_") || !name.ends_with(".json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(file.path()) else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let (Some(id), Some(title)) = (v["sessionId"].as_str(), v["title"].as_str()) else { continue };
                if is_session_id(id) {
                    found.push((
                        Peer { id: id.to_string(), title: title.to_string() },
                        v["isArchived"].as_bool().unwrap_or(false),
                        v["lastActivityAt"].as_u64().unwrap_or(0),
                    ));
                }
            }
        }
    }
    found
}

/// The session a mention refers to, if the desktop knows it. A name can match several; the live one wins.
pub fn resolve(mention: &Mention) -> Option<Peer> {
    let mut all = records();
    all.sort_by_key(|(_, archived, active)| (!*archived, *active));
    all.into_iter()
        .rev()
        .find(|(peer, _, _)| match mention {
            Mention::Id(id) => &peer.id == id,
            Mention::Name(name) => peer.title.eq_ignore_ascii_case(name),
        })
        .map(|(peer, _, _)| peer)
}

/// Show that session in the Claude desktop app.
pub fn open(id: &str) -> Result<(), String> {
    if !is_session_id(id) {
        return Err("not a session".into());
    }
    Command::new("open").arg(format!("claude://claude.ai/epitaxy/{id}")).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "local_00000000-1111-2222-3333-444444444444";

    #[test]
    fn ids_are_strict() {
        assert!(is_session_id(ID));
        assert!(!is_session_id("local_../../x"));
        assert!(!is_session_id(&format!("{ID}x")));
        assert!(!is_session_id("00000000-1111-2222-3333-444444444444"));
    }

    #[test]
    fn reads_an_incoming_message() {
        let line = format!(
            r#"{{"type":"user","message":{{"content":"Another Claude session sent a message: <cross-session-message from=\"uds:/tmp/cc-socks/1.sock\" from-session=\"{ID}\" from-name=\"Example session\" from-mode=\"prompting\"> hi </cross-session-message>"}}}}"#
        );
        assert_eq!(mention(&line), Some(Mention::Id(ID.into())));
        let no_id = line.replace(&format!(" from-session=\\\"{ID}\\\""), "");
        assert_eq!(mention(&no_id), Some(Mention::Name("Example session".into())));
    }

    #[test]
    fn reads_an_outgoing_message() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"SendMessage","input":{"to":"Example session","message":"hi"}}]}}"#;
        assert_eq!(mention(line), Some(Mention::Name("Example session".into())));
    }

    #[test]
    fn ignores_everything_else() {
        assert_eq!(mention(r#"{"type":"user","message":{"content":"hello"}}"#), None);
        assert_eq!(mention(r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"to":"x"}}]}}"#), None);
        assert_eq!(mention("not json"), None);
    }

    #[test]
    fn will_not_open_anything_but_a_session() {
        assert!(open("local_x; rm -rf /").is_err());
        assert!(open("https://example.com").is_err());
    }
}
