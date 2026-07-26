use serde_json::{json, Value};
use std::path::PathBuf;

const HOOK_EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "SubagentStop",
    "SessionEnd",
    "PreCompact",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct HookStatus {
    pub registered: bool,
    pub settings_path: String,
    pub exe_path: String,
}

fn claude_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

fn hook_command(exe_path: &str) -> String {
    format!("\"{}\" hook", exe_path)
}

fn is_needle_command(command: &str) -> bool {
    command.to_lowercase().contains("needle")
}

fn read_root(path: &PathBuf) -> Value {
    if !path.exists() {
        return json!({});
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_root(path: &PathBuf, root: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let _ = std::fs::copy(path, path.with_extension("json.needle-backup"));
    }
    let pretty = serde_json::to_string_pretty(root).unwrap_or_default();
    std::fs::write(path, pretty)
}

/// Garante que cada evento de hook relevante tenha uma entrada apontando
/// pro executável atual do Needle (`<exe> hook`). Remove qualquer entrada
/// antiga do Needle antes de inserir a atual, então é seguro chamar isso
/// toda vez que o app sobe (idempotente, também corrige o caminho depois
/// de uma reinstalação/atualização em outro diretório).
pub fn ensure_hooks_registered(exe_path: &str) -> std::io::Result<bool> {
    let Some(path) = claude_settings_path() else {
        return Ok(false);
    };

    let before = read_root(&path);
    let before_str = serde_json::to_string(&before).unwrap_or_default();

    let mut root = before;
    let root_obj = root.as_object_mut().expect("root é sempre um objeto JSON");
    let hooks_val = root_obj
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks_obj = hooks_val.as_object_mut().expect("hooks é sempre um objeto JSON");

    let command = hook_command(exe_path);

    for event in HOOK_EVENTS {
        let entries = hooks_obj
            .entry(event)
            .or_insert_with(|| json!([]));
        let arr = entries.as_array_mut().expect("entrada de hook é sempre um array");

        arr.retain(|group| !group_has_needle_command(group));
        arr.push(json!({
            "hooks": [{ "type": "command", "command": command }]
        }));
    }

    let after_str = serde_json::to_string(&root).unwrap_or_default();
    let changed = after_str != before_str;
    if changed {
        write_root(&path, &root)?;
    }
    Ok(changed)
}

/// Remove todas as entradas de hook do Needle do settings.json, sem mexer
/// em hooks de outras ferramentas.
pub fn remove_hooks() -> std::io::Result<bool> {
    let Some(path) = claude_settings_path() else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }

    let before = read_root(&path);
    let before_str = serde_json::to_string(&before).unwrap_or_default();

    let mut root = before;
    if let Some(hooks_obj) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for event in HOOK_EVENTS {
            if let Some(entries) = hooks_obj.get_mut(event).and_then(|e| e.as_array_mut()) {
                entries.retain(|group| !group_has_needle_command(group));
            }
        }
    }

    let after_str = serde_json::to_string(&root).unwrap_or_default();
    let changed = after_str != before_str;
    if changed {
        write_root(&path, &root)?;
    }
    Ok(changed)
}

fn group_has_needle_command(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|inner| {
            inner.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(is_needle_command)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn status(exe_path: &str) -> HookStatus {
    let path = claude_settings_path();
    let registered = path
        .as_ref()
        .map(|p| {
            let root = read_root(p);
            HOOK_EVENTS.iter().all(|event| {
                root.get("hooks")
                    .and_then(|h| h.get(event))
                    .and_then(|e| e.as_array())
                    .map(|arr| arr.iter().any(group_has_needle_command))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    HookStatus {
        registered,
        settings_path: path.map(|p| p.display().to_string()).unwrap_or_default(),
        exe_path: exe_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_command_wraps_path_and_subcommand() {
        assert_eq!(hook_command("C:/Program Files/Needle/needle.exe"), "\"C:/Program Files/Needle/needle.exe\" hook");
    }

    #[test]
    fn is_needle_command_matches_case_insensitively() {
        assert!(is_needle_command("\"C:/Foo/NEEDLE.exe\" hook"));
        assert!(!is_needle_command("node other-tool.js"));
    }

    #[test]
    fn group_has_needle_command_detects_nested_entries() {
        let group = json!({ "hooks": [{ "type": "command", "command": "\"needle.exe\" hook" }] });
        assert!(group_has_needle_command(&group));

        let other = json!({ "hooks": [{ "type": "command", "command": "node other.js" }] });
        assert!(!group_has_needle_command(&other));
    }
}
