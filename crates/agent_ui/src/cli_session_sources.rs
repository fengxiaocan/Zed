use std::{
    env,
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::cli_session_manager::{
    CliResumeRequest, CliSessionCapabilities, CliSessionCatalog, CliSessionDetails, CliSessionKey,
    CliSessionMessage, CliSessionMessageRole, CliSessionProvider, CliSessionSummary, CliTool,
    resume_arguments,
};

#[derive(Clone, Copy)]
enum JsonlKind {
    Codex,
    ClaudeCode,
}

struct JsonlSessionProvider {
    kind: JsonlKind,
    source_root: PathBuf,
    search_root: PathBuf,
}

impl JsonlSessionProvider {
    fn codex(root: PathBuf) -> Self {
        Self {
            kind: JsonlKind::Codex,
            search_root: root.clone(),
            source_root: root,
        }
    }

    fn claude_code(root: PathBuf) -> Self {
        Self {
            kind: JsonlKind::ClaudeCode,
            search_root: root.join("projects"),
            source_root: root,
        }
    }

    fn tool_id(&self) -> CliTool {
        match self.kind {
            JsonlKind::Codex => CliTool::Codex,
            JsonlKind::ClaudeCode => CliTool::ClaudeCode,
        }
    }

    fn session_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        match self.kind {
            JsonlKind::Codex => {
                collect_jsonl_files(&self.search_root.join("sessions"), &mut files, 0);
                collect_jsonl_files(&self.search_root.join("archived_sessions"), &mut files, 0);
            }
            JsonlKind::ClaudeCode => collect_jsonl_files(&self.search_root, &mut files, 0),
        }
        if matches!(self.kind, JsonlKind::ClaudeCode) {
            files.retain(|path| {
                !path
                    .components()
                    .any(|component| component.as_os_str() == "subagents")
            });
        }
        files.sort();
        files
    }

    fn parse_path(&self, path: &Path) -> anyhow::Result<ParsedTranscript> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let fallback_id = path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_string();
        let mut transcript = ParsedTranscript::new(fallback_id);

        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => {
                    transcript.is_partial = true;
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let entry = match serde_json::from_str::<Value>(&line) {
                Ok(entry) => entry,
                Err(_) => {
                    // A streaming writer may leave its final line incomplete. Retain the valid
                    // messages and mark the transcript partial instead of losing the session.
                    transcript.is_partial = true;
                    continue;
                }
            };
            transcript.absorb(&entry, self.kind);
        }

        if transcript.title.is_empty() {
            transcript.title = format!("{} session", self.tool_id().display_name());
        }
        Ok(transcript)
    }

    fn find_session_file(&self, key: &CliSessionKey) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            key.tool == self.tool_id() && key.source_root == self.source_root,
            "session key does not belong to this CLI source"
        );
        for path in self.session_files() {
            if path
                .file_stem()
                .and_then(OsStr::to_str)
                .is_some_and(|id| id == key.id)
            {
                return Ok(path);
            }
            if let Ok(transcript) = self.parse_path(&path)
                && transcript.id == key.id
            {
                return Ok(path);
            }
        }
        anyhow::bail!("the selected session file is no longer available")
    }

    fn summary(&self, path: &Path, transcript: &ParsedTranscript) -> CliSessionSummary {
        let program_name = match self.kind {
            JsonlKind::Codex => "codex",
            JsonlKind::ClaudeCode => "claude",
        };
        let mut capabilities = CliSessionCapabilities::full();
        if find_program_on_path(OsStr::new(program_name)).is_none() {
            capabilities.can_resume = false;
            capabilities.limitation = Some(format!(
                "{} is not installed or not available on PATH.",
                self.tool().display_name()
            ));
        }
        CliSessionSummary {
            key: CliSessionKey {
                tool: self.tool_id(),
                source_root: self.source_root.clone(),
                id: transcript.id.clone(),
            },
            title: transcript.title.clone(),
            working_directory: transcript.working_directory.clone(),
            created_at: transcript.created_at,
            last_activity_at: transcript.last_activity_at,
            summary: transcript.preview.clone(),
            message_count: transcript.messages.len(),
            is_archived: matches!(self.kind, JsonlKind::Codex)
                && path
                    .components()
                    .any(|component| component.as_os_str() == "archived_sessions"),
            capabilities,
        }
    }
}

impl CliSessionProvider for JsonlSessionProvider {
    fn tool(&self) -> CliTool {
        self.tool_id()
    }

    fn source_root(&self) -> &Path {
        &self.source_root
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
        let mut sessions = Vec::new();
        for path in self.session_files() {
            let Ok(transcript) = self.parse_path(&path) else {
                continue;
            };
            sessions.push(self.summary(&path, &transcript));
        }
        Ok(sessions)
    }

    fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
        let path = self.find_session_file(key)?;
        let transcript = self.parse_path(&path)?;
        Ok(CliSessionDetails {
            summary: self.summary(&path, &transcript),
            messages: transcript.messages,
            is_partial: transcript.is_partial,
        })
    }

    fn resume_request(&self, key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
        let details = self.read_session(key)?;
        let program_name = match self.kind {
            JsonlKind::Codex => "codex",
            JsonlKind::ClaudeCode => "claude",
        };
        Ok(CliResumeRequest {
            program: program_on_path(program_name)?,
            arguments: resume_arguments(&key.tool, &key.id),
            working_directory: details.summary.working_directory,
            environment: Vec::new(),
        })
    }
}

struct ParsedTranscript {
    id: String,
    title: String,
    preview: String,
    working_directory: Option<PathBuf>,
    created_at: Option<DateTime<Utc>>,
    last_activity_at: Option<DateTime<Utc>>,
    messages: Vec<CliSessionMessage>,
    is_partial: bool,
}

impl ParsedTranscript {
    fn new(id: String) -> Self {
        Self {
            id,
            title: String::new(),
            preview: String::new(),
            working_directory: None,
            created_at: None,
            last_activity_at: None,
            messages: Vec::new(),
            is_partial: false,
        }
    }

    fn absorb(&mut self, entry: &Value, kind: JsonlKind) {
        let event_type = string_at(entry, &["type"]);
        let payload = entry.get("payload").unwrap_or(&Value::Null);

        if matches!(kind, JsonlKind::Codex) && event_type == Some("session_meta") {
            if let Some(id) = string_at(payload, &["id"]) {
                self.id = id.to_string();
            }
            if let Some(cwd) = string_at(payload, &["cwd"]) {
                self.working_directory = Some(PathBuf::from(cwd));
            }
            self.record_created_time(string_at(payload, &["timestamp"]).and_then(parse_timestamp));
            return;
        }

        let mut timestamp =
            string_at(entry, &["timestamp", "created_at"]).and_then(parse_timestamp);

        let (role, content) = match kind {
            JsonlKind::Codex => {
                let (role, content) = if event_type == Some("event_msg") {
                    let subtype = string_at(payload, &["type"]);
                    let role = match subtype {
                        Some("user_message") => Some(CliSessionMessageRole::User),
                        Some("agent_message") => Some(CliSessionMessageRole::Assistant),
                        _ => None,
                    };
                    (role, payload.get("message").or_else(|| payload.get("text")))
                } else if event_type == Some("response_item")
                    && string_at(payload, &["type"]) == Some("message")
                {
                    (
                        role_from(string_at(payload, &["role"])),
                        payload.get("content"),
                    )
                } else {
                    (
                        role_from(event_type),
                        entry.get("message").or_else(|| entry.get("content")),
                    )
                };
                if timestamp.is_none() {
                    timestamp = string_at(payload, &["timestamp"]).and_then(parse_timestamp);
                }
                if self.working_directory.is_none()
                    && let Some(cwd) = string_at(payload, &["cwd"])
                {
                    self.working_directory = Some(PathBuf::from(cwd));
                }
                (role, content)
            }
            JsonlKind::ClaudeCode => {
                let role = role_from(event_type);
                let message = entry.get("message").unwrap_or(&Value::Null);
                let content = message.get("content").or_else(|| entry.get("content"));
                if timestamp.is_none() {
                    timestamp = string_at(entry, &["timestamp"]).and_then(parse_timestamp);
                }
                if let Some(id) = string_at(entry, &["sessionId", "session_id"])
                    && !id.is_empty()
                {
                    self.id = id.to_string();
                }
                if self.working_directory.is_none()
                    && let Some(cwd) = string_at(entry, &["cwd", "projectPath"])
                {
                    self.working_directory = Some(PathBuf::from(cwd));
                }
                (role, content)
            }
        };

        let Some(role) = role else { return };
        let text = content.map(extract_text).unwrap_or_default();
        if text.trim().is_empty() {
            return;
        }
        self.record_time(timestamp);
        if role == CliSessionMessageRole::User && self.title.is_empty() {
            self.title = first_line(&text, 96);
        }
        if self.preview.is_empty() {
            self.preview = first_line(&text, 180);
        }
        self.messages.push(CliSessionMessage {
            role,
            text,
            timestamp,
        });
    }

    fn record_time(&mut self, timestamp: Option<DateTime<Utc>>) {
        let Some(timestamp) = timestamp else { return };
        self.record_created_time(Some(timestamp));
        if self
            .last_activity_at
            .is_none_or(|activity| timestamp > activity)
        {
            self.last_activity_at = Some(timestamp);
        }
    }

    fn record_created_time(&mut self, timestamp: Option<DateTime<Utc>>) {
        let Some(timestamp) = timestamp else { return };
        if self.created_at.is_none_or(|created| timestamp < created) {
            self.created_at = Some(timestamp);
        }
    }
}

struct GrokBuildProvider {
    source_root: PathBuf,
    working_directory: PathBuf,
}

impl CliSessionProvider for GrokBuildProvider {
    fn tool(&self) -> CliTool {
        CliTool::GrokBuild
    }

    fn source_root(&self) -> &Path {
        &self.source_root
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
        let program = program_on_path("grok")?;
        let output = Command::new(program)
            .arg("--no-auto-update")
            .args(["sessions", "list", "-n", "200"])
            .current_dir(&self.working_directory)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "Grok Build could not list sessions: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let text = String::from_utf8_lossy(&output.stdout);
        let ids_and_titles = parse_grok_session_listing(&text);
        Ok(ids_and_titles
            .into_iter()
            .map(|(id, title)| self.session_summary(id, title))
            .collect())
    }

    fn search_sessions(&self, query: &str) -> anyhow::Result<Option<Vec<CliSessionSummary>>> {
        let output = Command::new(program_on_path("grok")?)
            .arg("--no-auto-update")
            .args(["sessions", "search"])
            .arg(query)
            .current_dir(&self.working_directory)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "Grok Build could not search sessions: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(Some(
            parse_grok_session_listing(&text)
                .into_iter()
                .map(|(id, title)| self.session_summary(id, title))
                .collect(),
        ))
    }

    fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
        self.ensure_key(key)?;
        let output = Command::new(program_on_path("grok")?)
            .arg("--no-auto-update")
            .arg("export")
            .arg(&key.id)
            .current_dir(&self.working_directory)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "Grok Build could not export this session: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let markdown = String::from_utf8_lossy(&output.stdout).into_owned();
        let title = parse_grok_export_title(&markdown)
            .unwrap_or_else(|| format!("Grok Build session {}", key.id));
        let summary = self.session_summary(key.id.clone(), title);
        Ok(CliSessionDetails {
            summary,
            messages: vec![CliSessionMessage {
                role: CliSessionMessageRole::Transcript,
                text: markdown,
                timestamp: None,
            }],
            is_partial: true,
        })
    }

    fn resume_request(&self, key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
        self.ensure_key(key)?;
        Ok(CliResumeRequest {
            program: program_on_path("grok")?,
            arguments: resume_arguments(&key.tool, &key.id),
            working_directory: Some(self.working_directory.clone()),
            environment: Vec::new(),
        })
    }
}

impl GrokBuildProvider {
    fn session_summary(&self, id: String, title: String) -> CliSessionSummary {
        CliSessionSummary {
            key: CliSessionKey {
                tool: CliTool::GrokBuild,
                source_root: self.source_root.clone(),
                id,
            },
            title,
            working_directory: Some(self.working_directory.clone()),
            created_at: None,
            last_activity_at: None,
            summary: String::new(),
            message_count: 0,
            is_archived: false,
            capabilities: CliSessionCapabilities {
                can_read_messages: true,
                can_search_messages: true,
                can_resume: true,
                limitation: Some(
                    "Grok Build sessions are scoped to the selected workspace. The initial list shows up to 200 recent sessions; search uses Grok's native title and prompt search."
                        .to_string(),
                ),
            },
        }
    }

    fn ensure_key(&self, key: &CliSessionKey) -> anyhow::Result<()> {
        anyhow::ensure!(
            key.tool == CliTool::GrokBuild && key.source_root == self.source_root,
            "session key does not belong to this Grok Build source"
        );
        Ok(())
    }
}

struct AntigravityProvider {
    source_root: PathBuf,
    cache_file: PathBuf,
}

impl CliSessionProvider for AntigravityProvider {
    fn tool(&self) -> CliTool {
        CliTool::Antigravity
    }

    fn source_root(&self) -> &Path {
        &self.source_root
    }

    fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
        let contents = match fs::read_to_string(&self.cache_file) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let entries = serde_json::from_str::<serde_json::Map<String, Value>>(&contents)?;
        Ok(entries
            .into_iter()
            .filter_map(|(working_directory, id)| {
                let id = id.as_str()?.to_string();
                let directory = PathBuf::from(&working_directory);
                let mut capabilities = CliSessionCapabilities::recent_reference_only(
                    if find_program_on_path(OsStr::new("agy")).is_some() {
                        "Antigravity's local cache contains only the latest conversation ID for each workspace; message history is fetched by the CLI from its backend."
                    } else {
                        "Antigravity's local cache contains only the latest conversation ID for each workspace; message history is fetched by the CLI from its backend, and agy is not available on PATH."
                    },
                );
                capabilities.can_resume = find_program_on_path(OsStr::new("agy")).is_some();
                Some(CliSessionSummary {
                    key: CliSessionKey {
                        tool: CliTool::Antigravity,
                        source_root: self.source_root.clone(),
                        id,
                    },
                    title: format!(
                        "Recent Antigravity session — {}",
                        directory
                            .file_name()
                            .and_then(OsStr::to_str)
                            .unwrap_or(&working_directory)
                    ),
                    working_directory: Some(directory),
                    created_at: None,
                    last_activity_at: None,
                    summary: String::new(),
                    message_count: 0,
                    is_archived: false,
                    capabilities,
                })
            })
            .collect())
    }

    fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
        self.ensure_key(key)?;
        let summary = self
            .list_sessions()?
            .into_iter()
            .find(|session| session.key == *key)
            .ok_or_else(|| {
                anyhow::anyhow!("the selected Antigravity reference is no longer available")
            })?;
        Ok(CliSessionDetails {
            summary,
            messages: Vec::new(),
            is_partial: true,
        })
    }

    fn resume_request(&self, key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
        let details = self.read_session(key)?;
        Ok(CliResumeRequest {
            program: program_on_path("agy")?,
            arguments: resume_arguments(&key.tool, &key.id),
            working_directory: details.summary.working_directory,
            environment: Vec::new(),
        })
    }
}

impl AntigravityProvider {
    fn ensure_key(&self, key: &CliSessionKey) -> anyhow::Result<()> {
        anyhow::ensure!(
            key.tool == CliTool::Antigravity && key.source_root == self.source_root,
            "session key does not belong to this Antigravity source"
        );
        Ok(())
    }
}

/// Builds providers for the user's default CLI data roots and the active workspace.
/// Data reads stay local; Grok uses its own read-only list/export commands.
pub fn default_cli_session_catalog(active_working_directory: Option<PathBuf>) -> CliSessionCatalog {
    let home = user_home_directory().unwrap_or_else(|| PathBuf::from("."));
    let codex_home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let claude_home = env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    let grok_home = env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".grok"));
    let antigravity_root = home.join(".gemini").join("antigravity-cli");
    let cwd = active_working_directory
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    CliSessionCatalog::new(vec![
        Box::new(JsonlSessionProvider::codex(codex_home)),
        Box::new(JsonlSessionProvider::claude_code(claude_home)),
        Box::new(GrokBuildProvider {
            source_root: grok_home,
            working_directory: cwd,
        }),
        Box::new(AntigravityProvider {
            source_root: antigravity_root.clone(),
            cache_file: antigravity_root
                .join("cache")
                .join("last_conversations.json"),
        }),
    ])
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 64 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_jsonl_files(&entry.path(), files, depth + 1);
        } else if file_type.is_file()
            && entry.path().extension().and_then(OsStr::to_str) == Some("jsonl")
        {
            files.push(entry.path());
        }
    }
}

fn string_at<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn role_from(role: Option<&str>) -> Option<CliSessionMessageRole> {
    match role {
        Some("user") => Some(CliSessionMessageRole::User),
        Some("assistant") | Some("agent") => Some(CliSessionMessageRole::Assistant),
        _ => None,
    }
}

fn extract_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str);
                if !matches!(kind, Some("text" | "input_text" | "output_text")) {
                    return None;
                }
                part.get("text").and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn first_line(text: &str, max_chars: usize) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(max_chars)
        .collect()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn parse_grok_session_listing(text: &str) -> Vec<(String, String)> {
    let mut sessions = Vec::new();
    for line in text.lines() {
        let Some(id) = line
            .split_whitespace()
            .map(|token| token.trim_matches(['|', '[', ']', '(', ')', ',']))
            .find(|candidate| looks_like_session_id(candidate))
        else {
            continue;
        };
        let title = line
            .replace(id, "")
            .trim()
            .trim_matches(['|', '—', '-', ' '])
            .to_string();
        sessions.push((
            id.to_string(),
            if title.is_empty() {
                id.to_string()
            } else {
                title
            },
        ));
    }
    sessions
}

fn parse_grok_export_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_start_matches('#').trim().to_string())
        .filter(|title| !title.is_empty())
}

fn looks_like_session_id(candidate: &str) -> bool {
    candidate.len() >= 32
        && candidate
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || ch == '-')
        && candidate.contains('-')
}

fn user_home_directory() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

fn program_on_path(name: &str) -> anyhow::Result<PathBuf> {
    find_program_on_path(OsStr::new(name))
        .ok_or_else(|| anyhow::anyhow!("{} was not found on PATH", name))
}

fn find_program_on_path(name: &OsStr) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    #[cfg(target_os = "windows")]
    let suffixes = ["", ".exe", ".cmd", ".bat"];
    #[cfg(not(target_os = "windows"))]
    let suffixes = [""];
    for directory in env::split_paths(&path) {
        for suffix in suffixes {
            let mut candidate = directory.join(name);
            if !suffix.is_empty() {
                candidate.set_extension(suffix.trim_start_matches('.'));
            }
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn codex_jsonl_metadata_and_messages_are_reconstructed() {
        let directory = tempdir().expect("temporary directory is created");
        let sessions_dir = directory.path().join("sessions").join("2026").join("10");
        fs::create_dir_all(&sessions_dir).expect("sessions directory is created");
        let path = sessions_dir.join("codex-id.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-id\",\"cwd\":\"C:\\\\work\\\\repo\",\"timestamp\":\"2026-10-17T04:00:00Z\"}}\n",
                "{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"Fix login timeout\"}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"The login request timed out.\"}]}}\n"
            ),
        )
        .expect("fixture is written");

        let provider = JsonlSessionProvider::codex(directory.path().to_path_buf());
        let sessions = provider.list_sessions().expect("sessions are listed");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].key.id, "codex-id");
        assert_eq!(sessions[0].title, "Fix login timeout");
        assert_eq!(
            sessions[0].working_directory,
            Some(PathBuf::from(r"C:\work\repo"))
        );
        assert_eq!(sessions[0].message_count, 2);
    }

    #[test]
    fn claude_jsonl_ignores_tool_blocks_and_keeps_text_messages() {
        let directory = tempdir().expect("temporary directory is created");
        let projects = directory.path().join("projects").join("work-repo");
        fs::create_dir_all(&projects).expect("project directory is created");
        fs::write(
            projects.join("claude-id.jsonl"),
            concat!(
                "{\"type\":\"user\",\"sessionId\":\"claude-id\",\"cwd\":\"C:\\\\work\\\\repo\",\"timestamp\":\"2026-10-17T04:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"Find the auth bug\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"name\":\"Read\"},{\"type\":\"text\",\"text\":\"The auth check is missing.\"}]}}\n"
            ),
        )
        .expect("fixture is written");

        let provider = JsonlSessionProvider::claude_code(directory.path().to_path_buf());
        let session = provider
            .list_sessions()
            .expect("sessions are listed")
            .remove(0);
        let details = provider
            .read_session(&session.key)
            .expect("session is readable");

        assert_eq!(details.messages.len(), 2);
        assert_eq!(details.messages[1].text, "The auth check is missing.");
        assert_eq!(
            details.summary.created_at,
            Some("2026-10-17T04:00:00Z".parse().unwrap())
        );
    }

    #[test]
    fn malformed_trailing_jsonl_is_retained_as_partial() {
        let directory = tempdir().expect("temporary directory is created");
        let sessions_dir = directory.path().join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions directory is created");
        let path = sessions_dir.join("session.jsonl");
        fs::write(
            &path,
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"hello\"}}\n{\"type\":",
        )
        .expect("fixture is written");

        let provider = JsonlSessionProvider::codex(directory.path().to_path_buf());
        let session = provider
            .list_sessions()
            .expect("sessions are listed")
            .remove(0);
        let details = provider
            .read_session(&session.key)
            .expect("valid messages remain readable");

        assert_eq!(details.messages.len(), 1);
        assert!(details.is_partial);
    }

    #[test]
    fn grok_listing_parser_ignores_headers_and_extracts_session_ids() {
        let id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let sessions = parse_grok_session_listing(&format!(
            "Sessions for C:\\work\\repo\n{id}  Fix checkout retry\n"
        ));

        assert_eq!(
            sessions,
            vec![(id.to_string(), "Fix checkout retry".to_string())]
        );
    }

    #[test]
    fn antigravity_cache_is_marked_as_a_recent_reference_only() {
        let directory = tempdir().expect("temporary directory is created");
        let cache_file = directory.path().join("last_conversations.json");
        fs::write(&cache_file, r#"{"C:\\work\\repo":"conversation-id"}"#)
            .expect("fixture is written");
        let provider = AntigravityProvider {
            source_root: directory.path().to_path_buf(),
            cache_file,
        };

        let session = provider.list_sessions().expect("cache is read").remove(0);
        let details = provider
            .read_session(&session.key)
            .expect("recent cache reference can be opened");

        assert_eq!(session.key.id, "conversation-id");
        assert!(!session.capabilities.can_read_messages);
        assert!(!session.capabilities.can_search_messages);
        assert!(
            session
                .capabilities
                .limitation
                .as_deref()
                .is_some_and(|note| note.contains("only the latest conversation ID"))
        );
        assert!(details.messages.is_empty());
        assert!(details.is_partial);
    }
}
