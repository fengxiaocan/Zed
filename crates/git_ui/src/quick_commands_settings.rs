use gpui::Pixels;
use settings::{RegisterSetting, Settings};
use settings_content::QuickCommandEntryContent;
use ui::px;
use workspace::dock::DockPosition;

/// A single configured quick command, ready to be rendered and run.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickCommand {
    pub name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub folder: Option<String>,
}

impl From<&QuickCommandEntryContent> for QuickCommand {
    fn from(entry: &QuickCommandEntryContent) -> Self {
        Self {
            name: entry.name.clone(),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
            folder: entry
                .folder
                .as_ref()
                .map(|folder| folder.trim().to_string())
                .filter(|folder| !folder.is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, RegisterSetting)]
pub struct QuickCommandsSettings {
    pub button: bool,
    pub dock: Option<DockPosition>,
    pub default_width: Pixels,
    pub starts_open: bool,
    pub commands: Vec<QuickCommand>,
}

impl QuickCommandsSettings {
    /// Dock follows quick_commands setting if configured, or falls back to Git Panel.
    pub fn dock(cx: &gpui::App) -> DockPosition {
        Self::get_global(cx).dock.unwrap_or_else(|| {
            use crate::git_panel_settings::GitPanelSettings;
            GitPanelSettings::get_global(cx).dock
        })
    }
}

impl Settings for QuickCommandsSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let qc = content.quick_commands.clone().unwrap_or_default();
        Self {
            button: qc.button.unwrap_or(true),
            dock: qc.dock.map(Into::into),
            default_width: px(qc.default_width.unwrap_or(360.0)),
            starts_open: qc.starts_open.unwrap_or(false),
            commands: qc.commands.iter().map(QuickCommand::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings_content::QuickCommandEntryContent;

    #[test]
    fn test_quick_command_folder_deserialization_compatibility() {
        // 兼容旧版：无 folder 字段的 json 能平滑解析
        let legacy_json = r#"{
            "name": "Build Project",
            "command": "./gradlew assembleDebug",
            "cwd": "/workspace"
        }"#;
        let entry: QuickCommandEntryContent = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(entry.name, "Build Project");
        assert_eq!(entry.command, "./gradlew assembleDebug");
        assert_eq!(entry.cwd.as_deref(), Some("/workspace"));
        assert_eq!(entry.folder, None);

        let cmd = QuickCommand::from(&entry);
        assert_eq!(cmd.folder, None);

        // 新版：包含 folder 字段
        let new_json = r#"{
            "name": "Clean Git",
            "command": "git clean -fd",
            "folder": "Git Tools"
        }"#;
        let entry_with_folder: QuickCommandEntryContent = serde_json::from_str(new_json).unwrap();
        assert_eq!(entry_with_folder.folder.as_deref(), Some("Git Tools"));

        let cmd_with_folder = QuickCommand::from(&entry_with_folder);
        assert_eq!(cmd_with_folder.folder.as_deref(), Some("Git Tools"));

        // 空白字符串转为 None
        let empty_folder_entry = QuickCommandEntryContent {
            name: "Test".into(),
            command: "echo 1".into(),
            cwd: None,
            folder: Some("   ".into()),
        };
        let cmd_empty = QuickCommand::from(&empty_folder_entry);
        assert_eq!(cmd_empty.folder, None);
    }
}

