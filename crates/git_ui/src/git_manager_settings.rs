use gpui::Pixels;
use settings::{RegisterSetting, Settings, UpdateProjectDirtyWorktree, UpdateProjectMode};
use ui::px;
use workspace::dock::DockPosition;

#[derive(Debug, Clone, PartialEq, RegisterSetting)]
pub struct GitManagerSettings {
    pub button: bool,
    pub default_width: Pixels,
    pub starts_open: bool,
    pub update_project_mode: UpdateProjectMode,
    pub update_project_dirty_worktree: UpdateProjectDirtyWorktree,
}

impl GitManagerSettings {
    /// Dock follows the existing Git Panel (spec decision D).
    pub fn dock(cx: &gpui::App) -> DockPosition {
        use crate::git_panel_settings::GitPanelSettings;
        GitPanelSettings::get_global(cx).dock
    }
}

impl Settings for GitManagerSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let gm = content.git_manager.clone().unwrap();
        Self {
            button: gm.button.unwrap(),
            default_width: px(gm.default_width.unwrap()),
            starts_open: gm.starts_open.unwrap(),
            update_project_mode: gm.update_project_mode.unwrap_or_default(),
            update_project_dirty_worktree: gm.update_project_dirty_worktree.unwrap_or_default(),
        }
    }
}
