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
}

impl From<&QuickCommandEntryContent> for QuickCommand {
    fn from(entry: &QuickCommandEntryContent) -> Self {
        Self {
            name: entry.name.clone(),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
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
