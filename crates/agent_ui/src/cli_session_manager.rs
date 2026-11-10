use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
};

use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CliTool {
    Codex,
    ClaudeCode,
    GrokBuild,
    Antigravity,
}

impl CliTool {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::GrokBuild => "Grok Build",
            Self::Antigravity => "Antigravity",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CliSessionKey {
    pub tool: CliTool,
    pub source_root: PathBuf,
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliSessionMessageRole {
    User,
    Assistant,
    Transcript,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliSessionMessage {
    pub role: CliSessionMessageRole,
    pub text: String,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliSessionCapabilities {
    pub can_read_messages: bool,
    pub can_search_messages: bool,
    pub can_resume: bool,
    pub limitation: Option<String>,
}

impl CliSessionCapabilities {
    pub fn full() -> Self {
        Self {
            can_read_messages: true,
            can_search_messages: true,
            can_resume: true,
            limitation: None,
        }
    }

    pub fn recent_reference_only(limitation: impl Into<String>) -> Self {
        Self {
            can_read_messages: false,
            can_search_messages: false,
            can_resume: true,
            limitation: Some(limitation.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliSessionSummary {
    pub key: CliSessionKey,
    pub title: String,
    pub working_directory: Option<PathBuf>,
    pub created_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub summary: String,
    pub message_count: usize,
    pub is_archived: bool,
    pub capabilities: CliSessionCapabilities,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliSessionDetails {
    pub summary: CliSessionSummary,
    pub messages: Vec<CliSessionMessage>,
    pub is_partial: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliResumeRequest {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
}

pub fn resume_arguments(tool: &CliTool, session_id: &str) -> Vec<OsString> {
    let (resume_flag, session_argument) = match tool {
        CliTool::Codex => ("resume", session_id),
        CliTool::ClaudeCode => ("--resume", session_id),
        CliTool::GrokBuild => ("--resume", session_id),
        CliTool::Antigravity => ("--conversation", session_id),
    };
    vec![resume_flag.into(), session_argument.into()]
}

pub trait CliSessionProvider: Send + Sync {
    fn tool(&self) -> CliTool;
    fn source_root(&self) -> &Path;
    fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>>;
    /// Returns native search matches when the CLI provides a cheaper or broader search path.
    /// `None` asks the catalog to search readable transcripts instead.
    fn search_sessions(&self, _query: &str) -> anyhow::Result<Option<Vec<CliSessionSummary>>> {
        Ok(None)
    }
    fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails>;
    fn resume_request(&self, key: &CliSessionKey) -> anyhow::Result<CliResumeRequest>;
}

pub trait CliSessionLauncher: Send + Sync {
    /// Starts a new operating-system terminal window and submits the resume request.
    fn launch(&self, request: &CliResumeRequest) -> anyhow::Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliSessionSearchResult {
    pub session: CliSessionSummary,
    pub matching_message_indices: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliSessionSearchSnapshot {
    pub results: Vec<CliSessionSearchResult>,
    pub source_issues: Vec<String>,
}

pub struct CliSessionCatalog {
    providers: Vec<Box<dyn CliSessionProvider>>,
}

impl CliSessionCatalog {
    pub fn new(providers: Vec<Box<dyn CliSessionProvider>>) -> Self {
        Self { providers }
    }

    pub fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
        Ok(self.list_snapshot().sessions)
    }

    pub fn list_snapshot(&self) -> CliSessionCatalogSnapshot {
        let mut sessions = Vec::new();
        let mut source_issues = Vec::new();
        for provider in &self.providers {
            match provider.list_sessions() {
                Ok(provider_sessions) => {
                    for session in &provider_sessions {
                        if let Some(limitation) = &session.capabilities.limitation
                            && !source_issues.contains(limitation)
                        {
                            source_issues.push(limitation.clone());
                        }
                    }
                    sessions.extend(provider_sessions);
                }
                Err(error) => {
                    source_issues.push(format!("{}: {error}", provider.tool().display_name()))
                }
            }
        }
        sessions.sort_by(|left, right| {
            right
                .last_activity_at
                .cmp(&left.last_activity_at)
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| {
                    left.key
                        .tool
                        .display_name()
                        .cmp(right.key.tool.display_name())
                })
                .then_with(|| left.key.source_root.cmp(&right.key.source_root))
                .then_with(|| left.key.id.cmp(&right.key.id))
        });
        CliSessionCatalogSnapshot {
            sessions,
            source_issues,
        }
    }

    pub fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
        self.provider_for(key)?.read_session(key)
    }

    pub fn search(&self, query: &str) -> anyhow::Result<Vec<CliSessionSearchResult>> {
        Ok(self.search_snapshot(query)?.results)
    }

    pub fn search_snapshot(&self, query: &str) -> anyhow::Result<CliSessionSearchSnapshot> {
        let query = query.trim();
        let normalized_query = query.to_lowercase();
        let mut snapshot = CliSessionSearchSnapshot::default();
        let catalog_snapshot = self.list_snapshot();
        snapshot.source_issues = catalog_snapshot.source_issues;
        let mut results = std::collections::HashMap::new();
        for session in &catalog_snapshot.sessions {
            let working_directory = session
                .working_directory
                .as_deref()
                .map(|path| path.to_string_lossy());
            let metadata_matches = query.is_empty()
                || [
                    session.title.to_lowercase(),
                    session.summary.to_lowercase(),
                    session.key.id.to_lowercase(),
                    working_directory
                        .as_ref()
                        .map(|path| path.to_lowercase())
                        .unwrap_or_default(),
                ]
                .iter()
                .any(|value| value.contains(&normalized_query));

            if query.is_empty() || metadata_matches {
                results.insert(
                    session.key.clone(),
                    CliSessionSearchResult {
                        session: session.clone(),
                        matching_message_indices: Vec::new(),
                    },
                );
            }
        }

        if !query.is_empty() {
            for provider in &self.providers {
                match provider.search_sessions(&query) {
                    Ok(Some(native_matches)) => {
                        for session in native_matches {
                            if session.key.tool == provider.tool()
                                && session.key.source_root == provider.source_root()
                            {
                                results.entry(session.key.clone()).or_insert_with(|| {
                                    CliSessionSearchResult {
                                        session,
                                        matching_message_indices: Vec::new(),
                                    }
                                });
                            }
                        }
                    }
                    Ok(None) => {
                        for session in catalog_snapshot.sessions.iter().filter(|session| {
                            session.key.tool == provider.tool()
                                && session.key.source_root == provider.source_root()
                                && session.capabilities.can_search_messages
                        }) {
                            match provider.read_session(&session.key) {
                                Ok(details) => {
                                    let matching_message_indices = details
                                        .messages
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, message)| {
                                            message.text.to_lowercase().contains(&normalized_query)
                                        })
                                        .map(|(index, _)| index)
                                        .collect::<Vec<_>>();
                                    if !matching_message_indices.is_empty() {
                                        results
                                            .entry(session.key.clone())
                                            .and_modify(|result: &mut CliSessionSearchResult| {
                                                result.matching_message_indices =
                                                    matching_message_indices.clone();
                                            })
                                            .or_insert_with(|| CliSessionSearchResult {
                                                session: session.clone(),
                                                matching_message_indices,
                                            });
                                    }
                                }
                                Err(error) => {
                                    let issue = format!(
                                        "{} session {}: full-text search unavailable: {error}",
                                        session.key.tool.display_name(),
                                        session.key.id
                                    );
                                    if !snapshot.source_issues.contains(&issue) {
                                        snapshot.source_issues.push(issue);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let issue = format!(
                            "{}: native session search failed: {error}",
                            provider.tool().display_name()
                        );
                        if !snapshot.source_issues.contains(&issue) {
                            snapshot.source_issues.push(issue);
                        }
                    }
                }
            }
        }

        let mut results = results.into_values().collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .session
                .last_activity_at
                .cmp(&left.session.last_activity_at)
                .then_with(|| left.session.title.cmp(&right.session.title))
                .then_with(|| {
                    left.session
                        .key
                        .tool
                        .display_name()
                        .cmp(right.session.key.tool.display_name())
                })
                .then_with(|| {
                    left.session
                        .key
                        .source_root
                        .cmp(&right.session.key.source_root)
                })
                .then_with(|| left.session.key.id.cmp(&right.session.key.id))
        });
        snapshot.results = results;
        Ok(snapshot)
    }

    pub fn continue_session(
        &self,
        key: &CliSessionKey,
        launcher: &dyn CliSessionLauncher,
    ) -> anyhow::Result<()> {
        let provider = self.provider_for(key)?;
        let request = provider.resume_request(key)?;
        launcher.launch(&request)
    }

    fn provider_for(&self, key: &CliSessionKey) -> anyhow::Result<&dyn CliSessionProvider> {
        for provider in &self.providers {
            if provider.tool() == key.tool && provider.source_root() == key.source_root {
                return Ok(provider.as_ref());
            }
        }
        anyhow::bail!("the selected CLI session is no longer available")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliSessionCatalogSnapshot {
    pub sessions: Vec<CliSessionSummary>,
    pub source_issues: Vec<String>,
}

/// Opens a new system terminal and invokes the selected CLI with its native resume arguments.
/// The first implementation targets native Windows; other platforms return a clear error.
#[derive(Default)]
pub struct SystemTerminalLauncher;

impl CliSessionLauncher for SystemTerminalLauncher {
    fn launch(&self, request: &CliResumeRequest) -> anyhow::Result<()> {
        let script = powershell_invocation(request)?;
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            script
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;

            let windows_terminal = find_executable(OsStr::new("wt.exe"));
            let use_windows_terminal = windows_terminal.is_some();
            let program = windows_terminal.unwrap_or_else(|| PathBuf::from("powershell.exe"));
            let mut command = Command::new(program);
            if use_windows_terminal {
                command.arg("-w").arg("new");
                if let Some(working_directory) = &request.working_directory {
                    command.arg("-d").arg(working_directory);
                }
                command.arg("powershell.exe");
            } else {
                if let Some(working_directory) = &request.working_directory {
                    command.current_dir(working_directory);
                }
                command.creation_flags(0x00000010); // CREATE_NEW_CONSOLE
            }
            command
                .arg("-NoLogo")
                .arg("-NoExit")
                .arg("-EncodedCommand")
                .arg(encoded);
            command.spawn()?;
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = encoded;
            anyhow::bail!("opening an external CLI terminal is currently supported on Windows only")
        }
    }
}

fn powershell_invocation(request: &CliResumeRequest) -> anyhow::Result<String> {
    let mut script = String::new();
    for (name, value) in &request.environment {
        let name = name.to_string_lossy();
        anyhow::ensure!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'),
            "invalid environment variable name in CLI resume request"
        );
        script.push_str("$env:");
        script.push_str(&name);
        script.push_str(" = ");
        script.push_str(&powershell_quote(&value.to_string_lossy()));
        script.push_str("; ");
    }
    script.push_str("& ");
    script.push_str(&powershell_quote(&request.program.to_string_lossy()));
    for argument in &request.arguments {
        script.push(' ');
        script.push_str(&powershell_quote(&argument.to_string_lossy()));
    }
    script.push_str(
        "; if ($LASTEXITCODE -ne 0) { Write-Host ('CLI exited with code ' + $LASTEXITCODE) }",
    );
    Ok(script)
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn find_executable(name: &OsStr) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let suffixes = ["", ".exe", ".cmd", ".bat"];
    for directory in std::env::split_paths(&path) {
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
    use std::{ffi::OsString, sync::Mutex};

    struct TestProvider {
        summary: CliSessionSummary,
        messages: Vec<CliSessionMessage>,
    }

    struct NativeSearchProvider {
        source_root: PathBuf,
        sessions: Vec<CliSessionSummary>,
        search_results: Vec<CliSessionSummary>,
        search_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        read_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    struct FailingProvider;

    impl CliSessionProvider for FailingProvider {
        fn tool(&self) -> CliTool {
            CliTool::GrokBuild
        }

        fn source_root(&self) -> &Path {
            Path::new("C:\\Users\\test\\.grok")
        }

        fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
            anyhow::bail!("fixture CLI is unavailable")
        }

        fn read_session(&self, _key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
            anyhow::bail!("fixture CLI is unavailable")
        }

        fn resume_request(&self, _key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
            anyhow::bail!("fixture CLI is unavailable")
        }
    }

    impl CliSessionProvider for TestProvider {
        fn tool(&self) -> CliTool {
            self.summary.key.tool.clone()
        }

        fn source_root(&self) -> &Path {
            &self.summary.key.source_root
        }

        fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
            Ok(vec![self.summary.clone()])
        }

        fn read_session(&self, key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
            anyhow::ensure!(key == &self.summary.key, "unexpected session key");
            Ok(CliSessionDetails {
                summary: self.summary.clone(),
                messages: self.messages.clone(),
                is_partial: false,
            })
        }

        fn resume_request(&self, key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
            anyhow::ensure!(key == &self.summary.key, "unexpected session key");
            Ok(CliResumeRequest {
                program: PathBuf::from("codex.exe"),
                arguments: vec![OsString::from("resume"), OsString::from(&key.id)],
                working_directory: self.summary.working_directory.clone(),
                environment: Vec::new(),
            })
        }
    }

    impl CliSessionProvider for NativeSearchProvider {
        fn tool(&self) -> CliTool {
            CliTool::GrokBuild
        }

        fn source_root(&self) -> &Path {
            &self.source_root
        }

        fn list_sessions(&self) -> anyhow::Result<Vec<CliSessionSummary>> {
            Ok(self.sessions.clone())
        }

        fn search_sessions(&self, _query: &str) -> anyhow::Result<Option<Vec<CliSessionSummary>>> {
            self.search_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Some(self.search_results.clone()))
        }

        fn read_session(&self, _key: &CliSessionKey) -> anyhow::Result<CliSessionDetails> {
            self.read_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            anyhow::bail!("native search must not export every session")
        }

        fn resume_request(&self, _key: &CliSessionKey) -> anyhow::Result<CliResumeRequest> {
            anyhow::bail!("not used by this test")
        }
    }

    #[derive(Default)]
    struct RecordingLauncher(Mutex<Option<CliResumeRequest>>);

    impl CliSessionLauncher for RecordingLauncher {
        fn launch(&self, request: &CliResumeRequest) -> anyhow::Result<()> {
            *self
                .0
                .lock()
                .map_err(|_| anyhow::anyhow!("launcher lock poisoned"))? = Some(request.clone());
            Ok(())
        }
    }

    fn test_catalog(session_id: &str, working_directory: &str) -> CliSessionCatalog {
        let summary = CliSessionSummary {
            key: CliSessionKey {
                tool: CliTool::Codex,
                source_root: PathBuf::from(r"C:\Users\test\.codex"),
                id: session_id.to_string(),
            },
            title: "Fix the terminal hang".to_string(),
            working_directory: Some(PathBuf::from(working_directory)),
            created_at: None,
            last_activity_at: None,
            summary: "The terminal stops responding after a retry.".to_string(),
            message_count: 1,
            is_archived: false,
            capabilities: CliSessionCapabilities::full(),
        };
        CliSessionCatalog::new(vec![Box::new(TestProvider {
            summary,
            messages: vec![CliSessionMessage {
                role: CliSessionMessageRole::Assistant,
                text: "重试后请求池仍持续增长。".to_string(),
                timestamp: None,
            }],
        })])
    }

    #[test]
    fn search_finds_terms_in_assistant_messages() {
        let catalog = test_catalog("older-session", r"C:\work\repo");

        let results = catalog.search("请求池").expect("search succeeds");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session.key.id, "older-session");
        assert_eq!(results[0].matching_message_indices, vec![0]);
    }

    #[test]
    fn native_provider_searches_once_without_reading_every_session() {
        let source_root = PathBuf::from(r"C:\Users\test\.grok");
        let make_summary = |id: &str, title: &str| CliSessionSummary {
            key: CliSessionKey {
                tool: CliTool::GrokBuild,
                source_root: source_root.clone(),
                id: id.to_string(),
            },
            title: title.to_string(),
            working_directory: Some(PathBuf::from(r"C:\work\repo")),
            created_at: None,
            last_activity_at: None,
            summary: String::new(),
            message_count: 0,
            is_archived: false,
            capabilities: CliSessionCapabilities::full(),
        };
        let sessions = (0..20)
            .map(|index| make_summary(&format!("listed-{index}"), "Unrelated session"))
            .collect();
        let search_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::default());
        let read_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::default());
        let provider = NativeSearchProvider {
            source_root: source_root.clone(),
            sessions,
            search_results: vec![make_summary("native-match", "Fix login timeout")],
            search_calls: search_calls.clone(),
            read_calls: read_calls.clone(),
        };
        let catalog = CliSessionCatalog::new(vec![Box::new(provider)]);

        let results = catalog.search("login").expect("native search succeeds");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session.key.id, "native-match");
        assert_eq!(search_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(read_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn continuing_a_session_launches_its_id_in_its_working_directory() {
        let catalog = test_catalog("older-session", r"C:\work\repo");
        let session = catalog.list_sessions().expect("listing succeeds").remove(0);
        let launcher = RecordingLauncher::default();

        catalog
            .continue_session(&session.key, &launcher)
            .expect("launch succeeds");

        let request = launcher
            .0
            .lock()
            .expect("launcher lock is not poisoned")
            .clone()
            .expect("selected session was launched");
        assert_eq!(request.arguments[0], OsString::from("resume"));
        assert_eq!(request.arguments[1], OsString::from("older-session"));
        assert_eq!(
            request.working_directory,
            Some(PathBuf::from(r"C:\work\repo"))
        );
    }

    #[test]
    fn powershell_arguments_are_quoted_as_literals() {
        let request = CliResumeRequest {
            program: PathBuf::from(r"C:\Program Files\Grok\grok.exe"),
            arguments: vec![OsString::from("--resume"), OsString::from("id'; exit 7; #")],
            working_directory: None,
            environment: Vec::new(),
        };

        let script = powershell_invocation(&request).expect("script is valid");

        assert!(script.contains("'C:\\Program Files\\Grok\\grok.exe'"));
        assert!(script.contains("'id''; exit 7; #'"));
    }

    #[test]
    fn resume_arguments_target_the_selected_session_for_every_cli() {
        let session_id = "the-selected-session";
        for tool in [
            CliTool::Codex,
            CliTool::ClaudeCode,
            CliTool::GrokBuild,
            CliTool::Antigravity,
        ] {
            let arguments = resume_arguments(&tool, session_id);
            assert_eq!(arguments[1], OsString::from(session_id));
        }
        assert_eq!(
            resume_arguments(&CliTool::Codex, session_id)[0],
            OsString::from("resume")
        );
        assert_eq!(
            resume_arguments(&CliTool::ClaudeCode, session_id)[0],
            OsString::from("--resume")
        );
        assert_eq!(
            resume_arguments(&CliTool::GrokBuild, session_id)[0],
            OsString::from("--resume")
        );
        assert_eq!(
            resume_arguments(&CliTool::Antigravity, session_id)[0],
            OsString::from("--conversation")
        );
    }

    #[test]
    fn unavailable_provider_does_not_hide_sessions_from_other_tools() {
        let mut catalog = test_catalog("older-session", r"C:\work\repo");
        catalog.providers.push(Box::new(FailingProvider));

        let snapshot = catalog
            .search_snapshot("does not match")
            .expect("partial catalog remains searchable");

        assert!(snapshot.results.is_empty());
        assert_eq!(snapshot.source_issues.len(), 1);
        assert!(snapshot.source_issues[0].contains("fixture CLI is unavailable"));
        assert_eq!(
            catalog
                .list_sessions()
                .expect("available source loads")
                .len(),
            1
        );
    }
}
